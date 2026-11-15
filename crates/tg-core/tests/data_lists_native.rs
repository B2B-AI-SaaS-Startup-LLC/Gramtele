use std::{cell::RefCell, rc::Rc};
use tg_core::data::{
    business::{data_business_common::*, data_business_info::*},
    data_messages::*,
    data_msg_id::*,
    data_peer_id::PeerId,
    data_premium_limits::*,
    data_sparse_ids::*,
};
use tg_core::storage::storage_sparse_ids_list::SparseIdsListResult;
use tg_rpl::Lifetime;

fn pos(n: i32) -> MessagePosition {
    MessagePosition {
        full_id: FullMsgId::new(PeerId::new(9), MsgId::new(n.into())),
        date: n,
    }
}
fn opt(n: Option<i32>) -> String {
    n.map_or("?".into(), |n| n.to_string()) + ","
}
fn result(r: MessagesResult) -> String {
    opt(r.count)
        + &opt(r.skipped_before)
        + &opt(r.skipped_after)
        + &r.message_ids
            .iter()
            .map(|p| format!("{};", p.full_id.msg.bare))
            .collect::<String>()
        + "\n"
}
fn sparse(r: SparseIdsSlice) -> String {
    opt(r.full_count())
        + &opt(r.skipped_before())
        + &opt(r.skipped_after())
        + &(0..r.size())
            .map(|i| format!("{};", r.at(i).bare))
            .collect::<String>()
        + "\n"
}
fn hex(b: Vec<u8>) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
struct Config {
    mode: i32,
}
impl LimitsContext for Config {
    fn app_config_limit(&self, key: &str, fallback: i32) -> i32 {
        if self.mode & 2 != 0 {
            100 + key.len() as i32
        } else {
            fallback
        }
    }
    fn is_premium(&self) -> bool {
        self.mode & 1 != 0
    }
    fn is_test_mode(&self) -> bool {
        false
    }
}
#[test]
fn native_lists_builders_merge_limits_and_business_tl() {
    let out = Rc::new(RefCell::new(String::new()));
    for mode in 0..4 {
        let c = Config { mode };
        let p = PremiumLimits::new(&c);
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsDefault,{}\n",
            p.channels_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsPremium,{}\n",
            p.channels_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsCurrent,{}\n",
            p.channels_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},similarChannelsDefault,{}\n",
            p.similar_channels_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},similarChannelsPremium,{}\n",
            p.similar_channels_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},similarChannelsCurrent,{}\n",
            p.similar_channels_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},gifsDefault,{}\n",
            p.gifs_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},gifsPremium,{}\n",
            p.gifs_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},gifsCurrent,{}\n",
            p.gifs_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},stickersFavedDefault,{}\n",
            p.stickers_faved_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},stickersFavedPremium,{}\n",
            p.stickers_faved_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},stickersFavedCurrent,{}\n",
            p.stickers_faved_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersDefault,{}\n",
            p.dialog_filters_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersPremium,{}\n",
            p.dialog_filters_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersCurrent,{}\n",
            p.dialog_filters_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogShareableFiltersDefault,{}\n",
            p.dialog_shareable_filters_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogShareableFiltersPremium,{}\n",
            p.dialog_shareable_filters_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogShareableFiltersCurrent,{}\n",
            p.dialog_shareable_filters_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersChatsDefault,{}\n",
            p.dialog_filters_chats_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersChatsPremium,{}\n",
            p.dialog_filters_chats_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersChatsCurrent,{}\n",
            p.dialog_filters_chats_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersLinksDefault,{}\n",
            p.dialog_filters_links_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersLinksPremium,{}\n",
            p.dialog_filters_links_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogFiltersLinksCurrent,{}\n",
            p.dialog_filters_links_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsPinnedDefault,{}\n",
            p.dialogs_pinned_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsPinnedPremium,{}\n",
            p.dialogs_pinned_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsPinnedCurrent,{}\n",
            p.dialogs_pinned_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsFolderPinnedDefault,{}\n",
            p.dialogs_folder_pinned_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsFolderPinnedPremium,{}\n",
            p.dialogs_folder_pinned_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},dialogsFolderPinnedCurrent,{}\n",
            p.dialogs_folder_pinned_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},topicsPinnedCurrent,{}\n",
            p.topics_pinned_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},savedSublistsPinnedDefault,{}\n",
            p.saved_sublists_pinned_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},savedSublistsPinnedPremium,{}\n",
            p.saved_sublists_pinned_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},savedSublistsPinnedCurrent,{}\n",
            p.saved_sublists_pinned_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsPublicDefault,{}\n",
            p.channels_public_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsPublicPremium,{}\n",
            p.channels_public_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},channelsPublicCurrent,{}\n",
            p.channels_public_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},captionLengthDefault,{}\n",
            p.caption_length_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},captionLengthPremium,{}\n",
            p.caption_length_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},captionLengthCurrent,{}\n",
            p.caption_length_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},messageLengthDefault,{}\n",
            p.message_length_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},messageLengthPremium,{}\n",
            p.message_length_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},messageLengthCurrent,{}\n",
            p.message_length_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},uploadMaxDefault,{}\n",
            p.upload_max_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},uploadMaxPremium,{}\n",
            p.upload_max_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},uploadMaxCurrent,{}\n",
            p.upload_max_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},aboutLengthDefault,{}\n",
            p.about_length_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},aboutLengthPremium,{}\n",
            p.about_length_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},aboutLengthCurrent,{}\n",
            p.about_length_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},contactNoteLengthCurrent,{}\n",
            p.contact_note_length_current()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},botsCreateDefault,{}\n",
            p.bots_create_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},botsCreatePremium,{}\n",
            p.bots_create_premium()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},aiComposeSavedTonesDefault,{}\n",
            p.ai_compose_saved_tones_default()
        ));
        out.borrow_mut().push_str(&format!(
            "premium,{mode},aiComposeSavedTonesPremium,{}\n",
            p.ai_compose_saved_tones_premium()
        ));
    }
    for n in 0..7 {
        let l = MessagesList::default();
        l.add_slice(
            (1..=n).map(|i| pos(i * 10)).collect(),
            FULL_MESSAGES_RANGE,
            Some(n),
        );
        for around in (5..=75).step_by(10) {
            for b in 0..3 {
                for a in 0..3 {
                    out.borrow_mut().push_str(&format!(
                        "list,{n},{around},{b},{a},{}",
                        result(l.snapshot(MessagesQuery {
                            around_id: pos(around),
                            limit_before: b,
                            limit_after: a
                        }))
                    ));
                }
            }
        }
        l.remove_one(pos(999));
        out.borrow_mut().push_str(&format!(
            "absent,{n},{}",
            result(l.snapshot(MessagesQuery {
                around_id: pos(20),
                limit_before: 10,
                limit_after: 10
            }))
        ));
        l.remove_less_than(pos(20));
        out.borrow_mut().push_str(&format!(
            "less,{n},{}",
            result(l.snapshot(MessagesQuery {
                around_id: pos(20),
                limit_before: 10,
                limit_after: 10
            }))
        ));
        l.invalidate_bottom();
        out.borrow_mut().push_str(&format!(
            "bottom,{n},{}",
            result(l.snapshot(MessagesQuery {
                around_id: pos(20),
                limit_before: 10,
                limit_after: 10
            }))
        ));
    }
    for mode in 0..4 {
        let b = SparseIdsSliceBuilder::new(30.into(), 1, 1);
        let life = Lifetime::new();
        let output = out.clone();
        b.insufficient_around().start(
            move |e| {
                output.borrow_mut().push_str(&format!(
                    "request,{mode},{},{}\n",
                    e.around_id.bare, e.direction as i32
                ))
            },
            |_| {},
            || {},
            &life,
        );
        b.check_insufficient();
        let r = SparseIdsListResult {
            count: (mode & 1 != 0).then_some(10),
            skipped_before: (mode & 2 != 0).then_some(0),
            skipped_after: None,
            message_ids: [10, 20, 30, 40, 50].into_iter().collect(),
        };
        b.apply_initial(&r);
        out.borrow_mut()
            .push_str(&format!("builder,{mode},initial,{}", sparse(b.snapshot())));
        b.remove_one(20.into());
        out.borrow_mut()
            .push_str(&format!("builder,{mode},remove,{}", sparse(b.snapshot())));
        b.invalidate_bottom();
        out.borrow_mut()
            .push_str(&format!("builder,{mode},bottom,{}", sparse(b.snapshot())));
        b.remove_all();
        out.borrow_mut()
            .push_str(&format!("builder,{mode},all,{}", sparse(b.snapshot())));
    }
    for m in 0..3 {
        for p in 0..3 {
            for dir in 0..2 {
                let key = Key::new(
                    PeerId::new(9),
                    0.into(),
                    PeerId::new(0),
                    PeerId::new(8),
                    if dir != 0 { (-20).into() } else { 20.into() },
                );
                let s = SparseIdsMergedSlice::from_parts(
                    key,
                    SparseIdsSlice::new(
                        [10.into(), 20.into()].into_iter().collect(),
                        Some(2),
                        (p != 0).then_some(p - 1),
                        Some(0),
                    ),
                    Some(SparseIdsSlice::new(
                        [
                            MsgId::new(SERVER_MAX_MSG_ID.bare - 30),
                            MsgId::new(SERVER_MAX_MSG_ID.bare - 20),
                        ]
                        .into_iter()
                        .collect(),
                        Some(2),
                        Some(0),
                        (m != 0).then_some(m - 1),
                    )),
                );
                let mut row = format!(
                    "merged,{m},{p},{dir},{}{}{}{},{},",
                    opt(s.full_count()),
                    opt(s.skipped_before()),
                    opt(s.skipped_after()),
                    s.size(),
                    (0..s.size())
                        .map(|i| {
                            let id = s.at(i);
                            format!("{}:{};", id.peer.value, id.msg.bare)
                        })
                        .collect::<String>()
                );
                row += &opt(s.index_of(FullMsgId::new(PeerId::new(9), 20.into())));
                if let Some(id) = s.nearest(if dir != 0 { (-25).into() } else { 15.into() }) {
                    row += &format!("{}:{}", id.peer.value, id.msg.bare);
                }
                row.push('\n');
                out.borrow_mut().push_str(&row);
            }
        }
    }
    for n in 0..5 {
        let h = WorkingHours {
            timezone_id: "Europe/Moscow".into(),
            intervals: WorkingIntervals {
                list: (0..n)
                    .map(|i| WorkingInterval {
                        start: i * 3600 + 17,
                        end: i * 3600 + 1801,
                    })
                    .collect(),
            },
        };
        out.borrow_mut().push_str(&format!(
            "hours,{n},{}\n",
            hex(tg_tl::primes_to_bytes(&tg_tl::serialize(
                &working_hours_to_mtp(&h)
            )))
        ));
    }
    for t in 1..4 {
        let s = AwaySchedule {
            kind: match t {
                1 => AwayScheduleType::Always,
                2 => AwayScheduleType::OutsideWorkingHours,
                _ => AwayScheduleType::Custom,
            },
            custom_interval: WorkingInterval {
                start: -123,
                end: 456,
            },
        };
        out.borrow_mut().push_str(&format!(
            "schedule,{t},{}\n",
            hex(tg_tl::primes_to_bytes(&tg_tl::serialize(
                &away_schedule_to_mtp(&s)
            )))
        ));
    }
    for mode in 0..2 {
        let a = AwaySettings {
            schedule: AwaySchedule {
                kind: AwayScheduleType::Always,
                ..Default::default()
            },
            shortcut_id: 7,
            offline_only: mode != 0,
            ..Default::default()
        };
        out.borrow_mut().push_str(&format!(
            "away,{mode},{}\n",
            hex(tg_tl::primes_to_bytes(&tg_tl::serialize(&away_to_mtp(
                &a,
                |_| panic!("empty recipients")
            ))))
        ));
    }
    let g = GreetingSettings {
        shortcut_id: 8,
        no_activity_days: 7,
        ..Default::default()
    };
    out.borrow_mut().push_str(&format!(
        "greeting,{}\n",
        hex(tg_tl::primes_to_bytes(&tg_tl::serialize(&greeting_to_mtp(
            &g,
            |_| panic!("empty recipients")
        ))))
    ));
    assert_eq!(
        *out.borrow(),
        include_str!("fixtures/data-lists/native.csv")
    );
}
