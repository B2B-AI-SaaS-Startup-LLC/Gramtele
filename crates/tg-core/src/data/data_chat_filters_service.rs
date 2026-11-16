//! Actual TL requests with explicit owner delivery; no server or History substitute.
use super::{data_chat_filters::*, data_peer_id::peer_from_mtp, data_store_keys::*};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{Timer, TimerHost};
use tg_rpl::{EventStream, Producer, Variable};
use tg_scheme as tl;
#[derive(Clone)]
pub enum Request {
    Load(tl::MessagesGetDialogFilters),
    Suggested(tl::MessagesGetSuggestedDialogFilters),
    ToggleTags(tl::MessagesToggleDialogFilterTags),
    Order(tl::MessagesUpdateDialogFiltersOrder),
    Edit(tl::ChatlistsEditExportedInvite),
    Delete(tl::ChatlistsDeleteExportedInvite),
    Links(tl::ChatlistsGetExportedInvites),
    More(tl::ChatlistsGetChatlistUpdates),
    Hide(tl::ChatlistsHideChatlistUpdates),
    Exceptions(tl::MessagesGetPeerDialogs),
}
#[derive(Clone)]
pub enum Effect {
    Send {
        id: i32,
        after: i32,
        request: Request,
    },
    Cancel(i32),
    Model(FilterEffect),
    LoadOnMain,
    CompletePreloadedOnMain,
    ProcessUsers(Vec<tl::User>),
    ProcessChats(Vec<tl::Chat>),
    ApplyPeerDialogs(tl::MessagesPeerDialogs),
    RequestMoreDialogs,
}
/// Every fact is read from the actual Session/History by the owner. No defaults.
pub trait FilterOwner {
    fn resolve_input(&self, input: &tl::InputPeer, chatlist: bool) -> Option<PeerKey>;
    fn input(&self, key: PeerKey) -> tl::InputPeer;
    fn peer(&self, id: super::data_peer_id::PeerId) -> PeerKey;
    fn folder_known(&self, key: PeerKey) -> bool;
    fn chats_full_size(&self) -> i32;
    fn hide_all_chats(&self) -> bool;
    fn updates_period_ms(&self) -> i64;
    fn effect(&self, effect: Effect);
}
#[derive(Default)]
struct More {
    missing: Vec<PeerKey>,
    last: i64,
    request: i32,
    watching: Weak<()>,
}
#[derive(Default)]
struct State {
    model: ChatFilters,
    load: i32,
    toggle: i32,
    order: i32,
    after: i32,
    suggested: i32,
    links_request: i32,
    exceptions: i32,
    links: BTreeMap<i32, Vec<ChatFilterLink>>,
    more: BTreeMap<i32, More>,
}
pub struct ChatFiltersService {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    owner: Rc<dyn FilterOwner>,
    state: RefCell<State>,
    changed: EventStream<()>,
    chatlist_changed: EventStream<i32>,
    color_changed: EventStream<TagColorChanged>,
    suggested_updated: EventStream<()>,
    links_updated: EventStream<i32>,
    more_updated: EventStream<i32>,
    tags: Variable<bool>,
    host: Rc<TimerHost>,
    timer: Timer,
}
impl ChatFiltersService {
    pub fn new(scope: &SessionScope, owner: Rc<dyn FilterOwner>, host: &Rc<TimerHost>) -> Rc<Self> {
        let s = Rc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            Self {
                scope: scope.id(),
                alive: scope.downgrade(),
                owner,
                state: RefCell::new(State {
                    model: ChatFilters::new(),
                    ..Default::default()
                }),
                changed: EventStream::new(),
                chatlist_changed: EventStream::new(),
                color_changed: EventStream::new(),
                suggested_updated: EventStream::new(),
                links_updated: EventStream::new(),
                more_updated: EventStream::new(),
                tags: Variable::new(false),
                host: host.clone(),
                timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            if s.alive.upgrade().is_some() {
                                s.check_load_more_chats_lists()
                            }
                        }
                    })),
                ),
            }
        });
        s.owner.effect(Effect::LoadOnMain);
        s
    }
    fn check(&self) {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended")
    }
    fn now(&self) -> i64 {
        self.host.now()
    }
    fn request(&self, id: i32, after: i32, request: Request) {
        self.check();
        self.owner.effect(Effect::Send { id, after, request })
    }
    fn cancel(&self, id: i32) {
        if id != 0 {
            self.owner.effect(Effect::Cancel(id))
        }
    }
    fn parse(&self, v: &tl::DialogFilter) -> ChatFilter {
        ChatFilter::from_tl(self.scope, v, |i, c| self.owner.resolve_input(i, c))
    }
    fn dispatch(&self, effects: Vec<FilterEffect>) {
        for e in effects {
            match e {
                FilterEffect::ListChanged => self.changed.fire(()),
                FilterEffect::ChatlistChanged(id) => self.chatlist_changed.fire(id),
                FilterEffect::TagColorChanged(c) => self.color_changed.fire(c),
                e => self.owner.effect(Effect::Model(e)),
            }
        }
    }
    pub fn changed(&self) -> Producer<()> {
        self.changed.events()
    }
    pub fn is_chatlist_changed(&self) -> Producer<i32> {
        self.chatlist_changed.events()
    }
    pub fn tag_color_changed(&self) -> Producer<TagColorChanged> {
        self.color_changed.events()
    }
    pub fn tags_enabled(&self) -> bool {
        self.tags.current()
    }
    pub fn tags_enabled_value(&self) -> Producer<bool> {
        self.tags.value()
    }
    pub fn tags_enabled_changes(&self) -> Producer<bool> {
        self.tags.changes()
    }
    pub fn list(&self) -> Vec<ChatFilter> {
        self.check();
        self.state.borrow().model.list().to_vec()
    }
    pub fn loaded(&self) -> bool {
        self.state.borrow().model.loaded()
    }
    pub fn received(&self, list: &[tl::DialogFilter]) {
        let mut position = 0;
        let mut changed = false;
        for wire in list {
            let parsed = self.parse(wire);
            if self.owner.hide_all_chats() && parsed.id == 0 && list.len() > 1 {
                continue;
            }
            let effects = {
                let mut s = self.state.borrow_mut();
                let m = &mut s.model;
                let mut e = vec![];
                let found = m.list[position..]
                    .iter()
                    .position(|f| f.id == parsed.id)
                    .map(|i| position + i);
                if let Some(i) = found {
                    if i != position {
                        m.list.swap(i, position);
                        changed = true
                    }
                    changed |= m.apply_change(position, parsed, &mut e);
                } else {
                    m.list
                        .insert(position, ChatFilter::default().with_id(parsed.id));
                    m.apply_change(position, parsed, &mut e);
                    changed = true;
                }
                e
            };
            self.dispatch(effects);
            position += 1;
        }
        loop {
            let effects = {
                let mut s = self.state.borrow_mut();
                if position >= s.model.list.len() {
                    break;
                }
                let mut e = vec![];
                s.model.apply_remove(position, &mut e);
                changed = true;
                e
            };
            self.dispatch(effects);
        }
        let hide = self.owner.hide_all_chats();
        let fire = {
            let mut s = self.state.borrow_mut();
            let m = &mut s.model;
            if !hide && !m.list.iter().any(|f| f.id == 0) {
                m.list.insert(0, ChatFilter::default());
            }
            if changed || !m.loaded || m.reloading {
                m.loaded = true;
                m.reloading = false;
                true
            } else {
                false
            }
        };
        if fire {
            self.changed.fire(())
        }
    }
    pub fn set(&self, f: ChatFilter) {
        let e = self.state.borrow_mut().model.set(f);
        self.dispatch(e)
    }
    pub fn remove(&self, id: i32) {
        let e = self.state.borrow_mut().model.remove(id);
        self.dispatch(e)
    }
    pub fn hide_all_changed(&self, hide: bool) {
        let e = self.state.borrow_mut().model.hide_all_changed(hide);
        self.dispatch(e)
    }
    pub fn set_preloaded(&self, list: &[tl::DialogFilter], tags: bool) {
        self.state.borrow_mut().load = -1;
        self.tags.set(tags);
        self.received(list);
        self.owner.effect(Effect::CompletePreloadedOnMain)
    }
    pub fn complete_preloaded(&self) {
        let mut s = self.state.borrow_mut();
        if s.load == -1 {
            s.load = 0
        }
    }
    pub fn load(&self, force: bool) {
        self.check();
        let (old, id) = {
            let mut s = self.state.borrow_mut();
            if s.load != 0 && !force {
                return;
            }
            let old = s.load;
            let id = next_id();
            s.load = id;
            (old, id)
        };
        self.cancel(old);
        self.request(id, 0, Request::Load(tl::MessagesGetDialogFilters));
    }
    pub fn reload(&self) {
        self.state.borrow_mut().model.mark_reloading();
        self.load(false)
    }
    pub fn load_reply(&self, result: Option<&tl::MessagesDialogFilters>) {
        if let Some(tl::MessagesDialogFilters::MessagesDialogFilters(d)) = result {
            self.tags.set(d.is_tags_enabled());
            self.received(&d.filters);
            self.state.borrow_mut().load = 0
        } else {
            let e = {
                let mut s = self.state.borrow_mut();
                s.load = 0;
                s.model.load_failed()
            };
            self.dispatch(e)
        }
    }
    pub fn request_toggle_tags(&self, value: bool) -> Option<i32> {
        let id = {
            let mut s = self.state.borrow_mut();
            if s.toggle != 0 {
                return None;
            }
            let id = next_id();
            s.toggle = id;
            id
        };
        self.request(
            id,
            0,
            Request::ToggleTags(tl::MessagesToggleDialogFilterTags::new(value)),
        );
        Some(id)
    }
    /// Owner invokes its supplied fail callback on false; state clears before it.
    pub fn toggle_tags_reply(&self, value: bool, success: bool) {
        if success {
            self.tags.set(value)
        }
        self.state.borrow_mut().toggle = 0
    }
    pub fn apply(&self, u: &tl::Update) {
        match u {
            tl::Update::UpdateDialogFilter(d) => {
                if let Some(f) = d.vfilter() {
                    let f = self.parse(f);
                    if !(self.owner.hide_all_chats() && f.id == 0) {
                        self.set(f)
                    }
                } else {
                    self.remove(d.id)
                }
            }
            tl::Update::UpdateDialogFilters(_) => self.load(true),
            tl::Update::UpdateDialogFilterOrder(d) => {
                let e = self.state.borrow_mut().model.apply_order(&d.order);
                if let Some(e) = e {
                    self.dispatch(e);
                    self.changed.fire(())
                } else {
                    self.load(true)
                }
            }
            _ => panic!("Update in ChatFilters::apply"),
        }
    }
    pub fn save_order(&self, order: Vec<i32>, after: i32) {
        let (old, id, dependency) = {
            let mut s = self.state.borrow_mut();
            if after != 0 {
                s.after = after
            }
            let old = s.order;
            let id = next_id();
            s.order = id;
            (old, id, s.after)
        };
        self.cancel(old);
        let e = self.state.borrow_mut().model.apply_order(&order);
        if let Some(e) = e {
            self.dispatch(e);
            self.changed.fire(())
        } else {
            self.load(true)
        }
        self.request(
            id,
            dependency,
            Request::Order(tl::MessagesUpdateDialogFiltersOrder::new(order)),
        )
    }
    pub fn request_suggested(&self) {
        let now = self.now();
        let id = {
            let mut s = self.state.borrow_mut();
            if !s.model.suggested_due(now, s.suggested != 0) {
                return;
            }
            let id = next_id();
            s.suggested = id;
            id
        };
        self.request(
            id,
            0,
            Request::Suggested(tl::MessagesGetSuggestedDialogFilters),
        )
    }
    pub fn suggested_reply(&self, result: Option<&[tl::DialogFilterSuggested]>) {
        let now = self.now();
        let list = result.map(|v| {
            v.iter()
                .map(
                    |tl::DialogFilterSuggested::DialogFilterSuggested(d)| SuggestedFilter {
                        filter: self.parse(&d.filter),
                        description: String::from_utf8_lossy(&d.description).into(),
                    },
                )
                .collect()
        });
        {
            let mut s = self.state.borrow_mut();
            s.suggested = 0;
            if let Some(v) = list {
                s.model.suggested_received(v, now)
            } else {
                s.model.suggested_failed(now)
            }
        }
        self.suggested_updated.fire(())
    }
    pub fn suggested_updated(&self) -> Producer<()> {
        self.suggested_updated.events()
    }
    pub fn suggested_loaded(&self) -> bool {
        self.state.borrow().model.suggested_loaded()
    }
    pub fn suggested_filters(&self) -> Vec<SuggestedFilter> {
        self.state.borrow().model.suggested_filters().to_vec()
    }
    pub fn add(&self, id: i32, invite: &tl::ExportedChatlistInvite) -> Option<ChatFilterLink> {
        if !self
            .state
            .borrow()
            .model
            .list()
            .iter()
            .any(|f| f.id == id && f.chatlist())
        {
            return None;
        }
        let tl::ExportedChatlistInvite::ExportedChatlistInvite(d) = invite;
        let link = ChatFilterLink {
            id,
            url: String::from_utf8_lossy(&d.url).into(),
            title: String::from_utf8_lossy(&d.title).into(),
            chats: d
                .peers
                .iter()
                .map(|p| self.owner.peer(peer_from_mtp(p)))
                .collect(),
        };
        for k in &link.chats {
            assert_eq!(k.scope(), self.scope)
        }
        let changed = {
            let mut s = self.state.borrow_mut();
            let links = s.links.entry(id).or_default();
            if let Some(i) = links.iter().position(|l| l.url == link.url) {
                if links[i] == link {
                    false
                } else {
                    links[i] = link.clone();
                    true
                }
            } else {
                links.push(link.clone());
                true
            }
        };
        if changed {
            self.links_updated.fire(id)
        }
        Some(link)
    }
    pub fn edit(&self, id: i32, url: &str, title: &str) {
        let changed = {
            let mut s = self.state.borrow_mut();
            let links = s.links.entry(id).or_default();
            if let Some(l) = links.iter_mut().find(|l| l.url == url) {
                l.title = title.into();
                true
            } else {
                false
            }
        };
        if changed {
            self.links_updated.fire(id);
            self.request(
                next_id(),
                0,
                Request::Edit(tl::ChatlistsEditExportedInvite::new(
                    tl::ChatlistsEditExportedInvite::F_TITLE,
                    tl::mtp_input_chatlist_dialog_filter(id),
                    url.as_bytes().to_vec(),
                    Some(title.as_bytes().to_vec()),
                    None,
                )),
            )
        }
    }
    pub fn destroy(&self, id: i32, url: &str) {
        let old = {
            let mut s = self.state.borrow_mut();
            let links = s.links.entry(id).or_default();
            let Some(i) = links.iter().position(|l| l.url == url) else {
                return;
            };
            links.remove(i);
            s.links_request
        };
        self.links_updated.fire(id);
        self.cancel(old);
        let request = next_id();
        self.state.borrow_mut().links_request = request;
        self.request(
            request,
            0,
            Request::Delete(tl::ChatlistsDeleteExportedInvite::new(
                tl::mtp_input_chatlist_dialog_filter(id),
                url.as_bytes().to_vec(),
            )),
        )
    }
    pub fn chatlist_links(self: &Rc<Self>, id: i32) -> Producer<Vec<ChatFilterLink>> {
        let w = Rc::downgrade(self);
        self.links_updated
            .events_starting_with(id)
            .filter(move |v| *v == id)
            .map(move |_| {
                w.upgrade().map_or_else(Vec::new, |s| {
                    s.state.borrow().links.get(&id).cloned().unwrap_or_default()
                })
            })
    }
    pub fn reload_chatlist_links(&self, id: i32) {
        let req = next_id();
        let old = {
            let mut s = self.state.borrow_mut();
            std::mem::replace(&mut s.links_request, req)
        };
        self.cancel(old);
        self.request(
            req,
            0,
            Request::Links(tl::ChatlistsGetExportedInvites::new(
                tl::mtp_input_chatlist_dialog_filter(id),
            )),
        )
    }
    pub fn links_reply(&self, id: i32, result: &tl::ChatlistsExportedInvites) {
        let tl::ChatlistsExportedInvites::ChatlistsExportedInvites(d) = result;
        self.owner.effect(Effect::ProcessUsers(d.users.clone()));
        self.owner.effect(Effect::ProcessChats(d.chats.clone()));
        self.state.borrow_mut().links.entry(id).or_default().clear();
        for i in &d.invites {
            self.add(id, i);
        }
        self.links_updated.fire(id)
    }
    pub fn load_next_exceptions(&self, chats_loaded: bool) -> bool {
        if self.state.borrow().exceptions != 0 {
            return true;
        }
        if !chats_loaded && self.owner.chats_full_size() < 100 {
            return false;
        }
        let mut inputs = vec![];
        loop {
            let id = self
                .state
                .borrow()
                .model
                .exceptions_to_load
                .front()
                .copied();
            let Some(id) = id else { break };
            let always = self
                .state
                .borrow()
                .model
                .list()
                .iter()
                .find(|f| f.id == id)
                .map(|f| f.always.clone())
                .unwrap_or_default();
            // Original collectExceptions appends to `inputs` and returns an EMPTY vector;
            // its nominal 100-per-request check therefore never limits this collection.
            for p in always {
                if !self.owner.folder_known(p) {
                    inputs.push(tl::mtp_input_dialog_peer(self.owner.input(p)))
                }
            }
            if inputs.len() > 100 {
                break;
            }
            self.state.borrow_mut().model.exceptions_to_load.pop_front();
        }
        if inputs.is_empty() {
            return false;
        }
        let id = next_id();
        self.state.borrow_mut().exceptions = id;
        self.request(
            id,
            0,
            Request::Exceptions(tl::MessagesGetPeerDialogs::new(inputs)),
        );
        true
    }
    pub fn exceptions_reply(&self, result: Option<tl::MessagesPeerDialogs>) {
        self.state.borrow_mut().exceptions = 0;
        if let Some(r) = result {
            self.owner.effect(Effect::ApplyPeerDialogs(r))
        }
        self.owner.effect(Effect::RequestMoreDialogs)
    }
    pub fn watch_more_chats(&self, id: i32) -> Rc<()> {
        assert_ne!(id, 0);
        let token = {
            let mut s = self.state.borrow_mut();
            let m = s.more.entry(id).or_default();
            if let Some(w) = m.watching.upgrade() {
                w
            } else {
                let w = Rc::new(());
                m.watching = Rc::downgrade(&w);
                w
            }
        };
        token
    }
    pub fn more_chats(&self, id: i32) -> Vec<PeerKey> {
        if id == 0 {
            return vec![];
        }
        self.state
            .borrow()
            .more
            .get(&id)
            .map_or_else(Vec::new, |e| e.missing.clone())
    }
    pub fn more_chats_value(self: &Rc<Self>, id: i32) -> Producer<Vec<PeerKey>> {
        if id == 0 {
            return tg_rpl::single(vec![]);
        }
        let w = Rc::downgrade(self);
        Producer::new(move |consumer| {
            let life = tg_rpl::Lifetime::new();
            if let Some(s) = w.upgrade() {
                let token = s.watch_more_chats(id);
                life.add(move || drop(token));
                let weak = Rc::downgrade(&s);
                s.more_updated
                    .events_starting_with(id)
                    .map(move |_| weak.upgrade().map_or_else(Vec::new, |s| s.more_chats(id)))
                    .start(
                        move |v| {
                            consumer.put_next(v);
                        },
                        |_| {},
                        || {},
                        &life,
                    );
                s.load_more_chats_list(id);
            }
            life
        })
    }
    pub fn more_chats_hide(&self, id: i32, local_only: bool) {
        if !local_only {
            self.request(
                next_id(),
                0,
                Request::Hide(tl::ChatlistsHideChatlistUpdates::new(
                    tl::mtp_input_chatlist_dialog_filter(id),
                )),
            )
        }
        let old = {
            let mut s = self.state.borrow_mut();
            let Some(e) = s.more.get_mut(&id) else { return };
            std::mem::take(&mut e.request)
        };
        self.cancel(old);
        let now = self.now();
        if let Some(e) = self.state.borrow_mut().more.get_mut(&id) {
            e.missing.clear();
            e.last = now;
        }
        self.more_updated.fire(id)
    }
    pub fn load_more_chats_list(&self, id: i32) {
        assert_ne!(id, 0);
        let now = self.now();
        let period = self.owner.updates_period_ms();
        let next = {
            let s = self.state.borrow();
            if !s.model.list().iter().any(|f| f.id == id && f.chatlist()) {
                return;
            }
            let Some(e) = s.more.get(&id) else { return };
            if e.watching.upgrade().is_none() || e.request != 0 {
                return;
            }
            if e.last != 0 {
                e.last + period
            } else {
                0
            }
        };
        if next > now {
            if !self.timer.is_active() {
                self.timer.call_once(next - now)
            }
            return;
        }
        let req = next_id();
        self.state.borrow_mut().more.get_mut(&id).unwrap().request = req;
        self.request(
            req,
            0,
            Request::More(tl::ChatlistsGetChatlistUpdates::new(
                tl::mtp_input_chatlist_dialog_filter(id),
            )),
        )
    }
    pub fn more_reply(&self, id: i32, result: Option<&tl::ChatlistsChatlistUpdates>) {
        let list = if let Some(tl::ChatlistsChatlistUpdates::ChatlistsChatlistUpdates(d)) = result {
            self.owner.effect(Effect::ProcessUsers(d.users.clone()));
            self.owner.effect(Effect::ProcessChats(d.chats.clone()));
            Some(
                d.missing_peers
                    .iter()
                    .map(|p| self.owner.peer(peer_from_mtp(p)))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let now = self.now();
        let changed = {
            let mut s = self.state.borrow_mut();
            let e = s.more.entry(id).or_default();
            e.request = 0;
            e.last = now;
            if let Some(list) = &list {
                let b = e.missing != *list;
                e.missing = list.clone();
                b
            } else {
                false
            }
        };
        if list.is_some() && !self.timer.is_active() {
            self.timer.call_once(self.owner.updates_period_ms())
        }
        if changed {
            self.more_updated.fire(id)
        }
    }
    pub fn check_load_more_chats_lists(&self) {
        let ids = self.state.borrow().more.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.load_more_chats_list(id)
        }
    }
}
fn next_id() -> i32 {
    tg_mtproto::mtp_instance::get_next_request_id()
}
/// Real Sender adapter; replies must be routed to the owner thread.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    after: i32,
    r: Request,
    reply: Arc<dyn Fn(Result<Vec<i32>, String>) + Send + Sync>,
) {
    macro_rules! send {
        ($r:expr) => {{
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .after_request(after)
                .done(move |v| reply(Ok(tl::tl::serialize(&v))))
                .fail(move |e| fail(Err(e.kind.clone())))
                .send();
        }};
    }
    match r {
        Request::Load(r) => send!(r),
        Request::Suggested(r) => send!(r),
        Request::ToggleTags(r) => send!(r),
        Request::Order(r) => send!(r),
        Request::Edit(r) => send!(r),
        Request::Delete(r) => send!(r),
        Request::Links(r) => send!(r),
        Request::More(r) => send!(r),
        Request::Hide(r) => send!(r),
        Request::Exceptions(r) => send!(r),
    }
}
impl ChatFiltersService {
    pub fn has(&self) -> bool {
        self.state.borrow().model.has()
    }
    pub fn archive_needed(&self) -> bool {
        self.state.borrow().model.archive_needed()
    }
    pub fn lookup_id(&self, index: i32, premium: bool) -> i32 {
        let hide = self.owner.hide_all_chats();
        self.state.borrow().model.lookup_id(index, premium, hide)
    }
    pub fn default_id(&self, premium: bool) -> i32 {
        self.lookup_id(0, premium)
    }
    pub fn move_all_to_front(&self) {
        self.state.borrow_mut().model.move_all_to_front()
    }
    pub fn clear(&self) {
        self.state.borrow_mut().model.clear()
    }
    pub fn apply_updated_pinned(&self, id: i32, dialogs: &[PeerKey], limit: usize) {
        for p in dialogs {
            assert_eq!(p.scope(), self.scope)
        }
        let e = self
            .state
            .borrow_mut()
            .model
            .apply_updated_pinned(id, dialogs, limit);
        self.dispatch(e)
    }
}
