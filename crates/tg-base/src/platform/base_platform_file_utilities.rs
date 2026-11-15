//! Linux filesystem and D-Bus folder effects; actual Qt adapter remains stage5.
#[cfg(all(unix, not(target_os = "macos")))]
pub use super::linux::base_file_utilities_linux::*;
