//! Origin ordering and exact recursive file-reference collection from the TL tree.
//! The source defines no persistence format for FileOrigin itself.
use super::{
    data_msg_id::{FullMsgId, FullStoryId, MsgId},
    data_peer_id::{PeerId, UserId},
};
use std::{cmp::Ordering, collections::BTreeMap};
use tg_scheme as tl;
#[derive(Clone, Debug, Default)]
pub struct FileOriginUserPhoto {
    pub user_id: UserId,
    pub photo_id: u64,
}
impl Ord for FileOriginUserPhoto {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.user_id, self.photo_id).cmp(&(o.user_id, o.photo_id))
    }
}
impl PartialOrd for FileOriginUserPhoto {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginUserPhoto {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginUserPhoto {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginFullUser {
    pub user_id: UserId,
}
impl Ord for FileOriginFullUser {
    fn cmp(&self, o: &Self) -> Ordering {
        self.user_id.cmp(&o.user_id)
    }
}
impl PartialOrd for FileOriginFullUser {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginFullUser {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginFullUser {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginPeerPhoto {
    pub peer_id: PeerId,
}
impl Ord for FileOriginPeerPhoto {
    fn cmp(&self, o: &Self) -> Ordering {
        self.peer_id.cmp(&o.peer_id)
    }
}
impl PartialOrd for FileOriginPeerPhoto {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginPeerPhoto {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginPeerPhoto {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginStickerSet {
    pub set_id: u64,
    pub access_hash: u64,
}
impl Ord for FileOriginStickerSet {
    fn cmp(&self, o: &Self) -> Ordering {
        self.set_id.cmp(&o.set_id)
    }
}
impl PartialOrd for FileOriginStickerSet {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginStickerSet {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginStickerSet {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginSavedGifs {}
impl Ord for FileOriginSavedGifs {
    fn cmp(&self, _o: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for FileOriginSavedGifs {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginSavedGifs {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginSavedGifs {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginWallpaper {
    pub paper_id: u64,
    pub access_hash: u64,
    pub owner_id: UserId,
    pub slug: String,
}
impl Ord for FileOriginWallpaper {
    fn cmp(&self, o: &Self) -> Ordering {
        self.paper_id.cmp(&o.paper_id)
    }
}
impl PartialOrd for FileOriginWallpaper {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginWallpaper {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginWallpaper {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginTheme {
    pub theme_id: u64,
    pub access_hash: u64,
}
impl Ord for FileOriginTheme {
    fn cmp(&self, o: &Self) -> Ordering {
        self.theme_id.cmp(&o.theme_id)
    }
}
impl PartialOrd for FileOriginTheme {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginTheme {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginTheme {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginRingtones {}
impl Ord for FileOriginRingtones {
    fn cmp(&self, _o: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for FileOriginRingtones {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginRingtones {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginRingtones {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginPremiumPreviews {}
impl Ord for FileOriginPremiumPreviews {
    fn cmp(&self, _o: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for FileOriginPremiumPreviews {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginPremiumPreviews {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginPremiumPreviews {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginWebPage {
    pub url: String,
}
impl Ord for FileOriginWebPage {
    fn cmp(&self, o: &Self) -> Ordering {
        self.url.encode_utf16().cmp(o.url.encode_utf16())
    }
}
impl PartialOrd for FileOriginWebPage {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginWebPage {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginWebPage {}
#[derive(Clone, Debug, Default)]
pub struct FileOriginCloudDraft {
    pub peer_id: PeerId,
    pub topic_root_id: MsgId,
    pub monoforum_peer_id: PeerId,
}
impl Ord for FileOriginCloudDraft {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.peer_id, self.topic_root_id, self.monoforum_peer_id).cmp(&(
            o.peer_id,
            o.topic_root_id,
            o.monoforum_peer_id,
        ))
    }
}
impl PartialOrd for FileOriginCloudDraft {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for FileOriginCloudDraft {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for FileOriginCloudDraft {}
pub type FileOriginMessage = FullMsgId;
pub type FileOriginStory = FullStoryId;
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileOrigin {
    #[default]
    None,
    Message(FullMsgId),
    UserPhoto(FileOriginUserPhoto),
    FullUser(FileOriginFullUser),
    PeerPhoto(FileOriginPeerPhoto),
    StickerSet(FileOriginStickerSet),
    SavedGifs(FileOriginSavedGifs),
    Wallpaper(FileOriginWallpaper),
    Theme(FileOriginTheme),
    Ringtones(FileOriginRingtones),
    PremiumPreviews(FileOriginPremiumPreviews),
    WebPage(FileOriginWebPage),
    CloudDraft(FileOriginCloudDraft),
    Story(FullStoryId),
}
impl FileOrigin {
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::None)
    }
}
impl From<FullMsgId> for FileOrigin {
    fn from(v: FullMsgId) -> Self {
        Self::Message(v)
    }
}
impl From<FileOriginUserPhoto> for FileOrigin {
    fn from(v: FileOriginUserPhoto) -> Self {
        Self::UserPhoto(v)
    }
}
impl From<FileOriginFullUser> for FileOrigin {
    fn from(v: FileOriginFullUser) -> Self {
        Self::FullUser(v)
    }
}
impl From<FileOriginPeerPhoto> for FileOrigin {
    fn from(v: FileOriginPeerPhoto) -> Self {
        Self::PeerPhoto(v)
    }
}
impl From<FileOriginStickerSet> for FileOrigin {
    fn from(v: FileOriginStickerSet) -> Self {
        Self::StickerSet(v)
    }
}
impl From<FileOriginSavedGifs> for FileOrigin {
    fn from(v: FileOriginSavedGifs) -> Self {
        Self::SavedGifs(v)
    }
}
impl From<FileOriginWallpaper> for FileOrigin {
    fn from(v: FileOriginWallpaper) -> Self {
        Self::Wallpaper(v)
    }
}
impl From<FileOriginTheme> for FileOrigin {
    fn from(v: FileOriginTheme) -> Self {
        Self::Theme(v)
    }
}
impl From<FileOriginRingtones> for FileOrigin {
    fn from(v: FileOriginRingtones) -> Self {
        Self::Ringtones(v)
    }
}
impl From<FileOriginPremiumPreviews> for FileOrigin {
    fn from(v: FileOriginPremiumPreviews) -> Self {
        Self::PremiumPreviews(v)
    }
}
impl From<FileOriginWebPage> for FileOrigin {
    fn from(v: FileOriginWebPage) -> Self {
        Self::WebPage(v)
    }
}
impl From<FileOriginCloudDraft> for FileOrigin {
    fn from(v: FileOriginCloudDraft) -> Self {
        Self::CloudDraft(v)
    }
}
impl From<FullStoryId> for FileOrigin {
    fn from(v: FullStoryId) -> Self {
        Self::Story(v)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileLocationId {
    Document(u64),
    Photo(u64),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdatedFileReferences {
    pub data: BTreeMap<FileLocationId, Vec<u8>>,
}
pub fn get_file_references(data: &impl FileReferenceSource) -> UpdatedFileReferences {
    let mut r = UpdatedFileReferences::default();
    data.collect_references(&mut r);
    r
}
pub trait FileReferenceSource {
    fn collect_references(&self, result: &mut UpdatedFileReferences);
}
impl<T: FileReferenceSource> FileReferenceSource for Vec<T> {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        for v in self {
            v.collect_references(r);
        }
    }
}
impl<T: FileReferenceSource> FileReferenceSource for Option<T> {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let Some(v) = self {
            v.collect_references(r);
        }
    }
}
impl<T: FileReferenceSource> FileReferenceSource for std::sync::Arc<T> {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        self.as_ref().collect_references(r);
    }
}
impl FileReferenceSource for tl::Photo {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::Photo::Photo(d) = self {
            r.data
                .entry(FileLocationId::Photo(d.id))
                .or_insert_with(|| d.file_reference.clone());
        }
    }
}
impl FileReferenceSource for tl::Document {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::Document::Document(d) = self {
            r.data
                .entry(FileLocationId::Document(d.id))
                .or_insert_with(|| d.file_reference.clone());
        }
    }
}
impl FileReferenceSource for tl::Page {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::Page::Page(d) = self;
        d.photos.collect_references(r);
        d.documents.collect_references(r);
    }
}
impl FileReferenceSource for tl::RichMessage {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::RichMessage::RichMessage(d) = self;
        d.photos.collect_references(r);
        d.documents.collect_references(r);
    }
}
impl FileReferenceSource for tl::WallPaper {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::WallPaper::WallPaper(d) = self {
            d.document.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::Theme {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::Theme::Theme(d) = self;
        d.document.collect_references(r);
    }
}
impl FileReferenceSource for tl::WebPageAttribute {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::WebPageAttribute::WebPageAttributeStory(d) => {
                d.story.collect_references(r);
            }
            tl::WebPageAttribute::WebPageAttributeTheme(d) => {
                d.documents.collect_references(r);
            }
            tl::WebPageAttribute::WebPageAttributeStickerSet(d) => {
                d.stickers.collect_references(r);
            }
            tl::WebPageAttribute::WebPageAttributeUniqueStarGift(d) => {
                d.gift.collect_references(r);
            }
            tl::WebPageAttribute::WebPageAttributeStarGiftCollection(d) => {
                d.icons.collect_references(r);
            }
            tl::WebPageAttribute::WebPageAttributeStarGiftAuction(d) => {
                d.gift.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::StarGift {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::StarGift::StarGift(d) => {
                d.sticker.collect_references(r);
            }
            tl::StarGift::StarGiftUnique(d) => {
                d.attributes.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::StarGiftAttribute {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::StarGiftAttribute::StarGiftAttributeModel(d) => {
                d.document.collect_references(r);
            }
            tl::StarGiftAttribute::StarGiftAttributePattern(d) => {
                d.document.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::WebPage {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::WebPage::WebPage(d) = self {
            d.document.collect_references(r);
            d.attributes.collect_references(r);
            d.photo.collect_references(r);
            d.cached_page.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::Game {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::Game::Game(d) = self;
        d.document.collect_references(r);
    }
}
impl FileReferenceSource for tl::MessageExtendedMedia {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessageExtendedMedia::MessageExtendedMedia(d) = self {
            d.media.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessageMedia {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::MessageMedia::MessageMediaPhoto(d) => {
                d.photo.collect_references(r);
            }
            tl::MessageMedia::MessageMediaDocument(d) => {
                d.document.collect_references(r);
                d.video_cover.collect_references(r);
                d.alt_documents.collect_references(r);
            }
            tl::MessageMedia::MessageMediaWebPage(d) => {
                d.webpage.collect_references(r);
            }
            tl::MessageMedia::MessageMediaGame(d) => {
                d.game.collect_references(r);
            }
            tl::MessageMedia::MessageMediaInvoice(d) => {
                d.extended_media.collect_references(r);
            }
            tl::MessageMedia::MessageMediaPaidMedia(d) => {
                d.extended_media.collect_references(r);
            }
            tl::MessageMedia::MessageMediaPoll(d) => {
                d.attached_media.collect_references(r);
                let tl::Poll::Poll(p) = &d.poll;
                for a in &p.answers {
                    if let tl::PollAnswer::PollAnswer(a) = a {
                        a.media.collect_references(r);
                    }
                }
                let tl::PollResults::PollResults(p) = &d.results;
                p.solution_media.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::MessageReplyHeader {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessageReplyHeader::MessageReplyHeader(d) = self {
            d.reply_media.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::DraftMessage {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::DraftMessage::DraftMessage(d) = self {
            d.rich_message.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::Dialog {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::Dialog::Dialog(d) = self {
            d.draft.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::Message {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::Message::Message(d) => {
                d.media.collect_references(r);
                d.reply_to.collect_references(r);
                d.rich_message.collect_references(r);
            }
            tl::Message::MessageService(d) => {
                d.action.collect_references(r);
                d.reply_to.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::MessageAction {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::MessageAction::MessageActionChatEditPhoto(d) => {
                d.photo.collect_references(r);
            }
            tl::MessageAction::MessageActionSuggestProfilePhoto(d) => {
                d.photo.collect_references(r);
            }
            tl::MessageAction::MessageActionSetChatWallPaper(d) => {
                d.wallpaper.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::StoryItem {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::StoryItem::StoryItem(d) = self {
            d.media.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesMessages {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::MessagesMessages::MessagesMessages(d) => {
                d.messages.collect_references(r);
            }
            tl::MessagesMessages::MessagesMessagesSlice(d) => {
                d.messages.collect_references(r);
            }
            tl::MessagesMessages::MessagesChannelMessages(d) => {
                d.messages.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::MessagesPeerDialogs {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::MessagesPeerDialogs::MessagesPeerDialogs(d) = self;
        d.messages.collect_references(r);
        d.dialogs.collect_references(r);
    }
}
impl FileReferenceSource for tl::PhotosPhotos {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::PhotosPhotos::PhotosPhotos(d) => {
                d.photos.collect_references(r);
            }
            tl::PhotosPhotos::PhotosPhotosSlice(d) => {
                d.photos.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::UsersUserFull {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::UsersUserFull::UsersUserFull(d) = self;
        let tl::UserFull::UserFull(d) = &d.full_user;
        d.personal_photo.collect_references(r);
        d.fallback_photo.collect_references(r);
        d.profile_photo.collect_references(r);
    }
}
impl FileReferenceSource for tl::ChatFull {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::ChatFull::ChatFull(d) => {
                d.chat_photo.collect_references(r);
            }
            tl::ChatFull::ChannelFull(d) => {
                d.chat_photo.collect_references(r);
            }
            tl::ChatFull::CommunityFull(d) => {
                d.chat_photo.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::MessagesChatFull {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::MessagesChatFull::MessagesChatFull(d) = self;
        d.full_chat.collect_references(r);
    }
}
impl FileReferenceSource for tl::ForumTopic {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::ForumTopic::ForumTopic(d) = self {
            d.draft.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesForumTopics {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::MessagesForumTopics::MessagesForumTopics(d) = self;
        d.messages.collect_references(r);
        d.topics.collect_references(r);
    }
}
impl FileReferenceSource for tl::MessagesRecentStickers {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessagesRecentStickers::MessagesRecentStickers(d) = self {
            d.stickers.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesFavedStickers {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessagesFavedStickers::MessagesFavedStickers(d) = self {
            d.stickers.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesStickerSet {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessagesStickerSet::MessagesStickerSet(d) = self {
            d.documents.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesSavedGifs {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::MessagesSavedGifs::MessagesSavedGifs(d) = self {
            d.gifs.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::SavedDialog {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::SavedDialog::MonoForumDialog(d) = self {
            d.draft.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::MessagesSavedDialogs {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        match self {
            tl::MessagesSavedDialogs::MessagesSavedDialogs(d) => {
                d.messages.collect_references(r);
                d.dialogs.collect_references(r);
            }
            tl::MessagesSavedDialogs::MessagesSavedDialogsSlice(d) => {
                d.messages.collect_references(r);
                d.dialogs.collect_references(r);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }
}
impl FileReferenceSource for tl::AccountSavedRingtones {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::AccountSavedRingtones::AccountSavedRingtones(d) = self {
            d.ringtones.collect_references(r);
        }
    }
}
impl FileReferenceSource for tl::HelpPremiumPromo {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::HelpPremiumPromo::HelpPremiumPromo(d) = self;
        d.videos.collect_references(r);
    }
}
impl FileReferenceSource for tl::MessagesWebPage {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::MessagesWebPage::MessagesWebPage(d) = self;
        d.webpage.collect_references(r);
    }
}
impl FileReferenceSource for tl::StoriesStories {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        let tl::StoriesStories::StoriesStories(d) = self;
        d.stories.collect_references(r);
    }
}
impl FileReferenceSource for tl::UsersSavedMusic {
    fn collect_references(&self, r: &mut UpdatedFileReferences) {
        if let tl::UsersSavedMusic::UsersSavedMusic(d) = self {
            d.documents.collect_references(r);
        }
    }
}
