//! UserData model; owner phases live in data_user_store. See data-user-map.md.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserDataFlag {
    Contact = 1 << 0,
    MutualContact = 1 << 1,
    Deleted = 1 << 2,
    Verified = 1 << 3,
    Scam = 1 << 4,
    Fake = 1 << 5,
    BotInlineGeo = 1 << 6,
    Blocked = 1 << 7,
    HasPhoneCalls = 1 << 8,
    PhoneCallsPrivate = 1 << 9,
    Support = 1 << 10,
    CanPinMessages = 1 << 11,
    DiscardMinPhoto = 1 << 12,
    SelfUser = 1 << 13,
    Premium = 1 << 14,
    UnofficialSecurityRisk = 1 << 15,
    VoiceMessagesForbidden = 1 << 16,
    PersonalPhoto = 1 << 17,
    StoriesHidden = 1 << 18,
    HasActiveStories = 1 << 19,
    HasUnreadStories = 1 << 20,
    RequiresPremiumToWrite = 1 << 21,
    HasRequirePremiumToWrite = 1 << 22,
    HasStarsPerMessage = 1 << 23,
    MessageMoneyRestrictionsKnown = 1 << 24,
    ReadDatesPrivate = 1 << 25,
    StoriesCorrespondent = 1 << 26,
    Forum = 1 << 27,
    HasActiveVideoStream = 1 << 28,
    NoForwardsMyEnabled = 1 << 29,
    NoForwardsPeerEnabled = 1 << 30,
}
tg_base::flag_type!(UserDataFlag: u32);
pub type UserDataFlags = tg_base::flags::Flags<UserDataFlag>;

