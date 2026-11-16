//! Original value contracts; no Session/History/UI stand-ins.
#[derive(Clone, Debug, Default)]
pub struct PremiumSubscriptionOption {
    pub months: i32,
    pub duration: String,
    pub discount: String,
    pub cost_per_month: String,
    pub cost_no_discount: String,
    pub cost_per_year: String,
    pub currency: String,
    pub total: String,
    pub bot_url: String,
}
pub type PremiumSubscriptionOptions = Vec<PremiumSubscriptionOption>;
