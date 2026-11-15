//! QJson/QByteArray behavior used by the real passkey service.
use serde_json::Value;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelyingParty {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct User {
    pub id: Vec<u8>,
    pub name: String,
    pub display_name: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CredentialParameter {
    pub type_: String,
    pub alg: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterData {
    pub rp: RelyingParty,
    pub user: User,
    pub challenge: Vec<u8>,
    pub pub_key_cred_params: Vec<CredentialParameter>,
    pub timeout: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Credential {
    pub id: Vec<u8>,
    pub type_: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginData {
    pub challenge: Vec<u8>,
    pub rp_id: String,
    pub allow_credentials: Vec<Credential>,
    pub user_verification: String,
    pub timeout: i32,
}
impl Default for RegisterData {
    fn default() -> Self {
        Self {
            rp: Default::default(),
            user: Default::default(),
            challenge: vec![],
            pub_key_cred_params: vec![],
            timeout: 60000,
        }
    }
}
impl Default for LoginData {
    fn default() -> Self {
        Self {
            challenge: vec![],
            rp_id: String::new(),
            allow_credentials: vec![],
            user_verification: String::new(),
            timeout: 60000,
        }
    }
}
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// Qt IgnoreBase64DecodingErrors: skip other alphabet/padding/junk, retain partial bytes.
pub fn decode_base64(data: &[u8], url: bool) -> Vec<u8> {
    let alphabet = if url {
        URL
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let (mut value, mut bits) = (0u32, 0);
    let mut out = vec![];
    for b in data {
        if let Some(n) = alphabet.iter().position(|c| c == b) {
            value = (value << 6) | n as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((value >> bits) as u8);
            }
        }
    }
    out
}
pub fn encode_base64_url(data: &[u8], padding: bool) -> String {
    let mut out = Vec::new();
    for c in data.chunks(3) {
        let (a, b, d) = (
            c[0],
            c.get(1).copied().unwrap_or(0),
            c.get(2).copied().unwrap_or(0),
        );
        out.push(URL[(a >> 2) as usize]);
        out.push(URL[(((a & 3) << 4) | (b >> 4)) as usize]);
        if c.len() > 1 {
            out.push(URL[(((b & 15) << 2) | (d >> 6)) as usize]);
        } else if padding {
            out.push(b'=');
        }
        if c.len() > 2 {
            out.push(URL[(d & 63) as usize]);
        } else if padding {
            out.push(b'=');
        }
    }
    String::from_utf8(out).unwrap()
}
fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").to_owned()
}
fn integer(v: &Value, default: i32) -> i32 {
    v.as_f64()
        .filter(|n| n.fract() == 0. && *n >= i32::MIN as f64 && *n <= i32::MAX as f64)
        .map_or(default, |n| n as i32)
}
fn array(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}
fn public_key(json: &[u8]) -> Option<Value> {
    let root: Value =
        serde_json::from_slice(json.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(json)).ok()?;
    let key = &root["publicKey"];
    key.as_object()
        .filter(|k| !k.is_empty())
        .map(|_| key.clone())
}
pub fn deserialize_register_data(json: &[u8]) -> Option<RegisterData> {
    let p = public_key(json)?;
    Some(RegisterData {
        rp: RelyingParty {
            id: text(&p["rp"]["id"]),
            name: text(&p["rp"]["name"]),
        },
        user: User {
            id: decode_base64(text(&p["user"]["id"]).as_bytes(), false),
            name: text(&p["user"]["name"]),
            display_name: text(&p["user"]["displayName"]),
        },
        challenge: decode_base64(text(&p["challenge"]).as_bytes(), true),
        pub_key_cred_params: array(&p["pubKeyCredParams"])
            .iter()
            .map(|c| CredentialParameter {
                type_: text(&c["type"]),
                alg: integer(&c["alg"], 0),
            })
            .collect(),
        timeout: integer(&p["timeout"], 60000),
    })
}
pub fn deserialize_login_data(json: &[u8]) -> Option<LoginData> {
    let p = public_key(json)?;
    Some(LoginData {
        challenge: decode_base64(text(&p["challenge"]).as_bytes(), true),
        rp_id: text(&p["rpId"]),
        user_verification: text(&p["userVerification"]),
        timeout: integer(&p["timeout"], 60000),
        allow_credentials: array(&p["allowCredentials"])
            .iter()
            .map(|c| Credential {
                id: decode_base64(text(&c["id"]).as_bytes(), true),
                type_: text(&c["type"]),
            })
            .collect(),
    })
}
fn serialize_client_data(challenge: &[u8], kind: &str) -> String {
    // Qt QJsonObject emits keys lexicographically, compact, with unescaped '/'.
    serde_json::json!({"type":kind,"challenge":encode_base64_url(challenge,false),"origin":"https://telegram.org","crossOrigin":false}).to_string()
}
pub fn serialize_client_data_create(challenge: &[u8]) -> String {
    serialize_client_data(challenge, "webauthn.create")
}
pub fn serialize_client_data_get(challenge: &[u8]) -> String {
    serialize_client_data(challenge, "webauthn.get")
}
