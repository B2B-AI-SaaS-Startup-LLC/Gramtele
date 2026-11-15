#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnreviewedAuth {
    pub hash: u64,
    pub unconfirmed: bool,
    pub date: i32,
    pub device: String,
    pub location: String,
}
