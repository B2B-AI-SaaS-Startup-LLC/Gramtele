#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UrlSchemeDescriptor {
    pub executable: String,
    pub arguments: String,
    pub protocol: String,
    pub protocol_name: String,
    pub short_app_name: String,
    pub long_app_name: String,
    pub display_app_name: String,
    pub display_app_description: String,
}

#[cfg(all(unix, not(target_os = "macos")))]
pub use super::linux::base_url_scheme_linux::{
    check_url_scheme, register_url_scheme, unregister_url_scheme, UrlSchemeContext,
};
