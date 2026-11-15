//! Emoji status data and service; owner mutations and actual Sender use typed effects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EmojiStatusCollectible {
    pub id: u64,
    pub document_id: u64,
    pub title: String,
    pub slug: String,
    pub pattern_document_id: u64,
    /// QColor as serialized RGBA, None for the invalid default QColor.
    pub center_color: Option<u32>,
    pub edge_color: Option<u32>,
    pub pattern_color: Option<u32>,
    pub text_color: Option<u32>,
}
impl EmojiStatusCollectible {
    pub fn is_present(&self) -> bool {
        self.id != 0
    }
}

use super::{data_store_keys::*, data_types::EmojiStatusId};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{DelayedCallTimer, Timer, TimerHost};
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
#[derive(Clone, Debug, Default)]
pub struct EmojiStatusData {
    pub id: EmojiStatusId,
    pub until: i32,
}
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Recent,
    Default,
    Colored,
    ChannelDefault,
    ChannelColored,
    Collectibles,
}
#[derive(Clone)]
pub enum Request {
    Recent(tl::AccountGetRecentEmojiStatuses),
    Default(tl::AccountGetDefaultEmojiStatuses),
    Colored(tl::MessagesGetStickerSet),
    ChannelDefault(tl::AccountGetDefaultEmojiStatuses),
    ChannelColored(tl::MessagesGetStickerSet),
    Collectibles(tl::AccountGetCollectibleEmojiStatuses),
    SetSelf(tl::AccountUpdateEmojiStatus),
    SetChannel(tl::ChannelsUpdateEmojiStatus),
}
#[derive(Clone)]
pub enum Effect {
    Send {
        id: i32,
        kind: Option<Type>,
        peer: Option<PeerKey>,
        request: Request,
    },
    Cancel(i32),
    SetPeerStatus {
        peer: PeerKey,
        id: EmojiStatusId,
        until: i32,
    },
    InvalidStatus,
}
#[derive(Clone)]
pub enum Reply {
    Statuses(Type, tl::AccountEmojiStatuses),
    Stickers(Type, tl::MessagesStickerSet),
    Failed(Type),
    SetFinished(PeerKey),
}
#[derive(Default)]
struct State {
    lists: [Vec<EmojiStatusId>; 6],
    hashes: [u64; 6],
    requests: [i32; 6],
    recent_scheduled: bool,
    collectibles: BTreeMap<u64, Rc<EmojiStatusCollectible>>,
    sent: BTreeMap<PeerKey, i32>,
    clearing: BTreeMap<PeerKey, i32>,
}
/// Owner binds effects synchronously (including SetPeerStatus) and routes Sender
/// replies to this model thread. There is no synthetic Main::Session here.
pub struct EmojiStatuses {
    scope: SessionScopeId,
    alive: Weak<super::data_store_keys::ScopeState>,
    state: RefCell<State>,
    updates: [EventStream<()>; 6],
    effect: Rc<dyn Fn(Effect)>,
    unix_now: Rc<dyn Fn() -> i32>,
    clearing_timer: Timer,
    refresh_timer: Timer,
    delayed: DelayedCallTimer,
}
impl EmojiStatuses {
    pub fn new(
        scope: &SessionScope,
        host: &Rc<TimerHost>,
        unix_now: Rc<dyn Fn() -> i32>,
        effect: Rc<dyn Fn(Effect)>,
    ) -> Rc<Self> {
        let result = Rc::new_cyclic(|weak: &Weak<Self>| {
            let clear = weak.clone();
            let refresh = weak.clone();
            Self {
                scope: scope.id(),
                alive: scope.downgrade(),
                state: RefCell::default(),
                updates: std::array::from_fn(|_| EventStream::new()),
                effect,
                unix_now,
                clearing_timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = clear.upgrade() {
                            if s.is_alive() {
                                s.process_clearing();
                            }
                        }
                    })),
                ),
                refresh_timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = refresh.upgrade() {
                            if s.is_alive() {
                                s.refresh_default();
                                s.refresh_channel_default();
                            }
                        }
                    })),
                ),
                delayed: DelayedCallTimer::in_host(host),
            }
        });
        result.refresh_default();
        result.refresh_colored();
        result.refresh_timer.call_each(3_600_000);
        result
    }
    fn is_alive(&self) -> bool {
        self.alive.upgrade().is_some()
    }
    fn check(&self) {
        assert!(self.is_alive(), "SessionScope ended");
    }
    fn emit(&self, e: Effect) {
        (self.effect)(e);
    }
    pub fn owner(&self) -> SessionScopeId {
        self.scope
    }
    pub fn session(&self) -> SessionScopeId {
        self.scope
    }
    pub fn list(&self, kind: Type) -> Vec<EmojiStatusId> {
        self.check();
        self.state.borrow().lists[kind as usize].clone()
    }
    pub fn updates(&self, kind: Type) -> Producer<()> {
        self.updates[kind as usize].events()
    }
    pub fn recent_updates(&self) -> Producer<()> {
        self.updates(Type::Recent)
    }
    pub fn default_updates(&self) -> Producer<()> {
        self.updates(Type::Default)
    }
    pub fn channel_default_updates(&self) -> Producer<()> {
        self.updates(Type::ChannelDefault)
    }
    pub fn collectibles_updates(&self) -> Producer<()> {
        self.updates(Type::Collectibles)
    }
    pub fn refresh_recent(&self) {
        self.request(Type::Recent);
    }
    pub fn refresh_default(&self) {
        self.request(Type::Default);
    }
    pub fn refresh_colored(&self) {
        self.request(Type::Colored);
    }
    pub fn refresh_channel_default(&self) {
        self.request(Type::ChannelDefault);
    }
    pub fn refresh_channel_colored(&self) {
        self.request(Type::ChannelColored);
    }
    pub fn refresh_collectibles(&self) {
        self.request(Type::Collectibles);
    }
    pub fn refresh_recent_delayed(self: &Rc<Self>) {
        self.check();
        {
            let mut s = self.state.borrow_mut();
            if s.requests[0] != 0 || s.recent_scheduled {
                return;
            }
            s.recent_scheduled = true;
        }
        let weak = Rc::downgrade(self);
        self.delayed.call(10_000, move || {
            if let Some(s) = weak.upgrade() {
                if s.is_alive() && s.state.borrow().recent_scheduled {
                    s.request(Type::Recent);
                }
            }
        });
    }
    fn request(&self, kind: Type) {
        self.check();
        let (id, hash) = {
            let mut s = self.state.borrow_mut();
            let i = kind as usize;
            if s.requests[i] != 0 {
                return;
            }
            if kind == Type::Recent {
                s.recent_scheduled = false;
            }
            let id = tg_mtproto::mtp_instance::get_next_request_id();
            s.requests[i] = id;
            (id, s.hashes[i])
        };
        let request = match kind {
            Type::Recent => Request::Recent(tl::AccountGetRecentEmojiStatuses { hash }),
            Type::Default => Request::Default(tl::AccountGetDefaultEmojiStatuses { hash }),
            // Source uses GetDefaultEmojiStatuses here, despite the separate
            // getChannelDefaultEmojiStatuses method existing in the schema.
            Type::ChannelDefault => {
                Request::ChannelDefault(tl::AccountGetDefaultEmojiStatuses { hash })
            }
            Type::Collectibles => {
                Request::Collectibles(tl::AccountGetCollectibleEmojiStatuses { hash })
            }
            Type::Colored => Request::Colored(tl::MessagesGetStickerSet {
                stickerset: tl::InputStickerSet::InputStickerSetEmojiDefaultStatuses(
                    Default::default(),
                ),
                hash: 0,
            }),
            Type::ChannelColored => Request::ChannelColored(tl::MessagesGetStickerSet {
                stickerset: tl::InputStickerSet::InputStickerSetEmojiChannelDefaultStatuses(
                    Default::default(),
                ),
                hash: 0,
            }),
        };
        self.emit(Effect::Send {
            id,
            kind: Some(kind),
            peer: None,
            request,
        });
    }
    /// First collectible ID wins; later server payloads retain the original Rc.
    pub fn intern_collectible(&self, data: EmojiStatusCollectible) -> EmojiStatusId {
        let mut s = self.state.borrow_mut();
        let collectible = s
            .collectibles
            .entry(data.id)
            .or_insert_with(|| Rc::new(data))
            .clone();
        EmojiStatusId {
            document_id: 0,
            collectible: Some(collectible),
        }
    }
    pub fn collectible_info(&self, id: u64) -> Option<Rc<EmojiStatusCollectible>> {
        self.state.borrow().collectibles.get(&id).cloned()
    }
    pub fn parse(&self, status: &tl::EmojiStatus) -> EmojiStatusData {
        self.check();
        match status {
            tl::EmojiStatus::EmojiStatus(d) => EmojiStatusData {
                id: EmojiStatusId {
                    document_id: d.document_id,
                    collectible: None,
                },
                until: d.until.unwrap_or_default(),
            },
            tl::EmojiStatus::EmojiStatusCollectible(d) => EmojiStatusData {
                id: self.intern_collectible(EmojiStatusCollectible {
                    id: d.collectible_id,
                    document_id: d.document_id,
                    title: String::from_utf8_lossy(&d.title).into_owned(),
                    slug: String::from_utf8_lossy(&d.slug).into_owned(),
                    pattern_document_id: d.pattern_document_id,
                    center_color: Some(rgb(d.center_color)),
                    edge_color: Some(rgb(d.edge_color)),
                    pattern_color: Some(rgb(d.pattern_color)),
                    text_color: Some(rgb(d.text_color)),
                }),
                until: d.until.unwrap_or_default(),
            },
            tl::EmojiStatus::EmojiStatusEmpty(_)
            | tl::EmojiStatus::InputEmojiStatusCollectible(_) => EmojiStatusData::default(),
        }
    }
    fn parse_list(&self, list: &[tl::EmojiStatus]) -> Vec<EmojiStatusId> {
        let mut result = Vec::with_capacity(list.len());
        for v in list {
            let v = self.parse(v);
            if v.id.is_present() {
                result.push(v.id);
            } else {
                self.emit(Effect::InvalidStatus);
            }
        }
        result
    }
    pub fn handle_reply(
        &self,
        reply: Reply,
        mut process_document: impl FnMut(&tl::Document) -> u64,
    ) {
        self.check();
        match reply {
            Reply::Failed(kind) => {
                let mut s = self.state.borrow_mut();
                s.requests[kind as usize] = 0;
                if !matches!(kind, Type::Colored | Type::ChannelColored) {
                    s.hashes[kind as usize] = 0;
                }
            }
            Reply::SetFinished(peer) => {
                self.state.borrow_mut().sent.remove(&peer);
            }
            Reply::Statuses(kind, result) => {
                assert!(!matches!(kind, Type::Colored | Type::ChannelColored));
                self.state.borrow_mut().requests[kind as usize] = 0;
                if let tl::AccountEmojiStatuses::AccountEmojiStatuses(d) = result {
                    self.state.borrow_mut().hashes[kind as usize] = d.hash;
                    let list = self.parse_list(&d.statuses);
                    self.state.borrow_mut().lists[kind as usize] = list;
                    self.updates[kind as usize].fire(());
                }
            }
            Reply::Stickers(kind, result) => {
                assert!(matches!(kind, Type::Colored | Type::ChannelColored));
                self.state.borrow_mut().requests[kind as usize] = 0;
                if let tl::MessagesStickerSet::MessagesStickerSet(d) = result {
                    self.state.borrow_mut().lists[kind as usize].clear();
                    for document in &d.documents {
                        let id = process_document(document);
                        self.state.borrow_mut().lists[kind as usize].push(EmojiStatusId {
                            document_id: id,
                            collectible: None,
                        });
                    }
                    self.updates[kind as usize].fire(());
                    if kind == Type::Colored {
                        self.refresh_collectibles();
                    }
                }
            }
        }
    }
    pub fn set_self(&self, self_user: UserKey, id: EmojiStatusId, until: i32) {
        self.set(self_user.peer(), true, None, id, until);
    }
    pub fn set(
        &self,
        peer: PeerKey,
        is_self: bool,
        channel: Option<tl::InputChannel>,
        id: EmojiStatusId,
        until: i32,
    ) {
        self.check();
        assert_eq!(peer.scope(), self.scope);
        let old = self
            .state
            .borrow_mut()
            .sent
            .insert(peer, 0)
            .unwrap_or_default();
        if old != 0 {
            self.emit(Effect::Cancel(old));
        }
        self.emit(Effect::SetPeerStatus {
            peer,
            id: id.clone(),
            until,
        });
        let status = status_to_mtp(&id, until);
        let request = if is_self {
            Some(Request::SetSelf(tl::AccountUpdateEmojiStatus {
                emoji_status: status,
            }))
        } else {
            channel.map(|channel| {
                Request::SetChannel(tl::ChannelsUpdateEmojiStatus {
                    channel,
                    emoji_status: status,
                })
            })
        };
        if let Some(request) = request {
            let request_id = tg_mtproto::mtp_instance::get_next_request_id();
            self.state.borrow_mut().sent.insert(peer, request_id);
            self.emit(Effect::Send {
                id: request_id,
                kind: None,
                peer: Some(peer),
                request,
            });
        }
    }
    pub fn register_automatic_clear(&self, peer: PeerKey, until: i32) {
        self.check();
        assert_eq!(peer.scope(), self.scope);
        if until == 0 {
            let empty = {
                let mut s = self.state.borrow_mut();
                s.clearing.remove(&peer);
                s.clearing.is_empty()
            };
            if empty {
                self.clearing_timer.cancel();
            }
        } else {
            let earliest = {
                let mut s = self.state.borrow_mut();
                let already = s.clearing.entry(peer).or_default();
                if *already == until {
                    return;
                }
                *already = until;
                s.clearing
                    .iter()
                    .min_by_key(|(_, until)| **until)
                    .map(|(&p, _)| p)
            };
            if earliest == Some(peer) {
                let now = (self.unix_now)();
                if now < until {
                    self.process_clearing_in(until - now);
                } else {
                    self.process_clearing();
                }
            }
        }
    }
    pub fn automatic_clear_at(&self, peer: PeerKey) -> i32 {
        self.state
            .borrow()
            .clearing
            .get(&peer)
            .copied()
            .unwrap_or_default()
    }
    fn process_clearing_in(&self, wait: i32) {
        self.clearing_timer
            .call_once((i64::from(wait) * 1000).min(6 * 60 * 60 * 1000));
    }
    pub fn process_clearing(&self) {
        self.check();
        let now = (self.unix_now)();
        let clearing = std::mem::take(&mut self.state.borrow_mut().clearing);
        let mut kept = BTreeMap::new();
        let mut min_wait = 0;
        for (peer, until) in clearing {
            if now < until {
                let wait = until - now;
                if min_wait == 0 || min_wait > wait {
                    min_wait = wait;
                }
                kept.insert(peer, until);
            } else {
                self.emit(Effect::SetPeerStatus {
                    peer,
                    id: EmojiStatusId::default(),
                    until: 0,
                });
            }
        }
        {
            let mut s = self.state.borrow_mut();
            for (peer, until) in kept {
                s.clearing.entry(peer).or_insert(until);
            }
        }
        // The source schedules from its taken map, even if reentrant peer
        // effects registered different deadlines in _clearing in the meantime.
        if min_wait != 0 {
            self.process_clearing_in(min_wait);
        } else {
            self.clearing_timer.cancel();
        }
    }
    pub fn clearing_remaining_time(&self) -> i64 {
        self.clearing_timer.remaining_time()
    }
}
fn rgb(v: i32) -> u32 {
    0xff00_0000 | (v as u32 & 0x00ff_ffff)
}
pub fn status_to_mtp(id: &EmojiStatusId, until: i32) -> tl::EmojiStatus {
    if !id.is_present() {
        tl::EmojiStatus::EmojiStatusEmpty(Default::default())
    } else if let Some(c) = &id.collectible {
        tl::EmojiStatus::InputEmojiStatusCollectible(
            tl::InputEmojiStatusCollectibleData {
                flags: tl::tl::Flags::empty(),
                collectible_id: c.id,
                until: (until != 0).then_some(until),
            }
            .into(),
        )
    } else {
        tl::EmojiStatus::EmojiStatus(
            tl::EmojiStatusData {
                flags: tl::tl::Flags::empty(),
                document_id: id.document_id,
                until: (until != 0).then_some(until),
            }
            .into(),
        )
    }
}
/// Typed real Sender adapter; owner delivers Reply on the model thread.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    kind: Option<Type>,
    peer: Option<PeerKey>,
    request: Request,
    reply: Arc<dyn Fn(Reply) + Send + Sync>,
) {
    macro_rules! statuses {
        ($r:expr) => {{
            let kind = kind.expect("list type");
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done(move |r| reply(Reply::Statuses(kind, r)))
                .fail(move |_| fail(Reply::Failed(kind)))
                .send();
        }};
    }
    macro_rules! stickers {
        ($r:expr) => {{
            let kind = kind.expect("list type");
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done(move |r| reply(Reply::Stickers(kind, r)))
                .fail(move |_| fail(Reply::Failed(kind)))
                .send();
        }};
    }
    macro_rules! set {
        ($r:expr) => {{
            let peer = peer.expect("set peer");
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done_plain(move || reply(Reply::SetFinished(peer)))
                .fail(move |_| fail(Reply::SetFinished(peer)))
                .send();
        }};
    }
    match request {
        Request::Recent(r) => statuses!(r),
        Request::Default(r) | Request::ChannelDefault(r) => statuses!(r),
        Request::Collectibles(r) => statuses!(r),
        Request::Colored(r) | Request::ChannelColored(r) => stickers!(r),
        Request::SetSelf(r) => set!(r),
        Request::SetChannel(r) => set!(r),
    }
}

