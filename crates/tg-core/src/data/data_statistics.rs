//! Original value contracts; no Session/History/UI stand-ins.
use super::data_statistics_chart::*;
#[derive(Clone, Debug, Default)]
pub struct StatisticalValue {
    pub value: f64,
    pub previous_value: f64,
    pub growth_rate_percentage: f64,
}
#[derive(Clone, Debug, Default)]
pub struct ChannelStatistics {
    pub start_date: i32,
    pub end_date: i32,
    pub member_count: StatisticalValue,
    pub mean_view_count: StatisticalValue,
    pub mean_share_count: StatisticalValue,
    pub mean_reaction_count: StatisticalValue,
    pub mean_story_view_count: StatisticalValue,
    pub mean_story_share_count: StatisticalValue,
    pub mean_story_reaction_count: StatisticalValue,
    pub enabled_notifications_percentage: f64,
    pub member_count_graph: StatisticalGraph,
    pub join_graph: StatisticalGraph,
    pub mute_graph: StatisticalGraph,
    pub view_count_by_hour_graph: StatisticalGraph,
    pub view_count_by_source_graph: StatisticalGraph,
    pub join_by_source_graph: StatisticalGraph,
    pub language_graph: StatisticalGraph,
    pub message_interaction_graph: StatisticalGraph,
    pub instant_view_interaction_graph: StatisticalGraph,
    pub reactions_by_emotion_graph: StatisticalGraph,
    pub story_interactions_graph: StatisticalGraph,
    pub story_reactions_by_emotion_graph: StatisticalGraph,
}
#[derive(Clone, Debug, Default)]
pub struct SupergroupStatistics {
    pub start_date: i32,
    pub end_date: i32,
    pub member_count: StatisticalValue,
    pub message_count: StatisticalValue,
    pub viewer_count: StatisticalValue,
    pub sender_count: StatisticalValue,
    pub member_count_graph: StatisticalGraph,
    pub join_graph: StatisticalGraph,
    pub join_by_source_graph: StatisticalGraph,
    pub language_graph: StatisticalGraph,
    pub message_content_graph: StatisticalGraph,
    pub action_graph: StatisticalGraph,
    pub day_graph: StatisticalGraph,
    pub week_graph: StatisticalGraph,
}
#[derive(Clone, Debug, Default)]
pub struct MessageStatistics {
    pub message_interaction_graph: StatisticalGraph,
    pub reactions_by_emotion_graph: StatisticalGraph,
    pub public_forwards: i32,
    pub private_forwards: i32,
    pub views: i32,
    pub reactions: i32,
}
#[derive(Clone, Debug, Default)]
pub struct AnyStatistics {
    pub channel: ChannelStatistics,
    pub supergroup: SupergroupStatistics,
    pub message: MessageStatistics,
    pub story: StoryStatistics,
}
pub type StoryStatistics = MessageStatistics;
impl ChannelStatistics {
    pub fn empty(&self) -> bool {
        self.start_date == 0 || self.end_date == 0
    }
    pub fn is_present(&self) -> bool {
        !self.empty()
    }
}
impl SupergroupStatistics {
    pub fn empty(&self) -> bool {
        self.start_date == 0 || self.end_date == 0
    }
    pub fn is_present(&self) -> bool {
        !self.empty()
    }
}
impl MessageStatistics {
    pub fn is_present(&self) -> bool {
        !self.message_interaction_graph.chart.empty() || self.views != 0
    }
}
