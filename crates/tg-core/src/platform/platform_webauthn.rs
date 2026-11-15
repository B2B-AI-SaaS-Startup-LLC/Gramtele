//! Actual platform_webauthn.h result values. Platform ceremonies/support probing
//! remain package33; no default/pretend WebAuthn implementation is provided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Error {
    #[default]
    None,
    Cancelled,
    UnsignedBuild,
    Other,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoginResult {
    pub client_data_json: Vec<u8>,
    pub credential_id: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub user_handle: Vec<u8>,
    pub error: Error,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterResult {
    pub credential_id: Vec<u8>,
    pub attestation_object: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub success: bool,
    pub error: Error,
}
