#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    PlayPause,
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    Quit,
    Raise,
    LoopNone,
    LoopTrack,
    LoopPlaylist,
    Shuffle,
    SkipBackward,
    SkipForward,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}
#[cfg(all(unix, not(target_os = "macos")))]
pub use super::linux::base_system_media_controls_linux::SystemMediaControls;
