//! Headless decision phases. Actual image decoding, dialogs and launch delivery:5/6/7/33.
use crate::core::mime_type::{file_extension, NameType};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageOpenSource {
    #[default]
    None,
    Location,
    Bytes,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageOpenCheck {
    pub mime: String,
    pub source: ImageOpenSource,
    pub size_over_limit: bool,
    pub readable: bool,
    pub open_in_app: bool,
}
pub fn launcher_would_warn(
    name_type: NameType,
    ip_reveal: bool,
    extension: &str,
    verified: bool,
    ip_warning: bool,
    no_warning: &BTreeSet<String>,
) -> bool {
    !verified
        && ((ip_reveal && ip_warning)
            || (matches!(name_type, NameType::Executable | NameType::Unknown)
                && !no_warning.contains(extension)))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchDecision {
    OpenWith,
    Launch,
    Confirm,
}
pub fn launch_decision(path: &str, warn: bool) -> LaunchDecision {
    if file_extension(path).is_empty() {
        LaunchDecision::OpenWith
    } else if warn {
        LaunchDecision::Confirm
    } else {
        LaunchDecision::Launch
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningChange {
    DisableIpWarning,
    AllowExtension,
}
pub fn confirmed_warning_change(checked: bool, ip: bool) -> Option<WarningChange> {
    checked.then_some(if ip {
        WarningChange::DisableIpWarning
    } else {
        WarningChange::AllowExtension
    })
}
/// Inputs are results of the actual MIME service/access guard/decoder, never guessed pixels.
pub fn check_image_open(
    size: i64,
    limit: i64,
    location: Option<(&str, bool)>,
    mime: &str,
    bytes_present: bool,
    readable: bool,
) -> ImageOpenCheck {
    if size >= limit {
        return ImageOpenCheck {
            size_over_limit: true,
            ..Default::default()
        };
    }
    let (source, mime) = if let Some((mime, true)) = location {
        (ImageOpenSource::Location, mime)
    } else if mime.starts_with("image/") && bytes_present {
        (ImageOpenSource::Bytes, mime)
    } else {
        return ImageOpenCheck::default();
    };
    let can = mime.starts_with("image/") && readable;
    ImageOpenCheck {
        mime: mime.into(),
        source,
        readable: can,
        open_in_app: can,
        size_over_limit: false,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveDecision {
    Ignore,
    ShowTheme,
    PlayPause,
    ShowPlayable,
    InspectPlugin,
    ShowImage,
    ShowMarkdownOrLaunch,
    Save,
    Nothing,
}
/// Pure precedence of ResolveDocument; plugin/image/markdown probes are staged by the owner.
pub fn resolve_document(
    is_null: bool,
    theme_loaded: bool,
    playable: bool,
    audio_voice_round: bool,
) -> ResolveDecision {
    if is_null {
        ResolveDecision::Ignore
    } else if theme_loaded {
        ResolveDecision::ShowTheme
    } else if playable {
        if audio_voice_round {
            ResolveDecision::PlayPause
        } else {
            ResolveDecision::ShowPlayable
        }
    } else {
        ResolveDecision::InspectPlugin
    }
}
pub fn resolve_after_save(
    plugin_opened: bool,
    image_opened: bool,
    path_present: bool,
    ready_or_failed: bool,
) -> ResolveDecision {
    if plugin_opened {
        ResolveDecision::Nothing
    } else if image_opened {
        ResolveDecision::ShowImage
    } else if path_present {
        ResolveDecision::ShowMarkdownOrLaunch
    } else if ready_or_failed {
        ResolveDecision::Save
    } else {
        ResolveDecision::Nothing
    }
}
// AyuGram: plugin metadata parsing and the info box remain UI5/6.
pub fn should_inspect_plugin(
    size: i64,
    limit: i64,
    controller: bool,
    filename: &str,
    path_present: bool,
) -> bool {
    size < limit && controller && filename.to_lowercase().ends_with(".plugin") && path_present
}
pub fn is_ip_revealing_path(
    path: &str,
    mime: &str,
    platform: crate::core::mime_type::FilenamePlatform,
) -> bool {
    use crate::core::mime_type::FilenamePlatform as P;
    let ext = file_extension(path).to_lowercase();
    let common = "htm html svg m4v m3u m3u8 xhtml xml kml kmz xspf";
    common.split_whitespace().any(|e|e==ext)||(platform==P::Windows&&ext=="wpl")||(platform==P::Mac&&"docx dotx docm dotm xlsx xltx xlsm xltm xlsb pptx ppsx potx pptm ppsm potm".split_whitespace().any(|e|e==ext))||"text/html image/svg+xml application/vnd.google-earth.kml+xml application/vnd.google-earth.kmz".split_whitespace().any(|e|e==mime)
}
/// Images::kReadBytesLimit, lib_ui/ui/image/image_prepare.h:109.
pub const READ_BYTES_LIMIT: i64 = 64 * 1024 * 1024;
pub fn check_image_open_in_app(
    size: i64,
    location: Option<(&str, bool)>,
    mime: &str,
    bytes_present: bool,
    readable: bool,
) -> ImageOpenCheck {
    check_image_open(
        size,
        READ_BYTES_LIMIT,
        location,
        mime,
        bytes_present,
        readable,
    )
}
