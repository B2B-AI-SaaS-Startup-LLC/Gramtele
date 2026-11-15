//! BusinessInfo model. The real self-user/Session and API delivery remain owner contracts (02/26/29).
use super::data_business_common::*;
use crate::data::data_store_keys::{DocumentRecordKey, SessionScopeId, UserKey};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
};
use tg_rpl::{EventStream, Producer, Variable};
use tg_scheme as tl;

/// Actual calls made to the owning session; no replacement UserData model.
pub trait BusinessOwner {
    fn scope(&self) -> SessionScopeId;
    fn business_details(&self) -> BusinessDetails;
    fn set_business_details(&self, details: BusinessDetails);
    fn input_user(&self, user: UserKey) -> tl::InputUser;
    fn input_document(&self, document: DocumentRecordKey) -> tl::InputDocument;
}
#[derive(Clone)]
pub enum Request {
    WorkingHours(tl::AccountUpdateBusinessWorkHours),
    Intro(tl::AccountUpdateBusinessIntro),
    Location(tl::AccountUpdateBusinessLocation),
    Away(tl::AccountUpdateBusinessAwayMessage),
    Greeting(tl::AccountUpdateBusinessGreetingMessage),
    Timezones(tl::HelpGetTimezonesList),
}
#[derive(Clone)]
pub enum Reply {
    Saved(i32),
    Failed(i32, String),
    Timezones(tl::HelpTimezonesList),
}
pub type Failure = Rc<dyn Fn(String)>;
enum Rollback {
    Hours(WorkingHours),
    Intro(ChatIntro),
    Location(BusinessLocation),
    Away(Option<AwaySettings>),
    Greeting(Option<GreetingSettings>),
}
pub struct BusinessInfo {
    owner: Rc<dyn BusinessOwner>,
    send: Rc<dyn Fn(i32, Request)>,
    pending: RefCell<BTreeMap<i32, (Rollback, Option<Failure>)>>,
    away: RefCell<Option<AwaySettings>>,
    greeting: RefCell<Option<GreetingSettings>>,
    away_changed: EventStream<()>,
    greeting_changed: EventStream<()>,
    timezones: Variable<Timezones>,
    timezones_hash: Cell<i32>,
    timezones_request_id: Cell<i32>,
}
impl BusinessInfo {
    pub fn new(owner: Rc<dyn BusinessOwner>, send: Rc<dyn Fn(i32, Request)>) -> Self {
        Self {
            owner,
            send,
            pending: RefCell::default(),
            away: RefCell::default(),
            greeting: RefCell::default(),
            away_changed: EventStream::new(),
            greeting_changed: EventStream::new(),
            timezones: Variable::new(Timezones::default()),
            timezones_hash: Cell::new(0),
            timezones_request_id: Cell::new(0),
        }
    }
    pub fn owner(&self) -> SessionScopeId {
        self.owner.scope()
    }
    fn save(&self, request: Request, rollback: Rollback, fail: Option<Failure>) {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.pending.borrow_mut().insert(id, (rollback, fail));
        // As in C++, dispatch precedes optimistic mutation. No model borrow spans send.
        (self.send)(id, request);
    }
    pub fn save_working_hours(&self, data: WorkingHours, fail: Option<Failure>) {
        let mut details = self.owner.business_details();
        if details.hours == data {
            return;
        }
        self.save(
            Request::WorkingHours(tl::AccountUpdateBusinessWorkHours {
                flags: Default::default(),
                business_work_hours: data.is_present().then(|| working_hours_to_mtp(&data)),
            }),
            Rollback::Hours(details.hours),
            fail,
        );
        details.hours = data;
        self.owner.set_business_details(details);
    }
    pub fn save_chat_intro(&self, data: ChatIntro, fail: Option<Failure>) {
        let mut details = self.owner.business_details();
        if details.intro == data {
            return;
        }
        let intro = tl::InputBusinessIntro::InputBusinessIntro(
            tl::InputBusinessIntroData {
                flags: Default::default(),
                title: data.title.as_bytes().to_vec(),
                description: data.description.as_bytes().to_vec(),
                sticker: data.sticker.map(|k| self.owner.input_document(k)),
            }
            .into(),
        );
        self.save(
            Request::Intro(tl::AccountUpdateBusinessIntro {
                flags: Default::default(),
                intro: data.is_present().then_some(intro),
            }),
            Rollback::Intro(details.intro),
            fail,
        );
        details.intro = data;
        self.owner.set_business_details(details);
    }
    pub fn save_location(&self, data: BusinessLocation, fail: Option<Failure>) {
        let mut details = self.owner.business_details();
        if details.location == data {
            return;
        }
        self.save(
            Request::Location(tl::AccountUpdateBusinessLocation {
                flags: Default::default(),
                geo_point: data.point.map(|p| {
                    tl::InputGeoPoint::InputGeoPoint(
                        tl::InputGeoPointData {
                            flags: Default::default(),
                            lat: p.lat,
                            long: p.lon,
                            accuracy_radius: None,
                        }
                        .into(),
                    )
                }),
                address: (!data.address.is_empty()).then(|| data.address.as_bytes().to_vec()),
            }),
            Rollback::Location(details.location),
            fail,
        );
        details.location = data;
        self.owner.set_business_details(details);
    }
    pub fn apply_away_settings(&self, data: AwaySettings) {
        if self.away.borrow().as_ref() == Some(&data) {
            return;
        }
        *self.away.borrow_mut() = Some(data);
        self.away_changed.fire(());
    }
    pub fn save_away_settings(&self, data: AwaySettings, fail: Option<Failure>) {
        let was = self.away.borrow().clone();
        if was.as_ref() == Some(&data) {
            return;
        }
        if !data.is_present() || data.shortcut_id != 0 {
            self.save(
                Request::Away(tl::AccountUpdateBusinessAwayMessage {
                    flags: Default::default(),
                    message: data
                        .is_present()
                        .then(|| away_to_mtp(&data, |k| self.owner.input_user(k))),
                }),
                Rollback::Away(was),
                fail,
            );
        }
        *self.away.borrow_mut() = Some(data);
        self.away_changed.fire(());
    }
    pub fn away_settings_loaded(&self) -> bool {
        self.away.borrow().is_some()
    }
    pub fn away_settings(&self) -> AwaySettings {
        self.away.borrow().clone().unwrap_or_default()
    }
    pub fn away_settings_changed(&self) -> Producer<()> {
        self.away_changed.events()
    }
    pub fn apply_greeting_settings(&self, data: GreetingSettings) {
        if self.greeting.borrow().as_ref() == Some(&data) {
            return;
        }
        *self.greeting.borrow_mut() = Some(data);
        self.greeting_changed.fire(());
    }
    pub fn save_greeting_settings(&self, data: GreetingSettings, fail: Option<Failure>) {
        let was = self.greeting.borrow().clone();
        if was.as_ref() == Some(&data) {
            return;
        }
        if !data.is_present() || data.shortcut_id != 0 {
            self.save(
                Request::Greeting(tl::AccountUpdateBusinessGreetingMessage {
                    flags: Default::default(),
                    message: data
                        .is_present()
                        .then(|| greeting_to_mtp(&data, |k| self.owner.input_user(k))),
                }),
                Rollback::Greeting(was),
                fail,
            );
        }
        *self.greeting.borrow_mut() = Some(data);
        self.greeting_changed.fire(());
    }
    pub fn greeting_settings_loaded(&self) -> bool {
        self.greeting.borrow().is_some()
    }
    pub fn greeting_settings(&self) -> GreetingSettings {
        self.greeting.borrow().clone().unwrap_or_default()
    }
    pub fn greeting_settings_changed(&self) -> Producer<()> {
        self.greeting_changed.events()
    }
    pub fn preload(&self) {
        self.preload_timezones();
    }
    pub fn preload_timezones(&self) {
        if !self.timezones.current().list.is_empty() || self.timezones_request_id.get() != 0 {
            return;
        }
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        (self.send)(
            id,
            Request::Timezones(tl::HelpGetTimezonesList {
                hash: self.timezones_hash.get(),
            }),
        );
        // Original never resets this ID, even on failure/not-modified/empty list.
        self.timezones_request_id.set(id);
    }
    pub fn timezones_value(&self) -> Producer<Timezones> {
        self.preload_timezones();
        self.timezones.value()
    }
    pub fn timezones_loaded(&self) -> bool {
        !self.timezones.current().list.is_empty()
    }
    pub fn handle_reply(&self, reply: Reply) {
        match reply {
            Reply::Saved(id) => {
                self.pending.borrow_mut().remove(&id);
            }
            Reply::Failed(id, error) => {
                let entry = self.pending.borrow_mut().remove(&id);
                if let Some((rollback, fail)) = entry {
                    match rollback {
                        Rollback::Hours(was) => {
                            let mut d = self.owner.business_details();
                            d.hours = was;
                            self.owner.set_business_details(d);
                        }
                        Rollback::Intro(was) => {
                            let mut d = self.owner.business_details();
                            d.intro = was;
                            self.owner.set_business_details(d);
                        }
                        Rollback::Location(was) => {
                            let mut d = self.owner.business_details();
                            d.location = was;
                            self.owner.set_business_details(d);
                        }
                        Rollback::Away(was) => {
                            *self.away.borrow_mut() = was;
                            self.away_changed.fire(());
                        }
                        Rollback::Greeting(was) => {
                            *self.greeting.borrow_mut() = was;
                            self.greeting_changed.fire(());
                        }
                    }
                    if let Some(fail) = fail {
                        fail(error);
                    }
                }
            }
            Reply::Timezones(tl::HelpTimezonesList::HelpTimezonesList(data)) => {
                self.timezones_hash.set(data.hash);
                self.timezones.set(Timezones {
                    list: data
                        .timezones
                        .iter()
                        .map(|tl::Timezone::Timezone(d)| Timezone {
                            id: String::from_utf8_lossy(&d.id).into_owned(),
                            name: String::from_utf8_lossy(&d.name).into_owned(),
                            utc_offset: d.utc_offset,
                        })
                        .collect(),
                });
            }
            Reply::Timezones(tl::HelpTimezonesList::HelpTimezonesListNotModified(_)) => {}
        }
    }
}
pub fn working_hours_to_mtp(data: &WorkingHours) -> tl::BusinessWorkHours {
    tl::BusinessWorkHours::BusinessWorkHours(
        tl::BusinessWorkHoursData {
            flags: Default::default(),
            timezone_id: data.timezone_id.as_bytes().to_vec(),
            weekly_open: data
                .intervals
                .normalized()
                .list
                .into_iter()
                .map(|i| {
                    tl::BusinessWeeklyOpen::BusinessWeeklyOpen(
                        tl::BusinessWeeklyOpenData {
                            start_minute: i.start / 60,
                            end_minute: i.end / 60,
                        }
                        .into(),
                    )
                })
                .collect(),
        }
        .into(),
    )
}
pub fn away_schedule_to_mtp(data: &AwaySchedule) -> tl::BusinessAwayMessageSchedule {
    use tl::BusinessAwayMessageSchedule as S;
    match data.kind {
        AwayScheduleType::Never => panic!("ToMTP requires a non-Never schedule"),
        AwayScheduleType::Always => {
            S::BusinessAwayMessageScheduleAlways(tl::BusinessAwayMessageScheduleAlwaysData {})
        }
        AwayScheduleType::OutsideWorkingHours => S::BusinessAwayMessageScheduleOutsideWorkHours(
            tl::BusinessAwayMessageScheduleOutsideWorkHoursData {},
        ),
        AwayScheduleType::Custom => S::BusinessAwayMessageScheduleCustom(
            tl::BusinessAwayMessageScheduleCustomData {
                start_date: data.custom_interval.start,
                end_date: data.custom_interval.end,
            }
            .into(),
        ),
    }
}
pub fn away_to_mtp(
    data: &AwaySettings,
    input_user: impl Fn(UserKey) -> tl::InputUser,
) -> tl::InputBusinessAwayMessage {
    tl::InputBusinessAwayMessage::InputBusinessAwayMessage(
        tl::InputBusinessAwayMessageData {
            flags: tl::tl::Flags::from_raw(u64::from(data.offline_only)),
            shortcut_id: data.shortcut_id,
            schedule: away_schedule_to_mtp(&data.schedule),
            recipients: for_messages_to_mtp(&data.recipients, input_user),
        }
        .into(),
    )
}
pub fn greeting_to_mtp(
    data: &GreetingSettings,
    input_user: impl Fn(UserKey) -> tl::InputUser,
) -> tl::InputBusinessGreetingMessage {
    tl::InputBusinessGreetingMessage::InputBusinessGreetingMessage(
        tl::InputBusinessGreetingMessageData {
            shortcut_id: data.shortcut_id,
            recipients: for_messages_to_mtp(&data.recipients, input_user),
            no_activity_days: data.no_activity_days,
        }
        .into(),
    )
}
/// Platform local UTC offset and corrected-server-time shift are explicit inputs.
/// Like min_element()->id in C++, requires a nonempty list; no invented timezone.
pub fn find_closest_timezone_id(
    list: &[Timezone],
    local_utc_offset: i32,
    server_time_shift: i32,
) -> &str {
    let delta = local_utc_offset.wrapping_sub(server_time_shift);
    &list
        .iter()
        .min_by_key(|v| {
            let mut d = v.utc_offset.wrapping_sub(delta);
            while d > 12 * 3600 {
                d -= 24 * 3600;
            }
            while d < -12 * 3600 {
                d += 24 * 3600;
            }
            d.abs()
        })
        .expect("FindClosestTimezoneId requires nonempty timezones")
        .id
}
/// Concrete Sender bridge. Owner delivers owned replies on the model thread.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: Request,
    reply: Arc<dyn Fn(Reply) + Send + Sync>,
) {
    macro_rules! save {
        ($r:expr) => {{
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done_plain(move || reply(Reply::Saved(id)))
                .fail(move |e| fail(Reply::Failed(id, e.kind.clone())))
                .send();
        }};
    }
    match request {
        Request::WorkingHours(r) => save!(r),
        Request::Intro(r) => save!(r),
        Request::Location(r) => save!(r),
        Request::Away(r) => save!(r),
        Request::Greeting(r) => save!(r),
        Request::Timezones(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(Reply::Timezones(r)))
                .send();
        }
    }
}
