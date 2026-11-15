//! Audio identity keeps stable document identity across Telegram ID conversion.
use super::{
    data_document::DocumentData, data_msg_id::FullMsgId, data_store_keys::DocumentRecordKey,
};
use std::{
    cmp::Ordering,
    sync::atomic::{AtomicU32, Ordering as AtomicOrdering},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Type {
    #[default]
    Unknown,
    Voice,
    Song,
    Video,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioMsgId {
    audio: Option<DocumentRecordKey>,
    kind: Type,
    context_id: FullMsgId,
    external_play_id: u32,
    changeable_playback_speed: bool,
}
impl AudioMsgId {
    // data_audio_msg_id.cpp:17,32 compares a TimeId(60) with the
    // millisecond duration() (data_document.cpp:1907): actual cutoff is 60ms.
    pub fn new(audio: &DocumentData, msg_id: FullMsgId, external_play_id: u32) -> Self {
        Self {
            audio: Some(audio.key()),
            kind: if audio.is_voice_message() || audio.is_video_message() {
                Type::Voice
            } else if audio.is_video_file() {
                Type::Video
            } else if audio.is_audio_file() {
                Type::Song
            } else {
                Type::Unknown
            },
            context_id: msg_id,
            external_play_id,
            changeable_playback_speed: audio.is_voice_message()
                || audio.is_video_message()
                || audio.duration() >= 60,
        }
    }
    pub fn create_external_play_id() -> u32 {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        loop {
            let n = NEXT.fetch_add(1, AtomicOrdering::Relaxed).wrapping_add(1);
            if n != 0 {
                return n;
            }
        }
    }
    pub fn for_video() -> Self {
        Self {
            kind: Type::Video,
            external_play_id: Self::create_external_play_id(),
            ..Self::default()
        }
    }
    pub fn kind(&self) -> Type {
        self.kind
    }
    pub fn audio(&self) -> Option<DocumentRecordKey> {
        self.audio
    }
    pub fn context_id(&self) -> FullMsgId {
        self.context_id
    }
    pub fn external_play_id(&self) -> u32 {
        self.external_play_id
    }
    pub fn changeable_playback_speed(&self) -> bool {
        self.changeable_playback_speed
    }
    pub fn is_present(&self) -> bool {
        self.audio.is_some() || self.external_play_id != 0
    }
    fn identity(&self) -> (Option<DocumentRecordKey>, FullMsgId, u32) {
        (self.audio, self.context_id, self.external_play_id)
    }
}
impl PartialEq for AudioMsgId {
    fn eq(&self, o: &Self) -> bool {
        self.identity() == o.identity()
    }
}
impl Eq for AudioMsgId {}
impl Ord for AudioMsgId {
    fn cmp(&self, o: &Self) -> Ordering {
        self.identity().cmp(&o.identity())
    }
}
impl PartialOrd for AudioMsgId {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
