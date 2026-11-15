//! Linux platform contracts. Qt NativeText rendering/native display facts remain5.
pub type GlobalShortcutKeyGeneric = u64;
#[cfg(all(unix, not(target_os = "macos")))]
pub use super::linux::base_global_shortcuts_linux::{
    allowed, available, is_toggle_full_screen_key, key_name, start, stop, GlobalShortcuts, KeyName,
};
