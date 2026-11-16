use super::data_store_keys::*;
/// Real game data; the owner's Session is represented by its actual scope.
#[derive(Clone, Debug)]
pub struct GameData {
    pub owner: SessionScopeId,
    pub id: u64,
    pub access_hash: u64,
    pub short_name: String,
    pub title: String,
    pub description: String,
    pub photo: Option<PhotoRecordKey>,
    pub document: Option<DocumentRecordKey>,
}
impl GameData {
    pub fn new(owner: SessionScopeId, id: u64) -> Self {
        Self {
            owner,
            id,
            access_hash: 0,
            short_name: String::new(),
            title: String::new(),
            description: String::new(),
            photo: None,
            document: None,
        }
    }
}