use super::{
    business::data_business_common::BusinessDetails,
    data_birthday::Birthday,
    data_chat_participant_status::*,
    data_flags::Flags,
    data_lastseen_status::LastseenStatus,
    data_peer::*,
    data_peer_bot_command::BotCommand,
    data_peer_common::StarsRating,
    data_peer_fields::{PeerFields, UserPeerFields},
    data_peer_id::*,
    data_store_keys::*,
    data_user_names::UsernamesInfo,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContactStatus {
    #[default]
    Unknown,
    Contact,
    NotContact,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CallsStatus {
    #[default]
    Unknown,
    Enabled,
    Disabled,
    Private,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SetBotPhotoOpenState {
    #[default]
    Unknown,
    OpenedWithHistory,
    OpenedEmpty,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotVerifierSettings {
    pub icon_id: u64,
    pub company: String,
    pub custom_description: String,
    pub can_modify_description: bool,
}
impl BotVerifierSettings {
    pub fn is_present(&self) -> bool {
        self.icon_id != 0
    }
}
/// Real bot fields. Forum ownership (08/18), inline-return entry (18) and
/// StarRefProgram's CreditsAmount (Claude/01) are explicitly absent, not stand-ins.
#[derive(Default)]
pub struct BotInfo {
    pub description: String,
    pub inline_placeholder: String,
    pub commands: Vec<BotCommand>,
    pub photo: Option<PhotoRecordKey>,
    pub document: Option<DocumentRecordKey>,
    pub menu_button_text: String,
    pub menu_button_url: String,
    pub privacy_policy_url: String,
    /// RGBA values: transparent is zero, matching QColor(0,0,0,0).
    pub app_colors: [u32; 4],
    pub start_token: String,
    pub group_admin_rights: ChatAdminRights,
    pub channel_admin_rights: ChatAdminRights,
    pub verifier_settings: Option<BotVerifierSettings>,
    pub version: i32,
    pub description_version: i32,
    pub active_users: i32,
    pub set_bot_photo_open_state: SetBotPhotoOpenState,
    pub inited: bool,
    pub reads_all_history: bool,
    pub cant_join_groups: bool,
    pub supports_attach_menu: bool,
    pub can_edit_information: bool,
    pub can_manage_emoji_status: bool,
    pub supports_business: bool,
    pub has_main_app: bool,
    pub user_creates_topics: bool,
    pub set_bot_photo_hidden: bool,
    pub can_manage_bots: bool,
    pub supports_guest_chat: bool,
    pub supports_guard: bool,
}
/// All effects are delivered by Peers after releasing its store borrow.
#[derive(Clone, Debug)]
pub enum UserEffect {
    Peer(PeerEffect),
    MaybeStopWatchForOffline(UserKey),
    BotChanged(UserKey),
    BotCommandsChanged(UserKey),
    UpdateChatListPostponed(UserKey),
    UpdateCommunityRegistration(UserKey),
    UpdateChatListSortPosition(UserKey),
    UpdateChatListExistence(UserKey),
    ClearSharingDisabledTime(UserKey),
    ReloadSensitiveContent,
    CheckTrustedPayForMessage(UserKey),
}
pub struct UserData {
    pub(crate) disallowed_gift_types: DisallowedGiftTypes,
    pub peer: PeerData,
    pub(crate) flags: Flags<UserDataFlag>,
    pub first_name: String,
    pub last_name: String,
    pub name_or_phone: String,
    pub bot_info: Option<BotInfo>,
    pub(crate) usernames: UsernamesInfo,
    pub(crate) lastseen: LastseenStatus,
    pub(crate) birthday: Birthday,
    pub(crate) common_chats_count: i32,
    pub(crate) peer_gifts_count: i32,
    pub(crate) stars_per_message: i32,
    pub(crate) contact_status: ContactStatus,
    pub(crate) calls_status: CallsStatus,
    pub(crate) business_details: BusinessDetails,
    pub unavailable_reasons: Vec<UnavailableReason>,
    pub phone: String,
    pub private_forward_name: String,
    pub(crate) stars_rating: StarsRating,
    pub(crate) personal_channel_id: ChannelId,
    pub(crate) personal_channel_message_id: super::data_msg_id::MsgId,
    pub(crate) linked_community_id: ChannelId,
    pub(crate) bot_manager_id: UserId,
    pub(crate) access_hash: u64,
}
impl UserData {
    pub fn new(peer: PeerData, is_self: bool) -> Self {
        assert!(peer_is_user(peer.id()));
        Self {
            disallowed_gift_types: DisallowedGiftTypes::empty(),
            peer,
            flags: Flags::new(if is_self {
                UserDataFlag::SelfUser.into()
            } else {
                UserDataFlags::empty()
            }),
            first_name: String::new(),
            last_name: String::new(),
            name_or_phone: String::new(),
            bot_info: None,
            usernames: UsernamesInfo::default(),
            lastseen: LastseenStatus::default(),
            birthday: Birthday::default(),
            common_chats_count: 0,
            peer_gifts_count: 0,
            stars_per_message: 0,
            contact_status: ContactStatus::Unknown,
            calls_status: CallsStatus::Unknown,
            business_details: BusinessDetails::default(),
            unavailable_reasons: vec![],
            phone: String::new(),
            private_forward_name: String::new(),
            stars_rating: StarsRating::default(),
            personal_channel_id: ChannelId::new(0),
            personal_channel_message_id: super::data_msg_id::MsgId::new(0),
            linked_community_id: ChannelId::new(0),
            bot_manager_id: UserId::new(0),
            access_hash: 0,
        }
    }
    pub fn key(&self) -> UserKey {
        UserKey::try_from(self.peer.key()).unwrap()
    }
    pub fn flags(&self) -> UserDataFlags {
        self.flags.current()
    }
    pub fn flags_value(&self) -> tg_rpl::Producer<super::data_flags::Change<UserDataFlag>> {
        self.flags.value()
    }
    pub fn is_self(&self) -> bool {
        (self.flags() & UserDataFlag::SelfUser).any()
    }
    pub fn is_bot(&self) -> bool {
        self.bot_info.is_some()
    }
    pub fn is_premium(&self, local_premium: bool, session_for_id_exists: bool) -> bool {
        // AyuGram: getSession(id.value), not merely the Self flag.
        (self.peer.id().value != 0 && local_premium && session_for_id_exists)
            || (self.flags() & UserDataFlag::Premium).any()
    }
    pub fn apply_min_photo(&self) -> bool {
        (self.flags() & UserDataFlag::DiscardMinPhoto).is_empty()
    }
    pub fn requires_premium_to_write(&self) -> bool {
        !self.is_self() && (self.flags() & UserDataFlag::RequiresPremiumToWrite).any()
    }
    pub fn can_send_ignore_money_restrictions(&self, production: bool) -> bool {
        !self.is_inaccessible()
            && !self.peer.is_replies_chat(production)
            && !self.peer.is_verify_codes()
    }
    pub fn allows_forwarding(&self) -> bool {
        true
    } // AyuGram, data_user.cpp:690.
    pub fn is_ayu_no_forwards(&self) -> bool {
        (self.flags() & (UserDataFlag::NoForwardsMyEnabled | UserDataFlag::NoForwardsPeerEnabled))
            .any()
    }
    pub fn is_contact(&self) -> bool {
        self.contact_status == ContactStatus::Contact
    }
    pub fn can_share_this_contact_fast(&self) -> bool {
        !self.phone.is_empty()
    }
    pub fn can_share_this_contact(&self, contact_phone: &str) -> bool {
        self.can_share_this_contact_fast() || !contact_phone.is_empty()
    }
    pub fn can_add_contact(&self, contact_phone: &str) -> bool {
        self.can_share_this_contact(contact_phone) && !self.is_contact()
    }
    pub fn has_calls(&self) -> bool {
        matches!(
            self.calls_status,
            CallsStatus::Enabled | CallsStatus::Private
        )
    }
    pub fn has_private_forward_name(&self) -> bool {
        !self.private_forward_name.is_empty()
    }
    pub fn username(&self) -> &str {
        self.usernames.username()
    }
    pub fn editable_username(&self) -> &str {
        self.usernames.editable_username()
    }
    pub fn usernames(&self) -> &[String] {
        self.usernames.usernames()
    }
    pub fn is_username_editable(&self, name: &str) -> bool {
        self.usernames.is_editable(name)
    }
    pub fn set_username(&mut self, name: &str) {
        self.usernames.set_username(name)
    }
    pub fn set_phone(&mut self, phone: &str) {
        self.phone = phone.into()
    }
    pub fn set_name_or_phone(&mut self, name: &str) {
        self.name_or_phone = name.into()
    }
    pub fn set_private_forward_name(&mut self, name: &str) {
        self.private_forward_name = name.into()
    }
    pub fn fields(&self, effective_premium: bool, forum: Option<PeerKey>) -> PeerFields<'_> {
        PeerFields::User(UserPeerFields {
            key: self.key(),
            forum,
            flags: &self.flags,
            usernames: &self.usernames,
            first_name: &self.first_name,
            last_name: &self.last_name,
            name_or_phone: &self.name_or_phone,
            bot_info_present: self.is_bot(),
            user_creates_topics: self
                .bot_info
                .as_ref()
                .is_some_and(|b| b.user_creates_topics),
            effective_premium,
            stars_per_message: self.stars_per_message,
            stars_rating: self.stars_rating,
            peer_gifts_count: self.peer_gifts_count,
        })
    }
    pub fn input_user(&self, message: Option<(tg_scheme::InputPeer, i32)>) -> tg_scheme::InputUser {
        if !self.peer.is_loaded() {
            if let Some((peer, msg_id)) = message {
                return tg_scheme::mtp_input_user_from_message(
                    peer,
                    msg_id,
                    peer_to_user(self.peer.id()).bare,
                );
            }
        }
        if self.is_self() {
            tg_scheme::mtp_input_user_self()
        } else {
            tg_scheme::mtp_input_user(peer_to_user(self.peer.id()).bare, self.access_hash)
        }
    }
}
pub fn lastseen_from_mtp(
    status: &tg_scheme::UserStatus,
    current: LastseenStatus,
) -> LastseenStatus {
    use tg_scheme::UserStatus::*;
    match status {
        UserStatusEmpty(_) => LastseenStatus::long_ago(false),
        UserStatusRecently(d) => {
            if current.is_local_online_value() {
                LastseenStatus::online_till_value(current.online_till(), true, d.is_by_me())
            } else {
                LastseenStatus::recently(d.is_by_me())
            }
        }
        UserStatusLastWeek(d) => LastseenStatus::within_week(d.is_by_me()),
        UserStatusLastMonth(d) => LastseenStatus::within_month(d.is_by_me()),
        UserStatusOnline(d) => LastseenStatus::online_till_value(d.expires, false, false),
        UserStatusOffline(d) => LastseenStatus::online_till_value(d.was_online, false, false),
    }
}
pub fn parse_stars_rating(r: Option<&tg_scheme::StarsRating>) -> StarsRating {
    let Some(tg_scheme::StarsRating::StarsRating(d)) = r else {
        return StarsRating::default();
    };
    StarsRating {
        level: d.level,
        stars: d.stars as i32,
        this_level_stars: d.current_level_stars as i32,
        next_level_stars: d.vnext_level_stars().unwrap_or_default() as i32,
    }
}

impl UserData {
    pub fn is_verified(&self) -> bool {
        (self.flags() & UserDataFlag::Verified).any()
    }
    pub fn is_scam(&self) -> bool {
        (self.flags() & UserDataFlag::Scam).any()
    }
    pub fn is_fake(&self) -> bool {
        (self.flags() & UserDataFlag::Fake).any()
    }
    pub fn is_bot_inline_geo(&self) -> bool {
        (self.flags() & UserDataFlag::BotInlineGeo).any()
    }
    pub fn is_support(&self) -> bool {
        (self.flags() & UserDataFlag::Support).any()
    }
    pub fn is_inaccessible(&self) -> bool {
        (self.flags() & UserDataFlag::Deleted).any()
    }
    pub fn has_personal_photo(&self) -> bool {
        (self.flags() & UserDataFlag::PersonalPhoto).any()
    }
    pub fn has_stories_hidden(&self) -> bool {
        (self.flags() & UserDataFlag::StoriesHidden).any()
    }
    pub fn has_require_premium_to_write(&self) -> bool {
        (self.flags() & UserDataFlag::HasRequirePremiumToWrite).any()
    }
    pub fn has_stars_per_message(&self) -> bool {
        (self.flags() & UserDataFlag::HasStarsPerMessage).any()
    }
    pub fn message_money_restrictions_known(&self) -> bool {
        (self.flags() & UserDataFlag::MessageMoneyRestrictionsKnown).any()
    }
    pub fn read_dates_private(&self) -> bool {
        (self.flags() & UserDataFlag::ReadDatesPrivate).any()
    }
    pub fn is_forum(&self) -> bool {
        (self.flags() & UserDataFlag::Forum).any()
    }
    pub fn has_active_stories(&self) -> bool {
        (self.flags() & UserDataFlag::HasActiveStories).any()
    }
    pub fn has_unread_stories(&self) -> bool {
        (self.flags() & UserDataFlag::HasUnreadStories).any()
    }
    pub fn has_active_video_stream(&self) -> bool {
        (self.flags() & UserDataFlag::HasActiveVideoStream).any()
    }
    pub fn stories_correspondent(&self) -> bool {
        (self.flags() & UserDataFlag::StoriesCorrespondent).any()
    }
    pub fn unofficial_security_risk(&self) -> bool {
        (self.flags() & UserDataFlag::UnofficialSecurityRisk).any()
    }
    pub fn access_hash(&self) -> u64 {
        self.access_hash
    }
    pub fn lastseen(&self) -> LastseenStatus {
        self.lastseen
    }
    pub fn birthday(&self) -> Birthday {
        self.birthday
    }
    pub fn common_chats_count(&self) -> i32 {
        self.common_chats_count
    }
    pub fn peer_gifts_count(&self) -> i32 {
        self.peer_gifts_count
    }
    pub fn stars_per_message(&self) -> i32 {
        self.stars_per_message
    }
    pub fn contact_status(&self) -> ContactStatus {
        self.contact_status
    }
    pub fn calls_status(&self) -> CallsStatus {
        self.calls_status
    }
    pub fn stars_rating(&self) -> StarsRating {
        self.stars_rating
    }
    pub fn personal_channel_id(&self) -> ChannelId {
        self.personal_channel_id
    }
    pub fn personal_channel_message_id(&self) -> super::data_msg_id::MsgId {
        self.personal_channel_message_id
    }
    pub fn linked_community_id(&self) -> ChannelId {
        self.linked_community_id
    }
    pub fn bot_manager_id(&self) -> UserId {
        self.bot_manager_id
    }
    pub fn business_details(&self) -> &BusinessDetails {
        &self.business_details
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisallowedGiftType {
    Limited = 1,
    Unlimited = 2,
    Unique = 4,
    FromChannels = 8,
    Premium = 16,
    SendHide = 32,
}
tg_base::flag_type!(DisallowedGiftType:u8);
pub type DisallowedGiftTypes = tg_base::flags::Flags<DisallowedGiftType>;
impl UserData {
    pub fn disallowed_gift_types(&self) -> DisallowedGiftTypes {
        self.disallowed_gift_types
    }
}
impl UserData {
    pub fn phone(&self) -> &str {
        &self.phone
    }
    pub fn private_forward_name(&self) -> &str {
        &self.private_forward_name
    }
    pub fn unavailable_reasons(&self) -> &[UnavailableReason] {
        &self.unavailable_reasons
    }
    pub fn set_unavailable_reasons_list(&mut self, reasons: Vec<UnavailableReason>) {
        self.unavailable_reasons = reasons
    }
}
