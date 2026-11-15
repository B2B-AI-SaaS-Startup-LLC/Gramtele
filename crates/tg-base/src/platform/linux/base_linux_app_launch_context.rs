use super::base_linux_xdg_activation_token::ActivationTokenProvider;
use std::collections::BTreeMap;
/// Gio launch-context mutation phase. Actual Gio subclass/Qt startup hook stays5.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppLaunchContext {
    overrides: BTreeMap<String, String>,
}
impl AppLaunchContext {
    pub fn new(parent_window_id: String) -> Self {
        let mut overrides = BTreeMap::new();
        if !parent_window_id.is_empty() {
            overrides.insert("PARENT_WINDOW_ID".into(), parent_window_id);
        }
        Self { overrides }
    }
    pub fn environment(&self, mut inherited: BTreeMap<String, String>) -> BTreeMap<String, String> {
        inherited.extend(self.overrides.clone());
        inherited
    }
    pub fn startup_notify_id(&self, provider: &dyn ActivationTokenProvider) -> Option<String> {
        provider.xdg_activation_token()
    }
}
