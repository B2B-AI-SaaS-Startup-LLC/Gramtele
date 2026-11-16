//! Subscription value records. Dates use the existing Unix-millisecond boundary;
//! local timezone/formatting belongs to29/5, not a fabricated QDateTime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeerSubscription {
    pub credits: u64,
    pub period: i32,
}
impl PeerSubscription {
    pub fn is_present(self) -> bool {
        self.credits > 0 && self.period > 0
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionEntry {
    pub id: String,
    pub invite_hash: String,
    pub title: String,
    pub slug: String,
    pub until: Option<std::time::SystemTime>,
    pub subscription: PeerSubscription,
    pub bare_peer_id: u64,
    pub photo_id: u64,
    pub cancelled: bool,
    pub cancelled_by_bot: bool,
    pub expired: bool,
    pub can_refulfill: bool,
}
impl SubscriptionEntry {
    pub fn is_present(&self) -> bool {
        !self.id.is_empty()
    }
}
