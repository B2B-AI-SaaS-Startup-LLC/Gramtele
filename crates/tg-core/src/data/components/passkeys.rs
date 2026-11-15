//! Passkeys service over real Sender requests; platform/Session facts are required.
use crate::{
    data::data_passkey_deserialize as format,
    platform::platform_webauthn::{LoginResult, RegisterResult},
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use tg_base::timer::TimerHost;
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PasskeyEntry {
    pub id: String,
    pub name: String,
    pub date: i32,
    pub software_emoji_id: u64,
    pub last_usage_date: i32,
}
pub fn from_tl(data: &tl::Passkey) -> PasskeyEntry {
    let tl::Passkey::Passkey(d) = data;
    PasskeyEntry {
        id: String::from_utf8_lossy(&d.id).into_owned(),
        name: String::from_utf8_lossy(&d.name).into_owned(),
        date: d.date,
        software_emoji_id: d.software_emoji_id.unwrap_or(0),
        last_usage_date: d.last_usage_date.unwrap_or(0),
    }
}
pub trait PasskeysOwner {
    fn webauthn_supported(&self) -> bool;
    fn account_passkeys_max(&self) -> i32;
    fn settings_display_passkeys(&self) -> bool;
    fn send(&self, id: i32, request: Request);
}
#[derive(Clone)]
pub enum Request {
    Init(tl::AccountInitPasskeyRegistration),
    Register(tl::AccountRegisterPasskey),
    Delete(tl::AccountDeletePasskey),
    List(tl::AccountGetPasskeys),
}
#[derive(Clone)]
pub enum Reply {
    Init(tl::AccountPasskeyRegistrationOptions),
    Register(tl::Passkey),
    Deleted,
    List(tl::AccountPasskeys),
    Failed(String),
}
type Done = Rc<dyn Fn()>;
type Fail = Rc<dyn Fn(String)>;
enum Callback {
    Init(Rc<dyn Fn(&format::RegisterData)>),
    Register(Done),
    Delete(Done, Fail),
    List,
}
#[derive(Default)]
struct State {
    list: Vec<PasskeyEntry>,
    pending: Option<format::RegisterData>,
    pending_time: i64,
    last_request: i64,
    list_request: i32,
    known: bool,
    callbacks: BTreeMap<i32, Callback>,
}
pub struct Passkeys {
    state: RefCell<State>,
    host: Rc<TimerHost>,
    owner: Rc<dyn PasskeysOwner>,
    updated: EventStream<()>,
}
impl Passkeys {
    pub fn new(host: Rc<TimerHost>, owner: Rc<dyn PasskeysOwner>) -> Self {
        Self {
            state: RefCell::default(),
            host,
            owner,
            updated: EventStream::new(),
        }
    }
    fn send(&self, request: Request, callback: Callback) -> i32 {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().callbacks.insert(id, callback);
        self.owner.send(id, request);
        id
    }
    pub fn init_registration(&self, done: Rc<dyn Fn(&format::RegisterData)>) {
        let cached = {
            let s = self.state.borrow();
            s.pending
                .as_ref()
                .filter(|p| self.host.now() - s.pending_time < i64::from(p.timeout))
                .cloned()
        };
        if let Some(p) = cached {
            done(&p);
            return;
        }
        self.state.borrow_mut().pending = None;
        self.send(
            Request::Init(tl::AccountInitPasskeyRegistration {}),
            Callback::Init(done),
        );
    }
    pub fn register_passkey(&self, result: &RegisterResult, done: Done) {
        self.send(
            Request::Register(tl::AccountRegisterPasskey {
                credential: registration_credential(result),
            }),
            Callback::Register(done),
        );
    }
    pub fn delete_passkey(&self, id: &str, done: Done, fail: Fail) {
        self.send(
            Request::Delete(tl::AccountDeletePasskey {
                id: id.as_bytes().to_vec(),
            }),
            Callback::Delete(done, fail),
        );
    }
    pub fn request_list(&self) -> Producer<()> {
        let (last, request) = {
            let s = self.state.borrow();
            (s.last_request, s.list_request)
        };
        if last == 0 || self.host.now() - last > 5000 {
            if request == 0 {
                self.load_list();
            }
            self.updated.events()
        } else {
            self.updated.events_starting_with(())
        }
    }
    pub fn list(&self) -> Vec<PasskeyEntry> {
        self.state.borrow().list.clone()
    }
    pub fn list_known(&self) -> bool {
        self.state.borrow().known
    }
    pub fn can_register(&self) -> bool {
        let max = self.owner.account_passkeys_max();
        self.owner.webauthn_supported() && self.state.borrow().list.len() < max as usize
    }
    pub fn possible(&self) -> bool {
        self.owner.settings_display_passkeys()
    }
    fn load_list(&self) {
        self.state.borrow_mut().last_request = self.host.now();
        let id = self.send(Request::List(tl::AccountGetPasskeys {}), Callback::List);
        self.state.borrow_mut().list_request = id;
    }
    /// Owner marshals actual Sender callbacks to its main queue and guards lifetime.
    pub fn handle_reply(&self, id: i32, reply: Reply) {
        let callback = self.state.borrow_mut().callbacks.remove(&id);
        let Some(callback) = callback else {
            return;
        };
        match (callback, reply) {
            (
                Callback::Init(done),
                Reply::Init(
                    tl::AccountPasskeyRegistrationOptions::AccountPasskeyRegistrationOptions(d),
                ),
            ) => {
                let tl::DataJSON::DataJSON(json) = &d.options;
                if let Some(p) = format::deserialize_register_data(&json.data) {
                    let mut s = self.state.borrow_mut();
                    s.pending = Some(p.clone());
                    s.pending_time = self.host.now();
                    drop(s);
                    done(&p);
                }
            }
            (Callback::Register(done), Reply::Register(p)) => {
                let mut s = self.state.borrow_mut();
                s.pending = None;
                s.list.push(from_tl(&p));
                drop(s);
                self.updated.fire(());
                done();
            }
            (Callback::Register(_), Reply::Failed(_)) => self.state.borrow_mut().pending = None,
            (Callback::Delete(done, _), Reply::Deleted) => {
                let mut s = self.state.borrow_mut();
                s.last_request = 0;
                s.known = false;
                drop(s);
                self.load_list();
                done();
            }
            (Callback::Delete(_, fail), Reply::Failed(e)) => fail(e),
            (Callback::List, Reply::List(tl::AccountPasskeys::AccountPasskeys(d))) => {
                let mut s = self.state.borrow_mut();
                s.list_request = 0;
                s.known = true;
                s.list = d.passkeys.iter().map(from_tl).collect();
                drop(s);
                self.updated.fire(());
            }
            (Callback::List, Reply::Failed(_)) => self.state.borrow_mut().list_request = 0,
            (Callback::Init(_), Reply::Failed(_)) => {}
            _ => panic!("Passkeys response type"),
        }
    }
}
pub fn registration_credential(result: &RegisterResult) -> tl::InputPasskeyCredential {
    let id = format::encode_base64_url(&result.credential_id, true).into_bytes();
    tl::mtp_input_passkey_credential_public_key(
        id.clone(),
        id,
        tl::mtp_input_passkey_response_register(
            tl::mtp_data_json(result.client_data_json.clone()),
            result.attestation_object.clone(),
        ),
    )
}
/// QString::split(':') requires exactly2 parts; only first is validated by source.
pub fn user_dc(handle: &[u8]) -> Option<i32> {
    let text = String::from_utf8_lossy(handle);
    let mut p = text.split(':');
    let first = p.next()?;
    p.next()?;
    if p.next().is_some() {
        return None;
    }
    let s = first.trim_matches(|c: char| c.is_ascii_whitespace());
    if !s
        .bytes()
        .enumerate()
        .all(|(i, b)| b.is_ascii_digit() || i == 0 && (b == b'+' || b == b'-'))
    {
        return None;
    }
    s.parse::<i32>()
        .ok()
        .filter(|n| *n > 0 && *n < tg_mtproto::core_types::DC_SHIFT)
}
pub fn login_credential(result: &LoginResult) -> tl::InputPasskeyCredential {
    let id = format::encode_base64_url(&result.credential_id, false).into_bytes();
    tl::mtp_input_passkey_credential_public_key(
        id.clone(),
        id,
        tl::mtp_input_passkey_response_login(
            tl::mtp_data_json(result.client_data_json.clone()),
            result.authenticator_data.clone(),
            result.signature.clone(),
            String::from_utf8_lossy(&result.user_handle)
                .as_bytes()
                .to_vec(),
        ),
    )
}
pub fn init_passkey_login(
    sender: &tg_mtproto::sender::Sender,
    api_id: i32,
    api_hash: Vec<u8>,
    done: Arc<dyn Fn(format::LoginData) + Send + Sync>,
) -> i32 {
    sender
        .request(tl::AuthInitPasskeyLogin { api_id, api_hash })
        .done(
            move |tl::AuthPasskeyLoginOptions::AuthPasskeyLoginOptions(d)| {
                let tl::DataJSON::DataJSON(json) = &d.options;
                if let Some(data) = format::deserialize_login_data(&json.data) {
                    done(data);
                }
            },
        )
        .send()
}
pub fn finish_passkey_login(
    sender: &tg_mtproto::sender::Sender,
    initial_dc: i32,
    result: &LoginResult,
    done: Arc<dyn Fn(tl::AuthAuthorization) + Send + Sync>,
    fail: Arc<dyn Fn(String) + Send + Sync>,
) -> Option<i32> {
    let Some(dc) = user_dc(&result.user_handle) else {
        fail("PASSKEY_HANDLE_INVALID".into());
        return None;
    };
    let from_auth_key_id = sender
        .instance()
        .get_keys_for_write()
        .iter()
        .find(|k| k.dc_id() == tg_mtproto::core_types::bare_dc_id(initial_dc))
        .map_or(0, |k| k.key_id());
    sender.instance().set_main_dc_id(dc);
    Some(
        sender
            .request(tl::AuthFinishPasskeyLogin {
                flags: tl::tl::Flags::from_raw(1),
                credential: login_credential(result),
                from_dc_id: Some(initial_dc),
                from_auth_key_id: Some(from_auth_key_id),
            })
            .to_dc(dc)
            .done(move |r| done(r))
            .fail(move |e| fail(e.kind.clone()))
            .send(),
    )
}
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    r: Request,
    reply: Arc<dyn Fn(i32, Reply) + Send + Sync>,
) {
    let fail = reply.clone();
    match r {
        Request::Init(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(id, Reply::Init(r)))
                .fail(move |e| fail(id, Reply::Failed(e.kind.clone())))
                .send();
        }
        Request::Register(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(id, Reply::Register(r)))
                .fail(move |e| fail(id, Reply::Failed(e.kind.clone())))
                .send();
        }
        Request::Delete(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |_| reply(id, Reply::Deleted))
                .fail(move |e| fail(id, Reply::Failed(e.kind.clone())))
                .send();
        }
        Request::List(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(id, Reply::List(r)))
                .fail(move |e| fail(id, Reply::Failed(e.kind.clone())))
                .send();
        }
    }
}
