use super::base_linux_library as library;
use crate::platform::base_platform_info::*;
use std::{path::Path, process::Command, sync::OnceLock};
pub fn why_system_becomes_outdated() -> OutdateReason {
    OutdateReason::IsOld
}
pub const fn is_linux() -> bool {
    true
}
pub const fn is_windows() -> bool {
    false
}
pub const fn is_windows_32_bit() -> bool {
    false
}
pub const fn is_windows_64_bit() -> bool {
    false
}
pub const fn is_windows_arm64() -> bool {
    false
}
pub const fn is_windows_store_build() -> bool {
    false
}
pub const fn is_windows7_or_greater() -> bool {
    false
}
pub const fn is_windows8_or_greater() -> bool {
    false
}
pub const fn is_windows8_point1_or_greater() -> bool {
    false
}
pub const fn is_windows10_or_greater() -> bool {
    false
}
pub const fn is_windows11_or_greater() -> bool {
    false
}
pub const fn is_mac() -> bool {
    false
}
pub const fn is_mac_store_build() -> bool {
    false
}
pub const fn is_mac10_12_or_greater() -> bool {
    false
}
pub const fn is_mac10_13_or_greater() -> bool {
    false
}
pub const fn is_mac10_14_or_greater() -> bool {
    false
}
pub const fn is_mac10_15_or_greater() -> bool {
    false
}
pub const fn is_mac11_0_or_greater() -> bool {
    false
}
pub const fn is_mac26_0_or_greater() -> bool {
    false
}
pub const fn metal_supported() -> bool {
    false
}
pub fn desktop_environment(value: &str) -> Vec<String> {
    value
        .split(':')
        .map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|v| !v.is_empty())
        .collect()
}
pub fn chassis_type_to_string(n: u32) -> &'static str {
    match n {
        3 | 4 | 6 | 7 | 13 => "Desktop",
        8 | 9 | 10 | 14 => "Laptop",
        11 => "Handset",
        17 | 28 | 29 => "Server",
        30 => "Tablet",
        31 | 32 => "Convertible",
        _ => "",
    }
}
pub fn version_segments(s: &str) -> Vec<i32> {
    let mut values = vec![];
    let mut rest = s;
    loop {
        // QVersionNumber uses qstrntoull: ASCII whitespace and plus are accepted.
        rest = rest.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
        if let Some(value) = rest.strip_prefix('+') {
            rest = value;
        }
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            break;
        }
        let Ok(value) = rest[..n].parse() else {
            break;
        };
        values.push(value);
        rest = &rest[n..];
        if !rest.starts_with('.') {
            break;
        }
        rest = &rest[1..];
    }
    values
}
pub fn glibc_less_228(name: &str, version: &str) -> bool {
    name == "glibc" && !version.is_empty() && version_segments(version) < vec![2, 28]
}
pub fn valid_libc_version(version: &str) -> bool {
    !version_segments(version).is_empty()
}
pub fn get_libc_name() -> String {
    library::libc_version().0
}
pub fn get_libc_version() -> String {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        let v = library::libc_version().1;
        if valid_libc_version(&v) {
            v
        } else {
            String::new()
        }
    })
    .clone()
}
pub fn when_system_becomes_outdated() -> Option<(i32, u8, u8)> {
    glibc_less_228(&get_libc_name(), &get_libc_version()).then_some((2023, 7, 1))
}
pub fn auto_update_version() -> i32 {
    if when_system_becomes_outdated().is_some() {
        2
    } else {
        6
    }
}
pub fn auto_update_key() -> &'static str {
    "linux"
}
pub fn device_model_from_values(
    product: &str,
    family: &str,
    board: &str,
    virtualization: &str,
    chassis: u32,
) -> String {
    let model = hardware_model(product, family, board).unwrap_or_else(|| {
        let v = virtualization
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_uppercase();
        if !v.is_empty() && v != "NONE" {
            v
        } else {
            chassis_type_to_string(chassis).into()
        }
    });
    finalize_device_model(&model)
}
fn hardware_model(product: &str, family: &str, board: &str) -> Option<String> {
    let product = product_name_to_device_model(&simplify_device_model(product));
    if !product.is_empty() {
        return Some(product);
    }
    let family = simplify_device_model(family);
    let board = simplify_device_model(board);
    let both = simplify_device_model(&format!("{family} {board}"));
    [both, board, family]
        .into_iter()
        .find(|v| is_device_model_ok(v))
}

