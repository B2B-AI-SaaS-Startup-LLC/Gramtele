//! Headless archive state. Actual Dialogs::MainList/Entry apply and presentation
//! remain package18/stage5; this type never fabricates a History or list row.
use super::data_store_keys::{PeerKey, ScopedKey, SessionScopeId};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderEffect {
    UpdateChatListEntry,
    UpdateChatListEntryPostponed,
    UpdateChatListSortPosition,
    UpdateChatListExistence,
    RequestDialogEntry,
}
pub struct Folder {
    scope: SessionScopeId,
    id: i32,
    last_histories: Vec<PeerKey>,
    view_version: i32,
    stories_count: u16,
    stories_unread_count: u16,
}
impl Folder {
    pub const ID: i32 = 1;
    pub fn new(scope: SessionScopeId, id: i32) -> Self {
        Self {
            scope,
            id,
            last_histories: vec![],
            view_version: 0,
            stories_count: 0,
            stories_unread_count: 0,
        }
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn last_histories(&self) -> &[PeerKey] {
        &self.last_histories
    }
    pub fn chat_list_view_version(&self) -> i32 {
        self.view_version
    }
    pub fn stories_count(&self) -> i32 {
        i32::from(self.stories_count)
    }
    pub fn stories_unread_count(&self) -> i32 {
        i32::from(self.stories_unread_count)
    }
    pub fn chat_list_name_version(&self) -> i32 {
        1
    }
    pub fn should_be_in_chat_list(&self, list_empty: bool) -> bool {
        !list_empty || self.stories_count > 0
    }
    pub fn chat_list_message_known(&self) -> bool {
        true
    }
    /// MainList's actual indexed order, with current message dates and pin state.
    pub fn reorder_last_histories(
        &mut self,
        indexed: impl IntoIterator<Item = PeerKey>,
        date: impl Fn(PeerKey) -> i32,
        pinned: impl Fn(PeerKey) -> bool,
    ) -> Vec<FolderEffect> {
        self.last_histories.clear();
        let mut checked = 0;
        for h in indexed {
            assert_eq!(h.scope(), self.scope);
            let d = date(h);
            let i = self.last_histories.partition_point(|x| date(*x) >= d);
            if self.last_histories.len() < 8 || i != self.last_histories.len() {
                self.last_histories.insert(i, h)
            }
            if self.last_histories.len() > 8 {
                self.last_histories.pop();
            }
            if !pinned(h) {
                checked += 1;
                if checked >= 8 {
                    break;
                }
            }
        }
        self.view_version = self.view_version.wrapping_add(1);
        vec![FolderEffect::UpdateChatListEntry]
    }
    pub fn register_one_effects(
        indexed_size: usize,
        cloud_unread_known: bool,
    ) -> Vec<FolderEffect> {
        if indexed_size == 1 {
            let mut e = vec![FolderEffect::UpdateChatListSortPosition];
            if !cloud_unread_known {
                e.push(FolderEffect::RequestDialogEntry)
            }
            e
        } else {
            vec![FolderEffect::UpdateChatListEntry]
        }
    }
    pub fn unregister_one_effects(list_empty: bool) -> Vec<FolderEffect> {
        if list_empty {
            vec![FolderEffect::UpdateChatListExistence]
        } else {
            vec![]
        }
    }
    pub fn update_stories_count(&mut self, count: i32, unread: i32) -> Vec<FolderEffect> {
        if self.stories_count() == count && self.stories_unread_count() == unread {
            return vec![];
        }
        let was = self.stories_count > 0;
        self.stories_count = count.min(65535) as u16;
        self.stories_unread_count = unread.min(65535) as u16; // Original clamps only above; negative wraps to ushort.
        let now = self.stories_count > 0;
        self.view_version = self.view_version.wrapping_add(1);
        vec![if was == now {
            FolderEffect::UpdateChatListEntryPostponed
        } else if now {
            FolderEffect::UpdateChatListSortPosition
        } else {
            FolderEffect::UpdateChatListExistence
        }]
    }
}
use super::data_unread_value::BadgesState;
impl Folder {
    pub fn fixed_on_top_index(&self) -> i32 {
        1
    }
    /// The archive itself has no HistoryItem (original returns nullptr).
    pub fn chat_list_message(&self) -> Option<super::data_store_keys::MessageKey> {
        None
    }
    /// BadgesForUnread(Chats,All), then Folder's force-muted and mark-count fix.
    pub fn chat_list_badges_state(state: super::data_unread_value::UnreadState) -> BadgesState {
        let counter = state.marks.wrapping_add(state.chats);
        let mark = counter == 1 && state.marks == 1;
        let unread = counter > 0;
        let n = if mark { 0 } else { counter };
        BadgesState {
            unread_counter: if unread && n == 0 { 1 } else { n },
            unread,
            unread_muted: true,
            mention: state.mentions > 0,
            mention_muted: true,
            reaction: state.reactions > 0,
            reaction_muted: true,
            poll: state.polls > 0,
            poll_muted: true,
        }
    }
}
