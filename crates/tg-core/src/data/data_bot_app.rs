use super::{
    data_peer_id::PeerId,
    data_store_keys::{DocumentRecordKey, PhotoRecordKey, SessionScope, SessionScopeId},
};
#[derive(Debug)]
pub struct BotAppData {
    owner: SessionScopeId,
    pub id: u64,
    pub bot_id: PeerId,
    pub short_name: String,
    pub title: String,
    pub description: String,
    pub photo: Option<PhotoRecordKey>,
    pub document: Option<DocumentRecordKey>,
    pub access_hash: u64,
    pub hash: u64,
}
impl BotAppData {
    pub fn new(owner: &SessionScope, id: u64) -> Self {
        Self {
            owner: owner.id(),
            id,
            bot_id: PeerId::default(),
            short_name: String::new(),
            title: String::new(),
            description: String::new(),
            photo: None,
            document: None,
            access_hash: 0,
            hash: 0,
        }
    }
    pub fn owner(&self) -> SessionScopeId {
        self.owner
    }
}
