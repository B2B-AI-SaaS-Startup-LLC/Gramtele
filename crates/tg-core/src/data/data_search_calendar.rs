//! Calendar cache/request model. Local-calendar conversion33 and History media18–20
//! are mandatory facts; actual DynamicImage thumbnails remain5, never fake images.
use super::{
    data_birthday::CalendarDate,
    data_msg_id::{FullMsgId, MsgId},
    data_search_controller::prepare_search_filter,
    data_store_keys::*,
};
use crate::storage::storage_shared_media::SharedMediaType;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use tg_scheme as tl;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CalendarPeriod {
    pub date: i32,
    pub min_msg_id: MsgId,
    pub max_msg_id: MsgId,
    pub count: i32,
}
impl From<&tl::SearchResultsCalendarPeriod> for CalendarPeriod {
    fn from(v: &tl::SearchResultsCalendarPeriod) -> Self {
        let tl::SearchResultsCalendarPeriod::SearchResultsCalendarPeriod(d) = v;
        Self {
            date: d.date,
            min_msg_id: MsgId::new(i64::from(d.min_msg_id)),
            max_msg_id: MsgId::new(i64::from(d.max_msg_id)),
            count: d.count,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct CalendarResult {
    pub periods: Vec<CalendarPeriod>,
    pub count: i32,
    pub min_date: i32,
    pub min_msg_id: MsgId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonthKey {
    pub peer: PeerKey,
    pub year: i32,
    pub month: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthDay {
    pub date: i32,
    pub msg_id: MsgId,
    pub origin: FullMsgId,
    pub photo: Option<PhotoRecordKey>,
    pub document: Option<DocumentRecordKey>,
}
pub trait Calendar {
    fn date(&self, time: i32) -> CalendarDate;
    fn day_start(&self, time: i32) -> i32;
    fn month_start(&self, year: i32, month: i32) -> i32;
    fn month_end(&self, year: i32, month: i32) -> i32;
    fn next_month_start(&self, year: i32, month: i32) -> i32;
}
type Finish = Rc<dyn Fn(Vec<MonthDay>)>;
#[derive(Default)]
struct MonthData {
    cache: Vec<MonthDay>,
    callbacks: Vec<Finish>,
    request: i32,
    offset_id: MsgId,
    offset_date: i32,
    loaded: bool,
    deferred: bool,
    used_date: i32,
}
#[derive(Clone)]
pub enum Effect {
    Send {
        key: MonthKey,
        id: i32,
        request: tl::MessagesGetSearchResultsCalendar,
    },
}
pub struct SearchCalendarController {
    peer: PeerKey,
    input: tl::InputPeer,
    type_: SharedMediaType,
    calendar: Rc<dyn Calendar>,
    months: RefCell<BTreeMap<MonthKey, MonthData>>,
    effect: Rc<dyn Fn(Effect)>,
}
impl SearchCalendarController {
    pub fn new(
        peer: PeerKey,
        input: tl::InputPeer,
        type_: SharedMediaType,
        calendar: Rc<dyn Calendar>,
        effect: Rc<dyn Fn(Effect)>,
    ) -> Rc<Self> {
        Rc::new(Self {
            peer,
            input,
            type_,
            calendar,
            months: RefCell::default(),
            effect,
        })
    }
    fn key(&self, date: i32) -> MonthKey {
        let d = self.calendar.date(date);
        MonthKey {
            peer: self.peer,
            year: d.year,
            month: d.month,
        }
    }
    pub fn month_days(&self, date: i32, finish: Finish) {
        let key = self.key(date);
        let cached = self
            .months
            .borrow()
            .get(&key)
            .filter(|m| m.loaded)
            .map(|m| m.cache.clone());
        if let Some(list) = cached {
            finish(list);
            return;
        }
        let send = {
            let mut months = self.months.borrow_mut();
            let newest = months
                .iter()
                .filter(|(_, m)| m.request != 0)
                .map(|(k, _)| *k)
                .max();
            let m = months.entry(key).or_default();
            m.callbacks.push(finish);
            if m.request == 0 && !m.deferred {
                if newest.is_some_and(|n| key < n) {
                    m.deferred = true;
                    false
                } else {
                    true
                }
            } else {
                false
            }
        };
        if send {
            self.perform_month_request(key)
        }
    }
    pub fn requesting_newest(&self) -> Option<MonthKey> {
        self.months
            .borrow()
            .iter()
            .filter(|(_, m)| m.request != 0)
            .map(|(k, _)| *k)
            .max()
    }
    pub fn send_deferred_requests(&self) {
        let keys = self
            .months
            .borrow()
            .iter()
            .filter(|(_, m)| m.deferred && !m.loaded && m.request == 0)
            .map(|(k, _)| *k)
            .collect::<Vec<_>>();
        for key in keys {
            let send = {
                let mut m = self.months.borrow_mut();
                let d = m.get_mut(&key).unwrap();
                if d.loaded || d.request != 0 {
                    false
                } else {
                    d.deferred = false;
                    true
                }
            };
            if send {
                self.perform_month_request(key)
            }
        }
    }
    pub fn perform_month_request(&self, key: MonthKey) {
        let end = self.calendar.month_end(key.year, key.month);
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        let (offset, date) = {
            let mut months = self.months.borrow_mut();
            let m = months.entry(key).or_default();
            m.request = id;
            let date = if m.offset_date == 0 {
                end
            } else {
                m.offset_date
            };
            m.used_date = date;
            (m.offset_id, date)
        };
        (self.effect)(Effect::Send {
            key,
            id,
            request: tl::MessagesGetSearchResultsCalendar::new(
                Default::default(),
                self.input.clone(),
                None,
                prepare_search_filter(Some(self.type_)),
                offset.bare as i32,
                date,
            ),
        });
    }
    pub fn finish_month(&self, key: MonthKey) {
        let (callbacks, list) = {
            let mut months = self.months.borrow_mut();
            let m = months.entry(key).or_default();
            m.loaded = true;
            (std::mem::take(&mut m.callbacks), m.cache.clone())
        };
        for cb in callbacks {
            cb(list.clone())
        }
    }
    pub fn failed(&self, key: MonthKey) {
        self.months.borrow_mut().entry(key).or_default().request = 0;
        self.finish_month(key);
        self.send_deferred_requests()
    }
    /// Feed actual message-store media, in server message order. First eligible media per day wins.
    pub fn collect_day_media(
        &self,
        media: impl IntoIterator<Item = (i32, MonthDay, bool)>,
    ) -> BTreeMap<i32, MonthDay> {
        let mut result = BTreeMap::new();
        for (date, mut day, document_is_video) in media {
            if day.photo.is_some() {
                day.document = None;
            }
            if day.photo.is_some() || day.document.is_some() && document_is_video {
                result.entry(self.calendar.day_start(date)).or_insert(day);
            }
        }
        result
    }
    pub fn fill_month(
        &self,
        key: MonthKey,
        periods: &[CalendarPeriod],
        media: &BTreeMap<i32, MonthDay>,
    ) {
        let mut months = self.months.borrow_mut();
        let m = months.entry(key).or_default();
        let mut seen = m.cache.iter().map(|d| d.date).collect::<BTreeSet<_>>();
        for p in periods {
            let d = self.calendar.date(p.date);
            if p.max_msg_id.is_zero() || d.year != key.year || d.month != key.month {
                continue;
            }
            let day = self.calendar.day_start(p.date);
            if seen.contains(&day) {
                continue;
            }
            if let Some(value) = media.get(&day) {
                seen.insert(day);
                let mut value = *value;
                value.date = day;
                value.msg_id = p.max_msg_id;
                m.cache.push(value);
            }
        }
    }
    pub fn reply(
        &self,
        key: MonthKey,
        result: &tl::MessagesSearchResultsCalendar,
        day_media: BTreeMap<i32, MonthDay>,
    ) {
        let tl::MessagesSearchResultsCalendar::MessagesSearchResultsCalendar(d) = result;
        let (used, no_more) = {
            let mut months = self.months.borrow_mut();
            let m = months.entry(key).or_default();
            m.request = 0;
            let unchanged =
                m.offset_id.bare == i64::from(d.min_msg_id) && m.offset_date == d.min_date;
            m.offset_id = MsgId::new(i64::from(d.min_msg_id));
            m.offset_date = d.min_date;
            (m.used_date, d.min_msg_id == 0 || unchanged)
        };
        let periods = d
            .periods
            .iter()
            .map(CalendarPeriod::from)
            .collect::<Vec<_>>();
        self.process_month_data(key, &periods, &day_media, d.min_date, used, no_more);
    }
    pub fn process_month_data(
        &self,
        key: MonthKey,
        periods: &[CalendarPeriod],
        media: &BTreeMap<i32, MonthDay>,
        min_date: i32,
        offset_date: i32,
        no_more: bool,
    ) {
        self.fill_month(key, periods, media);
        let before = min_date != 0
            && self.calendar.date(min_date)
                < CalendarDate {
                    year: key.year,
                    month: key.month,
                    day: 1,
                };
        if !no_more && !before {
            if self
                .months
                .borrow()
                .get(&key)
                .is_none_or(|m| m.request == 0)
            {
                self.perform_month_request(key)
            }
            return;
        }
        self.finish_month(key);
        self.fill_covered_months(key, periods, media, offset_date, no_more);
        self.send_deferred_requests();
    }
    pub fn fill_covered_months(
        &self,
        key: MonthKey,
        periods: &[CalendarPeriod],
        media: &BTreeMap<i32, MonthDay>,
        offset: i32,
        no_more: bool,
    ) {
        let mut oldest = 0;
        let mut months = BTreeSet::new();
        for p in periods {
            if oldest == 0 || p.date < oldest {
                oldest = p.date
            }
            let d = self.calendar.date(p.date);
            months.insert((d.year, d.month));
        }
        if oldest == 0 {
            return;
        }
        let oldest_day = self.calendar.date(oldest);
        for (year, month) in months {
            if (year, month) == (key.year, key.month)
                || !no_more
                    && (year, month, 1) <= (oldest_day.year, oldest_day.month, oldest_day.day)
                || self.calendar.next_month_start(year, month) > offset
            {
                continue;
            }
            let covered = MonthKey {
                peer: key.peer,
                year,
                month,
            };
            if self
                .months
                .borrow()
                .get(&covered)
                .is_some_and(|m| m.loaded || m.request != 0)
            {
                continue;
            }
            self.fill_month(covered, periods, media);
            self.finish_month(covered);
        }
    }
    pub fn resolve_msg_id_by_date(&self, date: i32) -> Option<MsgId> {
        let key = self.key(date);
        let day = self.calendar.day_start(date);
        self.months
            .borrow()
            .get(&key)?
            .cache
            .iter()
            .find(|d| d.date == day)
            .map(|d| d.msg_id)
    }
}
