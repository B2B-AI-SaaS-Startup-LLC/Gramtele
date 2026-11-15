//! Actual QWaylandWindow token request/single-shot signal and nested Qt loop
//! remain5. Required provider preserves null QString vs a non-null empty token.
pub trait ActivationTokenProvider {
    fn run_with_xdg_activation_token(&self, callback: Box<dyn FnOnce(Option<String>)>);
    fn xdg_activation_token(&self) -> Option<String>;
}
