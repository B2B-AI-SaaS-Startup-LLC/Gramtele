use serde_json::{json, Value};
use tg_core::{
    data::{
        components::passkeys as p, data_messages::LoadDirection, data_passkey_deserialize as f,
        data_reply_preview::preview_geometry, data_search_controller::prepare_search_filter,
    },
    platform::platform_webauthn::{LoginResult, RegisterResult},
    storage::storage_shared_media::SharedMediaType as T,
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hex_words(words: &[i32]) -> String {
    hex(&words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect::<Vec<_>>())
}
pub fn kind(index: usize) -> Option<T> {
    [
        Some(T::Photo),
        Some(T::Video),
        Some(T::PhotoVideo),
        Some(T::MusicFile),
        Some(T::File),
        Some(T::VoiceFile),
        Some(T::Link),
        Some(T::ChatPhoto),
        Some(T::RoundVoiceFile),
        Some(T::GIF),
        Some(T::RoundFile),
        Some(T::Pinned),
        Some(T::Poll),
        None,
    ][index]
}
#[test]
fn original_qt_passkey_json_credentials_handle_filters_geometry_and_offsets() {
    use tg_core::data::{
        data_changes::Changes, data_peer_id::peer_from_user, data_session::Peers,
        data_store_keys::SessionScope,
    };
    let inputs = include_str!("fixtures/data-small/inputs.jsonl")
        .lines()
        .collect::<Vec<_>>();
    let mut count = 0;
    let scope = SessionScope::new();
    let owner = super_owner::Owner::new(&scope);
    let peers = Peers::new(&scope, Changes::new(&scope));
    let peer = peers.ensure_peer(peer_from_user(5));
    for row in include_str!("fixtures/data-small/native.jsonl").lines() {
        let v: Value = serde_json::from_str(row).unwrap();
        count += 1;
        match v["kind"].as_str().unwrap() {
            "entry" => {
                let flags = v["flags"].as_u64().unwrap();
                let entry = p::from_tl(&tg_scheme::mtp_passkey(
                    tg_scheme::tl::Flags::from_raw(flags),
                    b"id".to_vec(),
                    b"name".to_vec(),
                    -3,
                    if flags & 1 != 0 { Some(65535) } else { None },
                    if flags & 2 != 0 { Some(9) } else { None },
                ));
                assert_eq!(
                    v["value"],
                    json!([
                        entry.id,
                        entry.name,
                        entry.date,
                        entry.software_emoji_id,
                        entry.last_usage_date
                    ])
                );
            }
            "credential" => {
                let n = v["n"].as_u64().unwrap();
                let bytes = (0..n).map(|i| (250 + i) as u8).collect::<Vec<_>>();
                let reg = RegisterResult {
                    credential_id: bytes.clone(),
                    attestation_object: b"a\0b".to_vec(),
                    client_data_json: b"{}".to_vec(),
                    ..Default::default()
                };
                let login = LoginResult {
                    credential_id: bytes.clone(),
                    client_data_json: b"{}".to_vec(),
                    authenticator_data: b"auth".to_vec(),
                    signature: b"sig".to_vec(),
                    user_handle: b"2:user".to_vec(),
                    ..Default::default()
                };
                assert_eq!(
                    v["register"],
                    hex_words(&tg_tl::serialize(&tg_scheme::AccountRegisterPasskey {
                        credential: p::registration_credential(&reg)
                    }))
                );
                assert_eq!(
                    v["login"],
                    hex_words(&tg_tl::serialize(&p::login_credential(&login)))
                );
                assert_eq!(v["create"], f::serialize_client_data_create(&bytes));
                assert_eq!(v["get"], f::serialize_client_data_get(&bytes));
            }
            "dc" => {
                assert_eq!(
                    v["value"],
                    p::user_dc(v["handle"].as_str().unwrap().as_bytes()).unwrap_or(0)
                );
            }
            "filter" => assert_eq!(
                v["wire"],
                hex_words(&tg_tl::serialize(&prepare_search_filter(kind(
                    v["type"].as_u64().unwrap() as usize
                ))))
            ),
            "geometry" => {
                let g = preview_geometry(
                    v["w"].as_i64().unwrap() as i32,
                    v["h"].as_i64().unwrap() as i32,
                    35,
                    v["ratio"].as_i64().unwrap() as i32,
                );
                assert_eq!(v["value"], json!([g.width, g.height, g.outer]));
            }
            "offset" => {
                use tg_core::data::{
                    data_msg_id::MsgId,
                    data_peer_id::PeerId,
                    data_search_controller::{prepare_history_request, prepare_search_request},
                };
                let id = MsgId::new(v["id"].as_str().unwrap().parse().unwrap());
                let d = [
                    LoadDirection::Around,
                    LoadDirection::Before,
                    LoadDirection::After,
                ][v["direction"].as_u64().unwrap() as usize];
                let r = prepare_search_request(
                    &owner,
                    &peers,
                    peer,
                    0.into(),
                    PeerId::default(),
                    None,
                    "q",
                    id,
                    d,
                )
                .unwrap();
                let h = prepare_history_request(&owner, peer, id, d);
                assert_eq!(v["search"], json!([r.offset_id, r.add_offset, r.limit]));
                assert_eq!(v["history"], json!([h.offset_id, h.add_offset, h.limit]));
            }
            "json" => {
                let bytes = inputs[v["index"].as_u64().unwrap() as usize].as_bytes();
                let r=f::deserialize_register_data(bytes).map(|r|json!({"rp_id":r.rp.id,"rp_name":r.rp.name,"user_id":hex(&r.user.id),"user_name":r.user.name,"display_name":r.user.display_name,"challenge":hex(&r.challenge),"params":r.pub_key_cred_params.iter().map(|p|json!({"type":p.type_,"alg":p.alg})).collect::<Vec<_>>(),"timeout":r.timeout})).unwrap_or(Value::Null);
                let l=f::deserialize_login_data(bytes).map(|l|json!({"challenge":hex(&l.challenge),"rp_id":l.rp_id,"verify":l.user_verification,"timeout":l.timeout,"credentials":l.allow_credentials.iter().map(|c|json!({"id":hex(&c.id),"type":c.type_})).collect::<Vec<_>>()})).unwrap_or(Value::Null);
                assert_eq!(v["register"], r, "{row}");
                assert_eq!(v["login"], l, "{row}");
            }
            _ => panic!("{row}"),
        }
    }
    assert_eq!(count, 135);
}
#[path = "support/data_small_owner.rs"]
mod super_owner;
