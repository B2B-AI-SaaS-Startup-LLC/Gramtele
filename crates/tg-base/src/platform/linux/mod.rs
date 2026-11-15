//! Port of `Telegram/lib_base/base/platform/linux/`: the files that need
//! neither GLib, D-Bus, X11 nor Qt.

pub mod base_battery_saving_linux;
pub mod base_file_utilities_linux;
pub mod base_haptic_linux;
pub mod base_info_linux;
pub mod base_last_input_linux;
pub mod base_layout_switch_linux;
pub mod base_linux_app_launch_context;
pub mod base_linux_dbus_utilities;
pub mod base_linux_library;
pub mod base_linux_xdg_activation_token;
pub mod base_linux_xdp_utilities;
pub mod base_network_reachability_linux;
pub mod base_power_save_blocker_linux;
pub mod base_process_linux;
pub mod base_screen_reader_state_linux;
pub mod base_system_unlock_linux;
pub mod base_url_scheme_linux;
