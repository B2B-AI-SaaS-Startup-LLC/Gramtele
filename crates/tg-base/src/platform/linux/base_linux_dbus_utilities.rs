use futures_core::Stream;
use std::{
    cell::{Cell, RefCell},
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use tg_runtime::{MainQueue, MainRef};
use zbus::{
    blocking::{Connection, MessageIterator, Proxy},
    MatchRule, Message, MessageStream,
};
pub const SERVICE: &str = "org.freedesktop.DBus";
pub const OBJECT_PATH: &str = "/org/freedesktop/DBus";
pub const INTERFACE: &str = SERVICE;
pub type Result<T> = zbus::Result<T>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartReply(pub u32);
impl StartReply {
    // Source enum starts at0, but direct D-Bus cast preserves1/2 (including2).
    pub const SUCCESS: Self = Self(0);
    pub const ALREADY_RUNNING: Self = Self(1);
}
fn interface(connection: &Connection) -> Result<Proxy<'static>> {
    Proxy::new_owned(connection.clone(), SERVICE, OBJECT_PATH, INTERFACE)
}
pub fn name_has_owner(connection: &Connection, name: &str) -> Result<bool> {
    interface(connection)?.call("NameHasOwner", &(name,))
}
pub fn list_activatable_names(connection: &Connection) -> Result<Vec<String>> {
    interface(connection)?.call("ListActivatableNames", &())
}
pub fn start_service_by_name(connection: &Connection, name: &str) -> Result<StartReply> {
    interface(connection)?
        .call("StartServiceByName", &(name, 0u32))
        .map(StartReply)
}
pub type StartResult = Box<dyn FnOnce() -> Result<StartReply> + Send>;
pub fn start_service_by_name_async(
    connection: Connection,
    name: String,
    queue: MainQueue,
    callback: impl FnOnce(StartResult) + Send + 'static,
) {
    if connection.is_closed() {
        callback(Box::new(|| {
            Err(zbus::Error::Failure("closed D-Bus connection".into()))
        }));
        return;
    }
    tg_runtime::async_(move || {
        let result = start_service_by_name(&connection, &name);
        queue.post(move || callback(Box::new(move || result)));
    });
}
struct SignalState {
    stream: RefCell<MessageStream>,
    callback: Box<dyn Fn(Message)>,
    waker: RefCell<Option<Waker>>,
    polling: Cell<bool>,
}
struct SignalWake {
    target: MainRef<SignalState>,
    queue: MainQueue,
    scheduled: std::sync::atomic::AtomicBool,
}
impl Wake for SignalWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        use std::sync::atomic::Ordering;
        if self.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let me = self.clone();
        self.target.on_queue(&self.queue, move |state| {
            me.scheduled.store(false, Ordering::Release);
            state.drain();
        });
    }
}
impl SignalState {
    fn drain(&self) {
        if self.polling.replace(true) {
            return;
        }
        loop {
            let waker = self.waker.borrow().as_ref().unwrap().clone();
            let message = {
                let mut stream = self.stream.borrow_mut();
                Pin::new(&mut *stream).poll_next(&mut Context::from_waker(&waker))
            };
            match message {
                Poll::Ready(Some(Ok(message))) => (self.callback)(message),
                Poll::Ready(Some(Err(_))) | Poll::Ready(None) | Poll::Pending => break,
            }
        }
        self.polling.set(false);
    }
}
/// zbus drives socket I/O; readiness wakes the existing owner queue. No loop/thread.
pub struct SignalSubscription {
    state: Rc<SignalState>,
}
impl SignalSubscription {
    pub fn new(
        connection: &Connection,
        rule: MatchRule<'static>,
        queue: MainQueue,
        callback: impl Fn(Message) + 'static,
    ) -> Result<Self> {
        let stream = MessageIterator::for_match_rule(rule, connection, None)?.into_inner();
        let state = Rc::new(SignalState {
            stream: RefCell::new(stream),
            callback: Box::new(callback),
            waker: RefCell::default(),
            polling: Cell::new(false),
        });
        let target = MainRef::new(&state);
        *state.waker.borrow_mut() = Some(Waker::from(Arc::new(SignalWake {
            target,
            queue,
            scheduled: std::sync::atomic::AtomicBool::new(false),
        })));
        state.waker.borrow().as_ref().unwrap().wake_by_ref();
        Ok(Self { state })
    }
    pub fn poll(&self) {
        self.state.drain();
    }
}
pub struct ServiceWatcher {
    _signal: SignalSubscription,
}
impl ServiceWatcher {
    pub fn new(
        connection: &Connection,
        service: &str,
        queue: MainQueue,
        callback: impl Fn(&str, &str, &str) + 'static,
    ) -> Result<Self> {
        let service = service.to_owned();
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(SERVICE)?
            .path(OBJECT_PATH)?
            .interface(INTERFACE)?
            .member("NameOwnerChanged")?
            .build();
        Ok(Self {
            _signal: SignalSubscription::new(connection, rule, queue, move |message| {
                if let Ok((name, old, new)) =
                    message.body().deserialize::<(String, String, String)>()
                {
                    if name == service {
                        callback(&name, &old, &new);
                    }
                }
            })?,
        })
    }
}
pub fn proxy(
    connection: &Connection,
    service: &str,
    path: &str,
    interface: &str,
) -> Result<Proxy<'static>> {
    Proxy::new_owned(
        connection.clone(),
        service.to_owned(),
        path.to_owned(),
        interface.to_owned(),
    )
}
pub fn signal_rule(
    service: &str,
    path: &str,
    interface: &str,
    member: &str,
) -> Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(service.to_owned())?
        .path(path.to_owned())?
        .interface(interface.to_owned())?
        .member(member.to_owned())?
        .build())
}
