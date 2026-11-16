use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    sync::Arc,
};
use tg_base::{
    flags::Flags,
    timer::{ManualClock, TimerHost},
};
use tg_core::data::{
    data_changes::{Changes, PeerFlag as P},
    data_chat_filters::*,
    data_chat_filters_service as service,
    data_flags::Flags as ReactiveFlags,
    data_folder::*,
    data_groups::*,
    data_lastseen_status::*,
    data_msg_id::*,
    data_peer::*,
    data_peer_id::*,
    data_peer_values as values,
    data_session::*,
    data_store_keys::*,
    data_unread_value as unread,
    data_user::*,
};
use tg_rpl::{Lifetime, Producer};
use tg_scheme as tl;
use tg_tl::Deserialize;
fn collect<T: Clone + 'static>(p: Producer<T>, l: &Lifetime) -> Rc<RefCell<Vec<T>>> {
    let v = Rc::new(RefCell::new(vec![]));
    let c = v.clone();
    p.start(move |x| c.borrow_mut().push(x), |_| {}, || {}, l);
    v
}
fn setup() -> (SessionScope, Rc<Peers>, Changes) {
    let scope = SessionScope::new();
    let c = Changes::with_queue(&scope, tg_runtime::MainQueue::new());
    let p = Rc::new(Peers::new_with_self(&scope, c.clone(), UserId::new(1)));
    (scope, p, c)
}
fn hex(words: &[i32]) -> String {
    words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn wire(s: &str) -> Vec<i32> {
    (0..s.len())
        .step_by(8)
        .map(|i| {
            i32::from_le_bytes(std::array::from_fn(|j| {
                u8::from_str_radix(&s[i + j * 2..i + j * 2 + 2], 16).unwrap()
            }))
        })
        .collect()
}
#[test]
fn native_user_values_rights_and_wire_reference() {
    let (_s, p, _c) = setup();
    let k = p.ensure_user(UserId::new(42));
    for line in include_str!("fixtures/data-user/native.jsonl").lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let n = |s: &str| v[s].as_i64().unwrap();
        let b = |s: &str| v[s].as_bool().unwrap();
        match v["op"].as_str().unwrap() {
            "user_flags" => {
                p.set_user_flags(k, UserDataFlags::from_raw(n("bits") as u32))
                    .unwrap();
                let r = p.read_user(k, |u| {
                    vec![
                        u.is_verified(),
                        u.is_scam(),
                        u.is_fake(),
                        u.is_bot_inline_geo(),
                        u.is_support(),
                        u.is_inaccessible(),
                        u.apply_min_photo(),
                        u.has_personal_photo(),
                        u.has_stories_hidden(),
                        u.has_require_premium_to_write(),
                        u.has_stars_per_message(),
                        u.message_money_restrictions_known(),
                        u.read_dates_private(),
                        u.allows_forwarding(),
                        u.is_ayu_no_forwards(),
                        u.has_active_stories(),
                        u.has_unread_stories(),
                        u.has_active_video_stream(),
                        u.stories_correspondent(),
                    ]
                });
                assert_eq!(serde_json::json!(r), v["result"], "{line}")
            }
            "channel_send" => {
                use tg_core::data::data_chat_participant_status::ChatRestrictions as R;
                assert_eq!(
                    values::channel_can_send_any_of(
                        Flags::from_raw(n("flags") as u64),
                        false,
                        b("boosts"),
                        R::empty(),
                        R::from_raw(n("rights") as i32),
                        R::from_raw(n("rights") as i32),
                        b("forbid")
                    ),
                    b("result"),
                    "{line}"
                )
            }
            "chat_send" => {
                use tg_core::data::data_chat_participant_status::*;
                assert_eq!(
                    values::chat_can_send_any_of(
                        Flags::from_raw(n("flags") as i32),
                        ChatAdminRights::from_raw(n("admin") as i32),
                        ChatRestrictions::from_raw(1 << 25),
                        ChatRestrictions::from_raw(1 << 25)
                    ),
                    b("result")
                )
            }
            "timeout" => {
                let s = LastseenStatus::from_serialized(n("raw") as u32);
                assert_eq!(
                    values::online_phrase_change_in_seconds(
                        s,
                        n("now") as i32,
                        n("tomorrow") as i32
                    ),
                    n("seconds") as i32
                );
                assert_eq!(
                    values::online_change_timeout(s, n("now") as i32, n("tomorrow") as i32),
                    n("ms")
                )
            }
            "lastseen" => {
                let w = wire(v["wire"].as_str().unwrap());
                let s = tl::UserStatus::read(&mut tg_tl::Reader::new(&w)).unwrap();
                assert_eq!(
                    lastseen_from_mtp(&s, LastseenStatus::from_serialized(n("current") as u32))
                        .serialize(),
                    n("result") as u32
                )
            }
            "filter_tl" => {
                let w = wire(v["wire"].as_str().unwrap());
                let f = tl::DialogFilter::read(&mut tg_tl::Reader::new(&w)).unwrap();
                let scope = SessionScope::new();
                let resolve = |i: &tl::InputPeer, _| {
                    if let tl::InputPeer::InputPeerChat(d) = i {
                        Some(scope.peer_key(peer_from_chat(d.chat_id)))
                    } else {
                        None
                    }
                };
                let f = ChatFilter::from_tl(scope.id(), &f, resolve);
                let out = f.tl(0, |k| tl::mtp_input_peer_chat(peer_to_chat(k.id()).bare));
                assert_eq!(
                    hex(&tg_tl::serialize(&out)),
                    v["wire"].as_str().unwrap(),
                    "{line}"
                );
            }
            "unread" => {
                let a = unread::UnreadState {
                    messages: 10,
                    chats: 3,
                    known: true,
                    ..Default::default()
                };
                let c = a - unread::UnreadState {
                    messages: 4,
                    chats: 2,
                    ..Default::default()
                };
                assert_eq!(c.messages, n("messages") as i32);
                assert_eq!(c.chats, n("chats") as i32);
                assert_eq!(c.known, b("known"))
            }
            _ => panic!("unknown native row"),
        }
    }
}
#[test]
fn user_store_scoped_identity_self_and_reentrant_flags() {
    let (s, p, _) = setup();
    let k = p.ensure_user(UserId::new(1));
    assert_eq!(p.ensure_peer(k.peer().id()), k.peer());
    assert!(p.read_user(k, UserData::is_self));
    let life = Lifetime::new();
    let pc = p.clone();
    let seen = collect(p.read_user(k, UserData::flags_value), &life);
    p.read_user(k, UserData::flags_value).start(
        move |_| {
            pc.read_user(k, UserData::is_self);
        },
        |_| {},
        || {},
        &life,
    );
    p.set_user_flags(k, UserDataFlag::Verified.into()).unwrap();
    assert!(p.read_user(k, UserData::is_self));
    assert_eq!(seen.borrow().len(), 2);
    assert!(!p.read_user(k, UserData::is_forum));
    assert_eq!(k.scope(), s.id());
}
#[test]
fn user_contact_is_independent_and_changes_are_deduplicated() {
    let (_s, p, c) = setup();
    let k = p.ensure_user(UserId::new(2));
    let l = Lifetime::new();
    let v = collect(c.peer_updates_for(k.peer(), P::IsContact), &l);
    p.add_user_flags(k, UserDataFlag::Contact.into()).unwrap();
    assert!(!p.read_user(k, UserData::is_contact));
    p.set_user_is_contact(k, true);
    p.set_user_is_contact(k, true);
    c.send_notifications();
    assert_eq!(v.borrow().len(), 1);
    assert!(!p.read_user(k, |u| u.can_add_contact("phone")));
    p.set_user_is_contact(k, false);
    p.edit_user(k, |u| u.set_phone("123"));
    assert!(p.read_user(k, |u| u.can_share_this_contact_fast()
        && u.can_add_contact("")));
}
#[test]
fn user_access_hash_deleted_and_sticky_self() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(1));
    p.set_user_access_hash(k, 55);
    assert_eq!(p.read_user(k, UserData::access_hash), 55);
    p.set_user_access_hash(k, u64::MAX);
    assert!(p.read_user(k, UserData::is_inaccessible));
    assert_eq!(p.read_user(k, UserData::access_hash), 0);
    assert!(p.read_user(k, UserData::is_self));
}
#[test]
fn bot_info_versions_commands_menu_and_description() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_bot_info_version(k, 1);
    p.edit_user(k, |u| {
        let b = u.bot_info.as_mut().unwrap();
        b.description = "old".into();
        b.commands.push(Default::default());
        b.inited = true
    });
    p.set_user_bot_info_version(k, 2);
    p.read_user(k, |u| {
        let b = u.bot_info.as_ref().unwrap();
        assert!(b.description.is_empty() && b.commands.is_empty() && !b.inited);
    });
    let v = tl::BotInfoData {
        flags: tl::BotInfoData::F_DESCRIPTION
            | tl::BotInfoData::F_COMMANDS
            | tl::BotInfoData::F_MENU_BUTTON,
        description: Some(b"hello".to_vec()),
        commands: Some(vec![tl::mtp_bot_command(
            Default::default(),
            b"go".to_vec(),
            b"Go".to_vec(),
        )]),
        menu_button: Some(tl::mtp_bot_menu_button(b"Open".to_vec(), b"url".to_vec())),
        ..Default::default()
    };
    let wire: tl::BotInfo = v.into();
    p.set_user_bot_info(k, &wire, |_| panic!("no photo"), |_| panic!("no document"));
    p.set_user_bot_info(k, &wire, |_| panic!("no photo"), |_| panic!("no document"));
    p.read_user(k, |u| {
        let b = u.bot_info.as_ref().unwrap();
        assert_eq!(b.description_version, 1);
        assert_eq!(b.commands[0].command, "go");
        assert_eq!(b.menu_button_url, "url");
        assert!(b.inited)
    });
    p.set_user_bot_info_version(k, -1);
    assert!(p.read_user(k, UserData::is_bot));
    assert_eq!(p.read_user(k, |u| u.bot_info.as_ref().unwrap().version), -1);
}
#[test]
fn user_action_hidden_online_and_service_exclusions() {
    let (_s, p, c) = setup();
    let k = p.ensure_user(UserId::new(2));
    let l = Lifetime::new();
    let log = collect(p.user_effects(), &l);
    p.user_made_action(k, 1700000000);
    assert_eq!(p.read_user(k, UserData::lastseen).online_till(), 1700000030);
    p.user_made_action(k, 1700000001);
    assert_eq!(log.borrow().len(), 1);
    c.send_notifications();
    let service = p.ensure_user(UserId::new(1000));
    p.user_made_action(service, 1700000000);
    assert_eq!(
        p.read_user(service, UserData::lastseen),
        LastseenStatus::default()
    );
}
#[test]
fn user_birthday_counts_rating_calls_and_business() {
    let (_s, p, c) = setup();
    let k = p.ensure_user(UserId::new(1));
    let l = Lifetime::new();
    let e = collect(p.user_effects(), &l);
    p.set_user_birthday(k, tg_core::data::data_birthday::Birthday::new(29, 2, 2000));
    assert!(e
        .borrow()
        .iter()
        .any(|e| matches!(e, UserEffect::ReloadSensitiveContent)));
    p.set_user_common_chats_count(k, 8);
    p.set_user_peer_gifts_count(k, 9);
    p.set_user_stars_per_message(k, 5);
    p.set_user_calls_status(k, CallsStatus::Private);
    assert!(p.read_user(k, UserData::has_calls));
    assert_eq!(p.read_user(k, UserData::common_chats_count), 8);
    assert_eq!(p.read_user(k, UserData::peer_gifts_count), 9);
    p.set_user_bot_manager_id(k, UserId::new(8));
    p.set_user_personal_channel(k, ChannelId::new(4), MsgId::new(5));
    p.set_user_linked_community_id(k, ChannelId::new(7));
    assert_eq!(
        p.read_user(k, UserData::linked_community_id),
        ChannelId::new(7)
    );
    p.set_user_business_details(k, Default::default());
    c.send_notifications();
}
#[test]
fn user_names_projection_and_ayu_filter() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_name(
        k,
        "  ",
        "Last",
        "Phone",
        "login",
        false,
        |a, b| format!("{a}/{b}"),
        NameIndex::default,
    );
    assert_eq!(p.read_user(k, |u| u.first_name.clone()), "Last");
    assert_eq!(p.read_user(k, |u| u.username().to_owned()), "login");
    p.set_user_name(
        k,
        "",
        "",
        "",
        "",
        true,
        |a, b| format!("{a}/{b}"),
        NameIndex::default,
    );
    assert_eq!(p.read_user(k, |u| u.first_name.clone()), "Last");
    assert!(p.read_user(k, |u| matches!(
        u.fields(false, None),
        tg_core::data::data_peer_fields::PeerFields::User(_)
    )));
}
#[test]
fn usernames_and_premium_forwarding_contracts() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_usernames(
        k,
        &vec![tg_core::data::data_user_names::Username {
            username: "name".into(),
            active: true,
            editable: true,
        }],
    );
    assert!(p.read_user(k, |u| u.is_username_editable("name")));
    assert!(p.read_user(k, |u| u.is_premium(true, true)));
    assert!(!p.read_user(k, |u| u.is_premium(true, false)));
    p.set_user_no_forwards_flags(k, true, false).unwrap();
    assert!(p.read_user(k, |u| u.allows_forwarding() && u.is_ayu_no_forwards()));
    p.set_user_no_forwards_flags(k, false, false).unwrap();
}
#[test]
fn user_input_and_profile_photo_real_keys() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_access_hash(k, 42);
    let input = p.read_user(k, |u| u.input_user(Some((tl::mtp_input_peer_chat(7), 8))));
    assert!(matches!(input, tl::InputUser::InputUserFromMessage(_)));
    p.update(k.peer(), |p| {
        p.set_loaded_status(LoadedStatus::Normal);
        PeerMutation::quiet(())
    });
    assert!(matches!(
        p.read_user(k, |u| u.input_user(None)),
        tl::InputUser::InputUser(_)
    ));
    p.set_user_photo(
        k,
        &tl::mtp_user_profile_photo_empty(),
        &tl::mtp_input_peer_user(2, 42),
        || false,
        || 100,
    )
    .unwrap();
    assert!(!p.read_user(k, UserData::has_personal_photo));
}
#[test]
fn online_descriptors_priority_and_boundaries() {
    use values::OnlinePhrase as O;
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(333000));
    assert_eq!(
        p.read_user(k, |u| values::user_online_text(u, 1700000000, false, false)),
        O::ServiceNotifications
    );
    let k = p.ensure_user(UserId::new(2));
    for (t, expected) in [
        (0, O::JustNow),
        (59, O::JustNow),
        (60, O::Minutes(1)),
        (3600, O::Hours(1)),
        (
            43200,
            O::Today {
                till: 1699956800,
                seconds: false,
            },
        ),
    ] {
        let s = LastseenStatus::online_till_value(1700000000 - t, false, false);
        assert_eq!(values::online_text(s, 1700000000, true, false), expected)
    }
    p.update_user_lastseen(k, LastseenStatus::recently(false));
    assert_eq!(
        p.read_user(k, |u| values::sort_by_online_value(u, 1700000000)),
        1699740800
    );
    p.update_user_lastseen(
        k,
        LastseenStatus::online_till_value(1700000001, false, false),
    );
    assert!(p.read_user(k, |u| values::is_user_online(u, 0, || 1700000000)));
    assert_eq!(
        p.read_user(k, |u| values::online_text_full(
            u, 1700000002, false, true, true
        )),
        O::Yesterday {
            till: 1700000001,
            seconds: true
        }
    );
}
#[test]
fn reactive_user_send_switches_premium_and_voice() {
    use tg_core::data::data_chat_participant_status::ChatRestriction as R;
    let f = ReactiveFlags::<UserDataFlag>::default();
    let premium = tg_rpl::Variable::new(false);
    let l = Lifetime::new();
    let v = collect(
        values::user_can_send_any_of_value(
            f.value(),
            R::SendVoiceMessages.into(),
            premium.value(),
            false,
        ),
        &l,
    );
    assert_eq!(*v.borrow(), [true]);
    f.add(UserDataFlag::RequiresPremiumToWrite.into());
    assert_eq!(v.borrow().last(), Some(&false));
    premium.set(true);
    assert_eq!(v.borrow().last(), Some(&true));
    f.add(UserDataFlag::VoiceMessagesForbidden.into());
    assert_eq!(v.borrow().last(), Some(&false));
    f.add(UserDataFlag::Deleted.into());
    premium.set(false);
    assert_eq!(v.borrow().last(), Some(&false));
}
fn facts() -> FilterHistoryFacts {
    FilterHistoryFacts {
        category: ChatFilterFlag::Groups,
        community: false,
        muted: false,
        unread: false,
        mention: false,
        folder_known: true,
        archived: false,
        fake_unread_while_opened: false,
    }
}
#[test]
fn filter_rules_precedence_community_muted_mentions_and_fake() {
    let s = SessionScope::new();
    let k = s.peer_key(peer_from_chat(1u64));
    let mut f = ChatFilter {
        flags: ChatFilterFlag::Groups
            | ChatFilterFlag::NoMuted
            | ChatFilterFlag::NoRead
            | ChatFilterFlag::NoArchived,
        ..Default::default()
    };
    let mut h = facts();
    assert!(!f.contains(k, h, false));
    h.mention = true;
    h.muted = true;
    assert!(f.contains(k, h, false));
    h.archived = true;
    assert!(!f.contains(k, h, false));
    f.always.insert(k);
    assert!(f.contains(k, h, false));
    f.never.insert(k);
    assert!(!f.contains(k, h, false));
    f.never.clear();
    h.community = true;
    assert!(f.contains(k, h, false));
    f.always.clear();
    assert!(!f.contains(k, h, false));
    h = facts();
    h.fake_unread_while_opened = true;
    assert!(f.contains(k, h, false));
    assert!(!f.contains(k, h, true));
}
#[test]
fn filter_with_chatlist_clears_static_and_equality_ignores_pins_id() {
    let mut a = ChatFilter {
        flags: ChatFilterFlag::Groups | ChatFilterFlag::StaticTitle,
        ..Default::default()
    };
    let mut b = a.with_id(99);
    let s = SessionScope::new();
    b.pinned.push(s.peer_key(peer_from_chat(1u64)));
    assert!(a == b);
    a = a.with_chatlist(true, true);
    assert!(a.chatlist() && a.has_my_links() && !a.static_title());
    let k = s.peer_key(peer_from_chat(1u64));
    a.always.insert(k);
    assert!(can_remove_from_chat_filter(&a, k, facts()));
    assert!(a.without_always(k, facts()).always.is_empty());
    assert_eq!(a.with_color_index(Some(7)).color_index, Some(7));
}
#[test]
fn filter_order_is_transactional_and_color_has_no_list_changed() {
    let mut m = ChatFilters::new();
    m.set(ChatFilter::default().with_id(2));
    m.set(ChatFilter::default().with_id(3));
    assert!(m.apply_order(&[3, 3, 0]).is_none());
    assert_eq!(m.list().iter().map(|f| f.id).collect::<Vec<_>>(), [0, 2, 3]);
    assert_eq!(m.apply_order(&[3, 0, 2]).unwrap().len(), 1);
    let e = m.set(ChatFilter::default().with_id(2).with_color_index(Some(1)));
    assert!(matches!(&e[..], [FilterEffect::TagColorChanged(_)]));
    assert_eq!(m.lookup_id(0, false, false), 0);
    assert_eq!(m.lookup_id(1, false, false), 3);
    assert_eq!(m.lookup_id(-1, true, true), 0);
}
#[test]
fn filter_received_hide_all_pinned_limits_and_suggested_backoff() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_chat(1u64));
    let b = s.peer_key(peer_from_chat(2u64));
    let mut m = ChatFilters::new();
    m.received(
        vec![ChatFilter::default(), ChatFilter::default().with_id(2)],
        true,
    );
    assert_eq!(m.list().len(), 1);
    assert!(m.loaded());
    m.apply_updated_pinned(2, &[a, b], 1);
    assert_eq!(m.list()[0].pinned, [a]);
    m.hide_all_changed(false);
    assert_eq!(m.list()[0].id, 0);
    assert!(m.has() && m.archive_needed());
    m.suggested_failed(1000);
    assert!(m.suggested_loaded());
    assert!(!m.suggested_due(10_800_999, false));
    assert!(m.suggested_due(10_801_000, false));
    m.remove(2);
    m.move_all_to_front();
    m.clear();
    assert!(m.list().is_empty());
}
fn keys(s: &SessionScope, n: i32) -> Vec<MessageKey> {
    let mut store = MessageKeys::new(s);
    (1..=n)
        .map(|i| {
            store.insert(FullMsgId::new(
                peer_from_chat(1u64),
                MsgId::new(i64::from(i)),
            ))
        })
        .collect()
}
#[test]
fn albums_registration_cap_refresh_uncapped_and_effect_order() {
    let s = SessionScope::new();
    let k = keys(&s, 11);
    let id = |k: MessageKey| k.id().msg.bare;
    let mut g = Groups::new(s.id());
    assert!(g.register_message(k[1], 7, true, id).is_empty());
    assert!(g.is_group_of_one(7));
    assert_eq!(g.register_message(k[0], 7, true, id).len(), 4);
    for key in &k[2..] {
        g.register_message(*key, 7, true, id);
    }
    assert_eq!(g.find(7).unwrap().items.len(), 10);
    let e = g.refresh_message(k[10], 7, true, true, false, id);
    assert_eq!(g.find(7).unwrap().items.len(), 11);
    assert_eq!(
        &e[..2],
        &[
            GroupEffect::RequestItemViewRefresh(k[0]),
            GroupEffect::InvalidateChatListEntry(k[0])
        ]
    );
    assert_eq!(g.find_item_to_edit(k[3], 7, |x| x != k[5]), k[5]);
    assert_eq!(g.find_item_to_edit(k[3], 7, |_| true), k[0]);
}
#[test]
fn albums_reorder_unregister_and_ineligible() {
    let s = SessionScope::new();
    let k = keys(&s, 3);
    let mut g = Groups::new(s.id());
    let ids = Rc::new(RefCell::new(vec![1, 2, 3]));
    let id = |x: MessageKey| ids.borrow()[x.id().msg.bare as usize - 1];
    for x in &k {
        g.register_message(*x, 7, true, id);
    }
    ids.borrow_mut()[0] = 4;
    g.refresh_message(k[0], 7, true, true, false, id);
    assert_eq!(g.find(7).unwrap().items, [k[1], k[2], k[0]]);
    assert_eq!(g.refresh_message(k[0], 7, false, true, false, id).len(), 5);
    g.unregister_message(k[1], 7);
    assert!(g.find(7).is_none());
    assert!(g.is_group_of_one(7));
    g.unregister_message(k[2], 7);
    assert!(!g.is_group_of_one(7));
    assert!(g
        .refresh_message(k[0], 7, true, false, false, id)
        .is_empty());
}
#[test]
fn archive_orders_dates_ignores_pin_for_sort_and_caps_nonpinned_scan() {
    let s = SessionScope::new();
    let keys = (1..=12)
        .map(|id| s.peer_key(peer_from_chat(id as u64)))
        .collect::<Vec<_>>();
    let mut f = Folder::new(s.id(), 1);
    f.reorder_last_histories(
        keys.iter().copied(),
        |k| peer_to_chat(k.id()).bare as i32,
        |k| peer_to_chat(k.id()).bare < 3,
    );
    assert_eq!(f.last_histories().len(), 8);
    assert_eq!(peer_to_chat(f.last_histories()[0].id()).bare, 10);
    assert!(f.chat_list_message_known());
    assert_eq!(f.chat_list_name_version(), 1);
    assert_eq!(
        Folder::register_one_effects(1, false),
        [
            FolderEffect::UpdateChatListSortPosition,
            FolderEffect::RequestDialogEntry
        ]
    );
    assert_eq!(
        Folder::unregister_one_effects(true),
        [FolderEffect::UpdateChatListExistence]
    );
}
#[test]
fn archive_stories_clamp_and_negative_ushort_wrap() {
    let s = SessionScope::new();
    let mut f = Folder::new(s.id(), 1);
    assert_eq!(
        f.update_stories_count(1, 1),
        [FolderEffect::UpdateChatListSortPosition]
    );
    assert!(f.should_be_in_chat_list(true));
    assert!(f.update_stories_count(1, 1).is_empty());
    assert_eq!(
        f.update_stories_count(0, 0),
        [FolderEffect::UpdateChatListExistence]
    );
    f.update_stories_count(-1, -2);
    assert_eq!(f.stories_count(), 65535);
    assert_eq!(f.stories_unread_count(), 65534);
    f.update_stories_count(100000, 100000);
    assert_eq!(f.stories_unread_count(), 65535);
}
#[test]
fn unread_producers_read_current_and_preserve_known() {
    let current = Rc::new(Cell::new(unread::UnreadState {
        messages: 8,
        known: true,
        ..Default::default()
    }));
    let changes = tg_rpl::EventStream::new();
    let l = Lifetime::new();
    let c = current.clone();
    let source = unread::main_list_unread_state(changes.events(), move || c.get());
    let v = collect(
        unread::unread_state_value(0, source, || {
            Some(unread::UnreadState {
                messages: 3,
                ..Default::default()
            })
        }),
        &l,
    );
    assert_eq!(v.borrow()[0].messages, 5);
    current.set(unread::UnreadState {
        messages: 10,
        ..Default::default()
    });
    changes.fire(Default::default());
    assert_eq!(v.borrow()[1].messages, 7);
    let setting = Rc::new(Cell::new(true));
    let sc = setting.clone();
    let event = tg_rpl::EventStream::new();
    let vv = collect(
        unread::include_muted_counter_folders_value(event.events(), move || sc.get()),
        &l,
    );
    setting.set(false);
    event.fire(());
    assert_eq!(*vv.borrow(), [true, false]);
}
struct FilterFactsOwner {
    peers: Rc<Peers>,
    effects: RefCell<Vec<service::Effect>>,
    hide: Cell<bool>,
    known: RefCell<BTreeSet<PeerKey>>,
    full: Cell<i32>,
}
impl service::FilterOwner for FilterFactsOwner {
    fn resolve_input(&self, p: &tl::InputPeer, chatlist: bool) -> Option<PeerKey> {
        match p {
            tl::InputPeer::InputPeerChat(d) => {
                Some(self.peers.ensure_peer(peer_from_chat(d.chat_id)))
            }
            tl::InputPeer::InputPeerUser(d) => {
                let k = self.peers.ensure_user(UserId::new(d.user_id));
                if chatlist {
                    self.peers.set_user_access_hash(k, d.access_hash)
                }
                Some(k.peer())
            }
            tl::InputPeer::InputPeerChannel(d) => {
                let k = self.peers.ensure_channel(ChannelId::new(d.channel_id));
                if chatlist {
                    self.peers.set_channel_access_hash(k, d.access_hash)
                }
                Some(k.peer())
            }
            tl::InputPeer::InputPeerSelf(_) => Some(self.peers.ensure_user(UserId::new(1)).peer()),
            _ => None,
        }
    }
    fn input(&self, p: PeerKey) -> tl::InputPeer {
        if peer_is_chat(p.id()) {
            tl::mtp_input_peer_chat(peer_to_chat(p.id()).bare)
        } else if peer_is_user(p.id()) {
            let k = UserKey::try_from(p).unwrap();
            tl::mtp_input_peer_user(
                peer_to_user(p.id()).bare,
                self.peers.read_user(k, UserData::access_hash),
            )
        } else {
            panic!("test has no channel input")
        }
    }
    fn peer(&self, id: PeerId) -> PeerKey {
        self.peers.ensure_peer(id)
    }
    fn folder_known(&self, k: PeerKey) -> bool {
        self.known.borrow().contains(&k)
    }
    fn chats_full_size(&self) -> i32 {
        self.full.get()
    }
    fn hide_all_chats(&self) -> bool {
        self.hide.get()
    }
    fn updates_period_ms(&self) -> i64 {
        1000
    }
    fn effect(&self, e: service::Effect) {
        self.effects.borrow_mut().push(e)
    }
}
fn filter_service() -> (
    SessionScope,
    Rc<FilterFactsOwner>,
    Rc<service::ChatFiltersService>,
    ManualClock,
    Rc<TimerHost>,
) {
    let (s, p, _) = setup();
    let o = Rc::new(FilterFactsOwner {
        peers: p,
        effects: RefCell::new(vec![]),
        hide: Cell::new(false),
        known: RefCell::new(BTreeSet::new()),
        full: Cell::new(100),
    });
    let clock = ManualClock::new(10000);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let service = service::ChatFiltersService::new(&s, o.clone(), &host);
    (s, o, service, clock, host)
}
fn sends(o: &FilterFactsOwner) -> usize {
    o.effects
        .borrow()
        .iter()
        .filter(|e| matches!(e, service::Effect::Send { .. }))
        .count()
}
#[test]
fn filter_service_load_preloaded_tags_and_reentrant_events() {
    let (_scope, o, s, _, _) = filter_service();
    assert!(matches!(o.effects.borrow()[0], service::Effect::LoadOnMain));
    let l = Lifetime::new();
    let weak = Rc::downgrade(&s);
    s.changed().start(
        move |_| {
            let s = weak.upgrade().unwrap();
            s.list();
        },
        |_| {},
        || {},
        &l,
    );
    s.set_preloaded(&[tl::mtp_dialog_filter_default()], true);
    assert!(s.loaded() && s.tags_enabled());
    let n = sends(&o);
    s.load(false);
    assert_eq!(sends(&o), n);
    s.complete_preloaded();
    s.load(false);
    s.load(false);
    assert_eq!(sends(&o), n + 1);
    s.reload();
    s.load_reply(None);
    let id = s.request_toggle_tags(false).unwrap();
    assert!(id > 0);
    assert!(s.request_toggle_tags(true).is_none());
    s.toggle_tags_reply(false, true);
    assert!(!s.tags_enabled());
    s.hide_all_changed(true);
}
#[test]
fn filter_service_suggestions_failure_uses_virtual_three_hour_delay() {
    let (_scope, o, s, clock, _) = filter_service();
    s.request_suggested();
    s.request_suggested();
    let n = sends(&o);
    s.suggested_reply(None);
    assert!(s.suggested_loaded());
    clock.advance(10_799_999);
    s.request_suggested();
    assert_eq!(sends(&o), n);
    clock.advance(1);
    s.request_suggested();
    assert_eq!(sends(&o), n + 1);
    s.suggested_reply(Some(&[]));
    assert!(s.suggested_filters().is_empty());
}
#[test]
fn filter_service_order_double_event_and_delete_before_unpin() {
    let (_scope, o, s, _, _) = filter_service();
    s.set(ChatFilter::default().with_id(2));
    let l = Lifetime::new();
    let v = collect(s.changed(), &l);
    s.apply(&tl::mtp_update_dialog_filter_order(vec![2, 0]));
    assert_eq!(v.borrow().len(), 2);
    s.save_order(vec![0, 2], 99);
    assert!(o
        .effects
        .borrow()
        .iter()
        .any(|e| matches!(e, service::Effect::Send { after: 99, .. })));
    s.remove(2);
    assert_eq!(s.list().len(), 1);
}
#[test]
fn filter_service_links_dedup_edits_and_real_tl_fields() {
    let (_scope, o, s, _, _) = filter_service();
    s.set(ChatFilter::default().with_id(2).with_chatlist(true, false));
    let l = Lifetime::new();
    let links = collect(s.chatlist_links(2), &l);
    let invite = tl::ExportedChatlistInviteData {
        url: b"https://t.me/addlist/slug".to_vec(),
        title: b"title".to_vec(),
        peers: vec![tl::mtp_peer_chat(10)],
        ..Default::default()
    }
    .into();
    let link = s.add(2, &invite).unwrap();
    assert_eq!(link.chats.len(), 1);
    s.add(2, &invite);
    assert_eq!(links.borrow().len(), 2);
    s.edit(2, &link.url, "new");
    assert_eq!(links.borrow().last().unwrap()[0].title, "new");
    s.reload_chatlist_links(2);
    s.destroy(2, &link.url);
    assert!(links.borrow().last().unwrap().is_empty());
    assert!(o
        .effects
        .borrow()
        .iter()
        .any(|e| matches!(e, service::Effect::Cancel(_))));
}
#[test]
fn filter_service_exception_original_overfill_and_retry() {
    let (_scope, o, s, _, _) = filter_service();
    let mut f = ChatFilter::default().with_id(2);
    for id in 1..=101 {
        f.always
            .insert(o.peers.ensure_peer(peer_from_chat(id as u64)));
    }
    s.set(f);
    assert!(s.load_next_exceptions(true));
    assert!(o.effects.borrow().iter().any(|e|matches!(e,service::Effect::Send{request:service::Request::Exceptions(r),..} if r.peers.len()==101)));
    s.exceptions_reply(None);
    assert!(s.load_next_exceptions(true));
}
#[test]
fn filter_service_more_watch_timer_drop_and_failure() {
    let (_scope, o, s, clock, host) = filter_service();
    s.set(ChatFilter::default().with_id(2).with_chatlist(true, false));
    let l = Lifetime::new();
    let v = collect(s.more_chats_value(2), &l);
    let initial = sends(&o);
    let r = tl::mtp_chatlists_chatlist_updates(vec![tl::mtp_peer_chat(7)], vec![], vec![]);
    s.more_reply(2, Some(&r));
    assert_eq!(v.borrow().last().unwrap().len(), 1);
    clock.advance(999);
    host.process();
    assert_eq!(sends(&o), initial);
    clock.advance(1);
    host.process();
    assert_eq!(sends(&o), initial + 1);
    s.more_reply(2, None);
    s.more_chats_hide(2, true);
    assert!(s.more_chats(2).is_empty());
    l.destroy();
    clock.advance(5000);
    host.process();
    assert_eq!(sends(&o), initial + 1);
}
#[test]
fn filters_actual_sender_success_and_rpc_error() {
    use std::sync::Mutex;
    use tg_mtproto::{
        mtp_instance::{get_next_request_id, Fields, Instance, Mode},
        mtproto_dc_options::Environment,
        mtproto_response::Response,
        sender::Sender,
    };
    let instance = Instance::new(Mode::Normal, Fields::new(Environment::Test));
    let sender = Sender::new(instance.clone());
    let replies = Arc::new(Mutex::new(vec![]));
    for fail in [false, true] {
        let id = get_next_request_id();
        let out = replies.clone();
        service::send_request(
            &sender,
            id,
            0,
            service::Request::ToggleTags(tl::MessagesToggleDialogFilterTags::new(true)),
            Arc::new(move |r| out.lock().unwrap().push(r)),
        );
        assert!(sender.pending(id));
        instance.process_callback(&Response {
            request_id: id,
            outer_msg_id: 0,
            reply: if fail {
                tg_tl::serialize(&tl::mtp_rpc_error(400, b"TEST_FAILED".to_vec()))
            } else {
                tg_tl::serialize(&true)
            },
        });
    }
    let v = replies.lock().unwrap();
    assert!(v[0].is_ok());
    assert_eq!(v[1], Err("TEST_FAILED".into()));
}
#[test]
fn user_full_flags_preserve_optional_knowledge_and_gift_bits() {
    let (_s, p, c) = setup();
    let k = p.ensure_user(UserId::new(2));
    let d = tl::UserFullData {
        flags: tl::UserFullData::F_SEND_PAID_MESSAGES_STARS
            | tl::UserFullData::F_CONTACT_REQUIRE_PREMIUM,
        send_paid_messages_stars: Some(7),
        ..Default::default()
    };
    p.apply_user_update_flags(k, &d).unwrap();
    assert!(p.read_user(k, |u| u.has_stars_per_message()
        && u.has_require_premium_to_write()
        && u.requires_premium_to_write()));
    p.apply_user_update_flags(k, &tl::UserFullData::default())
        .unwrap();
    assert!(p.read_user(k, |u| u.has_stars_per_message()
        && u.has_require_premium_to_write()
        && !u.requires_premium_to_write()
        && u.message_money_restrictions_known()));
    let d = tl::UserFullData {
        flags: tl::UserFullData::F_DISPLAY_GIFTS_BUTTON,
        ..Default::default()
    };
    p.apply_user_gift_settings(k, &d);
    assert_eq!(
        p.read_user(k, UserData::disallowed_gift_types),
        DisallowedGiftTypes::from(DisallowedGiftType::SendHide)
    );
    let rating = tl::StarsRatingData {
        level: -1,
        stars: u64::MAX,
        current_level_stars: 1,
        next_level_stars: Some(2),
        flags: tl::StarsRatingData::F_NEXT_LEVEL_STARS,
    }
    .into();
    let r = parse_stars_rating(Some(&rating));
    assert_eq!(r.stars, -1);
    p.set_user_stars_rating(k, r);
    let l = Lifetime::new();
    let v = collect(values::peer_stars_rating_value(p.clone(), &c, k.peer()), &l);
    assert_eq!(v.borrow()[0].stars, 0);
    assert_eq!(v.borrow()[0].level, -1);
}
#[test]
fn bot_colors_sentinel_and_wrong_user_rejected() {
    let (_s, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_bot_info_version(k, 1);
    let settings = tl::BotAppSettingsData {
        flags: tl::BotAppSettingsData::F_HEADER_COLOR,
        header_color: Some(-1),
        ..Default::default()
    }
    .into();
    let info: tl::BotInfo = tl::BotInfoData {
        flags: tl::BotInfoData::F_APP_SETTINGS | tl::BotInfoData::F_VERIFIER_SETTINGS,
        verifier_settings: Some(
            tl::BotVerifierSettingsData {
                icon: 55,
                company: b"Company".to_vec(),
                ..Default::default()
            }
            .into(),
        ),
        app_settings: Some(settings),
        ..Default::default()
    }
    .into();
    p.set_user_bot_info(
        k,
        &info,
        |_| panic!("unused photo"),
        |_| panic!("unused doc"),
    );
    assert_eq!(
        p.read_user(k, |u| u.bot_info.as_ref().unwrap().app_colors),
        [0; 4]
    );
    assert_eq!(
        p.read_user(k, |u| u
            .bot_info
            .as_ref()
            .unwrap()
            .verifier_settings
            .as_ref()
            .unwrap()
            .icon_id),
        55
    );
    let other: tl::BotInfo = tl::BotInfoData {
        flags: tl::BotInfoData::F_USER_ID | tl::BotInfoData::F_DESCRIPTION,
        user_id: Some(999),
        description: Some(b"wrong user".to_vec()),
        ..Default::default()
    }
    .into();
    p.set_user_bot_info(
        k,
        &other,
        |_| panic!("unused photo"),
        |_| panic!("unused doc"),
    );
    assert!(p.read_user(k, |u| u.bot_info.as_ref().unwrap().description.is_empty()));
}
#[test]
fn peer_value_streams_real_user_chat_channel_and_reactions() {
    use tg_core::data::{
        data_channel::ChannelDataFlag as G, data_chat::ChatDataFlag as C,
        data_chat_participant_status::*,
    };
    let (_scope, p, c) = setup();
    let u = p.ensure_user(UserId::new(1));
    let chat = p.ensure_chat(ChatId::new(2));
    let channel = p.ensure_channel(ChannelId::new(3));
    p.set_channel_flags(channel, Flags::empty());
    let l = Lifetime::new();
    let pin = collect(values::can_pin_messages_value(&p, chat.peer()), &l);
    assert_eq!(*pin.borrow(), [true]);
    p.add_chat_flags(chat, C::Left.into());
    assert_eq!(pin.borrow().last(), Some(&false));
    let manage = collect(values::peer_can_manage_group_call_value(&p, u.peer()), &l);
    assert_eq!(*manage.borrow(), [true]);
    let send = collect(
        values::can_send_any_of_value(
            &p,
            channel.peer(),
            ChatRestriction::SendOther.into(),
            tg_rpl::single(false),
            true,
            false,
        ),
        &l,
    );
    assert_eq!(send.borrow().last(), Some(&true));
    p.add_channel_flags(channel, G::Broadcast.into());
    assert_eq!(send.borrow().last(), Some(&false));
    p.set_channel_admin_rights(channel, ChatAdminRight::PostMessages.into(), u);
    assert_eq!(send.borrow().last(), Some(&true));
    let forward = collect(values::allows_forwarding_value(&p, u.peer()), &l);
    p.set_user_no_forwards_flags(u, true, false).unwrap();
    assert_eq!(forward.borrow().last(), Some(&false));
    let reactions = collect(
        values::peer_allowed_reactions_value(p.clone(), &c, chat.peer()),
        &l,
    );
    p.set_chat_allowed_reactions(
        chat,
        AllowedReactions {
            max_count: 5,
            ..Default::default()
        },
    );
    c.send_notifications();
    assert_eq!(reactions.borrow().last().unwrap().max_count, 5);
    assert_eq!(
        values::peer_allowed_reactions(&p, u.peer()).kind,
        AllowedReactionsType::All
    );
    let model = tg_rpl::Variable::new(AllowedReactions::default());
    let config = tg_rpl::Variable::new(11);
    let limit = collect(
        values::unique_reactions_limit_value(model.value(), config.value()),
        &l,
    );
    config.set(12);
    model.set(AllowedReactions {
        max_count: 5,
        ..Default::default()
    });
    assert_eq!(*limit.borrow(), [11, 12, 5]);
    assert!(
        !p.read_channel(channel, |c| values::channel_has_subscription_until_date(
            Some(c)
        ))
    );
    assert!(!p.read_channel(channel, values::channel_has_active_call));
}
#[test]
fn user_metadata_plain_fields_and_forum_boundary() {
    let (_scope, p, _c) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.edit_user(k, |u| {
        u.set_private_forward_name("hidden");
        u.set_phone("42");
        u.set_unavailable_reasons_list(vec![UnavailableReason {
            reason: "test".into(),
            text: "reason".into(),
        }]);
    });
    p.read_user(k, |u| {
        assert_eq!(u.phone(), "42");
        assert_eq!(u.private_forward_name(), "hidden");
        assert!(u.has_private_forward_name());
        assert_eq!(u.unavailable_reasons().len(), 1);
        assert!(u.can_send_ignore_money_restrictions(true));
        assert!(!u.business_details().is_present());
        assert_eq!(u.personal_channel_id(), ChannelId::new(0));
        assert_eq!(u.personal_channel_message_id(), MsgId::new(0));
        assert_eq!(u.bot_manager_id(), UserId::new(0));
        assert_eq!(u.calls_status(), CallsStatus::Unknown);
        assert_eq!(u.contact_status(), ContactStatus::Unknown);
        assert_eq!(u.birthday(), Default::default());
    });
    p.set_user_bot_info_version(k, 1);
    assert_eq!(
        p.add_user_flags(k, UserDataFlag::Forum.into()),
        Err(tg_core::data::data_user_store::UserForumRequired)
    );
    assert!(!p.read_user(k, UserData::is_forum));
    let b = BotVerifierSettings {
        icon_id: 1,
        ..Default::default()
    };
    assert!(b.is_present());
}
#[test]
fn topic_send_closed_stream_and_flag_helpers() {
    use tg_core::data::{
        data_changes::TopicFlag, data_channel::ChannelDataFlag as G,
        data_chat_participant_status::*,
    };
    let scope = SessionScope::new();
    let changes = Changes::with_queue(&scope, tg_runtime::MainQueue::new());
    let channel = ChannelKey::try_from(scope.peer_key(peer_from_channel(2u64))).unwrap();
    let topic = TopicKey {
        history: channel.peer(),
        root_id: MsgId::new(3),
    };
    let flags = ReactiveFlags::<G>::default();
    let rights = ReactiveFlags::<ChatRestriction>::default();
    let admin = ReactiveFlags::<ChatAdminRight>::default();
    let closed = Rc::new(Cell::new(false));
    let cc = closed.clone();
    let life = Lifetime::new();
    let v = collect(
        values::topic_can_send_any_of_value(
            flags.value(),
            rights.value(),
            rights.value(),
            admin.value(),
            changes.topic_flags_value(topic, TopicFlag::Closed),
            ChatRestriction::SendOther.into(),
            move || cc.get(),
            || false,
        ),
        &life,
    );
    assert_eq!(*v.borrow(), [true]);
    closed.set(true);
    changes.topic_updated(topic, TopicFlag::Closed);
    changes.send_notifications();
    assert_eq!(v.borrow().last(), Some(&false));
    flags.add(G::Creator.into());
    assert_eq!(v.borrow().last(), Some(&false));
}
#[test]
fn user_stories_birthday_tl_and_premium_flags_streams() {
    let (_scope, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    for state in [
        StoriesState::HasUnread,
        StoriesState::HasVideoStream,
        StoriesState::HasRead,
        StoriesState::None,
    ] {
        p.set_user_stories_state(k, state);
        p.read_user(k, |u| {
            assert_eq!(u.has_active_stories(), state != StoriesState::None);
            assert_eq!(u.has_unread_stories(), state == StoriesState::HasUnread);
            assert_eq!(
                u.has_active_video_stream(),
                state == StoriesState::HasVideoStream
            );
        });
    }
    p.set_user_stories_correspondent(k, true);
    assert!(p.read_user(k, UserData::stories_correspondent));
    let b = tl::mtp_birthday(tl::BirthdayData::F_YEAR, 29, 2, Some(2000));
    p.set_user_birthday_from_tl(k, Some(&b));
    assert_eq!(p.read_user(k, UserData::birthday).year(), 2000);
    p.set_user_birthday_from_tl(k, None);
    assert!(!p.read_user(k, UserData::birthday).valid());
    let flags = p.read_user(k, UserData::flags_value);
    let pc = p.clone();
    let life = Lifetime::new();
    let v = collect(
        values::peer_premium_value(flags, move || {
            pc.read_user(k, |u| u.is_premium(false, false))
        }),
        &life,
    );
    p.add_user_flags(k, UserDataFlag::Premium.into()).unwrap();
    assert_eq!(*v.borrow(), [false, true]);
    let am = collect(values::am_premium_value(true, tg_rpl::single(false)), &life);
    assert_eq!(*am.borrow(), [true]);
    let badges = Folder::chat_list_badges_state(unread::UnreadState {
        marks: 1,
        mentions: 2,
        ..Default::default()
    });
    assert!(
        badges.unread
            && badges.mention
            && badges.unread_muted
            && badges.mention_muted
            && badges.poll_muted
    );
    assert_eq!(badges.unread_counter, 1);
    assert!(!badges.empty());
    assert!(unread::BadgesState::default().empty());
    let s = SessionScope::new();
    let f = Folder::new(s.id(), 1);
    assert_eq!(f.fixed_on_top_index(), 1);
    assert!(f.chat_list_message().is_none());
}
#[test]
fn filter_receive_publishes_each_color_phase_before_next_insert() {
    let (_scope, _o, s, _, _) = filter_service();
    let life = Lifetime::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let out = seen.clone();
    let weak = Rc::downgrade(&s);
    s.tag_color_changed().start(
        move |v| {
            out.borrow_mut()
                .push((v.filter_id, weak.upgrade().unwrap().list().len()));
        },
        |_| {},
        || {},
        &life,
    );
    let filters = [2, 3].map(|id| {
        tl::DialogFilterData {
            flags: tl::DialogFilterData::F_COLOR,
            id,
            color: Some(1),
            title: tl::mtp_text_with_entities(vec![], vec![]),
            ..Default::default()
        }
        .into()
    });
    s.received(&filters);
    assert_eq!(*seen.borrow(), [(2, 2), (3, 3)]);
    assert_eq!(s.list().iter().map(|f| f.id).collect::<Vec<_>>(), [0, 2, 3]);
}
#[test]
fn native_filter_wire_truncation_and_unknown_input_rejection() {
    let scope = SessionScope::new();
    for line in include_str!("fixtures/data-user/native.jsonl").lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["op"] != "filter_tl" {
            continue;
        }
        let w = wire(v["wire"].as_str().unwrap());
        for len in 0..w.len() {
            assert!(tl::DialogFilter::read(&mut tg_tl::Reader::new(&w[..len])).is_err());
        }
        let f = tl::DialogFilter::read(&mut tg_tl::Reader::new(&w)).unwrap();
        let parsed = ChatFilter::from_tl(scope.id(), &f, |_, _| None);
        assert!(parsed.always.is_empty() && parsed.pinned.is_empty() && parsed.never.is_empty());
    }
}
#[test]
fn bot_version_callback_observes_cleared_commands_before_old_description_and_version_change() {
    let (_scope, p, _) = setup();
    let k = p.ensure_user(UserId::new(2));
    p.set_user_bot_info_version(k, 1);
    p.edit_user(k, |u| {
        let b = u.bot_info.as_mut().unwrap();
        b.description = "old".into();
        b.inited = true;
        b.commands.push(Default::default());
    });
    let life = Lifetime::new();
    let seen = Rc::new(RefCell::new(vec![]));
    let out = seen.clone();
    let weak = Rc::downgrade(&p);
    p.user_effects().start(
        move |e| {
            if matches!(e, UserEffect::BotCommandsChanged(_)) {
                let p = weak.upgrade().unwrap();
                let row = p.read_user(k, |u| {
                    let b = u.bot_info.as_ref().unwrap();
                    (b.version, b.description.clone(), b.inited, b.commands.len())
                });
                out.borrow_mut().push(row);
            }
        },
        |_| {},
        || {},
        &life,
    );
    p.set_user_bot_info_version(k, 2);
    assert_eq!(*seen.borrow(), [(1, "old".into(), true, 0)]);
    p.read_user(k, |u| {
        let b = u.bot_info.as_ref().unwrap();
        assert_eq!(b.version, 2);
        assert!(b.description.is_empty() && !b.inited)
    });
}
