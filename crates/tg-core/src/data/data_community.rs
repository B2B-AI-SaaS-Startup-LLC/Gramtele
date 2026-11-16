//! Community membership/date/order model. Real History/MainList and marked preview:18/01/5.
use super::{data_peer_id::*, data_session::Peers, data_store_keys::*};
use std::{collections::BTreeSet, rc::Rc};
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommunityLinkedPeer {
    pub peer: PeerKey,
    pub visible: Option<bool>,
    pub can_view_history: bool,
}
pub fn is_community_chat_viewable(linked: &CommunityLinkedPeer) -> bool {
    linked.can_view_history
}
pub fn peer_linked_community_id(peers: &Peers, peer: PeerKey) -> ChannelId {
    if let Ok(k) = ChannelKey::try_from(peer) {
        peers.read_channel(k, |c| c.linked_community_id())
    } else if let Ok(k) = UserKey::try_from(peer) {
        peers.read_user(k, |u| u.linked_community_id())
    } else {
        ChannelId::new(0)
    }
}
pub fn community_chat_joined(
    peers: &Peers,
    peer: PeerKey,
    chat_list_message_present: impl FnOnce(PeerKey) -> bool,
) -> bool {
    if let Ok(k) = ChannelKey::try_from(peer) {
        peers.read_channel(k, |c| c.am_in())
    } else if UserKey::try_from(peer).is_ok() {
        chat_list_message_present(peer)
    } else {
        false
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommunityEffect {
    EnsureRow,
    LinkedPeersChanged,
    SetLinkedCommunity { peer: PeerKey, id: ChannelId },
    UpdateRowSortPosition,
    RepaintRow,
    UpdateChatListExistence(PeerKey),
    RemoveFromChatList { peer: PeerKey, collapsed: bool },
    UpdateChatListSortPosition(PeerKey),
}
pub struct CommunityInfo {
    channel: ChannelKey,
    peers: Rc<Peers>,
    linked: Vec<CommunityLinkedPeer>,
    hidden: BTreeSet<PeerKey>,
    histories: BTreeSet<PeerKey>,
    other_histories: BTreeSet<PeerKey>,
    last: Vec<PeerKey>,
    date: i32,
    version: i32,
    collapsed: bool,
    linked_changes: EventStream<()>,
    refreshed: EventStream<()>,
}
impl CommunityInfo {
    pub fn new(peers: Rc<Peers>, channel: ChannelKey) -> Self {
        let collapsed = peers.read_channel(channel, |c| c.collapsed_in_dialogs());
        Self {
            channel,
            peers,
            linked: vec![],
            hidden: BTreeSet::new(),
            histories: BTreeSet::new(),
            other_histories: BTreeSet::new(),
            last: vec![],
            date: 0,
            version: 0,
            collapsed,
            linked_changes: EventStream::new(),
            refreshed: EventStream::new(),
        }
    }
    pub fn channel(&self) -> ChannelKey {
        self.channel
    }
    pub fn linked_peers(&self) -> &[CommunityLinkedPeer] {
        &self.linked
    }
    pub fn linked_peers_value(&self) -> Producer<()> {
        tg_rpl::single(()).then(self.linked_changes.events())
    }
    pub fn refreshed(&self) -> Producer<()> {
        self.refreshed.events()
    }
    pub fn is_hidden(&self, peer: PeerKey) -> bool {
        self.hidden.contains(&peer)
    }
    pub fn collapsed_in_dialogs(&self) -> bool {
        self.peers
            .read_channel(self.channel, |c| c.collapsed_in_dialogs())
    }
    pub fn collapsed_in_chat_lists(&self) -> bool {
        self.collapsed
    }
    pub fn histories(&self) -> &BTreeSet<PeerKey> {
        &self.histories
    }
    pub fn last_histories(&self) -> &[PeerKey] {
        &self.last
    }
    pub fn chats_list_date(&self) -> i32 {
        self.date
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn apply_link_effect(peers: &Peers, effect: CommunityEffect) {
        if let CommunityEffect::SetLinkedCommunity { peer, id } = effect {
            if let Ok(k) = ChannelKey::try_from(peer) {
                peers.set_channel_linked_community_id(k, id)
            } else if let Ok(k) = UserKey::try_from(peer) {
                peers.set_user_linked_community_id(k, id)
            }
        }
    }
    /// Returns ordered UI/History effects; caller publishes changes after its store borrow ends.
    pub fn apply_linked_peers(
        &mut self,
        list: &[tg_scheme::CommunityPeer],
    ) -> Vec<CommunityEffect> {
        let now = list
            .iter()
            .map(
                |tg_scheme::CommunityPeer::CommunityPeer(d)| CommunityLinkedPeer {
                    peer: self.peers.ensure_peer(peer_from_mtp(&d.peer)),
                    visible: d.vvisible(),
                    can_view_history: d.is_can_view_history(),
                },
            )
            .collect::<Vec<_>>();
        if now == self.linked {
            return vec![];
        }
        self.linked = now;
        self.hidden = self
            .linked
            .iter()
            .filter(|p| p.visible == Some(false))
            .map(|p| p.peer)
            .collect();
        let id = ChannelId::new(peer_to_bare_mtp_int(self.channel.peer().id()));
        let mut effects = Vec::new();
        for p in &self.linked {
            effects.push(CommunityEffect::SetLinkedCommunity { peer: p.peer, id });
        }
        for p in self.histories.iter().chain(self.other_histories.iter()) {
            if !self.linked.iter().any(|x| x.peer == *p)
                && peer_linked_community_id(&self.peers, *p) == id
            {
                effects.push(CommunityEffect::SetLinkedCommunity {
                    peer: *p,
                    id: ChannelId::new(0),
                });
            }
        }
        self.version = self.version.wrapping_add(1);
        effects.push(CommunityEffect::RepaintRow);
        effects.push(CommunityEffect::LinkedPeersChanged);
        effects
    }
    pub fn notify_linked_changed(&self) {
        self.linked_changes.fire(())
    }
    pub fn repaint_row(&self) {
        self.refreshed.fire(())
    }
    pub fn reorder_last_histories(
        &mut self,
        message_date: impl Fn(PeerKey) -> i32,
    ) -> Vec<CommunityEffect> {
        self.last.clear();
        for &h in &self.histories {
            let date = message_date(h);
            let i = self.last.partition_point(|x| message_date(*x) >= date);
            if self.last.len() < 20 || i != self.last.len() {
                self.last.insert(i, h)
            }
            if self.last.len() > 20 {
                self.last.pop();
            }
        }
        self.version = self.version.wrapping_add(1);
        vec![CommunityEffect::RepaintRow]
    }
    pub fn register_one(
        &mut self,
        history: PeerKey,
        joined: bool,
        time_id: i32,
        message_date: impl Fn(PeerKey) -> i32,
    ) -> Vec<CommunityEffect> {
        assert_eq!(history.scope(), self.channel.scope());
        let mut effects = vec![];
        if joined {
            if !self.histories.insert(history) {
                return effects;
            }
            self.date = self.date.max(time_id);
            effects.extend(self.reorder_last_histories(message_date));
            if self.collapsed_in_dialogs() {
                effects.push(CommunityEffect::UpdateChatListExistence(history))
            }
        } else if !self.other_histories.insert(history) {
            return effects;
        }
        effects.extend([
            CommunityEffect::EnsureRow,
            CommunityEffect::UpdateRowSortPosition,
        ]);
        effects
    }
    pub fn unregister_one(
        &mut self,
        history: PeerKey,
        time_id: i32,
        dates: impl Fn(PeerKey) -> i32,
        message_date: impl Fn(PeerKey) -> i32,
    ) -> Vec<CommunityEffect> {
        if !self.histories.remove(&history) {
            self.other_histories.remove(&history);
            return vec![];
        }
        if time_id >= self.date {
            self.date = self
                .histories
                .iter()
                .map(|h| dates(*h))
                .max()
                .unwrap_or(0)
                .max(0)
        }
        let mut e = self.reorder_last_histories(message_date);
        e.push(CommunityEffect::UpdateRowSortPosition);
        e
    }
    pub fn refresh_one_membership(
        &mut self,
        history: PeerKey,
        joined: bool,
        dates: impl Fn(PeerKey) -> i32,
        message_date: impl Fn(PeerKey) -> i32,
    ) -> Vec<CommunityEffect> {
        if joined {
            if self.histories.contains(&history) || !self.other_histories.remove(&history) {
                return vec![];
            }
            self.histories.insert(history);
            self.date = self.date.max(dates(history));
            let mut e = self.reorder_last_histories(message_date);
            if self.collapsed_in_dialogs() {
                e.push(CommunityEffect::UpdateChatListExistence(history))
            }
            e.push(CommunityEffect::UpdateRowSortPosition);
            e.push(CommunityEffect::LinkedPeersChanged);
            e
        } else {
            if !self.histories.remove(&history) {
                return vec![];
            }
            self.other_histories.insert(history);
            if dates(history) >= self.date {
                self.date = self
                    .histories
                    .iter()
                    .map(|h| dates(*h))
                    .max()
                    .unwrap_or(0)
                    .max(0)
            }
            let mut e = self.reorder_last_histories(message_date);
            e.push(CommunityEffect::UpdateRowSortPosition);
            e.push(CommunityEffect::LinkedPeersChanged);
            e
        }
    }
    pub fn one_chats_list_date_changed(
        &mut self,
        was: i32,
        now: i32,
        dates: impl Fn(PeerKey) -> i32,
    ) -> Vec<CommunityEffect> {
        if now >= self.date {
            self.date = now
        } else if was >= self.date {
            self.date = self
                .histories
                .iter()
                .map(|h| dates(*h))
                .max()
                .unwrap_or(0)
                .max(0)
        } else {
            return vec![];
        }
        vec![CommunityEffect::UpdateRowSortPosition]
    }
    pub fn one_unread_state_changed(&mut self) -> Vec<CommunityEffect> {
        self.version = self.version.wrapping_add(1);
        vec![CommunityEffect::RepaintRow]
    }
    /// Caller releases its store borrow before removing the planned old-list rows.
    pub fn collapsed_change_plan(&self, in_chat_list: impl Fn(PeerKey) -> bool) -> CollapsePlan {
        let mut seen = BTreeSet::new();
        let moved = self
            .histories
            .iter()
            .chain(self.other_histories.iter())
            .copied()
            .filter(|h| in_chat_list(*h) && seen.insert(*h))
            .collect();
        CollapsePlan {
            old_collapsed: self.collapsed,
            new_collapsed: self.collapsed_in_dialogs(),
            moved,
        }
    }
    pub fn finish_collapsed_change(&mut self, plan: CollapsePlan) -> Vec<CommunityEffect> {
        self.collapsed = plan.new_collapsed;
        let moved = plan.moved.into_iter().collect::<BTreeSet<_>>();
        let mut effects = moved
            .iter()
            .map(|h| CommunityEffect::UpdateChatListSortPosition(*h))
            .collect::<Vec<_>>();
        for h in &self.histories {
            if !moved.contains(h) {
                effects.push(CommunityEffect::UpdateChatListSortPosition(*h));
                effects.push(CommunityEffect::UpdateChatListExistence(*h));
            }
        }
        effects.extend([CommunityEffect::EnsureRow, CommunityEffect::RepaintRow]);
        effects
    }
}
#[derive(Clone, Debug)]
pub struct CollapsePlan {
    pub old_collapsed: bool,
    pub new_collapsed: bool,
    pub moved: Vec<PeerKey>,
}
