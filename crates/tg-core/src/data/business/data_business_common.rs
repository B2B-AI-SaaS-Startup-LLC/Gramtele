//! Business value contracts and pure scheduling/TL conversions.
use crate::core::mime_type::MimeDatabase;
use crate::data::{
    data_location::LocationPoint,
    data_media_store::Documents,
    data_peer_id::{peer_from_user, UserId},
    data_store_keys::*,
};
use tg_scheme as tl;
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusinessChatType {
    NewChats = 1,
    ExistingChats = 2,
    Contacts = 4,
    NonContacts = 8,
}
tg_base::flag_type!(BusinessChatType:u32);
pub type BusinessChatTypes = tg_base::flags::Flags<BusinessChatType>;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BusinessChats {
    pub types: BusinessChatTypes,
    pub list: Vec<UserKey>,
}
impl BusinessChats {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.list.is_empty()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BusinessRecipients {
    pub included: BusinessChats,
    pub excluded: BusinessChats,
    pub all_but_excluded: bool,
}
impl BusinessRecipients {
    pub fn make_valid(mut v: Self) -> Self {
        if v.included.is_empty() {
            v.all_but_excluded = true;
        }
        v
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusinessRecipientsType {
    Messages,
    Bots,
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatbotsPermission {
    ViewMessages = 1,
    ReplyToMessages = 2,
    MarkAsRead = 4,
    DeleteSent = 8,
    DeleteReceived = 16,
    EditName = 32,
    EditBio = 64,
    EditUserpic = 128,
    EditUsername = 256,
    ViewGifts = 512,
    SellGifts = 1024,
    GiftSettings = 2048,
    TransferGifts = 4096,
    TransferStars = 8192,
    ManageStories = 16384,
}
tg_base::flag_type!(ChatbotsPermission:u32);
pub type ChatbotsPermissions = tg_base::flags::Flags<ChatbotsPermission>;
fn recipients_flags(data: &BusinessRecipients) -> u64 {
    let c = if data.all_but_excluded {
        &data.excluded
    } else {
        &data.included
    };
    let f = c.types.value();
    u64::from(((f & 1) << 1) | ((f & 2) >> 1) | (f & 12))
        | if c.list.is_empty() { 0 } else { 16 }
        | if data.all_but_excluded { 32 } else { 0 }
}
/// Required inputUser lookup uses actual UserData's self/access hash state.
pub fn for_messages_to_mtp(
    data: &BusinessRecipients,
    input_user: impl Fn(UserKey) -> tl::InputUser,
) -> tl::InputBusinessRecipients {
    let c = if data.all_but_excluded {
        &data.excluded
    } else {
        &data.included
    };
    tl::InputBusinessRecipients::InputBusinessRecipients(
        tl::InputBusinessRecipientsData {
            flags: tl::tl::Flags::from_raw(recipients_flags(data)),
            users: (!c.list.is_empty()).then(|| c.list.iter().copied().map(input_user).collect()),
        }
        .into(),
    )
}
pub fn for_bots_to_mtp(
    data: &BusinessRecipients,
    input_user: impl Fn(UserKey) -> tl::InputUser,
) -> tl::InputBusinessBotRecipients {
    let c = if data.all_but_excluded {
        &data.excluded
    } else {
        &data.included
    };
    let exclude = !data.all_but_excluded && !data.excluded.is_empty();
    tl::InputBusinessBotRecipients::InputBusinessBotRecipients(
        tl::InputBusinessBotRecipientsData {
            flags: tl::tl::Flags::from_raw(recipients_flags(data) | if exclude { 64 } else { 0 }),
            users: (!c.list.is_empty()).then(|| c.list.iter().copied().map(&input_user).collect()),
            exclude_users: exclude
                .then(|| data.excluded.list.iter().copied().map(input_user).collect()),
        }
        .into(),
    )
}
fn chats_from_mtp(
    flags: u64,
    users: Option<&[u64]>,
    ensure_user: &impl Fn(UserId) -> UserKey,
) -> BusinessChats {
    BusinessChats {
        types: BusinessChatTypes::from_raw(
            (((flags & 1) << 1) | ((flags & 2) >> 1) | (flags & 12)) as u32,
        ),
        list: users
            .unwrap_or_default()
            .iter()
            .map(|&u| {
                let k = ensure_user(UserId::new(u));
                assert_eq!(k.peer().id(), peer_from_user(u));
                k
            })
            .collect(),
    }
}
pub fn recipients_from_mtp(
    data: &tl::BusinessRecipients,
    ensure_user: impl Fn(UserId) -> UserKey,
) -> BusinessRecipients {
    let tl::BusinessRecipients::BusinessRecipients(d) = data;
    let all = d.flags.value() & 32 != 0;
    let c = chats_from_mtp(d.flags.value(), d.users.as_deref(), &ensure_user);
    if all {
        BusinessRecipients {
            excluded: c,
            all_but_excluded: true,
            ..Default::default()
        }
    } else {
        BusinessRecipients {
            included: c,
            ..Default::default()
        }
    }
}
pub fn bot_recipients_from_mtp(
    data: &tl::BusinessBotRecipients,
    ensure_user: impl Fn(UserId) -> UserKey,
) -> BusinessRecipients {
    let tl::BusinessBotRecipients::BusinessBotRecipients(d) = data;
    let all = d.flags.value() & 32 != 0;
    let c = chats_from_mtp(d.flags.value(), d.users.as_deref(), &ensure_user);
    if all {
        BusinessRecipients {
            excluded: c,
            all_but_excluded: true,
            ..Default::default()
        }
    } else {
        BusinessRecipients {
            included: c,
            excluded: BusinessChats {
                types: Default::default(),
                list: d
                    .exclude_users
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|&u| ensure_user(UserId::new(u)))
                    .collect(),
            },
            all_but_excluded: false,
        }
    }
}
pub fn permissions_from_mtp(rights: &tl::BusinessBotRights) -> ChatbotsPermissions {
    let tl::BusinessBotRights::BusinessBotRights(d) = rights;
    ChatbotsPermissions::from_raw(1 | ((d.flags.value() as u32 & 0x3fff) << 1))
}
pub fn permissions_to_mtp(rights: ChatbotsPermissions) -> tl::BusinessBotRights {
    tl::BusinessBotRights::BusinessBotRights(
        tl::BusinessBotRightsData {
            flags: tl::tl::Flags::from_raw(((rights.value() >> 1) & 0x3fff) as u64),
        }
        .into(),
    )
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timezone {
    pub id: String,
    pub name: String,
    pub utc_offset: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timezones {
    pub list: Vec<Timezone>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkingInterval {
    pub start: i32,
    pub end: i32,
}
impl WorkingInterval {
    pub const DAY: i32 = 24 * 3600;
    pub const WEEK: i32 = 7 * Self::DAY;
    pub const IN_NEXT_DAY_MAX: i32 = 6 * 3600;
    pub fn is_present(self) -> bool {
        self.start < self.end
    }
    pub fn shifted(self, offset: i32) -> Self {
        Self {
            start: self.start.wrapping_add(offset),
            end: self.end.wrapping_add(offset),
        }
    }
    pub fn united(self, other: Self) -> Self {
        if !self.is_present() {
            other
        } else if !other.is_present() {
            self
        } else {
            Self {
                start: self.start.min(other.start),
                end: self.end.max(other.end),
            }
        }
    }
    pub fn intersected(self, other: Self) -> Self {
        let r = Self {
            start: self.start.max(other.start),
            end: self.end.min(other.end),
        };
        if r.is_present() {
            r
        } else {
            Self::default()
        }
    }
}
const DAY: i32 = WorkingInterval::DAY;
const WEEK: i32 = WorkingInterval::WEEK;
const NEXT: i32 = WorkingInterval::IN_NEXT_DAY_MAX;
fn interval(start: i32, end: i32) -> WorkingInterval {
    WorkingInterval { start, end }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkingIntervals {
    pub list: Vec<WorkingInterval>,
}
fn sort_and_merge(mut v: WorkingIntervals) -> WorkingIntervals {
    v.list.sort_unstable_by_key(|i| i.start);
    let mut i = 0;
    while i < v.list.len() {
        if i > 0
            && v.list[i].is_present()
            && v.list[i - 1].is_present()
            && v.list[i].start <= v.list[i - 1].end
        {
            v.list[i - 1] = v.list[i - 1].united(v.list[i]);
            v.list[i] = WorkingInterval::default();
        }
        if !v.list[i].is_present() {
            v.list.remove(i);
        } else {
            i += 1;
        }
    }
    v
}
fn move_tail_to_front(mut v: WorkingIntervals) -> WorkingIntervals {
    while let Some(last) = v.list.last_mut() {
        let tail = last.intersected(interval(WEEK, WEEK + DAY));
        if !tail.is_present() {
            break;
        }
        last.end = tail.start;
        if !last.is_present() {
            v.list.pop();
        }
        v.list.insert(0, tail.shifted(-WEEK));
    }
    v
}
impl WorkingIntervals {
    pub fn is_present(&self) -> bool {
        self.list.iter().any(|v| v.is_present())
    }
    pub fn normalized(&self) -> Self {
        sort_and_merge(move_tail_to_front(sort_and_merge(self.clone())))
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkingHours {
    pub intervals: WorkingIntervals,
    pub timezone_id: String,
}
impl WorkingHours {
    pub fn normalized(&self) -> Self {
        Self {
            intervals: self.intervals.normalized(),
            timezone_id: self.timezone_id.clone(),
        }
    }
    pub fn is_present(&self) -> bool {
        !self.timezone_id.is_empty() && !self.intervals.list.is_empty()
    }
}
pub fn extract_day_intervals(v: &WorkingIntervals, day: i32) -> WorkingIntervals {
    assert!((0..7).contains(&day));
    let mut r = WorkingIntervals::default();
    for &i in &v.list {
        for (a, b, shift) in [
            (day - 1, day + 2, day),
            (day + 6, day + 9, day + 7),
            (day - 8, day - 5, day - 7),
        ] {
            let now = i.intersected(interval(a * DAY, b * DAY));
            if now.is_present() {
                r.list.push(now.shifted(-shift * DAY));
            }
        }
    }
    r = r.normalized();
    r.list.retain(|i| i.end > 0 && i.start < DAY);
    if let Some(last) = r.list.last_mut() {
        if last.start <= 0 && last.end >= DAY {
            *last = interval(0, DAY);
        } else if last.end > DAY + NEXT {
            *last = last.intersected(interval(0, DAY));
        }
    }
    if let Some(first) = r.list.first_mut() {
        if first.start <= 0 {
            if first.start < 0 && first.end <= NEXT && first.start > -DAY {
                r.list.remove(0);
            } else {
                *first = first.intersected(interval(0, DAY));
                if !first.is_present() {
                    r.list.remove(0);
                }
            }
        }
    }
    r
}
pub fn is_full_open(day: &WorkingIntervals) -> bool {
    day.is_present()
        && matches!(day.list.first(),Some(i) if *i==interval(0,DAY-60)||*i==interval(0,DAY))
}
pub fn remove_day_intervals(v: &WorkingIntervals, day: i32) -> WorkingIntervals {
    let mut r = v.normalized();
    let shifted = interval(0, DAY).shifted(day.wrapping_mul(DAY));
    let before = interval(0, shifted.start);
    let after = interval(shifted.end, WEEK);
    let mut i = 0;
    let mut count = r.list.len();
    while i < count {
        let cur = r.list[i];
        if cur.end <= shifted.start
            || cur.start >= shifted.end
            || (cur.end <= shifted.start + NEXT
                && (cur.start < shifted.start
                    || (day == 0
                        && cur.start == shifted.start
                        && r.list.last().unwrap().end >= WEEK)))
        {
            i += 1;
            continue;
        }
        let first = cur.intersected(before);
        let second = cur.intersected(after);
        if first.is_present() {
            r.list[i] = first;
            // Source overwrites list[i] BEFORE intersecting 'after', losing a
            // spanning interval's suffix. Preserve data_business_common.cpp.
            let second = r.list[i].intersected(after);
            if second.is_present() {
                r.list.push(second);
            }
        } else if second.is_present() {
            r.list[i] = second;
        } else {
            r.list.remove(i);
            count -= 1;
            continue;
        }
        i += 1;
    }
    r.normalized()
}
pub fn replace_day_intervals(
    v: &WorkingIntervals,
    day: i32,
    replacement: WorkingIntervals,
) -> WorkingIntervals {
    let mut r = remove_day_intervals(v, day);
    r.list.extend(
        replacement
            .list
            .into_iter()
            .map(|i| i.shifted(day.wrapping_mul(DAY))),
    );
    r.normalized()
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BusinessLocation {
    pub address: String,
    pub point: Option<LocationPoint>,
}
impl BusinessLocation {
    pub fn is_present(&self) -> bool {
        !self.address.is_empty()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatIntro {
    pub title: String,
    pub description: String,
    pub sticker: Option<DocumentRecordKey>,
}
impl ChatIntro {
    pub fn custom_phrases(&self) -> bool {
        !self.title.is_empty() || !self.description.is_empty()
    }
    pub fn is_present(&self) -> bool {
        self.custom_phrases() || self.sticker.is_some()
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BusinessDetails {
    pub hours: WorkingHours,
    pub location: BusinessLocation,
    pub intro: ChatIntro,
}
impl BusinessDetails {
    pub fn is_present(&self) -> bool {
        self.hours.is_present() || self.location.is_present() || self.intro.is_present()
    }
}
pub fn details_from_mtp(
    documents: &Documents,
    database: &impl MimeDatabase,
    hours: Option<&tl::BusinessWorkHours>,
    location: Option<&tl::BusinessLocation>,
    intro: Option<&tl::BusinessIntro>,
) -> BusinessDetails {
    let mut r = BusinessDetails::default();
    if let Some(tl::BusinessWorkHours::BusinessWorkHours(d)) = hours {
        r.hours.timezone_id = String::from_utf8_lossy(&d.timezone_id).into_owned();
        r.hours.intervals.list = d
            .weekly_open
            .iter()
            .map(|tl::BusinessWeeklyOpen::BusinessWeeklyOpen(d)| {
                interval(
                    d.start_minute.wrapping_mul(60),
                    d.end_minute.wrapping_mul(60),
                )
            })
            .collect();
    }
    if let Some(tl::BusinessLocation::BusinessLocation(d)) = location {
        r.location.address = String::from_utf8_lossy(&d.address).into_owned();
        if let Some(tl::GeoPoint::GeoPoint(p)) = &d.geo_point {
            r.location.point = Some(LocationPoint::from_mtp(p));
        }
    }
    if let Some(tl::BusinessIntro::BusinessIntro(d)) = intro {
        r.intro.title = String::from_utf8_lossy(&d.title).into_owned();
        r.intro.description = String::from_utf8_lossy(&d.description).into_owned();
        if let Some(doc) = &d.sticker {
            let k = documents.process_document(doc, database);
            if documents.read(k, |d| d.sticker().is_some()).unwrap() {
                r.intro.sticker = Some(k);
            }
        }
    }
    r
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AwayScheduleType {
    #[default]
    Never,
    Always,
    OutsideWorkingHours,
    Custom,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AwaySchedule {
    pub kind: AwayScheduleType,
    pub custom_interval: WorkingInterval,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AwaySettings {
    pub recipients: BusinessRecipients,
    pub schedule: AwaySchedule,
    pub shortcut_id: i32,
    pub offline_only: bool,
}
impl AwaySettings {
    pub fn is_present(&self) -> bool {
        self.schedule.kind != AwayScheduleType::Never
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GreetingSettings {
    pub recipients: BusinessRecipients,
    pub no_activity_days: i32,
    pub shortcut_id: i32,
}
impl GreetingSettings {
    pub fn is_present(&self) -> bool {
        self.no_activity_days > 0
    }
}
pub fn away_from_mtp(
    message: Option<&tl::BusinessAwayMessage>,
    ensure_user: impl Fn(UserId) -> UserKey,
) -> AwaySettings {
    let Some(tl::BusinessAwayMessage::BusinessAwayMessage(d)) = message else {
        return AwaySettings::default();
    };
    let schedule = match &d.schedule {
        tl::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleAlways(_) => AwaySchedule {
            kind: AwayScheduleType::Always,
            ..Default::default()
        },
        tl::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleOutsideWorkHours(_) => {
            AwaySchedule {
                kind: AwayScheduleType::OutsideWorkingHours,
                ..Default::default()
            }
        }
        tl::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleCustom(d) => AwaySchedule {
            kind: AwayScheduleType::Custom,
            custom_interval: interval(d.start_date, d.end_date),
        },
    };
    AwaySettings {
        recipients: recipients_from_mtp(&d.recipients, ensure_user),
        schedule,
        shortcut_id: d.shortcut_id,
        offline_only: d.flags.value() & 1 != 0,
    }
}
pub fn greeting_from_mtp(
    message: Option<&tl::BusinessGreetingMessage>,
    ensure_user: impl Fn(UserId) -> UserKey,
) -> GreetingSettings {
    let Some(tl::BusinessGreetingMessage::BusinessGreetingMessage(d)) = message else {
        return GreetingSettings::default();
    };
    GreetingSettings {
        recipients: recipients_from_mtp(&d.recipients, ensure_user),
        no_activity_days: d.no_activity_days,
        shortcut_id: d.shortcut_id,
    }
}
