//! Getter contracts over the actual Main::Session/AppConfig boundary (29).
//! No Session stand-in and no cached limits: every call reads the owner.
pub trait LimitsContext {
    fn app_config_limit(&self, key: &str, fallback: i32) -> i32;
    fn is_premium(&self) -> bool;
    fn is_test_mode(&self) -> bool;
}
pub struct PremiumLimits<'a> {
    context: &'a dyn LimitsContext,
}
impl<'a> PremiumLimits<'a> {
    pub fn new(context: &'a dyn LimitsContext) -> Self {
        Self { context }
    }
    pub fn channels_default(&self) -> i32 {
        self.context.app_config_limit("channels_limit_default", 500)
    }
    pub fn channels_premium(&self) -> i32 {
        self.context
            .app_config_limit("channels_limit_premium", 1000)
    }
    pub fn channels_current(&self) -> i32 {
        if self.context.is_premium() {
            self.channels_premium()
        } else {
            self.channels_default()
        }
    }
    pub fn similar_channels_default(&self) -> i32 {
        self.context
            .app_config_limit("recommended_channels_limit_default", 10)
    }
    pub fn similar_channels_premium(&self) -> i32 {
        self.context
            .app_config_limit("recommended_channels_limit_premium", 100)
    }
    // Source uses channels limits here, not recommended_channels limits.
    // AyuGramDesktop@0f2a31bb data/data_premium_limits.cpp::similarChannelsCurrent.
    pub fn similar_channels_current(&self) -> i32 {
        if self.context.is_premium() {
            self.channels_premium()
        } else {
            self.channels_default()
        }
    }
    pub fn gifs_default(&self) -> i32 {
        self.context
            .app_config_limit("saved_gifs_limit_default", 200)
    }
    pub fn gifs_premium(&self) -> i32 {
        self.context
            .app_config_limit("saved_gifs_limit_premium", 400)
    }
    pub fn gifs_current(&self) -> i32 {
        if self.context.is_premium() {
            self.gifs_premium()
        } else {
            self.gifs_default()
        }
    }
    pub fn stickers_faved_default(&self) -> i32 {
        self.context
            .app_config_limit("stickers_faved_limit_default", 5)
    }
    pub fn stickers_faved_premium(&self) -> i32 {
        self.context
            .app_config_limit("stickers_faved_limit_premium", 10)
    }
    pub fn stickers_faved_current(&self) -> i32 {
        if self.context.is_premium() {
            self.stickers_faved_premium()
        } else {
            self.stickers_faved_default()
        }
    }
    pub fn dialog_filters_default(&self) -> i32 {
        self.context
            .app_config_limit("dialog_filters_limit_default", 10)
    }
    pub fn dialog_filters_premium(&self) -> i32 {
        self.context
            .app_config_limit("dialog_filters_limit_premium", 30)
    }
    pub fn dialog_filters_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialog_filters_premium()
        } else {
            self.dialog_filters_default()
        }
    }
    pub fn dialog_shareable_filters_default(&self) -> i32 {
        self.context
            .app_config_limit("chatlists_joined_limit_default", 2)
    }
    pub fn dialog_shareable_filters_premium(&self) -> i32 {
        self.context
            .app_config_limit("chatlists_joined_limit_premium", 20)
    }
    pub fn dialog_shareable_filters_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialog_shareable_filters_premium()
        } else {
            self.dialog_shareable_filters_default()
        }
    }
    pub fn dialog_filters_chats_default(&self) -> i32 {
        self.context
            .app_config_limit("dialog_filters_chats_limit_default", 100)
    }
    pub fn dialog_filters_chats_premium(&self) -> i32 {
        self.context
            .app_config_limit("dialog_filters_chats_limit_premium", 200)
    }
    pub fn dialog_filters_chats_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialog_filters_chats_premium()
        } else {
            self.dialog_filters_chats_default()
        }
    }
    pub fn dialog_filters_links_default(&self) -> i32 {
        self.context
            .app_config_limit("chatlist_invites_limit_default", 3)
    }
    pub fn dialog_filters_links_premium(&self) -> i32 {
        self.context
            .app_config_limit("chatlist_invites_limit_premium", 20)
    }
    pub fn dialog_filters_links_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialog_filters_links_premium()
        } else {
            self.dialog_filters_links_default()
        }
    }
    pub fn dialogs_pinned_default(&self) -> i32 {
        self.context
            .app_config_limit("dialogs_pinned_limit_default", 5)
    }
    pub fn dialogs_pinned_premium(&self) -> i32 {
        self.context
            .app_config_limit("dialogs_pinned_limit_premium", 10)
    }
    pub fn dialogs_pinned_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialogs_pinned_premium()
        } else {
            self.dialogs_pinned_default()
        }
    }
    pub fn dialogs_folder_pinned_default(&self) -> i32 {
        self.context
            .app_config_limit("dialogs_folder_pinned_limit_default", 100)
    }
    pub fn dialogs_folder_pinned_premium(&self) -> i32 {
        self.context
            .app_config_limit("dialogs_folder_pinned_limit_premium", 200)
    }
    pub fn dialogs_folder_pinned_current(&self) -> i32 {
        if self.context.is_premium() {
            self.dialogs_folder_pinned_premium()
        } else {
            self.dialogs_folder_pinned_default()
        }
    }
    pub fn topics_pinned_current(&self) -> i32 {
        self.context.app_config_limit("topics_pinned_limit", 5)
    }
    pub fn saved_sublists_pinned_default(&self) -> i32 {
        self.context
            .app_config_limit("saved_dialogs_pinned_limit_default", 5)
    }
    pub fn saved_sublists_pinned_premium(&self) -> i32 {
        self.context
            .app_config_limit("saved_dialogs_pinned_limit_premium", 100)
    }
    pub fn saved_sublists_pinned_current(&self) -> i32 {
        if self.context.is_premium() {
            self.saved_sublists_pinned_premium()
        } else {
            self.saved_sublists_pinned_default()
        }
    }
    pub fn channels_public_default(&self) -> i32 {
        self.context
            .app_config_limit("channels_public_limit_default", 10)
    }
    pub fn channels_public_premium(&self) -> i32 {
        self.context
            .app_config_limit("channels_public_limit_premium", 20)
    }
    pub fn channels_public_current(&self) -> i32 {
        if self.context.is_premium() {
            self.channels_public_premium()
        } else {
            self.channels_public_default()
        }
    }
    pub fn caption_length_default(&self) -> i32 {
        self.context
            .app_config_limit("caption_length_limit_default", 1024)
    }
    pub fn caption_length_premium(&self) -> i32 {
        self.context
            .app_config_limit("caption_length_limit_premium", 2048)
    }
    pub fn caption_length_current(&self) -> i32 {
        if self.context.is_premium() {
            self.caption_length_premium()
        } else {
            self.caption_length_default()
        }
    }
    pub fn message_length_default(&self) -> i32 {
        self.context
            .app_config_limit("message_length_limit_default", 4096)
    }
    pub fn message_length_premium(&self) -> i32 {
        self.context
            .app_config_limit("message_length_limit_premium", 8192)
    }
    pub fn message_length_current(&self) -> i32 {
        if self.context.is_premium() {
            self.message_length_premium()
        } else {
            self.message_length_default()
        }
    }
    pub fn upload_max_default(&self) -> i32 {
        self.context
            .app_config_limit("upload_max_fileparts_default", 4000)
    }
    pub fn upload_max_premium(&self) -> i32 {
        self.context
            .app_config_limit("upload_max_fileparts_premium", 8000)
    }
    pub fn upload_max_current(&self) -> i32 {
        if self.context.is_premium() {
            self.upload_max_premium()
        } else {
            self.upload_max_default()
        }
    }
    pub fn about_length_default(&self) -> i32 {
        self.context
            .app_config_limit("about_length_limit_default", 70)
    }
    pub fn about_length_premium(&self) -> i32 {
        self.context
            .app_config_limit("about_length_limit_premium", 140)
    }
    pub fn about_length_current(&self) -> i32 {
        if self.context.is_premium() {
            self.about_length_premium()
        } else {
            self.about_length_default()
        }
    }
    pub fn contact_note_length_current(&self) -> i32 {
        self.context
            .app_config_limit("contact_note_length_limit", 128)
    }
    pub fn max_boost_level(&self) -> i32 {
        self.context.app_config_limit(
            "boosts_channel_level_max",
            if self.context.is_test_mode() { 9 } else { 99 },
        )
    }
    pub fn bots_create_default(&self) -> i32 {
        self.context
            .app_config_limit("bots_create_limit_default", 20)
    }
    pub fn bots_create_premium(&self) -> i32 {
        self.context
            .app_config_limit("bots_create_limit_premium", 40)
    }
    pub fn ai_compose_saved_tones_default(&self) -> i32 {
        self.context
            .app_config_limit("aicompose_tone_saved_limit_default", 5)
    }
    pub fn ai_compose_saved_tones_premium(&self) -> i32 {
        self.context
            .app_config_limit("aicompose_tone_saved_limit_premium", 20)
    }
}
pub struct LevelLimits<'a> {
    context: &'a dyn LimitsContext,
}
impl<'a> LevelLimits<'a> {
    pub fn new(context: &'a dyn LimitsContext) -> Self {
        Self { context }
    }
    pub fn channel_color_level_min(&self) -> i32 {
        self.context.app_config_limit("channel_color_level_min", 5)
    }
    pub fn channel_bg_icon_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_bg_icon_level_min", 4)
    }
    pub fn channel_profile_bg_icon_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_profile_bg_icon_level_min", 7)
    }
    pub fn channel_emoji_status_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_emoji_status_level_min", 8)
    }
    pub fn channel_wallpaper_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_wallpaper_level_min", 9)
    }
    pub fn channel_custom_wallpaper_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_custom_wallpaper_level_min", 10)
    }
    pub fn channel_restrict_sponsored_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_restrict_sponsored_level_min", 20)
    }
    pub fn channel_auto_translate_level_min(&self) -> i32 {
        self.context
            .app_config_limit("channel_autotranslation_level_min", 3)
    }
    pub fn group_transcribe_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_transcribe_level_min", 6)
    }
    pub fn group_emoji_stickers_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_emoji_stickers_level_min", 4)
    }
    pub fn group_profile_bg_icon_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_profile_bg_icon_level_min", 5)
    }
    pub fn group_emoji_status_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_emoji_status_level_min", 8)
    }
    pub fn group_wallpaper_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_wallpaper_level_min", 9)
    }
    pub fn group_custom_wallpaper_level_min(&self) -> i32 {
        self.context
            .app_config_limit("group_custom_wallpaper_level_min", 10)
    }
}
