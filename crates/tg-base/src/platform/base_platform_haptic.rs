//! Port of the constant part of `base/platform/base_platform_haptic.h`.
//! Feedbackd D-Bus binding is in `linux::base_haptic_linux`.

/// Linux: swipe-back is always enabled (`IsSwipeBackEnabled`).
pub fn is_swipe_back_enabled() -> bool {
    true
}
