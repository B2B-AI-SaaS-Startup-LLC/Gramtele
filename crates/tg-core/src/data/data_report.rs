use super::data_msg_id::{MsgId, StoryId};
#[derive(Clone, Debug, Default)]
pub struct ReportInput {
    pub option_id: Vec<u8>,
    pub option_text: String,
    pub comment: String,
    pub ids: Vec<MsgId>,
    pub stories: Vec<StoryId>,
}
impl PartialEq for ReportInput {
    fn eq(&self, o: &Self) -> bool {
        self.option_id == o.option_id && self.comment == o.comment
    }
}
impl Eq for ReportInput {}
