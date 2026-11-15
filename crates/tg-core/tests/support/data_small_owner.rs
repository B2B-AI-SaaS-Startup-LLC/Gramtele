use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
};
use tg_core::{
    data::{
        data_messages::MessagePosition, data_msg_id::*, data_peer_id::*,
        data_search_controller::SearchOwner, data_store_keys::*,
    },
    storage::storage_shared_media::{SharedMediaType, SharedMediaTypesMask},
};
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
pub struct Owner {
    pub keys: RefCell<MessageKeys>,
    pub scope: SessionScopeId,
    pub peer: PeerKey,
    pub requests: RefCell<Vec<(i32, tl::MessagesSearch)>>,
    pub log: RefCell<Vec<String>>,
    pub removed: EventStream<MessageKey>,
    pub cleared: EventStream<PeerKey>,
    pub included: RefCell<BTreeSet<i64>>,
    pub forward: Cell<bool>,
    pub topic: Cell<MsgId>,
    pub mono: Cell<PeerId>,
}
impl Owner {
    pub fn new(s: &SessionScope) -> Self {
        Self {
            keys: RefCell::new(MessageKeys::new(s)),
            scope: s.id(),
            peer: s.peer_key(peer_from_user(5)),
            requests: RefCell::default(),
            log: RefCell::default(),
            removed: EventStream::new(),
            cleared: EventStream::new(),
            included: RefCell::default(),
            forward: Cell::new(true),
            topic: Cell::new(0.into()),
            mono: Cell::new(PeerId::default()),
        }
    }
}
impl SearchOwner for Owner {
    fn input_peer(&self, p: PeerKey) -> tl::InputPeer {
        assert_eq!(p.scope(), self.scope);
        if p.id().is::<0>() {
            tl::mtp_input_peer_user(peer_to_user(p.id()).bare, 0)
        } else if p.id().is::<1>() {
            tl::mtp_input_peer_chat(peer_to_chat(p.id()).bare)
        } else {
            tl::mtp_input_peer_channel(peer_to_channel(p.id()).bare, 0)
        }
    }
    fn process_users(&self, _: &[tl::User]) {
        self.log.borrow_mut().push("users".into());
    }
    fn process_chats(&self, _: &[tl::Chat]) {
        self.log.borrow_mut().push("chats".into());
    }
    fn process_topics(&self, _: PeerKey, _: &[tl::ForumTopic]) {
        self.log.borrow_mut().push("topics".into());
    }
    fn channel_pts_received(&self, _: ChannelKey, pts: i32) {
        self.log.borrow_mut().push(format!("pts:{pts}"));
    }
    fn add_existing_message(&self, m: &tl::Message) -> Option<MessageKey> {
        let tl::Message::MessageEmpty(d) = m else {
            panic!("fixture supports actual empty TL messages only")
        };
        self.log.borrow_mut().push(format!("add:{}", d.id));
        (d.id != 0).then(|| {
            self.keys
                .borrow_mut()
                .insert(FullMsgId::new(self.peer.id(), MsgId::new(d.id.into())))
        })
    }
    fn message_position(&self, m: MessageKey) -> MessagePosition {
        MessagePosition {
            full_id: m.id(),
            date: m.id().msg.bare as i32,
        }
    }
    fn shared_media_types(&self, m: MessageKey) -> SharedMediaTypesMask {
        if self.included.borrow().contains(&m.id().msg.bare) {
            SharedMediaType::Photo.into()
        } else {
            SharedMediaTypesMask::default()
        }
    }
    fn allows_forward(&self, _: MessageKey) -> bool {
        self.forward.get()
    }
    fn topic_root_id(&self, _: MessageKey) -> MsgId {
        self.topic.get()
    }
    fn sublist_peer_id(&self, _: MessageKey) -> PeerId {
        self.mono.get()
    }
    fn item_removed(&self) -> Producer<MessageKey> {
        self.removed.events()
    }
    fn history_cleared(&self) -> Producer<PeerKey> {
        self.cleared.events()
    }
    fn send_history_request(&self, _: PeerKey, r: tl::MessagesSearch) -> i32 {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.requests.borrow_mut().push((id, r));
        id
    }
    fn cancel_history_request(&self, id: i32) {
        self.log.borrow_mut().push(format!("cancel:{id}"));
    }
    fn finish_history_request(&self, id: i32) {
        self.log.borrow_mut().push(format!("finish:{id}"));
    }
}