pub fn device_model_pretty() -> String {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        let read = |name: &str| {
            std::fs::read_to_string(Path::new("/sys/class/dmi/id").join(name)).unwrap_or_default()
        };
        let product = read("product_name");
        let family = read("product_family");
        let board = read("board_name");
        if let Some(early) = hardware_model(&product, &family, &board) {
            return finalize_device_model(&early);
        }
        let virtualization = Command::new("systemd-detect-virt")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        device_model_from_values(
            &product,
            &family,
            &board,
            &virtualization,
            read("chassis_type").trim().parse().unwrap_or(0),
        )
    })
    .clone()
}
#[derive(Clone, Debug, Default)]
pub struct DesktopFacts {
    pub window_manager: String,
    pub x11: bool,
    pub wayland: bool,
    pub xwayland: bool,
}
pub fn system_version_from_values(
    uname: Option<(&str, &str)>,
    desktop: &str,
    facts: &DesktopFacts,
    libc: &str,
    version: &str,
    linux: bool,
) -> String {
    let mut result = vec![];
    if let Some((name, release)) = uname {
        result.push(name.to_owned());
        if !linux {
            result.push(release.to_owned());
        }
    } else {
        result.push("Unknown".into());
    }
    let env = desktop_environment(desktop);
    if env.is_empty() {
        if !facts.window_manager.is_empty() {
            result.push(facts.window_manager.clone());
        }
    } else {
        result.extend(env);
    }
    if facts.wayland {
        result.push("Wayland".into());
    } else if facts.xwayland {
        result.push("Xwayland".into());
    } else if facts.x11 {
        result.push("X11".into());
    }
    if !version.is_empty() {
        result.push(if libc.is_empty() { "libc" } else { libc }.into());
        result.push(version.into());
    }
    result.join(" ")
}
/// Qt/native-display facts are supplied by the real window/XCB layer5/part2.
pub fn system_version_pretty(facts: &DesktopFacts) -> String {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        let u = library::uname();
        system_version_from_values(
            u.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
            &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            facts,
            &get_libc_name(),
            &get_libc_version(),
            cfg!(target_os = "linux"),
        )
    })
    .clone()
}
pub fn headless_is_wayland() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| {
        library::probe_wayland().unwrap_or_else(|| std::env::var_os("WAYLAND_DISPLAY").is_some())
    })
}
/// Locale is the genuine toolkit/system locale; POSIX LANG is not QLocale.
pub fn system_country(locale_name: &str) -> String {
    locale_name
        .split('_')
        .next_back()
        .unwrap_or_default()
        .into()
}
pub fn system_language(locale_name: &str, ui_languages: &[String]) -> String {
    ui_languages
        .first()
        .cloned()
        .unwrap_or_else(|| locale_name.split('_').next().unwrap_or_default().into())
}
pub fn start(_settings: serde_json::Value) {}
pub fn finish() {}

/// Actual headless XCB branch; Qt/native-display selection remains stage5.
pub fn headless_is_x11() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        super::base_linux_xcb_utilities::shared_connection().is_some_and(|c| !c.has_error())
    })
}
pub fn xcb_desktop_facts(connection: &super::base_linux_xcb_library::Connection) -> DesktopFacts {
    DesktopFacts {
        window_manager: super::base_linux_xcb_utilities::window_manager(connection),
        x11: !connection.has_error(),
        wayland: false,
        xwayland: connection.extension("XWAYLAND").is_some(),
    }
}

pub fn headless_is_xwayland() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        super::base_linux_xcb_utilities::shared_connection()
            .is_some_and(|c| c.extension("XWAYLAND").is_some())
    })
}
