//! Data-only contracts; null QDateTime is None, valid dates use SystemTime.
use super::{data_msg_id::FullMsgId, data_peer_id::UserId};
use std::time::SystemTime;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoostsOverview {
    pub group: bool,
    pub mine: i32,
    pub level: i32,
    pub boost_count: i32,
    pub current_level_boost_count: i32,
    pub next_level_boost_count: i32,
    pub premium_member_count: i32,
    pub premium_member_percentage: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GiftCodeLink {
    pub text: String,
    pub link: String,
    pub slug: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Boost {
    pub id: String,
    pub user_id: UserId,
    pub giveaway_message: FullMsgId,
    pub date: Option<SystemTime>,
    pub expires_at: Option<SystemTime>,
    pub expires_after_months: i32,
    pub gift_code_link: GiftCodeLink,
    pub multiplier: i32,
    pub credits: u64,
    pub is_gift: bool,
    pub is_giveaway: bool,
    pub is_unclaimed: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OffsetToken {
    pub next: String,
    pub gifts: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoostsListSlice {
    pub list: Vec<Boost>,
    pub multiplied_total: i32,
    pub all_loaded: bool,
    pub token: OffsetToken,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoostPrepaidGiveaway {
    pub date: Option<SystemTime>,
    pub id: u64,
    pub credits: u64,
    pub months: i32,
    pub quantity: i32,
    pub boosts: i32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoostStatus {
    pub overview: BoostsOverview,
    pub first_slice_boosts: BoostsListSlice,
    pub first_slice_gifts: BoostsListSlice,
    pub prepaid_giveaway: Vec<BoostPrepaidGiveaway>,
    pub link: String,
}
