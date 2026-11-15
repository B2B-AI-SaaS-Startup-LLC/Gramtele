//! TopPeers model. Owner operations are synchronous effects; Sender replies are
//! explicitly routed back on the model thread, never under a store borrow.
use crate::data::{
    data_peer_id::{peer_from_mtp, peer_to_user},
    data_store_keys::*,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::qt_stream::{Reader, Writer};
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
const LIMIT: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopPeerType {
    Chat,
    BotGuestChat,
    BotApp,
}
#[derive(Clone, Debug)]
pub struct TopPeer {
    pub peer: PeerKey,
    pub rating: f64,
}
#[derive(Clone)]
pub enum Request {
    Get(tl::ContactsGetTopPeers),
    Reset(tl::ContactsResetTopPeerRating),
    Toggle(tl::ContactsToggleTopPeers),
}
#[derive(Clone)]
pub enum Effect {
    ReadSearchSuggestions,
    WriteSearchSuggestionsDelayed,
    LoadAfterChats,
    ProcessUsers(Vec<tl::User>),
    ProcessChats(Vec<tl::Chat>),
    UnexpectedCategory,
    Send { id: i32, request: Request },
}
#[derive(Clone)]
pub enum Reply {
    Loaded {
        result: tl::ContactsTopPeers,
        outer_msg_id: u64,
    },
    Failed,
    ToggleDone,
}
/// Owner's Serialize::writePeer/readPeer, not an invented peer encoding.
/// The complete peer persistence adapter remains storage/serialize_peer (13/29).
pub trait PeerCodec {
    fn write_peer(&self, peer: PeerKey, stream: &mut Writer);
    fn read_peer(&self, version: u32, stream: &mut Reader<'_>) -> Option<PeerKey>;
}
/// Mandatory facts from real UserData (package02), with no default bot model.
#[derive(Clone, Copy, Debug)]
pub struct IncrementFacts {
    pub is_bot: bool,
    pub bot_info_present: bool,
    pub supports_guest_chat: bool,
}
#[derive(Default)]
struct State {
    list: Vec<TopPeer>,
    last_received: i64,
    last_received_date: i32,
    request_id: i32,
    disabled: bool,
}
pub struct TopPeers {
    scope: SessionScopeId,
    alive: Weak<crate::data::data_store_keys::ScopeState>,
    kind: TopPeerType,
    state: RefCell<State>,
    updates: EventStream<()>,
    effect: Rc<dyn Fn(Effect)>,
}
impl TopPeers {
    pub fn new(scope: &SessionScope, kind: TopPeerType, effect: Rc<dyn Fn(Effect)>) -> Self {
        let result = Self {
            scope: scope.id(),
            alive: scope.downgrade(),
            kind,
            state: RefCell::default(),
            updates: EventStream::new(),
            effect,
        };
        result.load_after_chats();
        result
    }
    fn check(&self) {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
    }
    pub fn session(&self) -> SessionScopeId {
        self.scope
    }
    /// Bind owner's chatsListLoadedEvents(null) through its two on_main turns.
    pub fn load_after_chats(&self) {
        if self.kind == TopPeerType::Chat {
            self.emit(Effect::LoadAfterChats);
        }
    }
    fn emit(&self, effect: Effect) {
        (self.effect)(effect);
    }
    pub fn updates(&self) -> Producer<()> {
        self.updates.events()
    }
    pub fn list(&self) -> Vec<PeerKey> {
        self.check();
        self.emit(Effect::ReadSearchSuggestions);
        self.state.borrow().list.iter().map(|p| p.peer).collect()
    }
    pub fn ratings(&self) -> Vec<TopPeer> {
        self.state.borrow().list.clone()
    }
    pub fn disabled(&self) -> bool {
        self.check();
        self.emit(Effect::ReadSearchSuggestions);
        self.state.borrow().disabled
    }
    fn updated(&self) {
        self.updates.fire(());
        self.emit(Effect::WriteSearchSuggestionsDelayed);
    }
    pub fn remove(&self, peer: PeerKey, input: tl::InputPeer) {
        self.check();
        assert_eq!(peer.scope(), self.scope);
        let changed = {
            let mut s = self.state.borrow_mut();
            if let Some(i) = s.list.iter().position(|p| p.peer == peer) {
                s.list.remove(i);
                true
            } else {
                false
            }
        };
        if changed {
            self.updated();
        }
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().request_id = id;
        // Original overwrites the outstanding Get request ID with Reset's ID;
        // Reset has no callback clearing it. Preserve that reload-blocking quirk.
        self.emit(Effect::Send {
            id,
            request: Request::Reset(tl::ContactsResetTopPeerRating {
                category: type_to_category(self.kind),
                peer: input,
            }),
        });
    }
    pub fn increment(&self, peer: PeerKey, date: i32, decay: i32, facts: Option<IncrementFacts>) {
        self.check();
        assert_eq!(peer.scope(), self.scope);
        self.emit(Effect::ReadSearchSuggestions);
        let changed = {
            let mut s = self.state.borrow_mut();
            if s.disabled
                || date <= s.last_received_date
                || !super::super::data_peer_id::peer_is_user(peer.id())
                || !can_increment_peer(self.kind, facts)
            {
                return;
            }
            let mut changed = false;
            let mut i = if let Some(i) = s.list.iter().position(|p| p.peer == peer) {
                i
            } else {
                s.list.push(TopPeer { peer, rating: 0. });
                changed = true;
                s.list.len() - 1
            };
            s.list[i].rating += rating_delta(date, s.last_received_date, decay);
            while i > 0 && s.list[i].rating >= s.list[i - 1].rating {
                changed = true;
                s.list.swap(i, i - 1);
                i -= 1;
            }
            changed
        };
        if changed {
            self.updated();
        } else {
            self.emit(Effect::WriteSearchSuggestionsDelayed);
        }
    }
    pub fn reload(&self, now: i64) {
        self.check();
        {
            let s = self.state.borrow();
            if s.request_id != 0 || (s.last_received != 0 && s.last_received + 10_000 > now) {
                return;
            }
        }
        self.request();
    }
    pub fn request(&self) {
        self.check();
        let id = {
            let mut s = self.state.borrow_mut();
            if s.request_id != 0 {
                return;
            }
            s.request_id = tg_mtproto::mtp_instance::get_next_request_id();
            s.request_id
        };
        self.emit(Effect::Send {
            id,
            request: Request::Get(tl::ContactsGetTopPeers {
                flags: tl::tl::Flags::from_raw(type_to_get_flags(self.kind)),
                offset: 0,
                limit: LIMIT as i32,
                hash: self.count_hash(),
            }),
        });
    }
    pub fn toggle_disabled(&self, disabled: bool) {
        self.check();
        self.emit(Effect::ReadSearchSuggestions);
        let changed = {
            let mut s = self.state.borrow_mut();
            if disabled && (!s.disabled || !s.list.is_empty()) {
                s.disabled = true;
                s.list.clear();
                true
            } else if !disabled && s.disabled {
                s.disabled = false;
                true
            } else {
                false
            }
        };
        if changed {
            self.updated();
        }
        self.emit(Effect::Send {
            id: tg_mtproto::mtp_instance::get_next_request_id(),
            request: Request::Toggle(tl::ContactsToggleTopPeers { enabled: !disabled }),
        });
    }
    pub fn handle_reply(
        &self,
        reply: Reply,
        now: i64,
        ensure_peer: impl Fn(crate::data::data_peer_id::PeerId) -> PeerKey,
    ) {
        self.check();
        match reply {
            Reply::Failed => {
                let mut s = self.state.borrow_mut();
                s.last_received = now;
                s.request_id = 0;
            }
            Reply::ToggleDone => {
                if !self.state.borrow().disabled {
                    self.request();
                }
            }
            Reply::Loaded {
                result,
                outer_msg_id,
            } => {
                {
                    let mut s = self.state.borrow_mut();
                    s.last_received_date = (outer_msg_id >> 32) as i32;
                    s.last_received = now;
                    s.request_id = 0;
                }
                match result {
                    tl::ContactsTopPeers::ContactsTopPeers(d) => {
                        self.state.borrow_mut().disabled = false;
                        self.emit(Effect::ProcessUsers(d.users.clone()));
                        self.emit(Effect::ProcessChats(d.chats.clone()));
                        for category in &d.categories {
                            let tl::TopPeerCategoryPeers::TopPeerCategoryPeers(d) = category;
                            if !category_matches(self.kind, &d.category) {
                                self.emit(Effect::UnexpectedCategory);
                                continue;
                            }
                            let list = d
                                .peers
                                .iter()
                                .map(|tl::TopPeer::TopPeer(d)| {
                                    let key = ensure_peer(peer_from_mtp(&d.peer));
                                    assert_eq!(key.scope(), self.scope);
                                    TopPeer {
                                        peer: key,
                                        rating: d.rating,
                                    }
                                })
                                .collect();
                            self.state.borrow_mut().list = list;
                        }
                        self.updated();
                    }
                    tl::ContactsTopPeers::ContactsTopPeersDisabled(_) => {
                        let changed = {
                            let mut s = self.state.borrow_mut();
                            if !s.disabled {
                                s.list.clear();
                                s.disabled = true;
                                true
                            } else {
                                false
                            }
                        };
                        if changed {
                            self.updated();
                        }
                    }
                    tl::ContactsTopPeers::ContactsTopPeersNotModified(_) => {}
                }
            }
        }
    }
    pub fn count_hash(&self) -> u64 {
        let mut h = 0u64;
        for p in self.state.borrow().list.iter().take(LIMIT) {
            hash_update(&mut h, peer_to_user(p.peer.id()).bare);
        }
        h
    }
    pub fn serialize(&self, app_version: u32, codec: &impl PeerCodec) -> Vec<u8> {
        self.check();
        self.emit(Effect::ReadSearchSuggestions);
        let (disabled, list) = {
            let s = self.state.borrow();
            (
                s.disabled,
                s.list.iter().take(LIMIT).cloned().collect::<Vec<_>>(),
            )
        };
        if !disabled && list.is_empty() {
            return Vec::new();
        }
        let mut w = Writer::default();
        w.i32(app_version as i32);
        w.i32(i32::from(disabled));
        w.i32(list.len() as i32);
        for p in list {
            codec.write_peer(p.peer, &mut w);
            w.u64(serialize_rating(p.rating));
        }
        w.0
    }
    pub fn apply_local(&self, bytes: &[u8], codec: &impl PeerCodec) {
        self.check();
        {
            let mut s = self.state.borrow_mut();
            if s.last_received != 0 {
                return;
            }
            s.list.clear();
            s.disabled = false;
        }
        if bytes.is_empty() {
            return;
        }
        let mut r = Reader::new(bytes);
        let (Some(version), Some(disabled), Some(count)) = (r.i32(), r.i32(), r.i32()) else {
            return;
        };
        let mut list = Vec::new();
        for _ in 0..count as u32 {
            let peer = codec.read_peer(version as u32, &mut r);
            let rating = r.u64();
            let (Some(peer), Some(rating)) = (peer, rating) else {
                self.state.borrow_mut().list.clear();
                return;
            };
            assert_eq!(peer.scope(), self.scope);
            list.push(TopPeer {
                peer,
                rating: deserialize_rating(rating),
            });
        }
        let mut s = self.state.borrow_mut();
        s.list = list;
        s.disabled = disabled == 1;
    }
}
pub fn rating_delta(now: i32, was: i32, decay: i32) -> f64 {
    (f64::from(now.wrapping_sub(was)) / f64::from(decay)).exp()
}
pub fn serialize_rating(rating: f64) -> u64 {
    (rating.clamp(0., 1_000_000.) * 1_000_000.).round() as u64
}
pub fn deserialize_rating(rating: u64) -> f64 {
    rating.min(1_000_000_000_000) as f64 / 1_000_000.
}
pub fn hash_update(h: &mut u64, v: u64) {
    *h ^= *h >> 21;
    *h ^= *h << 35;
    *h ^= *h >> 4;
    *h = h.wrapping_add(v);
}
pub fn can_increment_peer(kind: TopPeerType, facts: Option<IncrementFacts>) -> bool {
    let Some(f) = facts else {
        return false;
    };
    match kind {
        TopPeerType::Chat => !f.is_bot,
        TopPeerType::BotGuestChat => f.is_bot && f.bot_info_present && f.supports_guest_chat,
        TopPeerType::BotApp => false,
    }
}
pub fn type_to_category(kind: TopPeerType) -> tl::TopPeerCategory {
    match kind {
        TopPeerType::Chat => tl::TopPeerCategory::TopPeerCategoryCorrespondents(Default::default()),
        TopPeerType::BotGuestChat => {
            tl::TopPeerCategory::TopPeerCategoryBotsGuestChat(Default::default())
        }
        TopPeerType::BotApp => tl::TopPeerCategory::TopPeerCategoryBotsApp(Default::default()),
    }
}
pub fn type_to_get_flags(kind: TopPeerType) -> u64 {
    match kind {
        TopPeerType::Chat => 1,
        TopPeerType::BotGuestChat => 1 << 17,
        TopPeerType::BotApp => 1 << 16,
    }
}
/// Real typed Sender adapter. The callback must route replies to the model's
/// main queue; it may capture a channel, not a non-Send Session/model borrow.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: Request,
    reply: Arc<dyn Fn(Reply) + Send + Sync>,
) {
    match request {
        Request::Get(r) => {
            let fail = reply.clone();
            sender
                .request(r)
                .override_id(id)
                .done_with_response(move |result, response| {
                    reply(Reply::Loaded {
                        result,
                        outer_msg_id: response.outer_msg_id,
                    })
                })
                .fail(move |_| fail(Reply::Failed))
                .send();
        }
        Request::Reset(r) => {
            sender.request(r).override_id(id).send();
        }
        Request::Toggle(r) => {
            sender
                .request(r)
                .override_id(id)
                .done_plain(move || reply(Reply::ToggleDone))
                .send();
        }
    }
}

fn category_matches(kind: TopPeerType, category: &tl::TopPeerCategory) -> bool {
    matches!(
        (kind, category),
        (
            TopPeerType::Chat,
            tl::TopPeerCategory::TopPeerCategoryCorrespondents(_)
        ) | (
            TopPeerType::BotGuestChat,
            tl::TopPeerCategory::TopPeerCategoryBotsGuestChat(_)
        ) | (
            TopPeerType::BotApp,
            tl::TopPeerCategory::TopPeerCategoryBotsApp(_)
        )
    )
}
