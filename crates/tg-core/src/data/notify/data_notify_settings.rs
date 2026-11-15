//! Notify service over actual Peers; missing Topic/History/UI/backend calls are
//! required key-based owner contracts (08/18/26/29/5), never fake record models.
use super::data_peer_notify_settings::*;
use crate::data::{
    data_changes::PeerFlag,
    data_document::DocumentData,
    data_document_media::DocumentMedia,
    data_media_store::Documents,
    data_msg_id::MsgId,
    data_peer::{PeerEffect, PeerMutation},
    data_peer_id::*,
    data_session::Peers,
    data_store_keys::*,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{Timer, TimerHost};
use tg_rpl::{EventStream, Lifetime, Producer};
use tg_runtime::MainQueue;
use tg_scheme as tl;
pub const MAX_NOTIFY_CHECK_DELAY: i64 = 24 * 3600 * 1000;
pub fn muted_from_until(until: i32, now: i32) -> (bool, i64) {
    let remaining = (i64::from(until) - i64::from(now)).max(0);
    (
        remaining > 0,
        if remaining > 0 {
            (remaining * 1000).min(MAX_NOTIFY_CHECK_DELAY)
        } else {
            MAX_NOTIFY_CHECK_DELAY
        },
    )
}
pub fn default_notify_type(peers: &Peers, peer: PeerKey) -> DefaultNotify {
    if peer_is_user(peer.id()) {
        DefaultNotify::User
    } else if peer_is_chat(peer.id())
        || peers.read_channel(ChannelKey::try_from(peer).unwrap(), |p| p.is_megagroup())
    {
        DefaultNotify::Group
    } else {
        DefaultNotify::Broadcast
    }
}
pub fn default_notify_to_mtp(kind: DefaultNotify) -> tl::InputNotifyPeer {
    match kind {
        DefaultNotify::User => tl::mtp_input_notify_users(),
        DefaultNotify::Group => tl::mtp_input_notify_chats(),
        DefaultNotify::Broadcast => tl::mtp_input_notify_broadcasts(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Default(DefaultNotify),
    Peer(PeerKey),
    Topic(TopicKey),
}
#[derive(Clone)]
pub enum Request {
    Settings(tl::AccountGetNotifySettings),
    Exceptions(tl::AccountGetNotifyExceptions),
}
#[derive(Clone)]
pub enum Reply {
    Settings {
        peer: tl::InputNotifyPeer,
        settings: tl::PeerNotifySettings,
    },
    Exceptions(tl::Updates),
}
#[derive(Clone)]
pub enum Effect {
    Send { id: i32, request: Request },
    ApplyUpdates(tl::Updates),
    UpdateDelayed(Target),
    CheckDelayed,
    SetHistoryMuted { peer: PeerKey, muted: bool },
    SetTopicMuted { topic: TopicKey, muted: bool },
    ClearIncomingPeer(PeerKey),
    ClearIncomingTopic(TopicKey),
    SaveRingtone(DocumentRecordKey),
    RequestRingtoneList,
}
pub trait NotifyOwner {
    fn self_user(&self) -> UserKey;
    fn unix_now(&self) -> i32;
    fn user_inaccessible(&self, user: UserKey) -> bool;
    fn input_peer(&self, peer: PeerKey) -> tl::InputPeer;
    fn input_channel(&self, channel: ChannelKey) -> tl::InputChannel;
    fn topic_loaded(&self, peer: PeerKey, root: MsgId) -> Option<TopicKey>;
    fn topic_settings(&self, topic: TopicKey) -> PeerNotifySettings;
    fn set_topic_settings(&self, topic: TopicKey, settings: PeerNotifySettings);
    fn topic_destroyed(&self, topic: TopicKey) -> Producer<()>;
    fn forum_topics(&self, peer: PeerKey) -> Vec<TopicKey>;
    fn community_histories(&self, peer: ChannelKey) -> Vec<PeerKey>;
    fn history_muted(&self, peer: PeerKey) -> Option<bool>;
    fn ringtone_list_updates(&self) -> Producer<()>;
    fn effect(&self, e: Effect);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RequestKey {
    peer: PeerId,
    root: MsgId,
}
pub struct NotifySettings {
    peers: Rc<Peers>,
    docs: Rc<Documents>,
    owner: Rc<dyn NotifyOwner>,
    queue: MainQueue,
    defaults: RefCell<[PeerNotifySettings; 3]>,
    default_updates: [EventStream<()>; 3],
    muted_peers: RefCell<BTreeSet<PeerKey>>,
    muted_topics: RefCell<BTreeMap<TopicKey, Rc<Lifetime>>>,
    timer: Timer,
    views: RefCell<BTreeMap<u64, Rc<RefCell<DocumentMedia>>>>,
    pending_ringtones: RefCell<Vec<u64>>,
    ringtone_lifetime: RefCell<Option<Lifetime>>,
    exceptions: RefCell<[BTreeSet<PeerKey>; 3]>,
    exception_requests: RefCell<[i32; 3]>,
    exceptions_scheduled: RefCell<[bool; 3]>,
    exceptions_updates: EventStream<DefaultNotify>,
    exceptions_realtime: EventStream<DefaultNotify>,
    requests: RefCell<BTreeMap<RequestKey, i32>>,
    weak: Weak<Self>,
}
impl NotifySettings {
    pub fn new(
        peers: Rc<Peers>,
        docs: Rc<Documents>,
        owner: Rc<dyn NotifyOwner>,
        queue: MainQueue,
        host: Rc<TimerHost>,
    ) -> Rc<Self> {
        peers.check_scope(owner.self_user().peer());
        assert_eq!(docs.scope(), owner.self_user().scope());
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            Self {
                peers,
                docs,
                owner,
                queue,
                defaults: RefCell::new(std::array::from_fn(|_| PeerNotifySettings::default())),
                default_updates: std::array::from_fn(|_| EventStream::new()),
                muted_peers: RefCell::default(),
                muted_topics: RefCell::default(),
                timer: Timer::in_host(
                    &host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            s.unmute_by_finished();
                        }
                    })),
                ),
                views: RefCell::default(),
                pending_ringtones: RefCell::default(),
                ringtone_lifetime: RefCell::default(),
                exceptions: RefCell::default(),
                exception_requests: RefCell::new([0; 3]),
                exceptions_scheduled: RefCell::new([false; 3]),
                exceptions_updates: EventStream::new(),
                exceptions_realtime: EventStream::new(),
                requests: RefCell::default(),
                weak: weak.clone(),
            }
        })
    }
    pub fn default_settings(&self, t: DefaultNotify) -> PeerNotifySettings {
        self.defaults.borrow()[t as usize].clone()
    }
    pub fn default_updates(&self, t: DefaultNotify) -> Producer<()> {
        self.default_updates[t as usize].events()
    }
    pub fn default_updates_for(&self, p: PeerKey) -> Producer<()> {
        self.default_updates(default_notify_type(&self.peers, p))
    }
    fn peer_settings(&self, p: PeerKey) -> PeerNotifySettings {
        self.peers.read(p, |p| p.notify().clone())
    }
    fn change(&self, t: Target, f: impl FnOnce(&mut PeerNotifySettings) -> bool) -> bool {
        match t {
            Target::Default(t) => f(&mut self.defaults.borrow_mut()[t as usize]),
            Target::Peer(p) => self
                .peers
                .update(p, |p| PeerMutation::quiet(f(p.notify_mut()))),
            Target::Topic(t) => {
                let mut data = self.owner.topic_settings(t);
                let changed = f(&mut data);
                if changed {
                    self.owner.set_topic_settings(t, data);
                }
                changed
            }
        }
    }
    fn community_fallback(&self, p: PeerKey) -> Option<PeerKey> {
        let k = ChannelKey::try_from(p).ok()?;
        let id = self.peers.read_channel(k, |c| {
            if c.is_community() {
                ChannelId::new(0)
            } else {
                c.linked_community_id()
            }
        });
        if id.is_zero() {
            None
        } else {
            self.peers.peer_loaded(peer_from_channel(id))
        }
    }
    fn input_target(&self, p: PeerKey) -> tl::InputNotifyPeer {
        if let Ok(c) = ChannelKey::try_from(p) {
            if self.peers.read_channel(c, |c| c.is_community()) {
                return tl::mtp_input_notify_community(self.owner.input_channel(c));
            }
        }
        tl::mtp_input_notify_peer(self.owner.input_peer(p))
    }
    pub fn request(&self, p: PeerKey) {
        if self.peer_settings(p).settings_unknown() {
            self.request_input(self.input_target(p));
        }
        let t = default_notify_type(&self.peers, p);
        if self.default_settings(t).settings_unknown() {
            self.request_input(default_notify_to_mtp(t));
        }
    }
    pub fn request_thread(&self, t: Target) {
        match t {
            Target::Peer(p) => self.request(p),
            Target::Topic(topic) => {
                if self.owner.topic_settings(topic).settings_unknown() {
                    self.request_input(tl::mtp_input_notify_forum_topic(
                        self.owner.input_peer(topic.history),
                        topic.root_id.bare as i32,
                    ));
                }
                self.request(topic.history);
            }
            Target::Default(t) => self.request_input(default_notify_to_mtp(t)),
        }
    }
    fn peer_from_input(&self, p: &tl::InputPeer) -> PeerId {
        match p {
            tl::InputPeer::InputPeerSelf(_) => self.owner.self_user().peer().id(),
            tl::InputPeer::InputPeerUser(d) => peer_from_user(d.user_id),
            tl::InputPeer::InputPeerChat(d) => peer_from_chat(d.chat_id),
            tl::InputPeer::InputPeerChannel(d) => peer_from_channel(d.channel_id),
            tl::InputPeer::InputPeerUserFromMessage(d) => peer_from_user(d.user_id),
            tl::InputPeer::InputPeerChannelFromMessage(d) => peer_from_channel(d.channel_id),
            tl::InputPeer::InputPeerEmpty(_) => panic!("Empty peer in NotifySettings::apply"),
        }
    }
    fn community_id(&self, c: &tl::InputChannel) -> PeerId {
        peer_from_channel(match c {
            tl::InputChannel::InputChannel(d) => d.channel_id,
            tl::InputChannel::InputChannelFromMessage(d) => d.channel_id,
            tl::InputChannel::InputChannelEmpty(_) => 0,
        })
    }
    fn input_key(&self, p: &tl::InputNotifyPeer) -> RequestKey {
        let (peer, root) = match p {
            tl::InputNotifyPeer::InputNotifyUsers(_) => (peer_from_user(1), 0),
            tl::InputNotifyPeer::InputNotifyChats(_) => (peer_from_chat(1), 0),
            tl::InputNotifyPeer::InputNotifyBroadcasts(_) => (peer_from_channel(1), 0),
            tl::InputNotifyPeer::InputNotifyPeer(d) => (self.peer_from_input(&d.peer), 0),
            tl::InputNotifyPeer::InputNotifyForumTopic(d) => {
                (self.peer_from_input(&d.peer), d.top_msg_id)
            }
            tl::InputNotifyPeer::InputNotifyCommunity(d) => (self.community_id(&d.community), 0),
        };
        RequestKey {
            peer,
            root: MsgId::new(root.into()),
        }
    }
    fn request_input(&self, peer: tl::InputNotifyPeer) {
        let bad = match &peer {
            tl::InputNotifyPeer::InputNotifyPeer(d) => {
                matches!(d.peer, tl::InputPeer::InputPeerEmpty(_))
            }
            tl::InputNotifyPeer::InputNotifyForumTopic(d) => {
                matches!(d.peer, tl::InputPeer::InputPeerEmpty(_))
            }
            tl::InputNotifyPeer::InputNotifyCommunity(d) => {
                matches!(d.community, tl::InputChannel::InputChannelEmpty(_))
            }
            _ => false,
        };
        if bad {
            return;
        }
        let key = self.input_key(&peer);
        if self.requests.borrow().contains_key(&key) {
            return;
        }
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.owner.effect(Effect::Send {
            id,
            request: Request::Settings(tl::AccountGetNotifySettings { peer }),
        });
        self.requests.borrow_mut().insert(key, id);
    }
    pub fn apply_input(&self, p: &tl::InputNotifyPeer, s: &tl::PeerNotifySettings) {
        match p {
            tl::InputNotifyPeer::InputNotifyUsers(_) => {
                self.apply(Target::Default(DefaultNotify::User), s)
            }
            tl::InputNotifyPeer::InputNotifyChats(_) => {
                self.apply(Target::Default(DefaultNotify::Group), s)
            }
            tl::InputNotifyPeer::InputNotifyBroadcasts(_) => {
                self.apply(Target::Default(DefaultNotify::Broadcast), s)
            }
            tl::InputNotifyPeer::InputNotifyPeer(d) => {
                self.apply_peer_id(self.peer_from_input(&d.peer), s)
            }
            tl::InputNotifyPeer::InputNotifyForumTopic(d) => self.apply_topic_id(
                self.peer_from_input(&d.peer),
                MsgId::new(d.top_msg_id.into()),
                s,
            ),
            tl::InputNotifyPeer::InputNotifyCommunity(d) => {
                self.apply_peer_id(self.community_id(&d.community), s)
            }
        }
    }
    pub fn apply_notify_peer(&self, p: &tl::NotifyPeer, s: &tl::PeerNotifySettings) {
        match p {
            tl::NotifyPeer::NotifyUsers(_) => self.apply(Target::Default(DefaultNotify::User), s),
            tl::NotifyPeer::NotifyChats(_) => self.apply(Target::Default(DefaultNotify::Group), s),
            tl::NotifyPeer::NotifyBroadcasts(_) => {
                self.apply(Target::Default(DefaultNotify::Broadcast), s)
            }
            tl::NotifyPeer::NotifyPeer(d) => self.apply_peer_id(peer_from_mtp(&d.peer), s),
            tl::NotifyPeer::NotifyForumTopic(d) => {
                self.apply_topic_id(peer_from_mtp(&d.peer), MsgId::new(d.top_msg_id.into()), s)
            }
            tl::NotifyPeer::NotifyCommunity(d) => {
                self.apply_peer_id(peer_from_channel(d.community_id), s)
            }
        }
    }
    pub fn apply_peer_id(&self, id: PeerId, s: &tl::PeerNotifySettings) {
        if let Some(p) = self.peers.peer_loaded(id) {
            self.apply(Target::Peer(p), s);
        }
    }
    pub fn apply_topic_id(&self, id: PeerId, root: MsgId, s: &tl::PeerNotifySettings) {
        if let Some(t) = self
            .peers
            .peer_loaded(id)
            .and_then(|p| self.owner.topic_loaded(p, root))
        {
            self.apply(Target::Topic(t), s);
        }
    }
    pub fn apply(&self, t: Target, s: &tl::PeerNotifySettings) {
        if self.change(t, |data| data.change(s)) {
            if let Target::Peer(p) = t {
                self.update_exception(p);
            }
            self.update_local(t);
            self.owner.effect(Effect::CheckDelayed);
        }
    }
    pub fn update(
        &self,
        t: Target,
        mute: MuteValue,
        silent: Option<bool>,
        sound: Option<NotifySound>,
        stories: Option<bool>,
    ) {
        let now = self.owner.unix_now();
        if self.change(t, |s| s.change_local(mute, silent, sound, stories, now)) {
            if let Target::Peer(p) = t {
                self.update_exception(p);
            }
            self.update_local(t);
            self.owner.effect(Effect::UpdateDelayed(t));
        }
    }
    pub fn reset_to_default(&self, t: Target) {
        assert!(!matches!(t, Target::Default(_)));
        if self.change(t, PeerNotifySettings::reset_to_default) {
            if let Target::Peer(p) = t {
                self.update_exception(p);
            }
            self.update_local(t);
            self.owner.effect(Effect::UpdateDelayed(t));
            self.owner.effect(Effect::CheckDelayed);
        }
    }
    pub fn forum_parent_mute_updated(&self, p: PeerKey) {
        for t in self.owner.forum_topics(p) {
            self.update_local(Target::Topic(t));
        }
    }
    pub fn community_parent_mute_updated(&self, p: ChannelKey) {
        for peer in self.owner.community_histories(p) {
            self.update_local(Target::Peer(peer));
        }
    }
    pub fn is_muted_with_delay(&self, t: Target) -> (bool, i64) {
        match t {
            Target::Default(t) => self
                .default_settings(t)
                .mute_until()
                .map_or((true, 0), |v| muted_from_until(v, self.owner.unix_now())),
            Target::Peer(p) => {
                let own = self.peer_settings(p);
                let until = own
                    .mute_until()
                    .or_else(|| {
                        self.community_fallback(p)
                            .and_then(|p| self.peer_settings(p).mute_until())
                    })
                    .or_else(|| {
                        self.default_settings(default_notify_type(&self.peers, p))
                            .mute_until()
                    });
                until.map_or((true, 0), |v| muted_from_until(v, self.owner.unix_now()))
            }
            Target::Topic(t) => self.owner.topic_settings(t).mute_until().map_or_else(
                || self.is_muted_with_delay(Target::Peer(t.history)),
                |v| muted_from_until(v, self.owner.unix_now()),
            ),
        }
    }
    pub fn is_muted(&self, t: Target) -> bool {
        self.is_muted_with_delay(t).0
    }
    pub fn silent_posts(&self, p: PeerKey) -> bool {
        self.peer_settings(p)
            .silent_posts()
            .or_else(|| {
                self.community_fallback(p)
                    .and_then(|p| self.peer_settings(p).silent_posts())
            })
            .or_else(|| {
                self.default_settings(default_notify_type(&self.peers, p))
                    .silent_posts()
            })
            .unwrap_or(false)
    }
    pub fn sound(&self, t: Target) -> NotifySound {
        match t {
            Target::Default(t) => self
                .default_settings(t)
                .sound()
                .cloned()
                .unwrap_or_default(),
            Target::Topic(t) => self
                .owner
                .topic_settings(t)
                .sound()
                .cloned()
                .unwrap_or_else(|| self.sound(Target::Peer(t.history))),
            Target::Peer(p) => {
                let own = if p == self.owner.self_user().peer() {
                    None
                } else {
                    self.peer_settings(p).sound().cloned()
                };
                own.or_else(|| {
                    self.community_fallback(p)
                        .and_then(|p| self.peer_settings(p).sound().cloned())
                })
                .or_else(|| {
                    self.default_settings(default_notify_type(&self.peers, p))
                        .sound()
                        .cloned()
                })
                .unwrap_or_default()
            }
        }
    }
    fn unknown_peer(&self, p: PeerKey, field: impl Fn(&PeerNotifySettings) -> bool) -> bool {
        let own = self.peer_settings(p);
        if own.settings_unknown() {
            true
        } else if field(&own) {
            false
        } else if let Some(c) = self.community_fallback(p) {
            let c = self.peer_settings(c);
            c.settings_unknown()
                || !field(&c)
                    && self
                        .default_settings(default_notify_type(&self.peers, p))
                        .settings_unknown()
        } else {
            self.default_settings(default_notify_type(&self.peers, p))
                .settings_unknown()
        }
    }
    pub fn mute_unknown(&self, t: Target) -> bool {
        match t {
            Target::Default(t) => self.default_settings(t).settings_unknown(),
            Target::Peer(p) => self.unknown_peer(p, |s| s.mute_until().is_some()),
            Target::Topic(t) => {
                let own = self.owner.topic_settings(t);
                own.settings_unknown()
                    || own.mute_until().is_none() && self.mute_unknown(Target::Peer(t.history))
            }
        }
    }
    pub fn sound_unknown(&self, t: Target) -> bool {
        match t {
            Target::Default(t) => self.default_settings(t).settings_unknown(),
            Target::Peer(p) => self.unknown_peer(p, |s| s.sound().is_some()),
            Target::Topic(t) => {
                let own = self.owner.topic_settings(t);
                own.settings_unknown()
                    || own.sound().is_none() && self.sound_unknown(Target::Peer(t.history))
            }
        }
    }
    pub fn silent_posts_unknown(&self, p: PeerKey) -> bool {
        self.unknown_peer(p, |s| s.silent_posts().is_some())
    }
    pub fn settings_unknown(&self, t: Target) -> bool {
        self.mute_unknown(t)
            || self.sound_unknown(t)
            || match t {
                Target::Peer(p) => self.silent_posts_unknown(p),
                _ => false,
            }
    }
    fn peer_keys(&self) -> Vec<PeerKey> {
        let scope = self.owner.self_user().scope();
        self.peers
            .peers
            .borrow()
            .keys()
            .map(|&id| PeerKey::from_scoped_id(scope, id))
            .collect()
    }
    fn update_local(&self, t: Target) {
        match t {
            Target::Default(kind) => {
                self.default_updates[kind as usize].fire(());
                for p in self.peer_keys() {
                    if default_notify_type(&self.peers, p) != kind {
                        continue;
                    }
                    let own = self.peer_settings(p);
                    let def = self.default_settings(kind);
                    if !own.settings_unknown()
                        && ((own.mute_until().is_none() && def.mute_until().is_some())
                            || (own.silent_posts().is_none() && def.silent_posts().is_some())
                            || (own.sound().is_none() && def.sound().is_some()))
                    {
                        self.update_local(Target::Peer(p));
                    }
                }
                self.cache_optional_sound(self.default_settings(kind).sound().cloned());
            }
            Target::Peer(p) => {
                let (muted, delay) = self.is_muted_with_delay(t);
                let history = self.owner.history_muted(p);
                if history.is_some_and(|old| old != muted) {
                    self.owner
                        .effect(Effect::SetHistoryMuted { peer: p, muted });
                } else {
                    self.peers
                        .dispatch(vec![PeerEffect::Updated(p, PeerFlag::Notifications.into())]);
                }
                if muted {
                    self.muted_peers.borrow_mut().insert(p);
                    self.unmute_by_finished_delayed(delay);
                    if history.is_some() {
                        self.owner.effect(Effect::ClearIncomingPeer(p));
                    }
                } else {
                    self.muted_peers.borrow_mut().remove(&p);
                }
                self.cache_optional_sound(self.peer_settings(p).sound().cloned());
            }
            Target::Topic(topic) => {
                let (muted, delay) = self.is_muted_with_delay(t);
                self.owner.effect(Effect::SetTopicMuted { topic, muted });
                if muted {
                    let life = self
                        .muted_topics
                        .borrow_mut()
                        .entry(topic)
                        .or_insert_with(|| Rc::new(Lifetime::new()))
                        .clone();
                    let weak = self.weak.clone();
                    self.owner.topic_destroyed(topic).start(
                        move |_| {
                            if let Some(s) = weak.upgrade() {
                                s.remove_muted_topic(topic);
                            }
                        },
                        |_| {},
                        || {},
                        &life,
                    );
                    self.unmute_by_finished_delayed(delay);
                    self.owner.effect(Effect::ClearIncomingTopic(topic));
                } else {
                    self.remove_muted_topic(topic);
                }
                self.cache_optional_sound(self.owner.topic_settings(topic).sound().cloned());
            }
        }
    }
    fn remove_muted_topic(&self, key: TopicKey) {
        let life = self.muted_topics.borrow_mut().remove(&key);
        if let Some(life) = life {
            life.destroy();
        }
    }
    fn unmute_by_finished_delayed(&self, delay: i64) {
        let delay = delay.min(MAX_NOTIFY_CHECK_DELAY);
        if !self.timer.is_active() || self.timer.remaining_time() > delay {
            self.timer.call_once(delay);
        }
    }
    pub fn unmute_by_finished(&self) {
        let mut min = 0;
        let peers = self
            .muted_peers
            .borrow()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for p in peers {
            let (muted, delay) = self.is_muted_with_delay(Target::Peer(p));
            if self.owner.history_muted(p).is_some() {
                self.owner
                    .effect(Effect::SetHistoryMuted { peer: p, muted });
            }
            if muted {
                if min == 0 || min > delay {
                    min = delay;
                }
            } else {
                self.muted_peers.borrow_mut().remove(&p);
            }
        }
        let topics = self
            .muted_topics
            .borrow()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for t in topics {
            let (muted, delay) = self.is_muted_with_delay(Target::Topic(t));
            self.owner.effect(Effect::SetTopicMuted { topic: t, muted });
            if muted {
                if min == 0 || min > delay {
                    min = delay;
                }
            } else {
                self.remove_muted_topic(t);
            }
        }
        if min != 0 {
            self.unmute_by_finished_delayed(min);
        }
    }
    pub fn cache_sound_id(&self, id: u64) {
        let key = self.docs.ensure_document(id);
        self.cache_sound(key);
    }
    pub fn cache_sound(&self, key: DocumentRecordKey) {
        if self.docs.read(key, DocumentData::is_null).unwrap() {
            return;
        }
        let (id, view) = self.docs.edit(key, |d| {
            let view = d.create_media_view();
            d.force_to_cache(true);
            (d.id(), view)
        });
        self.views.borrow_mut().entry(id).or_insert(view);
        self.owner.effect(Effect::SaveRingtone(key));
    }
    fn cache_optional_sound(&self, sound: Option<NotifySound>) {
        let Some(sound) = sound.filter(|s| s.id != 0) else {
            return;
        };
        let doc = self.docs.ensure_document(sound.id);
        if !self.docs.read(doc, DocumentData::is_null).unwrap() {
            self.cache_sound(doc);
            return;
        }
        self.pending_ringtones.borrow_mut().push(sound.id);
        if self.ringtone_lifetime.borrow().is_some() {
            return;
        }
        let life = Lifetime::new();
        let w = self.weak.clone();
        self.owner.ringtone_list_updates().start(
            move |_| {
                if let Some(s) = w.upgrade() {
                    let ids = std::mem::take(&mut *s.pending_ringtones.borrow_mut());
                    for id in ids {
                        s.cache_sound_id(id);
                    }
                    let life = s.ringtone_lifetime.borrow_mut().take();
                    if let Some(life) = life {
                        life.destroy();
                    }
                }
            },
            |_| {},
            || {},
            &life,
        );
        *self.ringtone_lifetime.borrow_mut() = Some(life);
        self.owner.effect(Effect::RequestRingtoneList);
    }
    pub fn lookup_ringtone(&self, id: u64) -> Option<Rc<RefCell<DocumentMedia>>> {
        if id == 0 {
            None
        } else {
            self.views.borrow().get(&id).cloned()
        }
    }
    fn skip_add_exception(&self, p: PeerKey) -> bool {
        if let Ok(user) = UserKey::try_from(p) {
            p == self.owner.self_user().peer() || self.owner.user_inaccessible(user)
        } else if let Ok(chat) = ChatKey::try_from(p) {
            self.peers
                .read_chat(chat, |p| p.is_deactivated() || p.is_forbidden())
        } else {
            self.peers
                .read_channel(ChannelKey::try_from(p).unwrap(), |p| {
                    p.is_forbidden() || p.is_community()
                })
        }
    }
    fn update_exception(&self, p: PeerKey) {
        let kind = default_notify_type(&self.peers, p);
        let exception = self.peer_settings(p).mute_until().is_some();
        let changed = if !exception {
            self.exceptions.borrow_mut()[kind as usize].remove(&p)
        } else if self.skip_add_exception(p) {
            false
        } else {
            self.exceptions.borrow_mut()[kind as usize].insert(p)
        };
        if changed {
            self.exceptions_updated(kind);
        }
    }
    fn exceptions_updated(&self, kind: DefaultNotify) {
        let schedule = !self.exceptions_scheduled.borrow().iter().any(|&v| v);
        if schedule {
            let w = self.weak.clone();
            self.queue.post_local(move || {
                if let Some(s) = w.upgrade() {
                    let scheduled = std::mem::take(&mut *s.exceptions_scheduled.borrow_mut());
                    for (i, &yes) in scheduled.iter().enumerate() {
                        if yes {
                            s.exceptions_updates.fire(
                                [
                                    DefaultNotify::User,
                                    DefaultNotify::Group,
                                    DefaultNotify::Broadcast,
                                ][i],
                            );
                        }
                    }
                }
            });
        }
        self.exceptions_scheduled.borrow_mut()[kind as usize] = true;
        self.exceptions_realtime.fire(kind);
    }
    pub fn exceptions_updates(&self) -> Producer<DefaultNotify> {
        self.exceptions_updates.events()
    }
    pub fn exceptions_updates_realtime(&self) -> Producer<DefaultNotify> {
        self.exceptions_realtime.events()
    }
    pub fn exceptions(&self, t: DefaultNotify) -> BTreeSet<PeerKey> {
        self.exceptions.borrow()[t as usize].clone()
    }
    pub fn clear_exceptions(&self, t: DefaultNotify) {
        let list = std::mem::take(&mut self.exceptions.borrow_mut()[t as usize]);
        if list.is_empty() {
            return;
        }
        for p in list {
            if self.change(Target::Peer(p), PeerNotifySettings::reset_to_default) {
                self.update_local(Target::Peer(p));
                self.owner.effect(Effect::UpdateDelayed(Target::Peer(p)));
            }
        }
        self.owner.effect(Effect::CheckDelayed);
        self.exceptions_updated(t);
    }
    pub fn load_exceptions(&self) {
        for t in [
            DefaultNotify::User,
            DefaultNotify::Group,
            DefaultNotify::Broadcast,
        ] {
            if self.exception_requests.borrow()[t as usize] != 0 {
                continue;
            }
            let id = tg_mtproto::mtp_instance::get_next_request_id();
            self.owner.effect(Effect::Send {
                id,
                request: Request::Exceptions(tl::AccountGetNotifyExceptions {
                    flags: Default::default(),
                    peer: Some(default_notify_to_mtp(t)),
                }),
            });
            self.exception_requests.borrow_mut()[t as usize] = id;
        }
    }
    pub fn handle_reply(&self, reply: Reply) {
        match reply {
            Reply::Settings { peer, settings } => {
                self.apply_input(&peer, &settings);
                self.requests.borrow_mut().remove(&self.input_key(&peer));
            }
            Reply::Exceptions(updates) => self.owner.effect(Effect::ApplyUpdates(updates)),
        }
    }
}
/// Sender callbacks carry owned replies; the owner routes them to the model thread.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: Request,
    reply: Arc<dyn Fn(Reply) + Send + Sync>,
) {
    match request {
        Request::Settings(r) => {
            let p = r.peer.clone();
            let fail_peer = p.clone();
            let fail = reply.clone();
            sender
                .request(r)
                .override_id(id)
                .done(move |settings| {
                    reply(Reply::Settings {
                        peer: p.clone(),
                        settings,
                    })
                })
                .fail(move |_| {
                    fail(Reply::Settings {
                        peer: fail_peer.clone(),
                        settings: empty_settings(),
                    })
                })
                .send();
        }
        Request::Exceptions(r) => {
            sender
                .request(r)
                .override_id(id)
                .done(move |u| reply(Reply::Exceptions(u)))
                .send();
        }
    }
}
fn empty_settings() -> tl::PeerNotifySettings {
    tl::PeerNotifySettings::PeerNotifySettings(
        tl::PeerNotifySettingsData {
            flags: Default::default(),
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        }
        .into(),
    )
}
