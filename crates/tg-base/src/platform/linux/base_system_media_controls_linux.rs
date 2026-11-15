use crate::platform::base_platform_system_media_controls::{Command, LoopStatus, PlaybackStatus};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};
use tg_rpl::{EventStream, Producer};
use tg_runtime::{MainQueue, MainRef, Queue};
use zbus::{
    blocking::Connection,
    fdo,
    message::{Header, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface, SignalEmitter},
    zvariant::{ObjectPath, OwnedValue, Str, Value},
    ObjectServer,
};
pub const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
pub const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
pub const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
pub const TRACK_PATH: &str = "/org/desktop_app/track/0";
pub fn convert_playback_status(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Playing => "Playing",
        PlaybackStatus::Paused => "Paused",
        PlaybackStatus::Stopped => "Stopped",
    }
}
pub fn convert_loop_status(status: LoopStatus) -> &'static str {
    match status {
        LoopStatus::None => "None",
        LoopStatus::Track => "Track",
        LoopStatus::Playlist => "Playlist",
    }
}
pub fn event_to_command(event: &str) -> Command {
    match event {
        "Pause" => Command::Pause,
        "Play" => Command::Play,
        "Stop" => Command::Stop,
        "PlayPause" => Command::PlayPause,
        "Next" => Command::Next,
        "Previous" => Command::Previous,
        "Quit" => Command::Quit,
        "Raise" => Command::Raise,
        _ => crate::unexpected!("EventToCommand in SystemMediaControls"),
    }
}
pub fn loop_status_to_command(status: &str) -> Command {
    match status {
        "None" => Command::LoopNone,
        "Track" => Command::LoopTrack,
        "Playlist" => Command::LoopPlaylist,
        _ => Command::None,
    }
}
pub fn scale_milliseconds(value: i32) -> i64 {
    value.wrapping_mul(1000) as i64
}
pub fn bus_name(
    application_name: &str,
    flatpak_id: Option<&str>,
    snap_name: Option<&str>,
) -> String {
    format!(
        "org.mpris.MediaPlayer2.{}",
        flatpak_id.or(snap_name).unwrap_or(application_name)
    )
}
#[derive(Clone, Debug, PartialEq)]
pub enum MetadataValue {
    Text(String),
    Strings(Vec<String>),
    Integer(i64),
    ObjectPath(String),
}
fn text_value(value: &str) -> OwnedValue {
    let value = value.split('\0').next().unwrap_or_default();
    OwnedValue::from(Str::from(value))
}
impl MetadataValue {
    fn to_value(&self) -> OwnedValue {
        match self {
            Self::Text(v) => text_value(v),
            Self::Strings(v) => {
                OwnedValue::try_from(Value::from(v.clone())).expect("owned string array")
            }
            Self::Integer(v) => OwnedValue::from(*v),
            Self::ObjectPath(v) => {
                OwnedValue::from(ObjectPath::try_from(v.as_str()).expect("actual track path"))
            }
        }
    }
}
struct State {
    identity: String,
    desktop_entry: String,
    can_raise: bool,
    can_next: bool,
    can_previous: bool,
    can_play_pause: bool,
    playback: String,
    loop_status: String,
    shuffle: bool,
    volume: f64,
    position: i64,
    duration: i64,
    metadata: BTreeMap<String, MetadataValue>,
}
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|e| e.into_inner())
}
#[derive(Clone)]
struct Bridge {
    owner: MainRef<SystemMediaControls>,
    queue: MainQueue,
    state: Arc<Mutex<State>>,
}
impl Bridge {
    async fn on_owner<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Rc<SystemMediaControls>) -> T + Send + 'static,
    ) -> fdo::Result<T> {
        let (send, receive) = async_channel::bounded(1);
        let owner = self.owner;
        self.queue.post(move || {
            let result = owner.upgrade().map(|owner| f(&owner));
            let _ = send.try_send(result);
        });
        receive
            .recv()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?
            .ok_or_else(|| fdo::Error::Failed("media controls owner destroyed".into()))
    }
    async fn get(&self, player: bool, property: &str) -> Option<fdo::Result<OwnedValue>> {
        if player && property == "Position" {
            return Some(
                self.on_owner(|o| o.read_position())
                    .await
                    .map(OwnedValue::from),
            );
        }
        let state = lock(&self.state);
        let value = if player {
            match property {
                "PlaybackStatus" => text_value(&state.playback),
                "LoopStatus" => text_value(&state.loop_status),
                "Rate" | "MinimumRate" | "MaximumRate" => OwnedValue::from(1f64),
                "Shuffle" => OwnedValue::from(state.shuffle),
                "Metadata" => OwnedValue::try_from(Value::from(
                    state
                        .metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_value()))
                        .collect::<HashMap<_, _>>(),
                ))
                .expect("metadata dictionary"),
                "Volume" => OwnedValue::from(state.volume),
                "CanGoNext" => OwnedValue::from(state.can_next),
                "CanGoPrevious" => OwnedValue::from(state.can_previous),
                "CanPlay" | "CanPause" => OwnedValue::from(state.can_play_pause),
                "CanSeek" | "CanControl" => OwnedValue::from(true),
                _ => return None,
            }
        } else {
            match property {
                "CanQuit" => OwnedValue::from(true),
                "CanRaise" => OwnedValue::from(state.can_raise),
                "HasTrackList" | "Fullscreen" | "CanSetFullscreen" => OwnedValue::from(false),
                "Identity" => text_value(&state.identity),
                "DesktopEntry" => text_value(&state.desktop_entry),
                "SupportedUriSchemes" | "SupportedMimeTypes" => {
                    OwnedValue::try_from(Value::from(Vec::<String>::new()))
                        .expect("empty string array")
                }
                _ => return None,
            }
        };
        Some(Ok(value))
    }
    async fn set(&self, property: &str, value: OwnedValue) -> fdo::Result<()> {
        let bad = |e: zbus::zvariant::Error| fdo::Error::InvalidArgs(e.to_string());
        match property {
            "LoopStatus" => {
                let value = String::try_from(value).map_err(bad)?;
                self.on_owner(move |o| o.set_loop_string(value)).await?;
            }
            "Shuffle" => {
                let value = bool::try_from(value).map_err(bad)?;
                self.on_owner(move |o| o.set_shuffle_remote(value)).await?;
            }
            "Volume" => {
                let value = f64::try_from(value).map_err(bad)?;
                self.on_owner(move |o| o.set_volume(value)).await?;
            }
            _ => return Err(fdo::Error::PropertyReadOnly(property.into())),
        }
        Ok(())
    }
    async fn method(&self, name: &str, message: &Message) -> fdo::Result<()> {
        let invalid = |e: zbus::Error| fdo::Error::InvalidArgs(e.to_string());
        match name {
            "Seek" => {
                let (offset,): (i64,) = message.body().deserialize().map_err(invalid)?;
                self.on_owner(move |o| {
                    let position = o.read_position().wrapping_add(offset);
                    o.seek
                        .fire(position as f64 / lock(&o.state).duration as f64);
                })
                .await?;
            }
            "SetPosition" => {
                let (_track, position): (zbus::zvariant::OwnedObjectPath, i64) =
                    message.body().deserialize().map_err(invalid)?;
                self.on_owner(move |o| {
                    let duration = lock(&o.state).duration;
                    o.seek.fire(position as f64 / duration as f64);
                })
                .await?;
            }
            _ => {
                message.body().deserialize::<()>().map_err(invalid)?;
                let command = event_to_command(name);
                self.on_owner(move |o| o.commands.fire(command)).await?;
            }
        }
        Ok(())
    }
}
const ROOT_PROPERTIES: &[&str] = &[
    "CanQuit",
    "CanRaise",
    "HasTrackList",
    "Identity",
    "DesktopEntry",
    "SupportedUriSchemes",
    "SupportedMimeTypes",
    "Fullscreen",
    "CanSetFullscreen",
];
const PLAYER_PROPERTIES: &[&str] = &[
    "PlaybackStatus",
    "LoopStatus",
    "Rate",
    "Shuffle",
    "Metadata",
    "Volume",
    "Position",
    "MinimumRate",
    "MaximumRate",
    "CanGoNext",
    "CanGoPrevious",
    "CanPlay",
    "CanPause",
    "CanSeek",
    "CanControl",
];
// Pinned Interface ABI is needed for GIO-compatible dedup/coalescing on Properties.Set.
macro_rules! export_interface {
    ($type:ident,$name:ident,$player:literal,$properties:ident) => {
        struct $type(Bridge);
        #[async_trait::async_trait]
        impl Interface for $type {
            fn name() -> InterfaceName<'static> {
                InterfaceName::from_static_str($name).unwrap()
            }
            fn spawn_tasks_for_methods(&self) -> bool {
                false
            }
            async fn get(
                &self,
                name: &str,
                _server: &ObjectServer,
                _connection: &zbus::Connection,
                _header: Option<&Header<'_>>,
                _emitter: &SignalEmitter<'_>,
            ) -> Option<fdo::Result<OwnedValue>> {
                self.0.get($player, name).await
            }
            async fn get_all(
                &self,
                _server: &ObjectServer,
                _connection: &zbus::Connection,
                _header: Option<&Header<'_>>,
                _emitter: &SignalEmitter<'_>,
            ) -> fdo::Result<HashMap<String, OwnedValue>> {
                let mut values = HashMap::new();
                for name in $properties {
                    if let Some(value) = self.0.get($player, name).await {
                        values.insert((*name).into(), value?);
                    }
                }
                Ok(values)
            }
            fn set<'a>(
                &'a self,
                name: &'a str,
                value: &'a Value<'_>,
                _server: &'a ObjectServer,
                _connection: &'a zbus::Connection,
                _header: Option<&'a Header<'_>>,
                _emitter: &'a SignalEmitter<'_>,
            ) -> DispatchResult2<'a> {
                if !$properties.contains(&name) {
                    return DispatchResult2::NotFound;
                }
                DispatchResult2::Async(Box::pin(async move {
                    self.0
                        .set(
                            name,
                            value
                                .try_to_owned()
                                .map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?,
                        )
                        .await
                }))
            }
            async fn set_mut(
                &mut self,
                _name: &str,
                _value: &Value<'_>,
                _server: &ObjectServer,
                _connection: &zbus::Connection,
                _header: Option<&Header<'_>>,
                _emitter: &SignalEmitter<'_>,
            ) -> Option<fdo::Result<()>> {
                None
            }
            fn call<'a>(
                &'a self,
                _server: &'a ObjectServer,
                connection: &'a zbus::Connection,
                message: &'a Message,
                name: MemberName<'a>,
            ) -> DispatchResult2<'a> {
                let known = if $player {
                    matches!(
                        name.as_str(),
                        "Next"
                            | "Previous"
                            | "Pause"
                            | "PlayPause"
                            | "Stop"
                            | "Play"
                            | "Seek"
                            | "SetPosition"
                    )
                } else {
                    matches!(name.as_str(), "Raise" | "Quit")
                };
                if !known {
                    return DispatchResult2::NotFound;
                }
                let name = name.to_string();
                DispatchResult2::new_async(connection, message, async move {
                    self.0.method(&name, message).await
                })
            }
            fn call_mut<'a>(
                &'a mut self,
                _server: &'a ObjectServer,
                _connection: &'a zbus::Connection,
                _message: &'a Message,
                _name: MemberName<'a>,
            ) -> DispatchResult2<'a> {
                DispatchResult2::NotFound
            }
            fn introspect_to_writer(&self, writer: &mut dyn std::fmt::Write, level: usize) {
                let xml = include_str!("mpris.xml");
                let start = xml.find(&format!("<interface name='{}'>", $name)).unwrap();
                let end = start + xml[start..].find("</interface>").unwrap() + "</interface>".len();
                for line in xml[start..end].lines() {
                    let _ = writeln!(writer, "{}{}", " ".repeat(level), line.trim_start());
                }
            }
        }
    };
}
export_interface!(Root, ROOT_INTERFACE, false, ROOT_PROPERTIES);
export_interface!(Player, PLAYER_INTERFACE, true, PLAYER_PROPERTIES);
type Pending = BTreeMap<(&'static str, &'static str), OwnedValue>;
pub struct SystemMediaControls {
    connection: Option<Connection>,
    queue: MainQueue,
    worker: Queue,
    state: Arc<Mutex<State>>,
    name: String,
    enabled: Cell<bool>,
    exported: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    updating: Cell<bool>,
    commands: EventStream<Command>,
    seek: EventStream<f64>,
    volume: EventStream<f64>,
    update_position: EventStream<()>,
    pending: RefCell<Pending>,
    scheduled: Cell<bool>,
}
impl SystemMediaControls {
    pub fn new(
        connection: Option<Connection>,
        queue: MainQueue,
        desktop_entry: String,
        name: String,
        wayland: bool,
    ) -> Rc<Self> {
        Rc::new(Self {
            connection,
            queue,
            worker: Queue::new(),
            state: Arc::new(Mutex::new(State {
                identity: desktop_entry.clone(),
                desktop_entry,
                can_raise: !wayland,
                can_next: false,
                can_previous: false,
                can_play_pause: false,
                playback: "Stopped".into(),
                loop_status: "None".into(),
                shuffle: false,
                volume: 0.,
                position: 0,
                duration: 0,
                metadata: BTreeMap::new(),
            })),
            name,
            enabled: Cell::new(false),
            exported: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
            updating: Cell::new(false),
            commands: EventStream::new(),
            seek: EventStream::new(),
            volume: EventStream::new(),
            update_position: EventStream::new(),
            pending: RefCell::default(),
            scheduled: Cell::new(false),
        })
    }
    pub fn init(self: &Rc<Self>) -> bool {
        self.clear_metadata();
        self.connection.is_some()
    }
    pub fn supported() -> bool {
        true
    }
    pub fn seeking_supported(&self) -> bool {
        true
    }
    pub fn volume_supported(&self) -> bool {
        true
    }
    pub fn exported(&self) -> bool {
        self.exported.load(Ordering::Acquire)
    }
    pub fn last_error(&self) -> Option<String> {
        lock(&self.error).clone()
    }
    pub fn set_enabled(self: &Rc<Self>, enabled: bool) {
        if self.enabled.replace(enabled) == enabled {
            return;
        }
        let Some(connection) = self.connection.clone() else {
            return;
        };
        let bridge = Bridge {
            owner: MainRef::new(self),
            queue: self.queue.clone(),
            state: self.state.clone(),
        };
        let name = self.name.clone();
        let exported = self.exported.clone();
        let error = self.error.clone();
        let completion = self.queue.clone();
        self.worker.async_(move || {
            let result = (|| -> zbus::Result<()> {
                let server = connection.object_server();
                if enabled {
                    server.at(OBJECT_PATH, Root(bridge.clone()))?;
                    server.at(OBJECT_PATH, Player(bridge))?;
                    server.at("/org/mpris", zbus::fdo::ObjectManager)?;
                    connection.request_name_with_flags(name.as_str(), Default::default())?;
                    exported.store(true, Ordering::Release);
                } else {
                    connection.release_name(name.as_str())?;
                    exported.store(false, Ordering::Release);
                    server.remove::<Player, _>(OBJECT_PATH)?;
                    server.remove::<Root, _>(OBJECT_PATH)?;
                    server.remove::<zbus::fdo::ObjectManager, _>("/org/mpris")?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                *lock(&error) = Some(e.to_string());
            }
            completion.post(|| {});
        });
    }
    fn changed(self: &Rc<Self>, interface: &'static str, name: &'static str, value: OwnedValue) {
        self.pending.borrow_mut().insert((interface, name), value);
        if self.scheduled.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        self.queue.post_local(move || {
            if let Some(owner) = weak.upgrade() {
                owner.scheduled.set(false);
                let pending = std::mem::take(&mut *owner.pending.borrow_mut());
                if !owner.exported() {
                    return;
                }
                let Some(connection) = &owner.connection else {
                    return;
                };
                let mut groups: BTreeMap<&str, HashMap<&str, OwnedValue>> = BTreeMap::new();
                for ((interface, name), value) in pending {
                    groups.entry(interface).or_default().insert(name, value);
                }
                for (interface, changes) in groups {
                    let _ = connection.emit_signal(
                        None::<&str>,
                        OBJECT_PATH,
                        "org.freedesktop.DBus.Properties",
                        "PropertiesChanged",
                        &(interface, changes, Vec::<String>::new()),
                    );
                }
            }
        });
    }
    pub fn set_application_name(self: &Rc<Self>, name: String) {
        if lock(&self.state).identity == name {
            return;
        }
        lock(&self.state).identity = name.clone();
        self.changed(ROOT_INTERFACE, "Identity", text_value(&name));
    }
    pub fn set_is_next_enabled(self: &Rc<Self>, value: bool) {
        if lock(&self.state).can_next == value {
            return;
        }
        lock(&self.state).can_next = value;
        self.changed(PLAYER_INTERFACE, "CanGoNext", value.into());
    }
    pub fn set_is_previous_enabled(self: &Rc<Self>, value: bool) {
        if lock(&self.state).can_previous == value {
            return;
        }
        lock(&self.state).can_previous = value;
        self.changed(PLAYER_INTERFACE, "CanGoPrevious", value.into());
    }
    pub fn set_is_play_pause_enabled(self: &Rc<Self>, value: bool) {
        if lock(&self.state).can_play_pause == value {
            return;
        }
        lock(&self.state).can_play_pause = value;
        self.changed(PLAYER_INTERFACE, "CanPlay", value.into());
        self.changed(PLAYER_INTERFACE, "CanPause", value.into());
    }
    /// Original Linux method is intentionally empty.
    pub fn set_is_stop_enabled(&self, _value: bool) {}
    /// Original Linux method is intentionally empty.
    pub fn set_playback_rate(&self, _value: f64) {}
    /// Original Linux method is intentionally empty.
    pub fn update_display(&self) {}
    pub fn set_playback_status(self: &Rc<Self>, status: PlaybackStatus) {
        let status = convert_playback_status(status);
        if lock(&self.state).playback == status {
            return;
        }
        lock(&self.state).playback = status.into();
        self.changed(PLAYER_INTERFACE, "PlaybackStatus", text_value(status));
    }
    pub fn set_loop_status(self: &Rc<Self>, status: LoopStatus) {
        self.set_loop_string(convert_loop_status(status).into());
    }
    fn set_loop_string(self: &Rc<Self>, status: String) {
        if lock(&self.state).loop_status == status {
            return;
        }
        lock(&self.state).loop_status = status.clone();
        self.changed(PLAYER_INTERFACE, "LoopStatus", text_value(&status));
        self.commands.fire(loop_status_to_command(&status));
    }
    fn shuffle_value(self: &Rc<Self>, value: bool) -> bool {
        if lock(&self.state).shuffle == value {
            return false;
        }
        lock(&self.state).shuffle = value;
        self.changed(PLAYER_INTERFACE, "Shuffle", value.into());
        true
    }
    pub fn set_shuffle(self: &Rc<Self>, value: bool) {
        self.shuffle_value(value);
    }
    fn set_shuffle_remote(self: &Rc<Self>, value: bool) {
        if self.shuffle_value(value) {
            self.commands.fire(Command::Shuffle);
        }
    }
    pub fn set_volume(self: &Rc<Self>, value: f64) {
        if lock(&self.state).volume == value {
            return;
        }
        lock(&self.state).volume = value;
        self.changed(PLAYER_INTERFACE, "Volume", value.into());
        self.volume.fire(value);
    }
    pub fn read_position(&self) -> i64 {
        if !self.updating.replace(true) {
            self.update_position.fire(());
            self.updating.set(false);
        }
        lock(&self.state).position
    }
    pub fn set_position(self: &Rc<Self>, position: i32) {
        let was = self.read_position();
        let value = scale_milliseconds(position);
        if lock(&self.state).position != value {
            lock(&self.state).position = value;
            self.changed(PLAYER_INTERFACE, "Position", value.into());
        }
        let now = self.read_position();
        let difference = was.wrapping_sub(now);
        if self.exported() && !(-1000000..=1000000).contains(&difference) {
            if let Some(connection) = &self.connection {
                let _ = connection.emit_signal(
                    None::<&str>,
                    OBJECT_PATH,
                    PLAYER_INTERFACE,
                    "Seeked",
                    &(now,),
                );
            }
        }
    }
    fn metadata_changed(self: &Rc<Self>) {
        let value = {
            let state = lock(&self.state);
            let map = state
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), v.to_value()))
                .collect::<HashMap<_, _>>();
            OwnedValue::try_from(Value::from(map)).expect("owned metadata")
        };
        self.changed(PLAYER_INTERFACE, "Metadata", value);
    }
    fn metadata_insert(self: &Rc<Self>, name: &str, value: MetadataValue) {
        let was = lock(&self.state)
            .metadata
            .insert(name.into(), value.clone());
        if was != Some(value) {
            self.metadata_changed();
        }
    }
    pub fn set_title(self: &Rc<Self>, value: String) {
        self.metadata_insert(
            "xesam:title",
            MetadataValue::Text(value.split('\0').next().unwrap_or_default().into()),
        );
    }
    pub fn set_artist(self: &Rc<Self>, value: String) {
        self.metadata_insert(
            "xesam:artist",
            MetadataValue::Strings(vec![value.split('\0').next().unwrap_or_default().into()]),
        );
    }
    pub fn set_duration(self: &Rc<Self>, value: i32) {
        let value = scale_milliseconds(value);
        lock(&self.state).duration = value;
        self.metadata_insert("mpris:length", MetadataValue::Integer(value));
    }
    /// QImage JPEG encoding (quality87) remains stage5; this accepts real encoded bytes.
    pub fn set_thumbnail_jpeg(self: &Rc<Self>, bytes: &[u8]) {
        use base64::Engine;
        let url = format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        self.metadata_insert("mpris:artUrl", MetadataValue::Text(url));
    }
    pub fn clear_thumbnail(self: &Rc<Self>) {
        if lock(&self.state).metadata.remove("mpris:artUrl").is_some() {
            self.metadata_changed();
        }
    }
    pub fn clear_metadata(self: &Rc<Self>) {
        let metadata = BTreeMap::from([(
            "mpris:trackid".into(),
            MetadataValue::ObjectPath(TRACK_PATH.into()),
        )]);
        if lock(&self.state).metadata == metadata {
            return;
        }
        lock(&self.state).metadata = metadata;
        self.metadata_changed();
    }
    pub fn metadata(&self) -> BTreeMap<String, MetadataValue> {
        lock(&self.state).metadata.clone()
    }
    pub fn command_requests(&self) -> Producer<Command> {
        self.commands.events()
    }
    pub fn seek_requests(&self) -> Producer<f64> {
        self.seek.events()
    }
    pub fn volume_change_requests(&self) -> Producer<f64> {
        self.volume.events()
    }
    pub fn update_position_requests(&self) -> Producer<()> {
        self.update_position.events()
    }
}
impl Drop for SystemMediaControls {
    fn drop(&mut self) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        let name = self.name.clone();
        let exported = self.exported.clone();
        self.worker.async_(move || {
            let _ = connection.release_name(name.as_str());
            exported.store(false, Ordering::Release);
            let server = connection.object_server();
            let _ = server.remove::<Player, _>(OBJECT_PATH);
            let _ = server.remove::<Root, _>(OBJECT_PATH);
            let _ = server.remove::<zbus::fdo::ObjectManager, _>("/org/mpris");
        });
    }
}
