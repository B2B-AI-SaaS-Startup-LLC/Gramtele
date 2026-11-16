//! UserData owner phases. No RefCell borrow survives Flags/Changes/effect fire.
use super::{
    data_changes::{PeerFlag as F, PeerFlags},
    data_peer::*,
    data_peer_id::*,
    data_session::{NameFields, NameIndex, PeerRecord, Peers},
    data_store_keys::*,
    data_user::*,
    data_user_names::Usernames,
};
impl Peers {
    pub fn ensure_user(&self, id: UserId) -> UserKey {
        UserKey::try_from(self.ensure_peer(peer_from_user(id))).unwrap()
    }
    pub fn read_user<R>(&self, k: UserKey, f: impl FnOnce(&UserData) -> R) -> R {
        self.check_scope(k.peer());
        let p = self.peers.borrow();
        match p.get(&k.peer().id()).expect("user not ensured") {
            PeerRecord::User(u) => f(u),
            _ => unreachable!(),
        }
    }
    /// Mutation phase only; do not fire producers within this closure.
    pub fn edit_user<R>(&self, k: UserKey, f: impl FnOnce(&mut UserData) -> R) -> R {
        self.check_scope(k.peer());
        let mut p = self.peers.borrow_mut();
        match p.get_mut(&k.peer().id()).expect("user not ensured") {
            PeerRecord::User(u) => f(u),
            _ => unreachable!(),
        }
    }
    pub fn user_effects(&self) -> tg_rpl::Producer<UserEffect> {
        self.user_effects.events()
    }
    pub fn dispatch_user(&self, effects: Vec<UserEffect>) {
        for e in effects {
            match e {
                UserEffect::Peer(e) => self.dispatch(vec![e]),
                e => self.user_effects.fire(e),
            }
        }
    }
    pub(crate) fn user_changed(&self, k: UserKey, f: impl Into<PeerFlags>) {
        self.dispatch(vec![PeerEffect::Updated(k.peer(), f.into())])
    }
    /// Non-bot Forum is stripped, Self is sticky. A real Forum lifecycle is
    /// package08/18; until then enabling Forum is rejected explicitly.
    pub fn set_user_flags(
        &self,
        k: UserKey,
        mut which: UserDataFlags,
    ) -> Result<(), UserForumRequired> {
        let (flags, bot) = self.read_user(k, |u| (u.flags.clone(), u.is_bot()));
        if !bot {
            which &= !UserDataFlags::from(UserDataFlag::Forum);
        }
        let diff = flags.current() ^ which;
        if (diff & UserDataFlag::Forum).any() {
            return Err(UserForumRequired);
        }
        if (diff & UserDataFlag::Deleted).any() {
            self.dispatch(vec![PeerEffect::InvalidateEmptyUserpic(k.peer())]);
        }
        flags.set(
            (flags.current() & UserDataFlag::SelfUser)
                | (which & !UserDataFlags::from(UserDataFlag::SelfUser)),
        );
        Ok(())
    }
    pub fn add_user_flags(&self, k: UserKey, f: UserDataFlags) -> Result<(), UserForumRequired> {
        let which = self.read_user(k, UserData::flags) | f;
        self.set_user_flags(k, which)
    }
    pub fn remove_user_flags(&self, k: UserKey, f: UserDataFlags) -> Result<(), UserForumRequired> {
        let which = self.read_user(k, UserData::flags) & !f;
        self.set_user_flags(k, which)
    }
    pub fn set_user_access_hash(&self, k: UserKey, h: u64) {
        if h == u64::MAX {
            let flags = self.edit_user(k, |u| {
                u.access_hash = 0;
                u.flags.clone()
            });
            flags.add(UserDataFlag::Deleted.into());
            self.dispatch(vec![PeerEffect::InvalidateEmptyUserpic(k.peer())]);
        } else {
            self.edit_user(k, |u| u.access_hash = h)
        }
    }
    pub fn set_user_is_contact(&self, k: UserKey, is: bool) {
        let changed = self.edit_user(k, |u| {
            let s = if is {
                ContactStatus::Contact
            } else {
                ContactStatus::NotContact
            };
            let c = u.contact_status != s;
            u.contact_status = s;
            c
        });
        if changed {
            self.user_changed(k, F::IsContact)
        }
    }
    pub fn update_user_lastseen(
        &self,
        k: UserKey,
        s: super::data_lastseen_status::LastseenStatus,
    ) -> bool {
        let changed = self.edit_user(k, |u| {
            if u.lastseen == s {
                return false;
            }
            u.lastseen = s;
            true
        });
        if changed {
            self.dispatch_user(vec![UserEffect::MaybeStopWatchForOffline(k)])
        }
        changed
    }
    pub fn user_made_action(&self, k: UserKey, when: i32) {
        let s = self.read_user(k, |u| {
            (!u.is_bot() && !u.peer.is_service_user() && when > 0).then_some(u.lastseen())
        });
        if let Some(s) = s {
            let till = s.online_till();
            if till < when.wrapping_add(1)
                && self.update_user_lastseen(
                    k,
                    super::data_lastseen_status::LastseenStatus::online_till_value(
                        when.wrapping_add(30),
                        till == 0 || s.is_local_online_value(),
                        s.is_hidden_by_me(),
                    ),
                )
            {
                self.user_changed(k, F::OnlineStatus)
            }
        }
    }
    pub fn set_user_birthday(&self, k: UserKey, b: super::data_birthday::Birthday) {
        let changed = self.edit_user(k, |u| {
            if u.birthday == b {
                return false;
            }
            u.birthday = b;
            true
        });
        if changed {
            self.user_changed(k, F::Birthday);
            if self.read_user(k, UserData::is_self) {
                self.dispatch_user(vec![UserEffect::ReloadSensitiveContent]);
            }
        }
    }
    pub fn set_user_birthday_from_tl(&self, k: UserKey, b: Option<&tg_scheme::Birthday>) {
        let b = if let Some(tg_scheme::Birthday::Birthday(d)) = b {
            super::data_birthday::Birthday::new(d.day, d.month, d.vyear().unwrap_or_default())
        } else {
            Default::default()
        };
        self.set_user_birthday(k, b)
    }
    pub fn set_user_usernames(&self, k: UserKey, n: &Usernames) {
        let f = self.edit_user(k, |u| {
            let a = u.username().to_owned();
            let b = u.usernames().to_vec();
            u.usernames.set_usernames(n);
            let mut f = PeerFlags::empty();
            if a != u.username() {
                f |= F::Username
            }
            if b != u.usernames() {
                f |= F::Usernames
            }
            f
        });
        self.user_changed(k, f)
    }
    pub fn set_user_bot_info_version(&self, k: UserKey, v: i32) {
        if v < 0 {
            self.edit_user(k, |u| {
                if let Some(b) = &mut u.bot_info {
                    b.version = -1
                }
            });
            return;
        }
        let old = self.read_user(k, |u| u.bot_info.as_ref().map(|b| b.version));
        let Some(old) = old else {
            self.edit_user(k, |u| {
                u.bot_info = Some(BotInfo {
                    version: v,
                    ..Default::default()
                })
            });
            self.dispatch_user(vec![UserEffect::BotChanged(k)]);
            return;
        };
        if old >= v {
            return;
        }
        let commands_changed = self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().expect("bot remains a bot");
            let changed = !b.commands.is_empty();
            b.commands.clear();
            changed
        });
        // Original data_user.cpp:435 fires BEFORE clearing description/version.
        // Keep every callback phase outside the Peers borrow.
        if commands_changed {
            self.dispatch_user(vec![UserEffect::BotCommandsChanged(k)])
        }
        self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().expect("bot remains a bot");
            b.description.clear();
            b.version = v;
            b.inited = false
        });
    }
    pub fn set_user_stories_correspondent(&self, k: UserKey, is: bool) {
        let flags = self.read_user(k, |u| u.flags.clone());
        if is {
            flags.add(UserDataFlag::StoriesCorrespondent.into())
        } else {
            flags.remove(UserDataFlag::StoriesCorrespondent.into())
        }
    }
    pub fn set_user_stories_state(&self, k: UserKey, state: StoriesState) {
        assert_ne!(state, StoriesState::Unknown);
        let flags = self.read_user(k, |u| u.flags.clone());
        let was = flags.current();
        let active = UserDataFlags::from(UserDataFlag::HasActiveStories);
        let unread = UserDataFlags::from(UserDataFlag::HasUnreadStories);
        let video = UserDataFlags::from(UserDataFlag::HasActiveVideoStream);
        match state {
            StoriesState::None => flags.remove(active | unread | video),
            StoriesState::HasRead => flags.set(active | (was & !(unread | video))),
            StoriesState::HasUnread => flags.set((was & !video) | active | unread),
            StoriesState::HasVideoStream => flags.set((was & !unread) | active | video),
            StoriesState::Unknown => unreachable!(),
        }
        if flags.current() != was {
            self.dispatch_user(vec![UserEffect::UpdateChatListPostponed(k)]);
            self.user_changed(k, F::StoriesState)
        }
    }
    pub fn set_user_personal_channel(
        &self,
        k: UserKey,
        c: ChannelId,
        m: super::data_msg_id::MsgId,
    ) {
        let changed = self.edit_user(k, |u| {
            let b = u.personal_channel_id != c || u.personal_channel_message_id != m;
            u.personal_channel_id = c;
            u.personal_channel_message_id = m;
            b
        });
        if changed {
            self.user_changed(k, F::PersonalChannel)
        }
    }
    pub fn set_user_linked_community_id(&self, k: UserKey, id: ChannelId) {
        let b = self.edit_user(k, |u| {
            let b = u.linked_community_id != id;
            u.linked_community_id = id;
            b
        });
        if b {
            self.dispatch_user(vec![
                UserEffect::UpdateCommunityRegistration(k),
                UserEffect::UpdateChatListSortPosition(k),
                UserEffect::UpdateChatListExistence(k),
            ])
        }
    }
    pub fn set_user_business_details(
        &self,
        k: UserKey,
        mut d: super::business::data_business_common::BusinessDetails,
    ) {
        d.hours = d.hours.normalized();
        let b = self.edit_user(k, |u| {
            if (!d.is_present() && !u.business_details.is_present())
                || (d.is_present() && u.business_details == d)
            {
                return false;
            }
            u.business_details = if d.is_present() {
                d
            } else {
                Default::default()
            };
            true
        });
        if b {
            self.user_changed(k, F::BusinessDetails)
        }
    }
    /// AyuGram name filtering runs before empty/trim handling. Language ordering
    /// and search index are actual caller services (01/5), not fabricated rules.
    #[allow(clippy::too_many_arguments)]
    pub fn set_user_name(
        &self,
        k: UserKey,
        first: &str,
        last: &str,
        phone: &str,
        username: &str,
        filter_zalgo: bool,
        format: impl Fn(&str, &str) -> String,
        index: impl Fn() -> NameIndex,
    ) {
        let first = if filter_zalgo {
            crate::ayu::utils::telegram_helpers::filter_zalgo(first)
        } else {
            first.into()
        };
        let last = if filter_zalgo {
            crate::ayu::utils::telegram_helpers::filter_zalgo(last)
        } else {
            last.into()
        };
        let names = self.edit_user(k, |u| {
            if !first.is_empty() || !last.is_empty() {
                if first.trim().is_empty() {
                    u.first_name = last;
                    u.last_name.clear()
                } else {
                    u.first_name = first;
                    u.last_name = last
                }
            }
            (u.first_name.clone(), u.last_name.clone())
        });
        let full = format(&names.0, &names.1);
        self.update_name_delayed(
            k.peer(),
            &full,
            phone,
            username,
            NameFields::User {
                editable_username: &|| self.read_user(k, |u| u.editable_username().to_owned()),
                name_or_phone: &|| self.read_user(k, |u| u.name_or_phone.clone()),
                set_username: &|n| self.edit_user(k, |u| u.set_username(n)),
                set_name_or_phone: &|n| self.edit_user(k, |u| u.set_name_or_phone(n)),
            },
            index,
        );
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserForumRequired;

impl Peers {
    pub fn set_user_common_chats_count(&self, k: UserKey, v: i32) {
        let b = self.edit_user(k, |u| {
            if u.common_chats_count == v {
                return false;
            }
            u.common_chats_count = v;
            true
        });
        if b {
            self.user_changed(k, F::CommonChats)
        }
    }
    pub fn set_user_peer_gifts_count(&self, k: UserKey, v: i32) {
        let b = self.edit_user(k, |u| {
            if u.peer_gifts_count == v {
                return false;
            }
            u.peer_gifts_count = v;
            true
        });
        if b {
            self.user_changed(k, F::PeerGifts)
        }
    }
    pub fn set_user_stars_per_message(&self, k: UserKey, v: i32) {
        let b = self.edit_user(k, |u| {
            if u.stars_per_message == v {
                return false;
            }
            u.stars_per_message = v;
            true
        });
        if b {
            self.user_changed(k, F::StarsPerMessage)
        }
        self.dispatch_user(vec![UserEffect::CheckTrustedPayForMessage(k)]);
    }
    pub fn set_user_calls_status(&self, k: UserKey, v: CallsStatus) {
        let b = self.edit_user(k, |u| {
            if u.calls_status == v {
                return false;
            }
            u.calls_status = v;
            true
        });
        if b {
            self.user_changed(k, F::HasCalls)
        }
    }
    pub fn set_user_stars_rating(&self, k: UserKey, v: super::data_peer_common::StarsRating) {
        let b = self.edit_user(k, |u| {
            if u.stars_rating == v {
                return false;
            }
            u.stars_rating = v;
            true
        });
        if b {
            self.user_changed(k, F::StarsRating)
        }
    }
    pub fn set_user_bot_manager_id(&self, k: UserKey, v: UserId) {
        let b = self.edit_user(k, |u| {
            if u.bot_manager_id == v {
                return false;
            }
            u.bot_manager_id = v;
            true
        });
        if b {
            self.user_changed(k, F::ManagedBot)
        }
    }
    pub fn set_user_no_forwards_flags(
        &self,
        k: UserKey,
        my: bool,
        peer: bool,
    ) -> Result<(), UserForumRequired> {
        let mut f = self.read_user(k, UserData::flags)
            & !(UserDataFlag::NoForwardsMyEnabled | UserDataFlag::NoForwardsPeerEnabled);
        if my {
            f |= UserDataFlag::NoForwardsMyEnabled
        }
        if peer {
            f |= UserDataFlag::NoForwardsPeerEnabled
        }
        self.set_user_flags(k, f)?;
        if !my && !peer {
            self.dispatch_user(vec![UserEffect::ClearSharingDisabledTime(k)])
        }
        Ok(())
    }
}
impl Peers {
    pub fn set_user_bot_info(
        &self,
        k: UserKey,
        wire: &tg_scheme::BotInfo,
        photo: impl Fn(&tg_scheme::Photo) -> PhotoRecordKey,
        document: impl Fn(&tg_scheme::Document) -> DocumentRecordKey,
    ) {
        let tg_scheme::BotInfo::BotInfo(d) = wire;
        if !self.read_user(k, UserData::is_bot)
            || d.vuser_id()
                .is_some_and(|id| peer_from_user(UserId::new(id)) != k.peer().id())
        {
            return;
        }
        // Original processes external media before comparing their stable identities.
        let description =
            String::from_utf8_lossy(d.vdescription().map_or(&[], Vec::as_slice)).into_owned();
        let description_changed = self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().unwrap();
            if b.description == description {
                return false;
            }
            b.description = description;
            b.description_version = b.description_version.wrapping_add(1);
            true
        });
        let _ = description_changed;
        let p = d.vdescription_photo().map(photo);
        self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().unwrap();
            if b.photo != p {
                b.photo = p;
                b.description_version = b.description_version.wrapping_add(1)
            }
        });
        let v = d.vdescription_document().map(document);
        self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().unwrap();
            if b.document != v {
                b.document = v;
                b.description_version = b.description_version.wrapping_add(1)
            }
        });
        let changed = self.edit_user(k, |u| {
            let b = u.bot_info.as_mut().unwrap();
            let commands = d.vcommands().map_or_else(Vec::new, |v| {
                v.iter()
                    .map(super::data_peer_bot_command::bot_command_from_tl)
                    .collect()
            });
            let cc = b.commands != commands;
            b.commands = commands;
            let cb = apply_bot_menu_button(
                &mut b.menu_button_text,
                &mut b.menu_button_url,
                d.vmenu_button(),
            );
            b.inited = true;
            let privacy =
                String::from_utf8_lossy(d.vprivacy_policy_url().map_or(&[], Vec::as_slice))
                    .into_owned();
            let cp = b.privacy_policy_url != privacy;
            b.privacy_policy_url = privacy;
            b.app_colors =
                if let Some(tg_scheme::BotAppSettings::BotAppSettings(a)) = d.vapp_settings() {
                    [
                        a.vheader_color(),
                        a.vheader_dark_color(),
                        a.vbackground_color(),
                        a.vbackground_dark_color(),
                    ]
                    .map(|v| {
                        v.filter(|v| *v as u32 != u32::MAX)
                            .map_or(0, |v| ((v as u32) & 0xffffff) << 8 | 255)
                    })
                } else {
                    [0; 4]
                };
            let parsed = d.vverifier_settings().map(
                |tg_scheme::BotVerifierSettings::BotVerifierSettings(v)| BotVerifierSettings {
                    icon_id: v.icon,
                    company: String::from_utf8_lossy(&v.company).into_owned(),
                    custom_description: String::from_utf8_lossy(
                        v.vcustom_description().map_or(&[], Vec::as_slice),
                    )
                    .into_owned(),
                    can_modify_description: v.is_can_modify_custom_description(),
                },
            );
            let cv = b.verifier_settings != parsed;
            b.verifier_settings = parsed;
            cc || cb || cp || cv
        });
        if changed {
            self.dispatch_user(vec![UserEffect::BotCommandsChanged(k)])
        }
    }
    /// Actual profile-photo location; no QImage or network loader is created.
    #[allow(clippy::too_many_arguments)]
    pub fn set_user_photo(
        &self,
        k: UserKey,
        photo: &tg_scheme::UserProfilePhoto,
        input: &tg_scheme::InputPeer,
        premium: impl Fn() -> bool,
        now: impl Fn() -> i64,
    ) -> Result<(), UserForumRequired> {
        match photo {
            tg_scheme::UserProfilePhoto::UserProfilePhoto(d) => {
                if d.is_personal() {
                    self.add_user_flags(k, UserDataFlag::PersonalPhoto.into())?
                } else {
                    self.remove_user_flags(k, UserDataFlag::PersonalPhoto.into())?
                }
                let self_id = if self.read_user(k, UserData::is_self) {
                    peer_to_user(k.peer().id())
                } else {
                    UserId::new(0)
                };
                let file = super::data_image_location::StorageFileLocation::peer_photo(
                    d.dc_id, self_id, input, d.photo_id, false,
                );
                let location = super::data_image_location::ImageLocation::new(
                    super::data_image_location::DownloadLocation::Storage(file),
                    160,
                    160,
                );
                self.set_userpic_checked(
                    k.peer(),
                    d.photo_id,
                    location,
                    d.is_has_video(),
                    premium,
                    now,
                );
            }
            tg_scheme::UserProfilePhoto::UserProfilePhotoEmpty(_) => {
                self.remove_user_flags(k, UserDataFlag::PersonalPhoto.into())?;
                self.update(k.peer(), PeerData::clear_userpic);
            }
        }
        Ok(())
    }
}
/// Real Forum lifecycle contract, implemented by package08/18, with an owned
/// guard keeping the taken Forum alive across synchronous flag callbacks.
pub trait UserForumOwner {
    type Taken;
    fn take_forum_data(&self, user: UserKey) -> Option<Self::Taken>;
    fn ensure_forum(&self, user: UserKey);
    fn forum_flag_changed(&self, user: UserKey);
    fn clear_userpics_reset(&self, taken: &Self::Taken);
}
impl Peers {
    pub fn set_user_flags_with_forum(
        &self,
        k: UserKey,
        mut which: UserDataFlags,
        owner: &impl UserForumOwner,
    ) {
        let (flags, bot) = self.read_user(k, |u| (u.flags.clone(), u.is_bot()));
        if !bot {
            which &= !UserDataFlags::from(UserDataFlag::Forum)
        }
        let diff = flags.current() ^ which;
        if (diff & UserDataFlag::Deleted).any() {
            self.dispatch(vec![PeerEffect::InvalidateEmptyUserpic(k.peer())])
        }
        let taken =
            (bot && (diff & UserDataFlag::Forum).any() && (which & UserDataFlag::Forum).is_empty())
                .then(|| owner.take_forum_data(k))
                .flatten();
        if (diff & UserDataFlag::Forum).any() && (which & UserDataFlag::Forum).any() && bot {
            owner.ensure_forum(k)
        }
        flags.set(
            (flags.current() & UserDataFlag::SelfUser)
                | (which & !UserDataFlags::from(UserDataFlag::SelfUser)),
        );
        if (diff & UserDataFlag::Forum).any() {
            owner.forum_flag_changed(k)
        }
        if let Some(taken) = &taken {
            owner.clear_userpics_reset(taken)
        }
    }
    pub fn set_user_disallowed_gift_types(&self, k: UserKey, v: DisallowedGiftTypes) {
        let b = self.edit_user(k, |u| {
            let b = u.disallowed_gift_types != v;
            u.disallowed_gift_types = v;
            b
        });
        if b {
            self.user_changed(k, F::GiftSettings)
        }
    }
    /// Scalar flag phase of ApplyUserUpdate; complete orchestration still needs
    /// genuine photos/notify/History/business/wallpaper/credits/stories owners.
    pub fn apply_user_update_flags(
        &self,
        k: UserKey,
        d: &tg_scheme::UserFullData,
    ) -> Result<(), UserForumRequired> {
        self.set_user_stars_per_message(
            k,
            d.vsend_paid_messages_stars().unwrap_or_default() as i32,
        );
        use UserDataFlag as U;
        let stars = self.read_user(k, UserData::stars_per_message) != 0;
        let mut mask = U::Blocked
            | U::HasPhoneCalls
            | U::PhoneCallsPrivate
            | U::CanPinMessages
            | U::VoiceMessagesForbidden
            | U::ReadDatesPrivate
            | U::MessageMoneyRestrictionsKnown
            | U::RequiresPremiumToWrite
            | U::UnofficialSecurityRisk;
        if d.is_contact_require_premium() {
            mask |= U::HasRequirePremiumToWrite
        }
        if stars {
            mask |= U::HasStarsPerMessage
        }
        let mut f = self.read_user(k, UserData::flags) & !mask;
        f |= U::MessageMoneyRestrictionsKnown;
        for (b, flag) in [
            (d.is_phone_calls_private(), U::PhoneCallsPrivate),
            (d.is_phone_calls_available(), U::HasPhoneCalls),
            (d.is_can_pin_message(), U::CanPinMessages),
            (d.is_blocked(), U::Blocked),
            (d.is_voice_messages_forbidden(), U::VoiceMessagesForbidden),
            (d.is_read_dates_private(), U::ReadDatesPrivate),
            (stars, U::HasStarsPerMessage),
            (d.is_contact_require_premium(), U::RequiresPremiumToWrite),
            (d.is_contact_require_premium(), U::HasRequirePremiumToWrite),
            (d.is_unofficial_security_risk(), U::UnofficialSecurityRisk),
        ] {
            if b {
                f |= flag
            }
        }
        self.set_user_flags(k, f)
    }
    pub fn apply_user_gift_settings(&self, k: UserKey, d: &tg_scheme::UserFullData) {
        use DisallowedGiftType as G;
        let mut f = DisallowedGiftTypes::empty();
        if let Some(tg_scheme::DisallowedGiftsSettings::DisallowedGiftsSettings(v)) =
            d.vdisallowed_gifts()
        {
            for (b, g) in [
                (v.is_disallow_unlimited_stargifts(), G::Unlimited),
                (v.is_disallow_limited_stargifts(), G::Limited),
                (v.is_disallow_unique_stargifts(), G::Unique),
                (v.is_disallow_premium_gifts(), G::Premium),
                (v.is_disallow_stargifts_from_channels(), G::FromChannels),
            ] {
                if b {
                    f |= g
                }
            }
        }
        if d.is_display_gifts_button() {
            f |= G::SendHide
        }
        self.set_user_disallowed_gift_types(k, f)
    }
}
