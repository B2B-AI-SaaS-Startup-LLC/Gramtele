//! Original value contracts; no Session/History/UI stand-ins.
use super::{data_msg_id::*, data_peer_id::UserId};
#[derive(Clone, Debug, Default)]
pub struct StatisticsMessageInteractionInfo {
    pub message_id: MsgId,
    pub story_id: StoryId,
    pub views_count: i32,
    pub forwards_count: i32,
    pub reactions_count: i32,
}
#[derive(Clone, Debug, Default)]
pub struct StatisticsMessageSenderInfo {
    pub user_id: UserId,
    pub sent_message_count: i32,
    pub average_character_count: i32,
}
#[derive(Clone, Debug, Default)]
pub struct StatisticsAdministratorActionsInfo {
    pub user_id: UserId,
    pub deleted_message_count: i32,
    pub banned_user_count: i32,
    pub restricted_user_count: i32,
}
#[derive(Clone, Debug, Default)]
pub struct StatisticsInviterInfo {
    pub user_id: UserId,
    pub added_member_count: i32,
}
#[derive(Clone, Debug, Default)]
pub struct StatisticsLists {
    pub recent_message_interactions: Vec<StatisticsMessageInteractionInfo>,
    pub top_senders: Vec<StatisticsMessageSenderInfo>,
    pub top_administrators: Vec<StatisticsAdministratorActionsInfo>,
    pub top_inviters: Vec<StatisticsInviterInfo>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecentPostId {
    pub message_id: FullMsgId,
    pub story_id: FullStoryId,
}
#[derive(Clone, Debug, Default)]
pub struct PublicForwardsSlice {
    pub list: Vec<RecentPostId>,
    pub total: i32,
    pub all_loaded: bool,
    pub token: String,
}
impl RecentPostId {
    pub fn valid(&self) -> bool {
        !self.message_id.is_zero() || self.story_id.valid()
    }
}
