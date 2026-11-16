//! Chat-filter rules and wire model. `wire_title` is the existing generated TL
//! value only. Internal title rendering/conversion is partial01/question1.
//! History membership is represented by scoped peer keys, not replacement histories.
use super::data_store_keys::{PeerKey, ScopedKey, SessionScopeId};
use std::collections::BTreeSet;
use tg_scheme as tl;
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatFilterFlag {
    Contacts = 1,
    NonContacts = 2,
    Groups = 4,
    Channels = 8,
    Bots = 16,
    NoMuted = 32,
    NoRead = 64,
    NoArchived = 128,
    RulesMask = 255,
    Chatlist = 256,
    HasMyLinks = 512,
    StaticTitle = 1024,
    NewChats = 2048,
    ExistingChats = 4096,
}
tg_base::flag_type!(ChatFilterFlag:u16);
pub type ChatFilterFlags = tg_base::flags::Flags<ChatFilterFlag>;
use ChatFilterFlag as F;
/// Required current History facts; no defaults or core object implementation.
#[derive(Clone, Copy, Debug)]
pub struct FilterHistoryFacts {
    pub category: ChatFilterFlag,
    pub community: bool,
    pub muted: bool,
    pub unread: bool,
    pub mention: bool,
    pub folder_known: bool,
    pub archived: bool,
    pub fake_unread_while_opened: bool,
}
#[derive(Clone, Default)]
pub struct ChatFilter {
    pub id: i32,
    pub wire_title: tl::TextWithEntities,
    pub icon_emoji: String,
    pub color_index: Option<u8>,
    pub flags: ChatFilterFlags,
    pub always: BTreeSet<PeerKey>,
    pub pinned: Vec<PeerKey>,
    pub never: BTreeSet<PeerKey>,
}
impl PartialEq for ChatFilter {
    fn eq(&self, o: &Self) -> bool {
        tg_scheme::tl::serialize(&self.wire_title) == tg_scheme::tl::serialize(&o.wire_title)
            && self.icon_emoji == o.icon_emoji
            && self.color_index == o.color_index
            && self.flags == o.flags
            && self.always == o.always
            && self.never == o.never
    }
}
impl Eq for ChatFilter {}
impl ChatFilter {
    pub fn with_id(&self, id: i32) -> Self {
        let mut r = self.clone();
        r.id = id;
        r
    }
    pub fn with_color_index(&self, color: Option<u8>) -> Self {
        let mut r = self.clone();
        r.color_index = color;
        r
    }
    pub fn with_chatlist(&self, chatlist: bool, has_my_links: bool) -> Self {
        let mut r = self.clone();
        r.flags &= F::RulesMask;
        if chatlist {
            r.flags |= F::Chatlist;
            if has_my_links {
                r.flags |= F::HasMyLinks
            }
        }
        r
    }
    pub fn static_title(&self) -> bool {
        (self.flags & F::StaticTitle).any()
    }
    pub fn chatlist(&self) -> bool {
        (self.flags & F::Chatlist).any()
    }
    pub fn has_my_links(&self) -> bool {
        (self.flags & F::HasMyLinks).any()
    }
    pub fn contains(&self, key: PeerKey, f: FilterHistoryFacts, ignore_fake_unread: bool) -> bool {
        if self.never.contains(&key) {
            return false;
        }
        if f.community {
            return self.always.contains(&key);
        }
        ((self.flags & f.category).any()
            && ((self.flags & F::NoMuted).is_empty()
                || !f.muted
                || (f.mention && f.folder_known && !f.archived))
            && ((self.flags & F::NoRead).is_empty()
                || f.unread
                || f.mention
                || (!ignore_fake_unread && f.fake_unread_while_opened))
            && ((self.flags & F::NoArchived).is_empty() || (f.folder_known && !f.archived)))
            || self.always.contains(&key)
    }
    pub fn without_always(&self, key: PeerKey, f: FilterHistoryFacts) -> Self {
        let mut r = self.clone();
        if can_remove_from_chat_filter(&r, key, f) {
            r.always.remove(&key);
        }
        r
    }
    /// Resolver is the real PeerFromInputMTP/Session history lookup. It also applies
    /// access hashes for Chatlist input peers, as in the original FromTL.
    pub fn from_tl(
        scope: SessionScopeId,
        v: &tl::DialogFilter,
        resolve: impl Fn(&tl::InputPeer, bool) -> Option<PeerKey>,
    ) -> Self {
        let mut r = Self::default();
        let (pin, inc, exc, chatlist) = match v {
            tl::DialogFilter::DialogFilter(d) => {
                r.id = d.id;
                r.wire_title = d.title.clone();
                r.icon_emoji =
                    String::from_utf8_lossy(d.vemoticon().map_or(&[], Vec::as_slice)).into();
                r.color_index = d.vcolor().map(|v| v as u8);
                for (b, f) in [
                    (d.is_contacts(), F::Contacts),
                    (d.is_non_contacts(), F::NonContacts),
                    (d.is_groups(), F::Groups),
                    (d.is_broadcasts(), F::Channels),
                    (d.is_bots(), F::Bots),
                    (d.is_exclude_muted(), F::NoMuted),
                    (d.is_exclude_read(), F::NoRead),
                    (d.is_exclude_archived(), F::NoArchived),
                    (d.is_title_noanimate(), F::StaticTitle),
                ] {
                    if b {
                        r.flags |= f
                    }
                }
                (
                    &d.pinned_peers,
                    &d.include_peers,
                    d.exclude_peers.as_slice(),
                    false,
                )
            }
            tl::DialogFilter::DialogFilterChatlist(d) => {
                r.id = d.id;
                r.wire_title = d.title.clone();
                r.icon_emoji =
                    String::from_utf8_lossy(d.vemoticon().map_or(&[], Vec::as_slice)).into();
                r.color_index = d.vcolor().map(|v| v as u8);
                r.flags |= F::Chatlist;
                if d.is_has_my_invites() {
                    r.flags |= F::HasMyLinks
                }
                if d.is_title_noanimate() {
                    r.flags |= F::StaticTitle
                }
                (&d.pinned_peers, &d.include_peers, &[][..], true)
            }
            tl::DialogFilter::DialogFilterDefault(_) => return r,
        };
        let get =
            |p: &tl::InputPeer| resolve(p, chatlist).inspect(|k| assert_eq!(k.scope(), scope));
        r.pinned = pin.iter().filter_map(&get).collect();
        r.always = inc.iter().filter_map(&get).collect();
        r.always.extend(r.pinned.iter().copied());
        r.never = exc.iter().filter_map(get).collect();
        r
    }
    /// Normalises to the original writer (emoticon always present; pins removed
    /// from include; Chatlist's has_my_invites is intentionally not written).
    pub fn tl(
        &self,
        replace_id: i32,
        input: impl Fn(PeerKey) -> tl::InputPeer,
    ) -> tl::DialogFilter {
        let id = if replace_id != 0 { replace_id } else { self.id };
        let pinned = self.pinned.iter().map(|p| input(*p)).collect();
        let include = self
            .always
            .iter()
            .filter(|p| !self.pinned.contains(p))
            .map(|p| input(*p))
            .collect();
        if self.chatlist() {
            let mut f = tl::DialogFilterChatlistData::F_EMOTICON;
            if self.color_index.is_some() {
                f |= tl::DialogFilterChatlistData::F_COLOR
            }
            if self.static_title() {
                f |= tl::DialogFilterChatlistData::F_TITLE_NOANIMATE
            }
            tl::mtp_dialog_filter_chatlist(
                f,
                id,
                self.wire_title.clone(),
                Some(self.icon_emoji.as_bytes().to_vec()),
                self.color_index.map(i32::from),
                pinned,
                include,
            )
        } else {
            let mut f = tl::DialogFilterData::F_EMOTICON;
            if self.color_index.is_some() {
                f |= tl::DialogFilterData::F_COLOR
            }
            if self.static_title() {
                f |= tl::DialogFilterData::F_TITLE_NOANIMATE
            }
            for (a, b) in [
                (F::Contacts, tl::DialogFilterData::F_CONTACTS),
                (F::NonContacts, tl::DialogFilterData::F_NON_CONTACTS),
                (F::Groups, tl::DialogFilterData::F_GROUPS),
                (F::Channels, tl::DialogFilterData::F_BROADCASTS),
                (F::Bots, tl::DialogFilterData::F_BOTS),
                (F::NoMuted, tl::DialogFilterData::F_EXCLUDE_MUTED),
                (F::NoRead, tl::DialogFilterData::F_EXCLUDE_READ),
                (F::NoArchived, tl::DialogFilterData::F_EXCLUDE_ARCHIVED),
            ] {
                if (self.flags & a).any() {
                    f |= b
                }
            }
            tl::mtp_dialog_filter(
                f,
                id,
                self.wire_title.clone(),
                Some(self.icon_emoji.as_bytes().to_vec()),
                self.color_index.map(i32::from),
                pinned,
                include,
                self.never.iter().map(|p| input(*p)).collect(),
            )
        }
    }
}
pub fn can_remove_from_chat_filter(
    filter: &ChatFilter,
    key: PeerKey,
    f: FilterHistoryFacts,
) -> bool {
    (filter.always.len() > 1 || (filter.flags & !(F::NoRead | F::NoArchived | F::NoMuted)).any())
        && filter.contains(key, f, false)
}
#[derive(Clone)]
pub struct SuggestedFilter {
    pub filter: ChatFilter,
    pub description: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatFilterLink {
    pub id: i32,
    pub url: String,
    pub title: String,
    pub chats: Vec<PeerKey>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagColorChanged {
    pub filter_id: i32,
    pub color_existence_changed: bool,
}
/// State transitions are deliberately separate from delivery to real lists/API.
#[derive(Clone)]
pub enum FilterEffect {
    ListChanged,
    RulesChanged {
        before: ChatFilter,
        after: ChatFilter,
    },
    ApplyPinned(i32, Vec<PeerKey>),
    ChatlistChanged(i32),
    TagColorChanged(TagColorChanged),
    RequestMoreDialogs,
}
#[derive(Default)]
pub struct ChatFilters {
    pub(crate) list: Vec<ChatFilter>,
    pub(crate) loaded: bool,
    pub(crate) reloading: bool,
    pub exceptions_to_load: std::collections::VecDeque<i32>,
    suggested: Vec<SuggestedFilter>,
    suggested_last_received: i64,
}
impl ChatFilters {
    pub fn new() -> Self {
        Self {
            list: vec![ChatFilter::default()],
            ..Default::default()
        }
    }
    pub fn list(&self) -> &[ChatFilter] {
        &self.list
    }
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    pub fn has(&self) -> bool {
        self.list.len() > 1
    }
    pub fn clear(&mut self) {
        self.list.clear()
    }
    pub fn mark_reloading(&mut self) {
        self.reloading = true
    }
    pub fn load_failed(&mut self) -> Vec<FilterEffect> {
        if std::mem::take(&mut self.reloading) {
            vec![FilterEffect::ListChanged]
        } else {
            vec![]
        }
    }
    pub(crate) fn apply_change(
        &mut self,
        pos: usize,
        updated: ChatFilter,
        e: &mut Vec<FilterEffect>,
    ) -> bool {
        let old = &self.list[pos];
        assert_eq!(old.id, updated.id);
        let id = old.id;
        let exceptions = old.always != updated.always;
        let rules = exceptions
            || (old.flags & F::RulesMask) != (updated.flags & F::RulesMask)
            || old.never != updated.never;
        let pinned = old.pinned != updated.pinned;
        let chatlist =
            old.chatlist() != updated.chatlist() || old.has_my_links() != updated.has_my_links();
        let list = rules
            || pinned
            || tg_scheme::tl::serialize(&old.wire_title)
                != tg_scheme::tl::serialize(&updated.wire_title)
            || old.static_title() != updated.static_title()
            || old.icon_emoji != updated.icon_emoji;
        let color = old.color_index != updated.color_index;
        let existence = old.color_index.is_some() != updated.color_index.is_some();
        if !list && !chatlist && !color {
            return false;
        }
        let was = std::mem::replace(&mut self.list[pos], updated);
        let now = &self.list[pos];
        if rules {
            e.push(FilterEffect::RulesChanged {
                before: was,
                after: now.clone(),
            });
            if exceptions && !now.always.is_empty() {
                self.exceptions_to_load.push_back(id);
                e.push(FilterEffect::RequestMoreDialogs)
            }
        }
        if pinned {
            e.push(FilterEffect::ApplyPinned(id, now.pinned.clone()))
        }
        if chatlist {
            e.push(FilterEffect::ChatlistChanged(id))
        }
        if color {
            e.push(FilterEffect::TagColorChanged(TagColorChanged {
                filter_id: id,
                color_existence_changed: existence,
            }))
        }
        list
    }
    pub fn set(&mut self, f: ChatFilter) -> Vec<FilterEffect> {
        if f.id == 0 {
            return vec![];
        }
        let mut e = vec![];
        if let Some(p) = self.list.iter().position(|v| v.id == f.id) {
            if self.apply_change(p, f, &mut e) {
                e.push(FilterEffect::ListChanged)
            }
        } else {
            self.list.push(ChatFilter::default().with_id(f.id));
            self.apply_change(self.list.len() - 1, f, &mut e);
            e.push(FilterEffect::ListChanged)
        }
        e
    }
    pub(crate) fn apply_remove(&mut self, pos: usize, e: &mut Vec<FilterEffect>) {
        let old = self.list.remove(pos); // Remove before any unpin side effects (original crash fix).
        if !old.always.is_empty() || !old.never.is_empty() || (old.flags & F::RulesMask).any() {
            e.push(FilterEffect::RulesChanged {
                before: old.clone(),
                after: ChatFilter::default().with_id(old.id),
            })
        }
        if !old.pinned.is_empty() {
            e.push(FilterEffect::ApplyPinned(old.id, vec![]))
        }
        if old.chatlist() || old.has_my_links() {
            e.push(FilterEffect::ChatlistChanged(old.id))
        }
        if old.color_index.is_some() {
            e.push(FilterEffect::TagColorChanged(TagColorChanged {
                filter_id: old.id,
                color_existence_changed: true,
            }))
        }
    }
    pub fn remove(&mut self, id: i32) -> Vec<FilterEffect> {
        let mut e = vec![];
        if let Some(p) = self.list.iter().position(|f| f.id == id) {
            self.apply_remove(p, &mut e);
            e.push(FilterEffect::ListChanged)
        }
        e
    }
    pub fn received(&mut self, list: Vec<ChatFilter>, hide_all: bool) -> Vec<FilterEffect> {
        let n = list.len();
        let mut pos = 0;
        let mut changed = false;
        let mut e = vec![];
        for f in list {
            if hide_all && f.id == 0 && n > 1 {
                continue;
            }
            let i = self.list[pos..]
                .iter()
                .position(|v| v.id == f.id)
                .map(|i| i + pos);
            if let Some(i) = i {
                if i == pos {
                    changed |= self.apply_change(pos, f, &mut e)
                } else {
                    self.list.swap(i, pos);
                    self.apply_change(pos, f, &mut e);
                    changed = true
                }
            } else {
                self.list.insert(pos, ChatFilter::default().with_id(f.id));
                self.apply_change(pos, f, &mut e);
                changed = true
            }
            pos += 1;
        }
        while pos < self.list.len() {
            self.apply_remove(pos, &mut e);
            changed = true
        }
        if !hide_all && !self.list.iter().any(|f| f.id == 0) {
            self.list.insert(0, ChatFilter::default())
        }
        if changed || !self.loaded || self.reloading {
            self.loaded = true;
            self.reloading = false;
            e.push(FilterEffect::ListChanged)
        }
        e
    }
    pub fn move_all_to_front(&mut self) {
        if self.list.first().is_some_and(|f| f.id == 0) {
            return;
        }
        if let Some(i) = self.list.iter().position(|f| f.id == 0) {
            self.list.remove(i);
        }
        self.list.insert(0, ChatFilter::default());
    }
    /// Success yields events, including an empty vector for a valid unchanged order.
    pub fn apply_order(&mut self, order: &[i32]) -> Option<Vec<FilterEffect>> {
        if order.len() != self.list.len() {
            return None;
        }
        let mut indices = self.list.iter().map(|f| f.id).collect::<Vec<_>>();
        for (i, id) in order.iter().enumerate() {
            let p = indices[i..].iter().position(|v| v == id)? + i;
            indices.swap(i, p)
        }
        let mut changed = false;
        for (i, id) in order.iter().enumerate() {
            let p = self.list[i..].iter().position(|f| f.id == *id).unwrap() + i;
            if i != p {
                self.list.swap(i, p);
                changed = true
            }
        }
        Some(if changed {
            vec![FilterEffect::ListChanged]
        } else {
            vec![]
        })
    }
    pub fn apply_updated_pinned(
        &mut self,
        id: i32,
        dialogs: &[PeerKey],
        limit: usize,
    ) -> Vec<FilterEffect> {
        let mut f = self
            .list
            .iter()
            .find(|f| f.id == id)
            .expect("filter not found")
            .clone();
        f.pinned.clear();
        for p in dialogs {
            if f.always.contains(p) {
                f.pinned.push(*p)
            } else if f.always.len() < limit {
                f.always.insert(*p);
                f.pinned.push(*p)
            }
        }
        self.set(f)
    }
    pub fn archive_needed(&self) -> bool {
        self.list
            .iter()
            .any(|f| (f.flags & F::NoArchived).is_empty())
    }
    pub fn lookup_id(&self, index: i32, premium: bool, hide_all: bool) -> i32 {
        if index < 0 || index as usize >= self.list.len() {
            return 0;
        }
        let index = index as usize;
        // AyuGram: invalid index must safely return zero when hiding All Chats.
        if premium || self.list[0].id == 0 || hide_all {
            return self.list[index].id;
        }
        let all = self
            .list
            .iter()
            .position(|f| f.id == 0)
            .unwrap_or(self.list.len());
        if index == 0 {
            0
        } else if index <= all {
            self.list[index - 1].id
        } else {
            self.list[index].id
        }
    }
    pub fn hide_all_changed(&mut self, hide: bool) -> Vec<FilterEffect> {
        if !self.loaded {
            return vec![];
        }
        if hide {
            if self.list.len() <= 1 {
                return vec![];
            }
            if let Some(i) = self.list.iter().position(|f| f.id == 0) {
                self.list.remove(i);
                return vec![FilterEffect::ListChanged];
            }
        } else if !self.list.iter().any(|f| f.id == 0) {
            self.list.insert(0, ChatFilter::default());
            return vec![FilterEffect::ListChanged];
        }
        vec![]
    }
    pub fn suggested_due(&self, now: i64, request_pending: bool) -> bool {
        !request_pending
            && (self.suggested_last_received <= 0
                || now - self.suggested_last_received >= 7_200_000)
    }
    pub fn suggested_received(&mut self, list: Vec<SuggestedFilter>, now: i64) {
        self.suggested = list;
        self.suggested_last_received = now
    }
    pub fn suggested_failed(&mut self, now: i64) {
        self.suggested_last_received = now + 3_600_000
    } // Failure delays next request by three hours, not one.
    pub fn suggested_loaded(&self) -> bool {
        self.suggested_last_received > 0
    }
    pub fn suggested_filters(&self) -> &[SuggestedFilter] {
        &self.suggested
    }
}