impl EmojiStatuses {
    /// Connect the real Peers store's PeerData::setEmojiStatus effect. The
    /// application retains this subscription with its model lifetime.
    pub fn bind_peer_effects(
        self: &Rc<Self>,
        peers: &super::data_session::Peers,
    ) -> tg_rpl::Lifetime {
        let weak = Rc::downgrade(self);
        let life = tg_rpl::Lifetime::new();
        peers.effects().start(
            move |effect| {
                if let super::data_peer::PeerEffect::RegisterEmojiStatusClear { peer, until } =
                    effect
                {
                    if let Some(service) = weak.upgrade() {
                        if service.is_alive() {
                            service.register_automatic_clear(peer, until);
                        }
                    }
                }
            },
            |_| {},
            || {},
            &life,
        );
        life
    }
}
/// Execute the model mutation against the actual store, including Changes and
/// automatic-clear registration. All Peers borrows end before effect delivery.
pub fn apply_peer_status_effect(peers: &super::data_session::Peers, effect: &Effect) -> bool {
    if let Effect::SetPeerStatus { peer, id, until } = effect {
        peers.update(*peer, |p| p.set_emoji_status(id.clone(), *until));
        true
    } else {
        false
    }
}
pub fn send_effect(
    sender: &tg_mtproto::sender::Sender,
    effect: &Effect,
    reply: Arc<dyn Fn(Reply) + Send + Sync>,
) -> bool {
    match effect {
        Effect::Send {
            id,
            kind,
            peer,
            request,
        } => {
            send_request(sender, *id, *kind, *peer, request.clone(), reply);
            true
        }
        Effect::Cancel(id) => {
            sender.cancel(*id);
            true
        }
        _ => false,
    }
}
