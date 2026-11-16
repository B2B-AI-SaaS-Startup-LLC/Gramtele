//! Unread projection over real list/setting producers. UnreadState's scalar
//! contract comes from dialogs_common.h; MainList ownership is package18/5.
use tg_rpl::Producer;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnreadState {
    pub messages: i32,
    pub messages_muted: i32,
    pub chats: i32,
    pub chats_muted: i32,
    pub marks: i32,
    pub marks_muted: i32,
    pub reactions: i32,
    pub reactions_muted: i32,
    pub mentions: i32,
    pub polls: i32,
    pub polls_muted: i32,
    pub known: bool,
}
impl std::ops::AddAssign for UnreadState {
    fn add_assign(&mut self, o: Self) {
        self.messages = self.messages.wrapping_add(o.messages);
        self.messages_muted = self.messages_muted.wrapping_add(o.messages_muted);
        self.chats = self.chats.wrapping_add(o.chats);
        self.chats_muted = self.chats_muted.wrapping_add(o.chats_muted);
        self.marks = self.marks.wrapping_add(o.marks);
        self.marks_muted = self.marks_muted.wrapping_add(o.marks_muted);
        self.reactions = self.reactions.wrapping_add(o.reactions);
        self.reactions_muted = self.reactions_muted.wrapping_add(o.reactions_muted);
        self.mentions = self.mentions.wrapping_add(o.mentions);
        self.polls = self.polls.wrapping_add(o.polls);
        self.polls_muted = self.polls_muted.wrapping_add(o.polls_muted);
    }
}
impl std::ops::SubAssign for UnreadState {
    fn sub_assign(&mut self, o: Self) {
        self.messages = self.messages.wrapping_sub(o.messages);
        self.messages_muted = self.messages_muted.wrapping_sub(o.messages_muted);
        self.chats = self.chats.wrapping_sub(o.chats);
        self.chats_muted = self.chats_muted.wrapping_sub(o.chats_muted);
        self.marks = self.marks.wrapping_sub(o.marks);
        self.marks_muted = self.marks_muted.wrapping_sub(o.marks_muted);
        self.reactions = self.reactions.wrapping_sub(o.reactions);
        self.reactions_muted = self.reactions_muted.wrapping_sub(o.reactions_muted);
        self.mentions = self.mentions.wrapping_sub(o.mentions);
        self.polls = self.polls.wrapping_sub(o.polls);
        self.polls_muted = self.polls_muted.wrapping_sub(o.polls_muted);
    }
}
impl std::ops::Add for UnreadState {
    type Output = Self;
    fn add(mut self, o: Self) -> Self {
        self += o;
        self
    }
}
impl std::ops::Sub for UnreadState {
    type Output = Self;
    fn sub(mut self, o: Self) -> Self {
        self -= o;
        self
    }
}
pub fn main_list_unread_state(
    changes: Producer<UnreadState>,
    current: impl Fn() -> UnreadState + 'static,
) -> Producer<UnreadState> {
    tg_rpl::single(())
        .then(changes.map(|_| ()))
        .map(move |_| current())
}
pub fn main_list_map_unread_state(state: UnreadState, archive: Option<UnreadState>) -> UnreadState {
    archive.map_or(state, |a| state - a)
}
pub fn unread_state_value(
    filter_id: i32,
    list: Producer<UnreadState>,
    archive: impl Fn() -> Option<UnreadState> + 'static,
) -> Producer<UnreadState> {
    if filter_id > 0 {
        list
    } else {
        list.map(move |s| main_list_map_unread_state(s, archive()))
    }
}
/// `changed` is precisely Notify::SettingsChange::IncludeMuted, selected by its owner.
pub fn include_muted_counter_folders_value(
    changed: Producer<()>,
    current: impl Fn() -> bool + 'static,
) -> Producer<bool> {
    tg_rpl::single(()).then(changed).map(move |_| current())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BadgesState {
    pub unread_counter: i32,
    pub unread: bool,
    pub unread_muted: bool,
    pub mention: bool,
    pub mention_muted: bool,
    pub reaction: bool,
    pub reaction_muted: bool,
    pub poll: bool,
    pub poll_muted: bool,
}
impl BadgesState {
    pub fn empty(self) -> bool {
        !self.unread && !self.mention && !self.reaction && !self.poll
    }
}
