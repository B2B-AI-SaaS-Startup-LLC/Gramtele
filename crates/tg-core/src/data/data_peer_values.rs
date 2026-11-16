//! Derived values and RPL streams. Calendar/locale rendering remains in lang01;
//! online phrase descriptors preserve the original decision order without text.
use super::{
    data_channel::ChannelDataFlag as G, data_chat::ChatDataFlag as C,
    data_chat_participant_status::*, data_flags::Change, data_lastseen_status::LastseenStatus,
    data_peer_common::StarsRating, data_user::*,
};
use tg_base::flags::{FlagType, Flags};
use tg_rpl::Producer;
pub fn flags_value_with_mask<E: FlagType + 'static>(
    value: Producer<Change<E>>,
    mask: Flags<E>,
) -> Producer<Flags<E>> {
    value
        .filter(move |v| (v.diff & mask).any())
        .map(move |v| v.value & mask)
}
pub fn single_flag_value<E: FlagType + 'static>(
    value: Producer<Change<E>>,
    flag: E,
) -> Producer<bool> {
    let mask = Flags::from(flag);
    value
        .filter(move |v| (v.diff & mask).any())
        .map(move |v| (v.value & mask).any())
}
pub fn sort_by_online_value(user: &UserData, now: i32) -> i32 {
    if user.peer.is_service_user() || user.is_bot() {
        return -1;
    }
    let s = user.lastseen();
    let till = s.online_till();
    if till != 0 {
        till
    } else if s.is_recently() {
        now.wrapping_sub(3 * 86400)
    } else if s.is_within_week() {
        now.wrapping_sub(7 * 86400)
    } else if s.is_within_month() {
        now.wrapping_sub(30 * 86400)
    } else {
        0
    }
}
pub fn online_phrase_change_in_seconds(
    s: LastseenStatus,
    now: i32,
    seconds_until_local_tomorrow: i32,
) -> i32 {
    let till = s.online_till();
    if till > now {
        return (i64::from(till) - i64::from(now)).min(i64::from(i32::MAX)) as i32;
    }
    if s.is_hidden() {
        return i32::MAX;
    }
    let elapsed = now.wrapping_sub(till);
    let minutes = elapsed / 60;
    if minutes < 60 {
        return (minutes + 1) * 60 - elapsed;
    }
    let hours = elapsed / 3600;
    if hours < 12 {
        return (hours + 1) * 3600 - elapsed;
    }
    seconds_until_local_tomorrow.max(0)
}
pub fn online_change_timeout(
    s: LastseenStatus,
    now: i32,
    seconds_until_local_tomorrow: i32,
) -> i64 {
    (i64::from(online_phrase_change_in_seconds(
        s,
        now,
        seconds_until_local_tomorrow,
    )) * 1000)
        .clamp(1000, 86400000)
}
pub fn user_online_change_timeout(user: &UserData, now: i32, tomorrow: i32) -> i64 {
    if user.peer.is_service_user() || user.is_bot() {
        86400000
    } else {
        online_change_timeout(user.lastseen(), now, tomorrow)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnlinePhrase {
    ServiceNotifications,
    Support,
    Bot,
    BotUsers(i32),
    Online,
    Offline,
    Recently,
    WithinWeek,
    WithinMonth,
    JustNow,
    Minutes(i32),
    Hours(i32),
    Today { till: i32, seconds: bool },
    Yesterday { till: i32, seconds: bool },
    Date { till: i32 },
    DateTime { till: i32, seconds: bool },
}
fn common(s: LastseenStatus, now: i32) -> Option<OnlinePhrase> {
    Some(if s.is_online(now) {
        OnlinePhrase::Online
    } else if s.is_long_ago() {
        OnlinePhrase::Offline
    } else if s.is_recently() {
        OnlinePhrase::Recently
    } else if s.is_within_week() {
        OnlinePhrase::WithinWeek
    } else if s.is_within_month() {
        OnlinePhrase::WithinMonth
    } else if s.is_hidden() {
        OnlinePhrase::Recently
    } else {
        return None;
    })
}
fn special(u: &UserData) -> Option<OnlinePhrase> {
    Some(if u.peer.is_notifications_user() {
        OnlinePhrase::ServiceNotifications
    } else if u.is_support() {
        OnlinePhrase::Support
    } else if let Some(b) = &u.bot_info {
        if b.active_users != 0 {
            OnlinePhrase::BotUsers(b.active_users)
        } else {
            OnlinePhrase::Bot
        }
    } else if u.peer.is_service_user() {
        OnlinePhrase::Support
    } else {
        return None;
    })
}
/// same_day/yesterday come from the platform's local calendar, not UTC division.
pub fn online_text(s: LastseenStatus, now: i32, same_day: bool, yesterday: bool) -> OnlinePhrase {
    if let Some(p) = common(s, now) {
        return p;
    }
    let till = s.online_till();
    let minutes = now.wrapping_sub(till) / 60;
    if minutes == 0 {
        return OnlinePhrase::JustNow;
    }
    if minutes < 60 {
        return OnlinePhrase::Minutes(minutes);
    }
    let hours = now.wrapping_sub(till) / 3600;
    if hours < 12 {
        return OnlinePhrase::Hours(hours);
    }
    date_phrase(till, same_day, yesterday, false, false)
}
fn date_phrase(till: i32, today: bool, yesterday: bool, full: bool, seconds: bool) -> OnlinePhrase {
    if today {
        OnlinePhrase::Today { till, seconds }
    } else if yesterday {
        OnlinePhrase::Yesterday { till, seconds }
    } else if full {
        OnlinePhrase::DateTime { till, seconds }
    } else {
        OnlinePhrase::Date { till }
    }
}
pub fn user_online_text(u: &UserData, now: i32, today: bool, yesterday: bool) -> OnlinePhrase {
    special(u).unwrap_or_else(|| online_text(u.lastseen(), now, today, yesterday))
}
pub fn online_text_full(
    u: &UserData,
    now: i32,
    today: bool,
    yesterday: bool,
    show_seconds: bool,
) -> OnlinePhrase {
    special(u)
        .or_else(|| common(u.lastseen(), now))
        .unwrap_or_else(|| {
            date_phrase(
                u.lastseen().online_till(),
                today,
                yesterday,
                true,
                show_seconds,
            )
        })
}
pub fn online_text_active(u: &UserData, now: i32) -> bool {
    !u.peer.is_service_user() && !u.is_bot() && u.lastseen().is_online(now)
}
pub fn is_user_online(u: &UserData, now: i32, clock: impl FnOnce() -> i32) -> bool {
    online_text_active(u, if now == 0 { clock() } else { now })
}
pub fn user_can_send_any_of(
    flags: UserDataFlags,
    rights: ChatRestrictions,
    premium: bool,
    replies_or_verify: bool,
) -> bool {
    !replies_or_verify
        && (flags & UserDataFlag::Deleted).is_empty()
        && ((flags & UserDataFlag::RequiresPremiumToWrite).is_empty() || premium)
        && ((rights & !(ChatRestriction::SendVoiceMessages | ChatRestriction::SendVideoMessages))
            .any()
            || (flags & UserDataFlag::VoiceMessagesForbidden).is_empty())
}
pub fn chat_can_send_any_of(
    flags: Flags<C>,
    admin: ChatAdminRights,
    defaults: ChatRestrictions,
    rights: ChatRestrictions,
) -> bool {
    (flags & (C::Deactivated | C::Forbidden | C::Left)).is_empty()
        && ((flags & C::Creator).any() || admin.any() || (rights & !defaults).any())
}
pub fn channel_can_send_any_of(
    flags: Flags<G>,
    post: bool,
    unrestricted_by_boosts: bool,
    send: ChatRestrictions,
    defaults: ChatRestrictions,
    rights: ChatRestrictions,
    forbid_in_forums: bool,
) -> bool {
    if (flags & G::MonoforumDisabled).any() {
        return false;
    }
    let allowed = (flags & (G::Left | G::Forbidden)).is_empty()
        || ((flags & G::HasLink).any() && (flags & G::JoinToWrite).is_empty())
        || (flags & G::Monoforum).any();
    // Original data_peer_values.cpp:330 uses logical &&, converting to raw bit 0,
    // NOT the default restriction mask. Native fixture pins that observable quirk.
    let restricted =
        send | ChatRestrictions::from_raw((defaults.any() && !unrestricted_by_boosts) as i32);
    allowed
        && !(forbid_in_forums && (flags & G::Forum).any())
        && (post
            || (flags & G::Creator).any()
            || ((flags & G::Broadcast).is_empty() && (rights & !restricted).any()))
}
pub fn user_can_send_any_of_value(
    flags: Producer<Change<UserDataFlag>>,
    rights: ChatRestrictions,
    premium: Producer<bool>,
    replies_or_verify: bool,
) -> Producer<bool> {
    if replies_or_verify {
        return tg_rpl::single(false);
    }
    let allowed = flags_value_with_mask(
        flags.clone(),
        UserDataFlag::Deleted | UserDataFlag::RequiresPremiumToWrite,
    )
    .map(move |f| {
        if (f & UserDataFlag::Deleted).any() {
            tg_rpl::single(false)
        } else if (f & UserDataFlag::RequiresPremiumToWrite).is_empty() {
            tg_rpl::single(true)
        } else {
            premium.clone()
        }
    })
    .flatten_latest();
    if (rights & !(ChatRestriction::SendVoiceMessages | ChatRestriction::SendVideoMessages)).any() {
        allowed
    } else {
        tg_rpl::combine((
            allowed,
            single_flag_value(flags, UserDataFlag::VoiceMessagesForbidden),
        ))
        .map(|(a, b)| a && !b)
    }
}
pub fn chat_can_send_any_of_value(
    flags: Producer<Change<C>>,
    admin: Producer<Change<ChatAdminRight>>,
    defaults: Producer<Change<ChatRestriction>>,
    rights: ChatRestrictions,
) -> Producer<bool> {
    tg_rpl::combine((
        flags_value_with_mask(flags, C::Deactivated | C::Forbidden | C::Left | C::Creator),
        admin,
        flags_value_with_mask(defaults, rights),
    ))
    .map(move |(f, a, d)| chat_can_send_any_of(f, a.value, d, rights))
}
pub fn channel_can_send_any_of_value(
    flags: Producer<Change<G>>,
    admin: Producer<Change<ChatAdminRight>>,
    boosts: Producer<bool>,
    send: Producer<Change<ChatRestriction>>,
    defaults: Producer<Change<ChatRestriction>>,
    rights: ChatRestrictions,
    forbid_in_forums: bool,
) -> Producer<bool> {
    let mask = G::Left
        | G::Forum
        | G::JoinToWrite
        | G::Monoforum
        | G::HasLink
        | G::Forbidden
        | G::Creator
        | G::Broadcast
        | G::MonoforumDisabled;
    tg_rpl::combine((
        flags_value_with_mask(flags, mask),
        single_flag_value(admin, ChatAdminRight::PostMessages),
        boosts,
        flags_value_with_mask(send, rights),
        flags_value_with_mask(defaults, rights),
    ))
    .map(move |(f, p, b, s, d)| channel_can_send_any_of(f, p, b, s, d, rights, forbid_in_forums))
}
pub fn allows_forwarding_user_value(flags: Producer<Change<UserDataFlag>>) -> Producer<bool> {
    tg_rpl::combine((
        single_flag_value(flags.clone(), UserDataFlag::NoForwardsMyEnabled),
        single_flag_value(flags, UserDataFlag::NoForwardsPeerEnabled),
    ))
    .map(|(my, peer)| !my && !peer)
}
pub fn peer_premium_value(
    flags: Producer<Change<UserDataFlag>>,
    current: impl Fn() -> bool + 'static,
) -> Producer<bool> {
    flags
        .filter(|c| (c.diff & UserDataFlag::Premium).any())
        .map(move |_| current())
}
pub fn am_premium_value(local_premium: bool, peer: Producer<bool>) -> Producer<bool> {
    if local_premium {
        tg_rpl::single(true)
    } else {
        peer
    }
}
pub fn stars_rating_value(
    changes: Producer<super::data_changes::PeerUpdate>,
    current: impl Fn() -> StarsRating + 'static,
    is_self: impl Fn() -> bool + 'static,
) -> Producer<StarsRating> {
    changes.map(move |_| {
        let mut r = current();
        if !is_self() && r.level < 0 {
            r.stars = 0
        }
        r
    })
}
pub fn unique_reactions_limit(model_max: i32, config_max: Option<i32>) -> i32 {
    if model_max != 0 {
        model_max
    } else {
        config_max.unwrap_or(11)
    }
}
pub fn unique_reactions_limit_value(
    model: Producer<super::data_peer::AllowedReactions>,
    config: Producer<i32>,
) -> Producer<i32> {
    tg_rpl::combine((model, config.distinct_until_changed()))
        .map(|(m, c)| unique_reactions_limit(m.max_count, Some(c)))
}
pub fn chat_can_pin_messages_value(
    flags: Producer<Change<C>>,
    admin: Producer<Change<ChatAdminRight>>,
    defaults: Producer<Change<ChatRestriction>>,
) -> Producer<bool> {
    tg_rpl::combine((
        flags_value_with_mask(flags, C::Deactivated | C::Forbidden | C::Left | C::Creator),
        single_flag_value(admin, ChatAdminRight::PinMessages),
        single_flag_value(defaults, ChatRestriction::PinMessages),
    ))
    .map(|(f, a, d)| {
        (f & (C::Deactivated | C::Forbidden | C::Left)).is_empty()
            && ((f & C::Creator).any() || a || !d)
    })
}
pub fn channel_can_pin_messages_value(
    is_creator: bool,
    is_megagroup: bool,
    flags: Producer<Change<G>>,
    admin: Producer<Change<ChatAdminRight>>,
    defaults: Producer<Change<ChatRestriction>>,
    restrictions: Producer<Change<ChatRestriction>>,
) -> Producer<bool> {
    if is_creator {
        return tg_rpl::single(true);
    }
    if !is_megagroup {
        return single_flag_value(admin, ChatAdminRight::EditMessages);
    }
    tg_rpl::combine((
        single_flag_value(admin, ChatAdminRight::PinMessages),
        single_flag_value(defaults, ChatRestriction::PinMessages),
        flags_value_with_mask(flags, G::Username | G::Location),
        restrictions,
    ))
    .map(|(a, d, f, r)| {
        a || (f.is_empty() && !d && (r.value & ChatRestriction::PinMessages).is_empty())
    })
}
pub fn can_manage_group_call_value(
    is_creator: bool,
    admin: Producer<Change<ChatAdminRight>>,
) -> Producer<bool> {
    if is_creator {
        tg_rpl::single(true)
    } else {
        single_flag_value(admin, ChatAdminRight::ManageCall)
    }
}
pub fn channel_has_active_call(channel: &super::data_channel::ChannelData) -> bool {
    (channel.flags() & G::CallNotEmpty).any()
}
pub fn channel_has_subscription_until_date(
    channel: Option<&super::data_channel::ChannelData>,
) -> bool {
    channel.is_some_and(|c| c.subscription_until_date() > 0)
}
use super::{
    data_changes::{Changes, PeerFlag},
    data_peer::{AllowedReactions, AllowedReactionsType},
    data_peer_id::*,
    data_session::Peers,
    data_store_keys::*,
};
use std::rc::Rc;
pub fn can_send_any_of_value(
    peers: &Peers,
    key: PeerKey,
    rights: ChatRestrictions,
    premium: Producer<bool>,
    production: bool,
    forbid_in_forums: bool,
) -> Producer<bool> {
    if peer_is_user(key.id()) {
        peers.read_user(UserKey::try_from(key).unwrap(), |u| {
            user_can_send_any_of_value(
                u.flags_value(),
                rights,
                premium,
                u.peer.is_replies_chat(production) || u.peer.is_verify_codes(),
            )
        })
    } else if peer_is_chat(key.id()) {
        peers.read_chat(ChatKey::try_from(key).unwrap(), |c| {
            chat_can_send_any_of_value(
                c.flags_value(),
                c.admin_rights_value(),
                c.default_restrictions_value(),
                rights,
            )
        })
    } else {
        peers.read_channel(ChannelKey::try_from(key).unwrap(), |c| {
            channel_can_send_any_of_value(
                c.flags_value(),
                c.admin_rights_value(),
                c.unrestricted_by_boosts_value(),
                c.restrictions_value(),
                c.default_restrictions_value(),
                rights,
                forbid_in_forums,
            )
        })
    }
}
pub fn can_pin_messages_value(peers: &Peers, key: PeerKey) -> Producer<bool> {
    if peer_is_user(key.id()) {
        peers.read_user(UserKey::try_from(key).unwrap(), |u| {
            single_flag_value(u.flags_value(), UserDataFlag::CanPinMessages)
        })
    } else if peer_is_chat(key.id()) {
        peers.read_chat(ChatKey::try_from(key).unwrap(), |c| {
            chat_can_pin_messages_value(
                c.flags_value(),
                c.admin_rights_value(),
                c.default_restrictions_value(),
            )
        })
    } else {
        peers.read_channel(ChannelKey::try_from(key).unwrap(), |c| {
            channel_can_pin_messages_value(
                c.am_creator(),
                c.is_megagroup(),
                c.flags_value(),
                c.admin_rights_value(),
                c.default_restrictions_value(),
                c.restrictions_value(),
            )
        })
    }
}
pub fn allows_forwarding_value(peers: &Peers, key: PeerKey) -> Producer<bool> {
    if peer_is_user(key.id()) {
        peers.read_user(UserKey::try_from(key).unwrap(), |u| {
            allows_forwarding_user_value(u.flags_value())
        })
    } else if peer_is_chat(key.id()) {
        peers.read_chat(ChatKey::try_from(key).unwrap(), |c| {
            single_flag_value(c.flags_value(), C::NoForwards).map(|f| !f)
        })
    } else {
        peers.read_channel(ChannelKey::try_from(key).unwrap(), |c| {
            single_flag_value(c.flags_value(), G::NoForwards).map(|f| !f)
        })
    }
}
pub fn peer_can_manage_group_call_value(peers: &Peers, key: PeerKey) -> Producer<bool> {
    if peer_is_user(key.id()) {
        tg_rpl::single(peers.read_user(UserKey::try_from(key).unwrap(), UserData::is_self))
    } else if peer_is_chat(key.id()) {
        peers.read_chat(ChatKey::try_from(key).unwrap(), |c| {
            can_manage_group_call_value(c.am_creator(), c.admin_rights_value())
        })
    } else {
        peers.read_channel(ChannelKey::try_from(key).unwrap(), |c| {
            can_manage_group_call_value(c.am_creator(), c.admin_rights_value())
        })
    }
}
pub fn peer_allowed_reactions(peers: &Peers, key: PeerKey) -> AllowedReactions {
    if peer_is_chat(key.id()) {
        peers.read_chat(ChatKey::try_from(key).unwrap(), |c| {
            c.allowed_reactions().clone()
        })
    } else if peer_is_channel(key.id()) {
        peers.read_channel(ChannelKey::try_from(key).unwrap(), |c| {
            c.allowed_reactions().clone()
        })
    } else {
        AllowedReactions {
            kind: AllowedReactionsType::All,
            ..Default::default()
        }
    }
}
pub fn peer_allowed_reactions_value(
    peers: Rc<Peers>,
    changes: &Changes,
    key: PeerKey,
) -> Producer<AllowedReactions> {
    changes
        .peer_flags_value(key, PeerFlag::Reactions)
        .map(move |_| peer_allowed_reactions(&peers, key))
}
pub fn peer_stars_rating_value(
    peers: Rc<Peers>,
    changes: &Changes,
    key: PeerKey,
) -> Producer<StarsRating> {
    if let Ok(user) = UserKey::try_from(key) {
        let p = peers.clone();
        stars_rating_value(
            changes.peer_flags_value(key, PeerFlag::StarsRating),
            move || peers.read_user(user, UserData::stars_rating),
            move || p.read_user(user, UserData::is_self),
        )
    } else {
        tg_rpl::single(StarsRating::default())
    }
}
#[allow(clippy::too_many_arguments)]
pub fn can_send_value(
    peers: &Peers,
    key: PeerKey,
    right: ChatRestriction,
    premium: Producer<bool>,
    production: bool,
    forbid_in_forums: bool,
) -> Producer<bool> {
    can_send_any_of_value(
        peers,
        key,
        right.into(),
        premium,
        production,
        forbid_in_forums,
    )
}
pub fn can_send_texts_value(
    peers: &Peers,
    key: PeerKey,
    premium: Producer<bool>,
    production: bool,
    forbid_in_forums: bool,
) -> Producer<bool> {
    can_send_value(
        peers,
        key,
        ChatRestriction::SendOther,
        premium,
        production,
        forbid_in_forums,
    )
}
pub fn can_send_anything_value(
    peers: &Peers,
    key: PeerKey,
    premium: Producer<bool>,
    production: bool,
    forbid_in_forums: bool,
) -> Producer<bool> {
    can_send_any_of_value(
        peers,
        key,
        all_send_restrictions(),
        premium,
        production,
        forbid_in_forums,
    )
}
/// Real ForumTopic::Closed/ManageTopics streams are supplied by packages08/16.
#[allow(clippy::too_many_arguments)]
pub fn topic_can_send_any_of_value(
    flags: Producer<Change<G>>,
    send: Producer<Change<ChatRestriction>>,
    defaults: Producer<Change<ChatRestriction>>,
    admin: Producer<Change<ChatAdminRight>>,
    closed_changes: Producer<super::data_changes::TopicUpdate>,
    rights: ChatRestrictions,
    closed: impl Fn() -> bool + 'static,
    can_toggle_closed: impl Fn() -> bool + 'static,
) -> Producer<bool> {
    let mask = G::Left | G::JoinToWrite | G::HasLink | G::Forbidden | G::Creator;
    tg_rpl::combine((
        flags_value_with_mask(flags, mask),
        flags_value_with_mask(send, rights),
        flags_value_with_mask(defaults, rights),
        flags_value_with_mask(admin, ChatAdminRight::ManageTopics.into()),
        closed_changes,
    ))
    .map(move |(f, s, d, _, _)| {
        let allowed = (f & (G::Left | G::Forbidden)).is_empty()
            || ((f & G::HasLink).any() && (f & G::JoinToWrite).is_empty());
        allowed
            && ((f & G::Creator).any() || (s.is_empty() && d.is_empty()))
            && (!closed() || can_toggle_closed())
    })
}
