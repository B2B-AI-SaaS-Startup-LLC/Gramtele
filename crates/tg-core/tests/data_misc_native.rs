//! Captured outputs of unchanged pinned C++ functions, not Rust roundtrips.
use tg_core::data::{
    business::data_business_common::*, components::top_peers as top, data_auto_download as auto,
    data_file_origin::*,
};
use tg_scheme as tl;
fn bytes(s: &str) -> Vec<u8> {
    if s == "-" {
        return vec![];
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn pairs(s: &str) -> WorkingIntervals {
    WorkingIntervals {
        list: s
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (a, b) = s.split_once(':').unwrap();
                WorkingInterval {
                    start: a.parse().unwrap(),
                    end: b.parse().unwrap(),
                }
            })
            .collect(),
    }
}
fn refs<T: tl::tl::Deserialize + FileReferenceSource>(b: &[u8]) -> String {
    let v: T = tl::tl::deserialize_exact(&tl::tl::bytes_to_primes(b)).unwrap();
    let r = get_file_references(&v);
    r.data
        .iter()
        .map(|(id, r)| {
            format!(
                "{}:{};",
                match id {
                    FileLocationId::Document(id) => format!("d{id}"),
                    FileLocationId::Photo(id) => format!("p{id}"),
                },
                hex(r)
            )
        })
        .collect()
}
#[test]
fn native_auto_serialization_truncation_versions_and_limits() {
    let mut count = 0;
    for line in include_str!("fixtures/data-misc/native.csv").lines() {
        let v: Vec<_> = line.split(',').collect();
        match v[0] {
            "auto" => {
                let mut f = auto::Full::default();
                assert_eq!(f.set_from_serialized(&bytes(v[1])), v[2] == "1", "{line}");
                assert_eq!(hex(&f.serialize()), v[3], "{line}");
                count += 1;
            }
            "limit" => {
                let s = auto::SOURCES[v[1].parse::<usize>().unwrap()];
                let t = auto::TYPES[v[2].parse::<usize>().unwrap()];
                let limit = v[3].parse::<i64>().unwrap();
                let n = v[4].parse::<i64>().unwrap();
                let mut f = auto::Full::default();
                if limit >= 0 {
                    f.set_bytes_limit(s, t, limit);
                }
                assert_eq!(f.bytes_limit(s, t), v[5].parse::<i64>().unwrap(), "{line}");
                assert_eq!(f.should_download(s, t, n), v[6] == "1", "{line}");
                count += 1;
            }
            _ => {}
        }
    }
    assert!(count > 1400);
}
#[test]
fn native_business_normalization_extract_remove_replace() {
    let mut count = 0;
    for line in include_str!("fixtures/data-misc/native.csv")
        .lines()
        .filter(|s| s.starts_with("business,"))
    {
        let v: Vec<_> = line.split(',').collect();
        let day = v[1].parse().unwrap();
        let input = pairs(v[2]);
        assert_eq!(input.normalized(), pairs(v[3]), "{line}");
        let extracted = extract_day_intervals(&input, day);
        assert_eq!(extracted, pairs(v[4]), "{line}");
        assert_eq!(is_full_open(&extracted), v[5] == "1", "{line}");
        assert_eq!(remove_day_intervals(&input, day), pairs(v[6]), "{line}");
        assert_eq!(
            replace_day_intervals(
                &input,
                day,
                WorkingIntervals {
                    list: vec![WorkingInterval {
                        start: 3600,
                        end: 7200
                    }]
                }
            ),
            pairs(v[7]),
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 84);
}
#[test]
fn native_top_rating_rounding_and_decay() {
    for line in include_str!("fixtures/data-misc/native.csv")
        .lines()
        .filter(|s| s.starts_with("rating,"))
    {
        let v: Vec<_> = line.split(',').collect();
        let r = v[1].parse().unwrap();
        let encoded = top::serialize_rating(r);
        assert_eq!(encoded, v[5].parse::<u64>().unwrap());
        let decoded = v[6].parse::<f64>().unwrap();
        assert!((top::deserialize_rating(encoded) - decoded).abs() < 1e-8);
        let delta = top::rating_delta(
            v[2].parse().unwrap(),
            v[3].parse().unwrap(),
            v[4].parse().unwrap(),
        );
        let expected = v[7].parse::<f64>().unwrap();
        assert!((delta - expected).abs() < 1e-12);
    }
}
#[test]
fn native_file_reference_accumulator_all_variants() {
    let mut count = 0;
    for line in include_str!("fixtures/data-misc/native.csv")
        .lines()
        .filter(|s| s.starts_with("refs,"))
    {
        let v: Vec<_> = line.split(',').collect();
        let b = bytes(v[2]);
        let result = match v[1] {
            "Photo" => refs::<tl::Photo>(&b),
            "Document" => refs::<tl::Document>(&b),
            "Page" => refs::<tl::Page>(&b),
            "RichMessage" => refs::<tl::RichMessage>(&b),
            "WallPaper" => refs::<tl::WallPaper>(&b),
            "Theme" => refs::<tl::Theme>(&b),
            "WebPageAttribute" => refs::<tl::WebPageAttribute>(&b),
            "StarGift" => refs::<tl::StarGift>(&b),
            "StarGiftAttribute" => refs::<tl::StarGiftAttribute>(&b),
            "WebPage" => refs::<tl::WebPage>(&b),
            "Game" => refs::<tl::Game>(&b),
            "MessageExtendedMedia" => refs::<tl::MessageExtendedMedia>(&b),
            "MessageMedia" => refs::<tl::MessageMedia>(&b),
            "MessageReplyHeader" => refs::<tl::MessageReplyHeader>(&b),
            "DraftMessage" => refs::<tl::DraftMessage>(&b),
            "Dialog" => refs::<tl::Dialog>(&b),
            "Message" => refs::<tl::Message>(&b),
            "StoryItem" => refs::<tl::StoryItem>(&b),
            "messages.Messages" => refs::<tl::MessagesMessages>(&b),
            "messages.PeerDialogs" => refs::<tl::MessagesPeerDialogs>(&b),
            "photos.Photos" => refs::<tl::PhotosPhotos>(&b),
            "users.UserFull" => refs::<tl::UsersUserFull>(&b),
            "ChatFull" => refs::<tl::ChatFull>(&b),
            "messages.ChatFull" => refs::<tl::MessagesChatFull>(&b),
            "ForumTopic" => refs::<tl::ForumTopic>(&b),
            "messages.ForumTopics" => refs::<tl::MessagesForumTopics>(&b),
            "messages.RecentStickers" => refs::<tl::MessagesRecentStickers>(&b),
            "messages.FavedStickers" => refs::<tl::MessagesFavedStickers>(&b),
            "messages.StickerSet" => refs::<tl::MessagesStickerSet>(&b),
            "messages.SavedGifs" => refs::<tl::MessagesSavedGifs>(&b),
            "SavedDialog" => refs::<tl::SavedDialog>(&b),
            "messages.SavedDialogs" => refs::<tl::MessagesSavedDialogs>(&b),
            "account.SavedRingtones" => refs::<tl::AccountSavedRingtones>(&b),
            "help.PremiumPromo" => refs::<tl::HelpPremiumPromo>(&b),
            "messages.WebPage" => refs::<tl::MessagesWebPage>(&b),
            "stories.Stories" => refs::<tl::StoriesStories>(&b),
            "users.SavedMusic" => refs::<tl::UsersSavedMusic>(&b),
            _ => panic!("unmapped root {}", v[1]),
        };
        assert_eq!(result, v[3], "{line}");
        count += 1;
    }
    assert!(count > 150);
}
#[test]
fn native_audio_default_external_and_origin_ordering() {
    let text = include_str!("fixtures/data-misc/native.csv");
    assert!(text.contains("audio,0,1,3,0,1,1,1,1"));
    assert!(text.contains("origin,0,1,0,0,0,0,1,1"));
    let a = tg_core::data::data_audio_msg_id::AudioMsgId::for_video();
    let b = tg_core::data::data_audio_msg_id::AudioMsgId::for_video();
    assert!(a < b && a != b && a.is_present());
    let a = FileOrigin::StickerSet(FileOriginStickerSet {
        set_id: 1,
        access_hash: 2,
    });
    let b = FileOrigin::StickerSet(FileOriginStickerSet {
        set_id: 1,
        access_hash: 999,
    });
    assert_eq!(a, b);
}
#[test]
fn native_business_rights_and_top_peer_hash() {
    let mut rows = 0;
    for line in include_str!("fixtures/data-misc/native.csv").lines() {
        let v: Vec<_> = line.split(',').collect();
        match v[0] {
            "rights" => {
                rows += 1;
                let bits = v[1].parse::<u64>().unwrap();
                let rights = tg_core::data::business::data_business_common::permissions_from_mtp(
                    &tl::mtp_business_bot_rights(tl::tl::Flags::from_raw(bits)),
                );
                assert_eq!(rights.value(), v[2].parse::<u32>().unwrap());
                let tl::BusinessBotRights::BusinessBotRights(d) =
                    tg_core::data::business::data_business_common::permissions_to_mtp(rights);
                assert_eq!(d.flags.value(), v[3].parse::<u64>().unwrap());
            }
            "hash" => {
                rows += 1;
                let count = v[1].parse::<u64>().unwrap();
                let value = v[2].parse::<u64>().unwrap();
                let mut hash = 0;
                for i in 0..count {
                    top::hash_update(&mut hash, value.wrapping_add(i));
                }
                assert_eq!(hash, v[3].parse::<u64>().unwrap());
            }
            _ => {}
        }
    }
    assert_eq!(rows, 33);
}
