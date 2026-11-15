//! GroupCall data model. Network delivery/engine/session registry are explicit
//! owner effects; no Calls engine or Main::Session substitute is constructed.
use super::{
    data_changes::PeerFlag, data_channel::ChannelDataFlag, data_chat::ChatDataFlag,
    data_peer::PeerEffect, data_peer_id::*, data_session::Peers, data_store_keys::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{Timer, TimerHost};
use tg_rpl::{EventStream, Lifetime, Producer, Variable};
use tg_scheme as tl;
pub const SOUND_STATUS_KEPT_FOR: i64 = 1500;
pub const REQUEST_PER_PAGE: i32 = 50;
pub fn rtmp_endpoint_id() -> &'static [u8] {
    b"unified"
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastSpokeTimes {
    pub anything: i64,
    pub voice: i64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoParams {
    pub endpoint: Vec<u8>,
    pub source_groups: Vec<(Vec<u8>, Vec<u32>)>,
    pub additional_ssrc: u32,
    pub paused: bool,
}
impl VideoParams {
    fn empty(&self) -> bool {
        self.additional_ssrc == 0 && (self.endpoint.is_empty() || self.source_groups.is_empty())
    }
    fn parse(data: Option<&tl::GroupCallParticipantVideo>) -> Self {
        data.map_or_else(
            Self::default,
            |tl::GroupCallParticipantVideo::GroupCallParticipantVideo(d)| Self {
                endpoint: d.endpoint.clone(),
                source_groups: d.source_groups.iter().map(
                    |tl::GroupCallParticipantVideoSourceGroup::GroupCallParticipantVideoSourceGroup(g)| {
                        (g.semantics.clone(), g.sources.iter().map(|&n| n as u32).collect())
                    },
                ).collect(),
                additional_ssrc: d.audio_source.unwrap_or(0) as u32,
                paused: d.is_paused(),
            },
        )
    }
    fn equal(&self, now: Option<&tl::GroupCallParticipantVideo>) -> bool {
        now.map_or_else(|| self.empty(), |v| *self == Self::parse(Some(v)))
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParticipantVideoParams {
    pub camera: VideoParams,
    pub screen: VideoParams,
}
pub fn parse_video_params(
    camera: Option<&tl::GroupCallParticipantVideo>,
    screen: Option<&tl::GroupCallParticipantVideo>,
    existing: Option<&Rc<ParticipantVideoParams>>,
) -> Option<Rc<ParticipantVideoParams>> {
    if camera.is_none() && screen.is_none() {
        return None;
    }
    if let Some(p) = existing.filter(|p| p.camera.equal(camera) && p.screen.equal(screen)) {
        return Some(p.clone());
    }
    Some(Rc::new(ParticipantVideoParams {
        camera: VideoParams::parse(camera),
        screen: VideoParams::parse(screen),
    }))
}
#[derive(Clone, Debug)]
pub struct GroupCallParticipant {
    pub peer: PeerKey,
    pub video_params: Option<Rc<ParticipantVideoParams>>,
    pub date: i32,
    pub last_active: i32,
    pub raised_hand_rating: u64,
    pub ssrc: u32,
    pub volume: i32,
    pub sounding: bool,
    pub speaking: bool,
    pub additional_sounding: bool,
    pub additional_speaking: bool,
    pub muted: bool,
    pub muted_by_me: bool,
    pub can_self_unmute: bool,
    pub only_min_loaded: bool,
    pub video_joined: bool,
    pub apply_volume_from_min: bool,
}
impl GroupCallParticipant {
    pub fn camera_endpoint(&self) -> &[u8] {
        self.video_params
            .as_ref()
            .map_or(&[], |v| v.camera.endpoint.as_slice())
    }
    pub fn screen_endpoint(&self) -> &[u8] {
        self.video_params
            .as_ref()
            .map_or(&[], |v| v.screen.endpoint.as_slice())
    }
    pub fn camera_paused(&self) -> bool {
        self.video_params.as_ref().is_some_and(|v| v.camera.paused)
    }
    pub fn screen_paused(&self) -> bool {
        self.video_params.as_ref().is_some_and(|v| v.screen.paused)
    }
    pub fn additional_audio_ssrc(&self) -> u32 {
        self.video_params
            .as_ref()
            .map_or(0, |v| v.screen.additional_ssrc)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupCallOrigin {
    Group,
    Conference,
    VideoStream,
}
#[derive(Clone, Debug)]
pub struct ParticipantUpdate {
    pub was: Option<GroupCallParticipant>,
    pub now: Option<GroupCallParticipant>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplySliceSource {
    FullReloaded,
    SliceLoaded,
    UnknownLoaded,
    UpdateReceived,
    UpdateConstructed,
}
#[derive(Clone)]
pub enum Request {
    Participants(tl::PhoneGetGroupParticipants),
    Reload(tl::PhoneGetGroupCall),
    SaveSendAs(tl::PhoneSaveDefaultSendAs),
}
#[derive(Clone)]
pub enum Reply {
    Participants(Result<tl::PhoneGroupParticipants, ()>),
    Reload(Result<tl::PhoneGroupCall, ()>),
}
#[derive(Clone)]
pub enum Effect {
    Register(GroupCallKey),
    Unregister(GroupCallKey),
    Cancel(i32),
    Send {
        id: i32,
        request: Request,
    },
    ProcessUsersChats {
        users: Vec<tl::User>,
        chats: Vec<tl::Chat>,
    },
    ApplyChecked(tl::Update),
    Discard {
        peer: PeerKey,
        call: GroupCallKey,
        data: tl::GroupCallDiscardedData,
    },
    UnregisterInvited {
        call: GroupCallKey,
        user: UserKey,
    },
}
/// Required calls to real Calls/Session services, owned by packages9/13/26/29.
pub trait GroupCallOwner {
    fn self_user(&self) -> UserKey;
    fn unix_now(&self) -> i32;
    fn in_call(&self, call: GroupCallKey) -> bool;
    fn can_manage(&self, peer: PeerKey) -> bool;
    fn input_peer(&self, peer: PeerKey) -> tl::InputPeer;
    fn effect(&self, e: Effect);
}
#[derive(Clone, Default)]
struct UnknownBatch {
    ssrcs: BTreeMap<u32, LastSpokeTimes>,
    peers: BTreeMap<PeerId, LastSpokeTimes>,
}
#[derive(Default)]
struct State {
    version: i32,
    participants: Vec<GroupCallParticipant>,
    ssrcs: BTreeMap<u32, PeerKey>,
    finishes: BTreeMap<PeerKey, i64>,
    server_count: i32,
    next_offset: Vec<u8>,
    all_loaded: bool,
    creator: bool,
    rtmp: bool,
    listeners_hidden: bool,
    join_muted: bool,
    can_change_join_muted: bool,
    can_change_messages_enabled: bool,
    record_video: bool,
    joined_to_top: bool,
    invite_link: String,
    queued: Vec<(i32, u8, tl::Update)>,
    applying: bool,
    saved_full: Option<tl::PhoneGroupCall>,
    participants_request: i32,
    reload_request: i32,
    reload_last_finished: i64,
    unknown_request: i32,
    unknown_ssrcs: BTreeMap<u32, LastSpokeTimes>,
    unknown_peers: BTreeMap<PeerId, LastSpokeTimes>,
    unknown_batch: UnknownBatch,
}
pub struct GroupCall {
    key: GroupCallKey,
    access_hash: u64,
    peer: Cell<PeerKey>,
    origin: GroupCallOrigin,
    peers: Rc<Peers>,
    owner: Rc<dyn GroupCallOwner>,
    host: Rc<TimerHost>,
    state: RefCell<State>,
    reload_timer: Timer,
    speaking_timer: Timer,
    life: Lifetime,
    loaded_changes: EventStream<bool>,
    participant_updates: EventStream<ParticipantUpdate>,
    participant_speaking: EventStream<PeerKey>,
    participants_reloaded: EventStream<()>,
    participants_resolved: EventStream<BTreeMap<u32, LastSpokeTimes>>,
    stale_ids: EventStream<BTreeSet<UserId>>,
    with_access: Variable<BTreeSet<UserId>>,
    title: Variable<String>,
    record_start: Variable<i32>,
    schedule_date: Variable<i32>,
    schedule_subscribed: Variable<bool>,
    unmuted_video_limit: Variable<i32>,
    messages_enabled: Variable<bool>,
    messages_min_price: Variable<i32>,
    full_count: Variable<i32>,
    send_as: Variable<PeerKey>,
}
macro_rules! value_getters {
    ($get:ident,$stream:ident,$field:ident,$ty:ty) => {
        pub fn $get(&self) -> $ty {
            self.$field.current()
        }
        pub fn $stream(&self) -> Producer<$ty> {
            self.$field.value()
        }
    };
}
impl GroupCall {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: GroupCallKey,
        peer: PeerKey,
        access_hash: u64,
        schedule_date: i32,
        rtmp: bool,
        origin: GroupCallOrigin,
        peers: Rc<Peers>,
        owner: Rc<dyn GroupCallOwner>,
        host: Rc<TimerHost>,
    ) -> Rc<Self> {
        assert_eq!(key.scope(), peer.scope());
        peers.check_scope(peer);
        assert_eq!(key.scope(), owner.self_user().scope());
        let me = owner.self_user().peer();
        let this = Rc::new_cyclic(|weak: &Weak<Self>| {
            let a = weak.clone();
            let b = weak.clone();
            Self {
                key,
                access_hash,
                peer: Cell::new(peer),
                origin,
                peers,
                owner,
                host: host.clone(),
                state: RefCell::new(State {
                    rtmp,
                    listeners_hidden: rtmp,
                    can_change_join_muted: true,
                    can_change_messages_enabled: true,
                    ..Default::default()
                }),
                reload_timer: Timer::in_host(
                    &host,
                    Some(Rc::new(move || {
                        if let Some(s) = a.upgrade() {
                            s.reload();
                        }
                    })),
                ),
                speaking_timer: Timer::in_host(
                    &host,
                    Some(Rc::new(move || {
                        if let Some(s) = b.upgrade() {
                            s.check_finish_speaking_by_active();
                        }
                    })),
                ),
                life: Lifetime::new(),
                loaded_changes: EventStream::new(),
                participant_updates: EventStream::new(),
                participant_speaking: EventStream::new(),
                participants_reloaded: EventStream::new(),
                participants_resolved: EventStream::new(),
                stale_ids: EventStream::new(),
                with_access: Variable::new(BTreeSet::new()),
                title: Variable::new(String::new()),
                record_start: Variable::new(0),
                schedule_date: Variable::new(schedule_date),
                schedule_subscribed: Variable::new(false),
                unmuted_video_limit: Variable::new(0),
                messages_enabled: Variable::new(false),
                messages_min_price: Variable::new(0),
                full_count: Variable::new(0),
                send_as: Variable::new(me),
            }
        });
        if origin != GroupCallOrigin::Group {
            this.owner.effect(Effect::Register(key));
        }
        if origin == GroupCallOrigin::Conference {
            let w = Rc::downgrade(&this);
            this.participant_updates.events().start(
                move |u| {
                    if let Some(s) = w.upgrade() {
                        if let Some(p) = u.was.filter(|_| u.now.is_none()) {
                            let list = s.with_access.current();
                            if p.peer != s.owner.self_user().peer() {
                                if let Some(id) = Some(peer_to_user(p.peer.id()))
                                    .filter(|id| !id.is_zero() && list.contains(id))
                                {
                                    s.stale_ids.fire([id].into_iter().collect());
                                }
                            }
                        }
                    }
                },
                |_| {},
                || {},
                &this.life,
            );
            let w = Rc::downgrade(&this);
            this.with_access.changes().start(
                move |list| {
                    if !list.is_empty() {
                        if let Some(s) = w.upgrade() {
                            if s.participants_loaded() {
                                s.check_stale_participants();
                            } else {
                                s.request_participants();
                            }
                        }
                    }
                },
                |_| {},
                || {},
                &this.life,
            );
        }
        this
    }
    pub fn key(&self) -> GroupCallKey {
        self.key
    }
    pub fn id(&self) -> u64 {
        self.key.id()
    }
    pub fn session(&self) -> SessionScopeId {
        self.key.scope()
    }
    pub fn peer(&self) -> PeerKey {
        self.peer.get()
    }
    pub fn origin(&self) -> GroupCallOrigin {
        self.origin
    }
    pub fn loaded(&self) -> bool {
        self.state.borrow().version > 0
    }
    pub fn loaded_value(&self) -> Producer<bool> {
        if self.loaded() {
            tg_rpl::single(true)
        } else {
            tg_rpl::single(false).then(self.loaded_changes.events())
        }
    }
    pub fn rtmp(&self) -> bool {
        self.state.borrow().rtmp
    }
    pub fn creator(&self) -> bool {
        self.state.borrow().creator
    }
    pub fn can_manage(&self) -> bool {
        if self.origin == GroupCallOrigin::Conference {
            self.creator()
        } else {
            self.owner.can_manage(self.peer())
        }
    }
    pub fn listeners_hidden(&self) -> bool {
        self.state.borrow().listeners_hidden
    }
    pub fn blockchain_may_be_empty(&self) -> bool {
        self.state.borrow().version < 2
    }
    pub fn input(&self) -> tl::InputGroupCall {
        tl::mtp_input_group_call(self.id(), self.access_hash)
    }
    value_getters!(title, title_value, title, String);
    value_getters!(
        record_start_date,
        record_start_date_value,
        record_start,
        i32
    );
    value_getters!(schedule_date, schedule_date_value, schedule_date, i32);
    value_getters!(
        schedule_start_subscribed,
        schedule_start_subscribed_value,
        schedule_subscribed,
        bool
    );
    value_getters!(full_count, full_count_value, full_count, i32);
    value_getters!(
        messages_enabled,
        messages_enabled_value,
        messages_enabled,
        bool
    );
    value_getters!(
        messages_min_price,
        messages_min_price_value,
        messages_min_price,
        i32
    );
    value_getters!(resolve_send_as, send_as_value, send_as, PeerKey);
    value_getters!(
        participants_with_access_current,
        participants_with_access_value,
        with_access,
        BTreeSet<UserId>
    );
    pub fn set_title(&self, v: String) {
        self.title.set(v);
    }
    pub fn record_start_date_changes(&self) -> Producer<i32> {
        self.record_start.changes()
    }
    pub fn schedule_date_changes(&self) -> Producer<i32> {
        self.schedule_date.changes()
    }
    pub fn unmuted_video_limit(&self) -> i32 {
        self.unmuted_video_limit.current()
    }
    pub fn record_video(&self) -> bool {
        self.state.borrow().record_video
    }
    pub fn can_change_messages_enabled(&self) -> bool {
        self.state.borrow().can_change_messages_enabled
    }
    pub fn conference_invite_link(&self) -> String {
        self.state.borrow().invite_link.clone()
    }
    pub fn join_muted(&self) -> bool {
        self.state.borrow().join_muted
    }
    pub fn can_change_join_muted(&self) -> bool {
        self.state.borrow().can_change_join_muted
    }
    pub fn joined_to_top(&self) -> bool {
        self.state.borrow().joined_to_top
    }
    pub fn set_join_muted_locally(&self, v: bool) {
        self.state.borrow_mut().join_muted = v;
    }
    pub fn set_messages_enabled_locally(&self, v: bool) {
        self.messages_enabled.set(v);
    }
    pub fn set_peer(&self, peer: PeerKey) {
        assert_eq!(peer.scope(), self.key.scope());
        let from = ChatKey::try_from(self.peer()).expect("migration source chat");
        let to = ChannelKey::try_from(peer).expect("migration destination channel");
        assert!(
            self.peers.read_channel(to, |p| p.am_in()),
            "source migrateFrom/migrateTo require membership"
        );
        assert_eq!(
            self.peers.read_chat(from, |p| p.get_migrate_to_channel()),
            Some(to)
        );
        assert_eq!(
            self.peers.read_channel(to, |p| p.get_migrate_from_chat()),
            Some(from)
        );
        self.peer.set(peer);
    }
    pub fn participants(&self) -> Vec<GroupCallParticipant> {
        self.state.borrow().participants.clone()
    }
    pub fn participants_loaded(&self) -> bool {
        self.state.borrow().all_loaded
    }
    pub fn participant_by_peer(&self, peer: PeerKey) -> Option<GroupCallParticipant> {
        self.state
            .borrow()
            .participants
            .iter()
            .find(|p| p.peer == peer)
            .cloned()
    }
    pub fn participant_peer_by_audio_ssrc(&self, ssrc: u32) -> Option<PeerKey> {
        self.state.borrow().ssrcs.get(&ssrc).copied()
    }
    pub fn participant_by_endpoint(&self, endpoint: &[u8]) -> Option<GroupCallParticipant> {
        if endpoint.is_empty() {
            return None;
        }
        self.state
            .borrow()
            .participants
            .iter()
            .find(|p| p.camera_endpoint() == endpoint || p.screen_endpoint() == endpoint)
            .cloned()
    }
    pub fn participant_updated(&self) -> Producer<ParticipantUpdate> {
        self.participant_updates.events()
    }
    pub fn participant_speaking(&self) -> Producer<PeerKey> {
        self.participant_speaking.events()
    }
    pub fn participants_reloaded(&self) -> Producer<()> {
        self.participants_reloaded.events()
    }
    pub fn participants_resolved(&self) -> Producer<BTreeMap<u32, LastSpokeTimes>> {
        self.participants_resolved.events()
    }
    pub fn stale_participant_ids(&self) -> Producer<BTreeSet<UserId>> {
        self.stale_ids.events()
    }
    pub fn set_participants_with_access(&self, list: BTreeSet<UserId>) {
        self.with_access.set(list);
        if self.participants_loaded() {
            self.check_stale_participants();
        } else {
            self.request_participants();
        }
    }
    pub fn set_participants_loaded(&self) {
        self.state.borrow_mut().all_loaded = true;
        self.check_stale_participants();
    }
    pub fn check_stale_participants(&self) {
        let list = self.with_access.current();
        if list.is_empty() {
            return;
        }
        let mut existing = BTreeSet::from([peer_to_user(self.owner.self_user().peer().id())]);
        for p in self.participants() {
            if let Some(id) = Some(peer_to_user(p.peer.id())).filter(|id| !id.is_zero()) {
                existing.insert(id);
            }
        }
        let stale = list.difference(&existing).copied().collect::<BTreeSet<_>>();
        if !stale.is_empty() {
            self.stale_ids.fire(stale);
        }
    }
    fn send(&self, request: Request) -> i32 {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.owner.effect(Effect::Send { id, request });
        id
    }
    fn cancel(&self, id: i32) {
        self.owner.effect(Effect::Cancel(id));
    }
    pub fn save_send_as(&self, peer: PeerKey) {
        assert_eq!(peer.scope(), self.key.scope());
        self.send_as.set(peer);
        self.send(Request::SaveSendAs(tl::PhoneSaveDefaultSendAs {
            call: self.input(),
            send_as: self.owner.input_peer(peer),
        }));
    }
    fn set_server_participants_count(&self, n: i32) {
        self.state.borrow_mut().server_count = n;
        self.change_peer_empty_call_flag();
    }
    fn change_peer_empty_call_flag(&self) {
        let peer = self.peer();
        let count = self.state.borrow().server_count;
        if let Ok(k) = ChatKey::try_from(peer) {
            if self.peers.read_chat(k, |p| p.group_call()) != Some(self.key) {
                return;
            }
            let was = self
                .peers
                .read_chat(k, |p| (p.flags() & ChatDataFlag::CallNotEmpty).any());
            if was != (count > 0) {
                if count > 0 {
                    self.peers
                        .add_chat_flags(k, ChatDataFlag::CallNotEmpty.into());
                } else {
                    self.peers
                        .remove_chat_flags(k, ChatDataFlag::CallNotEmpty.into());
                }
                self.peers
                    .dispatch(vec![PeerEffect::Updated(peer, PeerFlag::GroupCall.into())]);
            }
        } else if let Ok(k) = ChannelKey::try_from(peer) {
            if self.peers.read_channel(k, |p| p.group_call()) != Some(self.key) {
                return;
            }
            let was = self
                .peers
                .read_channel(k, |p| (p.flags() & ChannelDataFlag::CallNotEmpty).any());
            if was != (count > 0) {
                if count > 0 {
                    self.peers
                        .add_channel_flags(k, ChannelDataFlag::CallNotEmpty.into());
                } else {
                    self.peers
                        .remove_channel_flags(k, ChannelDataFlag::CallNotEmpty.into());
                }
                self.peers
                    .dispatch(vec![PeerEffect::Updated(peer, PeerFlag::GroupCall.into())]);
            }
        }
    }
    fn compute_participants_count(&self) {
        let count = {
            let s = self.state.borrow();
            if s.all_loaded && !s.listeners_hidden {
                s.participants.len() as i32
            } else {
                (s.participants.len() as i32).max(s.server_count)
            }
        };
        self.full_count.set(count);
    }
    pub fn apply_participants_slice(
        &self,
        list: &[tl::GroupCallParticipant],
        source: ApplySliceSource,
    ) {
        for tl::GroupCallParticipant::GroupCallParticipant(d) in list {
            let peer = self.peers.ensure_peer(peer_from_mtp(&d.peer));
            let was = self.participant_by_peer(peer);
            if d.is_left() {
                if let Some(old) = was {
                    {
                        let mut s = self.state.borrow_mut();
                        s.ssrcs.remove(&old.ssrc);
                        s.ssrcs.remove(&old.additional_audio_ssrc());
                        s.finishes.remove(&peer);
                        s.participants.retain(|p| p.peer != peer);
                    }
                    if source != ApplySliceSource::FullReloaded {
                        self.participant_updates.fire(ParticipantUpdate {
                            was: Some(old),
                            now: None,
                        });
                    }
                }
                let mut s = self.state.borrow_mut();
                if s.server_count > 0 {
                    s.server_count -= 1;
                }
                continue;
            }
            if let Some(about) = &d.about {
                self.peers
                    .update(peer, |p| p.set_about(&String::from_utf8_lossy(about)));
            }
            // setAbout may synchronously fire Changes; source snapshots *i only
            // afterwards, so retain an in-place speaking/volume mutation there.
            let was = self.participant_by_peer(peer);
            let can_self_unmute = !d.is_muted() || d.is_can_self_unmute();
            let value = GroupCallParticipant {
                peer,
                video_params: if source == ApplySliceSource::UpdateConstructed {
                    was.as_ref().and_then(|p| p.video_params.clone())
                } else {
                    parse_video_params(
                        d.video.as_ref(),
                        d.presentation.as_ref(),
                        was.as_ref().and_then(|p| p.video_params.as_ref()),
                    )
                },
                date: d.date,
                last_active: d
                    .active_date
                    .unwrap_or_else(|| was.as_ref().map_or(0, |p| p.last_active)),
                raised_hand_rating: d.raise_hand_rating.unwrap_or(0),
                ssrc: d.source as u32,
                volume: if d.is_min() && was.as_ref().is_some_and(|p| !p.apply_volume_from_min) {
                    was.as_ref().unwrap().volume
                } else {
                    d.volume.unwrap_or(10000)
                },
                muted_by_me: match was.as_ref().filter(|_| d.is_min()) {
                    Some(was) => was.muted_by_me,
                    None => d.is_muted_by_you(),
                },
                apply_volume_from_min: match was.as_ref().filter(|_| d.is_min()) {
                    Some(was) => was.apply_volume_from_min,
                    None => d.is_min() || d.is_volume_by_admin(),
                },
                only_min_loaded: d.is_min() && was.as_ref().is_none_or(|p| p.only_min_loaded),
                video_joined: d.is_video_joined(),
                sounding: can_self_unmute && was.as_ref().is_some_and(|p| p.sounding),
                speaking: can_self_unmute && was.as_ref().is_some_and(|p| p.speaking),
                additional_sounding: can_self_unmute
                    && was.as_ref().is_some_and(|p| p.additional_sounding),
                additional_speaking: can_self_unmute
                    && was.as_ref().is_some_and(|p| p.additional_speaking),
                muted: d.is_muted(),
                can_self_unmute,
            };
            {
                let mut s = self.state.borrow_mut();
                if let Some(old) = &was {
                    if old.ssrc != value.ssrc {
                        s.ssrcs.remove(&old.ssrc);
                        if value.ssrc != 0 {
                            s.ssrcs.entry(value.ssrc).or_insert(peer);
                        }
                    }
                    if old.additional_audio_ssrc() != value.additional_audio_ssrc() {
                        s.ssrcs.remove(&old.additional_audio_ssrc());
                        if value.additional_audio_ssrc() != 0 {
                            s.ssrcs.entry(value.additional_audio_ssrc()).or_insert(peer);
                        }
                    }
                    let i = s.participants.iter().position(|p| p.peer == peer).unwrap();
                    s.participants[i] = value.clone();
                } else {
                    if value.ssrc != 0 {
                        s.ssrcs.entry(value.ssrc).or_insert(peer);
                    }
                    if value.additional_audio_ssrc() != 0 {
                        s.ssrcs.entry(value.additional_audio_ssrc()).or_insert(peer);
                    }
                    s.participants.push(value.clone());
                }
                if d.is_just_joined() {
                    s.server_count += 1;
                }
            }
            if source != ApplySliceSource::FullReloaded {
                self.participant_updates.fire(ParticipantUpdate {
                    was: was.clone(),
                    now: Some(value),
                });
            }
            if was.is_none() {
                if let Ok(user) = UserKey::try_from(peer) {
                    self.owner.effect(Effect::UnregisterInvited {
                        call: self.key,
                        user,
                    });
                }
            }
        }
        if source == ApplySliceSource::UpdateReceived {
            self.change_peer_empty_call_flag();
            self.compute_participants_count();
        }
    }
    pub fn apply_local_update(&self, u: &tl::UpdateGroupCallParticipantsData) {
        self.apply_participants_slice(&u.participants, ApplySliceSource::UpdateConstructed);
    }
    pub fn apply_call_fields(&self, d: &tl::GroupCallData) {
        let initial = self.state.borrow().version == 0;
        {
            let mut s = self.state.borrow_mut();
            s.version = if d.version == 0 { 1 } else { d.version };
            s.rtmp = d.is_rtmp_stream();
            s.creator = d.is_creator();
            s.listeners_hidden = d.is_listeners_hidden();
            if d.is_conference() {
                s.can_change_join_muted = false;
                s.join_muted = false;
                s.can_change_messages_enabled = d.is_messages_enabled();
            } else {
                s.can_change_join_muted = d.is_can_change_join_muted();
                s.join_muted = d.is_join_muted();
                s.can_change_messages_enabled = d.is_can_change_messages_enabled();
            }
        }
        if !d.is_conference() || !d.is_messages_enabled() || initial {
            self.messages_enabled.set(d.is_messages_enabled());
        }
        self.messages_min_price
            .set(d.send_paid_messages_stars.unwrap_or(0) as i32);
        self.state.borrow_mut().joined_to_top = !d.is_join_date_asc();
        self.set_server_participants_count(d.participants_count);
        self.change_peer_empty_call_flag();
        self.title
            .set(String::from_utf8_lossy(d.title.as_deref().unwrap_or_default()).into_owned());
        self.state.borrow_mut().record_video = d.is_record_video_active();
        self.record_start.set(d.record_start_date.unwrap_or(0));
        self.schedule_date.set(d.schedule_date.unwrap_or(0));
        self.schedule_subscribed
            .set(d.is_schedule_start_subscribed());
        self.unmuted_video_limit.set(d.unmuted_video_limit);
        {
            let mut s = self.state.borrow_mut();
            s.all_loaded = s.server_count == s.participants.len() as i32;
            s.invite_link =
                String::from_utf8_lossy(d.invite_link.as_deref().unwrap_or_default()).into_owned();
        }
        if let Some(peer) = &d.default_send_as {
            self.send_as
                .set(self.peers.ensure_peer(peer_from_mtp(peer)));
        }
        if initial {
            self.loaded_changes.fire(true);
        }
    }
    fn discard(&self, d: &tl::GroupCallDiscardedData) {
        self.owner.effect(Effect::Discard {
            peer: self.peer(),
            call: self.key,
            data: d.clone(),
        });
    }
    fn process_users_chats(&self, c: &tl::PhoneGroupCall) {
        let tl::PhoneGroupCall::PhoneGroupCall(d) = c;
        self.owner.effect(Effect::ProcessUsersChats {
            users: d.users.clone(),
            chats: d.chats.clone(),
        });
    }
    fn process_full_call_fields(&self, c: &tl::PhoneGroupCall) {
        let tl::PhoneGroupCall::PhoneGroupCall(d) = c;
        match &d.call {
            tl::GroupCall::GroupCall(c) => {
                {
                    let mut s = self.state.borrow_mut();
                    s.participants.clear();
                    s.finishes.clear();
                    s.ssrcs.clear();
                    s.all_loaded = false;
                }
                self.apply_participants_slice(&d.participants, ApplySliceSource::FullReloaded);
                self.state.borrow_mut().next_offset = d.participants_next_offset.clone();
                self.apply_call_fields(c);
            }
            tl::GroupCall::GroupCallDiscarded(c) => self.discard(c),
        }
    }
    pub fn process_full_call(&self, c: &tl::PhoneGroupCall) {
        self.process_users_chats(c);
        self.process_full_call_fields(c);
        self.finish_participants_slice_request();
        self.participants_reloaded.fire(());
    }
    fn finish_participants_slice_request(&self) {
        self.compute_participants_count();
        self.process_queued_updates(false);
    }
    pub fn enqueue_update(&self, u: &tl::Update) {
        let initial = self.state.borrow().version == 0;
        match u {
            tl::Update::UpdateGroupCall(d) => match &d.call {
                tl::GroupCall::GroupCall(c) => {
                    let (v, a) = {
                        let s = self.state.borrow();
                        (s.version, s.applying)
                    };
                    if !a && (v == 0 || v == c.version) {
                        self.apply_enqueued_update(u);
                    } else if v == 0 || v <= c.version {
                        self.queue(c.version, 2, u.clone());
                    }
                }
                tl::GroupCall::GroupCallDiscarded(c) => self.discard(c),
            },
            tl::Update::UpdateGroupCallParticipants(d) => {
                let inc = d
                    .participants
                    .iter()
                    .any(|tl::GroupCallParticipant::GroupCallParticipant(p)| p.is_versioned());
                let required = d.version - i32::from(inc);
                let (v, a) = {
                    let s = self.state.borrow();
                    (s.version, s.applying)
                };
                if !a && v == required {
                    self.apply_enqueued_update(u);
                } else if v <= required {
                    self.queue(d.version, if inc { 0 } else { 1 }, u.clone());
                }
            }
            _ => panic!("Type in GroupCall::enqueueUpdate"),
        };
        self.process_queued_updates(initial);
    }
    fn queue(&self, v: i32, kind: u8, u: tl::Update) {
        let mut s = self.state.borrow_mut();
        let i = s.queued.partition_point(|e| (e.0, e.1) <= (v, kind));
        s.queued.insert(i, (v, kind, u));
    }
    fn apply_enqueued_update(&self, u: &tl::Update) {
        assert!(!self.state.borrow().applying);
        self.state.borrow_mut().applying = true;
        match u {
            tl::Update::UpdateGroupCall(d) => match &d.call {
                tl::GroupCall::GroupCall(c) => {
                    self.apply_call_fields(c);
                    self.compute_participants_count();
                }
                tl::GroupCall::GroupCallDiscarded(c) => self.discard(c),
            },
            tl::Update::UpdateGroupCallParticipants(d) => {
                self.state.borrow_mut().version = if d.version == 0 { 1 } else { d.version };
                self.apply_participants_slice(&d.participants, ApplySliceSource::UpdateReceived);
            }
            _ => panic!("Type in GroupCall::applyEnqueuedUpdate"),
        };
        self.owner.effect(Effect::ApplyChecked(u.clone()));
        self.state.borrow_mut().applying = false;
    }
    fn process_queued_updates(&self, initial: bool) {
        let size = {
            let s = self.state.borrow();
            if s.version == 0 || s.applying {
                return;
            }
            s.queued.len()
        };
        loop {
            let item = {
                let mut s = self.state.borrow_mut();
                let Some((v, t, _)) = s.queued.first() else {
                    break;
                };
                let inc = *t == 0;
                if *v < s.version || (*v == s.version && inc && !initial) {
                    s.queued.remove(0);
                    continue;
                } else if *v == s.version || (*v == s.version + 1 && inc) {
                    Some(s.queued.remove(0).2)
                } else {
                    None
                }
            };
            if let Some(u) = item {
                self.apply_enqueued_update(&u);
            } else {
                break;
            }
        }
        let n = self.state.borrow().queued.len();
        if n == 0 {
            self.reload_timer.cancel();
        } else if n != size || !self.reload_timer.is_active() {
            self.reload_timer.call_once(3000);
        }
    }
    pub fn request_participants(&self) {
        let (old, offset) = {
            let mut s = self.state.borrow_mut();
            if s.saved_full.is_none()
                && (s.participants_request != 0 || s.reload_request != 0 || s.all_loaded)
            {
                return;
            }
            let old = std::mem::take(&mut s.participants_request);
            let offset = if let Some(tl::PhoneGroupCall::PhoneGroupCall(d)) = &s.saved_full {
                d.participants_next_offset.clone()
            } else {
                s.next_offset.clone()
            };
            (old, offset)
        };
        self.cancel(old);
        let id = self.send(Request::Participants(tl::PhoneGetGroupParticipants {
            call: self.input(),
            ids: vec![],
            sources: vec![],
            offset,
            limit: REQUEST_PER_PAGE,
        }));
        self.state.borrow_mut().participants_request = id;
    }
    fn process_saved_full_call(&self) -> bool {
        let (saved, id) = {
            let mut s = self.state.borrow_mut();
            let Some(saved) = s.saved_full.take() else {
                return false;
            };
            let id = std::mem::take(&mut s.reload_request);
            s.reload_last_finished = self.host.now();
            (saved, id)
        };
        self.cancel(id);
        self.process_full_call_fields(&saved);
        true
    }
    pub fn reload_if_stale(&self) {
        let last = self.state.borrow().reload_last_finished;
        if self.full_count() == 0 && !self.participants_loaded()
            || last == 0
            || self.host.now() > last + 16000
        {
            self.reload();
        }
    }
    pub fn reload(&self) {
        let old = {
            let mut s = self.state.borrow_mut();
            if s.reload_request != 0 || s.applying {
                return;
            }
            std::mem::take(&mut s.participants_request)
        };
        self.cancel(old);
        loop {
            let item = {
                let mut s = self.state.borrow_mut();
                if s.queued.is_empty() {
                    None
                } else {
                    Some(s.queued.remove(0).2)
                }
            };
            if let Some(u) = item {
                self.apply_enqueued_update(&u);
            } else {
                break;
            }
        }
        self.reload_timer.cancel();
        let id = self.send(Request::Reload(tl::PhoneGetGroupCall {
            call: self.input(),
            limit: 3,
        }));
        self.state.borrow_mut().reload_request = id;
    }
    pub fn request_participants_after_reload(&self, c: &tl::PhoneGroupCall) -> bool {
        let tl::PhoneGroupCall::PhoneGroupCall(d) = c;
        let size = match &d.call {
            tl::GroupCall::GroupCall(c) => c.participants_count,
            _ => 0,
        };
        (d.participants.len() as i32) < size
            && d.participants.len() < self.state.borrow().participants.len()
    }
    pub fn handle_reply(&self, id: i32, reply: Reply) {
        match reply {
            Reply::Reload(reply) => {
                if id != self.state.borrow().reload_request {
                    return;
                }
                match reply {
                    Ok(c) => {
                        if self.request_participants_after_reload(&c) {
                            self.state.borrow_mut().saved_full = Some(c.clone());
                            self.process_users_chats(&c);
                            self.request_participants();
                            return;
                        }
                        {
                            let mut s = self.state.borrow_mut();
                            s.reload_request = 0;
                            s.reload_last_finished = self.host.now();
                        }
                        self.process_full_call(&c);
                    }
                    Err(()) => {
                        let mut s = self.state.borrow_mut();
                        s.reload_request = 0;
                        s.reload_last_finished = self.host.now();
                    }
                }
            }
            Reply::Participants(reply) => {
                if id == self.state.borrow().unknown_request {
                    self.handle_unknown_reply(reply);
                    return;
                }
                if id != self.state.borrow().participants_request {
                    return;
                }
                self.state.borrow_mut().participants_request = 0;
                let reloaded = self.process_saved_full_call();
                match reply {
                    Ok(tl::PhoneGroupParticipants::PhoneGroupParticipants(d)) => {
                        self.state.borrow_mut().next_offset = d.next_offset.clone();
                        self.owner.effect(Effect::ProcessUsersChats {
                            users: d.users.clone(),
                            chats: d.chats.clone(),
                        });
                        self.apply_participants_slice(
                            &d.participants,
                            if reloaded {
                                ApplySliceSource::FullReloaded
                            } else {
                                ApplySliceSource::SliceLoaded
                            },
                        );
                        self.set_server_participants_count(d.count);
                        if d.participants.is_empty() {
                            self.set_participants_loaded();
                        }
                    }
                    Err(()) => {
                        self.set_server_participants_count(self.participants().len() as i32);
                        self.set_participants_loaded();
                    }
                }
                self.finish_participants_slice_request();
                if reloaded {
                    self.participants_reloaded.fire(());
                }
            }
        }
    }
    pub fn apply_last_spoke(&self, ssrc: u32, when: LastSpokeTimes, now: i64) {
        let Some(peer) = self.participant_peer_by_audio_ssrc(ssrc) else {
            self.state.borrow_mut().unknown_ssrcs.insert(ssrc, when);
            self.request_unknown_participants();
            return;
        };
        let p = self
            .participant_by_peer(peer)
            .expect("ssrc points to participant");
        self.state.borrow_mut().finishes.remove(&peer);
        let sounding = when.anything + SOUND_STATUS_KEPT_FOR >= now && p.can_self_unmute;
        let speaking = sounding && when.voice + SOUND_STATUS_KEPT_FOR >= now;
        if speaking {
            self.participant_speaking.fire(peer);
        }
        let p = self
            .participant_by_peer(peer)
            .expect("participant lifetime across source callback");
        let additional = ssrc != p.ssrc;
        let (old_sound, old_speak) = if additional {
            (p.additional_sounding, p.additional_speaking)
        } else {
            (p.sounding, p.speaking)
        };
        if old_sound != sounding || old_speak != speaking {
            let was = p.clone();
            let mut p = p;
            if additional {
                p.additional_sounding = sounding;
                p.additional_speaking = speaking;
            } else {
                p.sounding = sounding;
                p.speaking = speaking;
            }
            self.replace_participant(p.clone());
            self.participant_updates.fire(ParticipantUpdate {
                was: Some(was),
                now: Some(p),
            });
        }
    }
    fn replace_participant(&self, p: GroupCallParticipant) {
        let mut s = self.state.borrow_mut();
        let i = s
            .participants
            .iter()
            .position(|a| a.peer == p.peer)
            .expect("participant exists");
        s.participants[i] = p;
    }
    pub fn apply_active_update(&self, id: PeerId, when: LastSpokeTimes, loaded: Option<PeerKey>) {
        if self.owner.in_call(self.key) {
            return;
        }
        let participant = loaded.and_then(|p| self.participant_by_peer(p));
        if participant.as_ref().is_none_or(|p| p.only_min_loaded) {
            self.state.borrow_mut().unknown_peers.insert(id, when);
            self.request_unknown_participants();
        }
        let Some(mut p) = participant.filter(|p| p.can_self_unmute) else {
            return;
        };
        let was = p.clone();
        let now = self.host.now();
        let last_active = self.owner.unix_now() - ((now - when.anything) / 1000) as i32;
        let finishes = when.anything + 6000;
        if last_active <= p.last_active || finishes <= now {
            return;
        }
        self.state.borrow_mut().finishes.insert(p.peer, finishes);
        if !self.speaking_timer.is_active() {
            self.speaking_timer.call_once(finishes - now);
        }
        p.last_active = last_active;
        p.speaking = true;
        p.can_self_unmute = true;
        self.replace_participant(p.clone());
        if !was.speaking || !was.can_self_unmute {
            self.participant_updates.fire(ParticipantUpdate {
                was: Some(was),
                now: Some(p),
            });
        }
    }
    pub fn check_finish_speaking_by_active(&self) {
        let now = self.host.now();
        let (stop, nearest) = {
            let mut s = self.state.borrow_mut();
            let stop = s
                .finishes
                .iter()
                .filter_map(|(&p, &t)| (now >= t).then_some(p))
                .collect::<Vec<_>>();
            for p in &stop {
                s.finishes.remove(p);
            }
            let nearest = s.finishes.values().copied().min();
            (stop, nearest)
        };
        for peer in stop {
            let mut p = self
                .participant_by_peer(peer)
                .expect("active speaker exists");
            if p.speaking {
                let was = p.clone();
                p.speaking = false;
                self.replace_participant(p.clone());
                self.participant_updates.fire(ParticipantUpdate {
                    was: Some(was),
                    now: Some(p),
                });
            }
        }
        if let Some(t) = nearest {
            self.speaking_timer.call_once(t - now);
        }
    }
    pub fn set_in_call(&self) {
        let latest = self.host.now() + 1000;
        let restart = {
            let mut s = self.state.borrow_mut();
            s.unknown_peers.clear();
            if s.finishes.is_empty() {
                return;
            }
            let mut restart = true;
            for t in s.finishes.values_mut() {
                if *t > latest {
                    *t = latest;
                } else {
                    restart = false;
                }
            }
            restart
        };
        if restart {
            self.speaking_timer.call_once(1000);
        }
    }
    pub fn resolve_participants(&self, ssrcs: &BTreeSet<u32>) {
        if ssrcs.is_empty() {
            return;
        }
        for &id in ssrcs {
            self.state.borrow_mut().unknown_ssrcs.entry(id).or_default();
        }
        self.request_unknown_participants();
    }
    fn request_unknown_participants(&self) {
        let batch = {
            let mut s = self.state.borrow_mut();
            if s.unknown_request != 0 || s.unknown_ssrcs.is_empty() && s.unknown_peers.is_empty() {
                return;
            }
            let mut batch = UnknownBatch::default();
            if s.unknown_ssrcs.len() < REQUEST_PER_PAGE as usize {
                batch.ssrcs = std::mem::take(&mut s.unknown_ssrcs);
            } else {
                for _ in 0..REQUEST_PER_PAGE {
                    let (k, v) = s.unknown_ssrcs.pop_last().unwrap();
                    batch.ssrcs.insert(k, v);
                }
            }
            let available = REQUEST_PER_PAGE as usize - batch.ssrcs.len();
            if s.unknown_peers.len() < available {
                batch.peers = std::mem::take(&mut s.unknown_peers);
            } else {
                for _ in 0..available {
                    let (k, v) = s.unknown_peers.pop_last().unwrap();
                    batch.peers.insert(k, v);
                }
            }
            s.unknown_batch = batch.clone();
            batch
        };
        let mut ids = vec![];
        for &peer in batch.peers.keys() {
            if let Some(id) = Some(peer_to_user(peer)).filter(|id| !id.is_zero()) {
                ids.push(tl::mtp_input_peer_user(id.bare, 0));
            } else if let Some(id) = Some(peer_to_chat(peer)).filter(|id| !id.is_zero()) {
                ids.push(tl::mtp_input_peer_chat(id.bare));
            } else if let Some(id) = Some(peer_to_channel(peer)).filter(|id| !id.is_zero()) {
                ids.push(tl::mtp_input_peer_channel(id.bare, 0));
            }
        }
        let request = tl::PhoneGetGroupParticipants {
            call: self.input(),
            ids,
            sources: batch.ssrcs.keys().map(|&n| n as i32).collect(),
            offset: vec![],
            limit: REQUEST_PER_PAGE,
        };
        let id = self.send(Request::Participants(request));
        self.state.borrow_mut().unknown_request = id;
    }
    fn handle_unknown_reply(&self, reply: Result<tl::PhoneGroupParticipants, ()>) {
        let batch = self.state.borrow().unknown_batch.clone();
        let success = reply.is_ok();
        if let Ok(tl::PhoneGroupParticipants::PhoneGroupParticipants(d)) = reply {
            self.owner.effect(Effect::ProcessUsersChats {
                users: d.users.clone(),
                chats: d.chats.clone(),
            });
            self.apply_participants_slice(&d.participants, ApplySliceSource::UnknownLoaded);
        }
        self.state.borrow_mut().unknown_request = 0;
        if success {
            let now = self.host.now();
            for (&ssrc, &when) in &batch.ssrcs {
                if when.anything != 0 || when.voice != 0 {
                    self.apply_last_spoke(ssrc, when, now);
                }
                self.state.borrow_mut().unknown_ssrcs.remove(&ssrc);
            }
            for (&id, &when) in &batch.peers {
                if let Some(peer) = self
                    .peers
                    .peer_loaded(id)
                    .filter(|p| self.participant_by_peer(*p).is_some())
                {
                    self.apply_active_update(id, when, Some(peer));
                }
                self.state.borrow_mut().unknown_peers.remove(&id);
            }
            if !batch.ssrcs.is_empty() {
                self.participants_resolved.fire(batch.ssrcs);
            }
        } else {
            let mut s = self.state.borrow_mut();
            for id in batch.ssrcs.keys() {
                s.unknown_ssrcs.remove(id);
            }
            for id in batch.peers.keys() {
                s.unknown_peers.remove(id);
            }
        }
        self.request_unknown_participants();
    }
}
impl Drop for GroupCall {
    fn drop(&mut self) {
        if self.origin != GroupCallOrigin::Group {
            self.owner.effect(Effect::Unregister(self.key));
        }
        let s = self.state.get_mut();
        for id in [s.unknown_request, s.participants_request, s.reload_request] {
            self.owner.effect(Effect::Cancel(id));
        }
    }
}
/// Real Sender bridge; deliver replies through the Session's model-thread guard.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    request: Request,
    reply: Arc<dyn Fn(i32, Reply) + Send + Sync>,
) {
    match request {
        Request::Participants(r) => {
            let fail = reply.clone();
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(id, Reply::Participants(Ok(r))))
                .fail(move |_| fail(id, Reply::Participants(Err(()))))
                .send();
        }
        Request::Reload(r) => {
            let fail = reply.clone();
            sender
                .request(r)
                .override_id(id)
                .done(move |r| reply(id, Reply::Reload(Ok(r))))
                .fail(move |_| fail(id, Reply::Reload(Err(()))))
                .send();
        }
        Request::SaveSendAs(r) => {
            sender.request(r).override_id(id).send();
        }
    }
}

/// Route only the concrete Sender operations; engine/registry effects stay9/29.
pub fn send_effect(
    sender: &tg_mtproto::sender::Sender,
    e: &Effect,
    reply: Arc<dyn Fn(i32, Reply) + Send + Sync>,
) -> bool {
    match e {
        Effect::Send { id, request } => {
            send_request(sender, *id, request.clone(), reply);
            true
        }
        Effect::Cancel(id) => {
            sender.cancel(*id);
            true
        }
        _ => false,
    }
}
