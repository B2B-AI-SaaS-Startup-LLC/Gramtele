//! Gift values. Internal text01/question1, CreditsAmount(Claude) and
//! Ui::ColorCollectible5 are absent explicitly; see data-themes-map.md.
use super::{
    data_msg_id::MsgId, data_peer_colors::Color, data_peer_id::PeerId, data_store_keys::*,
};
use std::{collections::BTreeSet, rc::Rc};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueGiftRarity(pub i32);
impl UniqueGiftRarity {
    pub const DEFAULT: Self = Self(0);
    pub const UNCOMMON: Self = Self(-1);
    pub const RARE: Self = Self(-2);
    pub const EPIC: Self = Self(-3);
    pub const LEGENDARY: Self = Self(-4);
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniqueGiftAttribute {
    pub name: String,
    pub rarity_value: i32,
}
impl UniqueGiftAttribute {
    pub fn rarity_type(&self) -> UniqueGiftRarity {
        UniqueGiftRarity(self.rarity_value.min(0))
    }
    pub fn rarity_permille(&self) -> i32 {
        self.rarity_value.max(0)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueGiftModel {
    pub attribute: UniqueGiftAttribute,
    pub document: DocumentRecordKey,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueGiftPattern {
    pub attribute: UniqueGiftAttribute,
    pub document: DocumentRecordKey,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniqueGiftBackdrop {
    pub attribute: UniqueGiftAttribute,
    pub center_color: Color,
    pub edge_color: Color,
    pub pattern_color: Color,
    pub text_color: Color,
    pub id: i32,
}
#[derive(Clone, Debug, Default)]
pub struct UniqueGiftAttributes {
    pub models: Vec<UniqueGiftModel>,
    pub backdrops: Vec<UniqueGiftBackdrop>,
    pub patterns: Vec<UniqueGiftPattern>,
}
#[derive(Clone, Debug, Default)]
pub struct UniqueGiftOriginalDetails {
    pub sender_id: PeerId,
    pub recipient_id: PeerId,
    pub date: i32,
}
#[derive(Clone, Debug, Default)]
pub struct UniqueGiftValue {
    pub currency: String,
    pub value_price: i64,
    pub value_price_usd: i64,
    pub initial_sale_price: i64,
    pub initial_sale_date: i32,
    pub last_sale_price: i64,
    pub last_sale_date: i32,
    pub average_price: i64,
    pub minimum_price: i64,
    pub for_sale_on_telegram: i32,
    pub for_sale_on_fragment: i32,
    pub fragment_url: String,
    pub last_sale_fragment: bool,
}
#[derive(Clone, Debug)]
pub struct UniqueGift {
    pub id: u64,
    pub initial_gift_id: u64,
    pub slug: String,
    pub title: String,
    pub gift_address: String,
    pub owner_address: String,
    pub owner_name: String,
    pub owner_id: PeerId,
    pub host_id: PeerId,
    pub released_by: Option<PeerKey>,
    pub theme_user: Option<PeerKey>,
    pub nano_ton_for_resale: i64,
    pub craft_chance_permille: i32,
    pub stars_for_resale: i32,
    pub stars_for_transfer: i32,
    pub stars_min_offer: i32,
    pub number: i32,
    pub only_accept_ton: bool,
    pub can_be_theme: bool,
    pub crafted: bool,
    pub burned: bool,
    pub export_at: i32,
    pub can_transfer_at: i32,
    pub can_resell_at: i32,
    pub can_craft_at: i32,
    pub model: UniqueGiftModel,
    pub pattern: UniqueGiftPattern,
    pub backdrop: UniqueGiftBackdrop,
    pub original_details: UniqueGiftOriginalDetails,
    pub value: Option<Rc<UniqueGiftValue>>,
}
impl UniqueGift {
    pub fn new(
        id: u64,
        model: UniqueGiftModel,
        pattern: UniqueGiftPattern,
        backdrop: UniqueGiftBackdrop,
    ) -> Self {
        Self {
            id,
            initial_gift_id: 0,
            slug: Default::default(),
            title: Default::default(),
            gift_address: Default::default(),
            owner_address: Default::default(),
            owner_name: Default::default(),
            owner_id: PeerId::new(0),
            host_id: PeerId::new(0),
            released_by: Default::default(),
            theme_user: Default::default(),
            nano_ton_for_resale: -1,
            craft_chance_permille: 0,
            stars_for_resale: -1,
            stars_for_transfer: -1,
            stars_min_offer: -1,
            number: 0,
            only_accept_ton: false,
            can_be_theme: false,
            crafted: false,
            burned: false,
            export_at: 0,
            can_transfer_at: 0,
            can_resell_at: 0,
            can_craft_at: 0,
            model,
            pattern,
            backdrop,
            original_details: Default::default(),
            value: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct StarGiftBackground {
    pub center: Color,
    pub edge: Color,
    pub text: Color,
}
#[derive(Clone, Debug)]
pub struct StarGift {
    pub document: DocumentRecordKey,
    pub id: u64,
    pub unique: Option<Rc<UniqueGift>>,
    pub background: Option<Rc<StarGiftBackground>>,
    pub stars: i64,
    pub stars_converted: i64,
    pub stars_to_upgrade: i64,
    pub stars_resell_min: i64,
    pub released_by: Option<PeerKey>,
    pub resell_title: String,
    pub resell_count: i32,
    pub auction_slug: String,
    pub auction_gifts_per_round: i32,
    pub auction_start_date: i32,
    pub limited_left: i32,
    pub limited_count: i32,
    pub per_user_total: i32,
    pub per_user_remains: i32,
    pub upgrade_variants: i32,
    pub first_sale_date: i32,
    pub last_sale_date: i32,
    pub locked_until_date: i32,
    pub resell_ton_only: bool,
    pub require_premium: bool,
    pub peer_color_available: bool,
    pub upgradable: bool,
    pub birthday: bool,
    pub sold_out: bool,
}
impl StarGift {
    pub fn new(document: DocumentRecordKey) -> Self {
        Self {
            document,
            id: 0,
            unique: Default::default(),
            background: Default::default(),
            stars: 0,
            stars_converted: 0,
            stars_to_upgrade: 0,
            stars_resell_min: 0,
            released_by: Default::default(),
            resell_title: Default::default(),
            resell_count: 0,
            auction_slug: Default::default(),
            auction_gifts_per_round: 0,
            auction_start_date: 0,
            limited_left: 0,
            limited_count: 0,
            per_user_total: 0,
            per_user_remains: 0,
            upgrade_variants: 0,
            first_sale_date: 0,
            last_sale_date: 0,
            locked_until_date: 0,
            resell_ton_only: false,
            require_premium: false,
            peer_color_available: false,
            upgradable: false,
            birthday: false,
            sold_out: false,
        }
    }
    pub fn auction(&self) -> bool {
        !self.auction_slug.is_empty()
    }
}
impl PartialEq for StarGift {
    fn eq(&self, o: &Self) -> bool {
        self.document == o.document
            && self.id == o.id
            && match (&self.unique, &o.unique) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && match (&self.background, &o.background) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.stars == o.stars
            && self.stars_converted == o.stars_converted
            && self.stars_to_upgrade == o.stars_to_upgrade
            && self.stars_resell_min == o.stars_resell_min
            && self.released_by == o.released_by
            && self.resell_title == o.resell_title
            && self.resell_count == o.resell_count
            && self.auction_slug == o.auction_slug
            && self.auction_gifts_per_round == o.auction_gifts_per_round
            && self.auction_start_date == o.auction_start_date
            && self.limited_left == o.limited_left
            && self.limited_count == o.limited_count
            && self.per_user_total == o.per_user_total
            && self.per_user_remains == o.per_user_remains
            && self.upgrade_variants == o.upgrade_variants
            && self.first_sale_date == o.first_sale_date
            && self.last_sale_date == o.last_sale_date
            && self.locked_until_date == o.locked_until_date
            && self.resell_ton_only == o.resell_ton_only
            && self.require_premium == o.require_premium
            && self.peer_color_available == o.peer_color_available
            && self.upgradable == o.upgradable
            && self.birthday == o.birthday
            && self.sold_out == o.sold_out
    }
}
impl Eq for StarGift {}
#[derive(Clone, Debug)]
pub struct SavedStarGift {
    pub info: StarGift,
    pub manage_id: SavedStarGiftId,
    pub collection_ids: Vec<i32>,
    pub stars_converted: i64,
    pub stars_upgraded_by_sender: i64,
    pub stars_for_details_remove: i64,
    pub gift_prepay_upgrade_hash: String,
    pub from_id: PeerId,
    pub date: i32,
    pub gift_num: i32,
    pub upgrade_separate: bool,
    pub upgradable: bool,
    pub anonymous: bool,
    pub pinned: bool,
    pub hidden: bool,
    pub mine: bool,
}
impl SavedStarGift {
    pub fn new(info: StarGift, manage_id: SavedStarGiftId) -> Self {
        Self {
            info,
            manage_id,
            collection_ids: Default::default(),
            stars_converted: Default::default(),
            stars_upgraded_by_sender: Default::default(),
            stars_for_details_remove: Default::default(),
            gift_prepay_upgrade_hash: Default::default(),
            from_id: Default::default(),
            date: Default::default(),
            gift_num: Default::default(),
            upgrade_separate: Default::default(),
            upgradable: Default::default(),
            anonymous: Default::default(),
            pinned: Default::default(),
            hidden: Default::default(),
            mine: Default::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct GiftUpgradeResult {
    pub info: StarGift,
    pub manage_id: SavedStarGiftId,
    pub date: i32,
    pub stars_for_details_remove: i32,
    pub saved: bool,
}
#[derive(Clone, Debug, Default)]
pub struct GiftCollection {
    pub id: i32,
    pub count: i32,
    pub title: String,
    pub icon: Option<DocumentRecordKey>,
    pub hash: u64,
}
#[derive(Clone, Debug)]
pub struct UniqueGiftModelCount {
    pub model: UniqueGiftModel,
    pub count: i32,
}
#[derive(Clone, Debug)]
pub struct UniqueGiftBackdropCount {
    pub backdrop: UniqueGiftBackdrop,
    pub count: i32,
}
#[derive(Clone, Debug)]
pub struct UniqueGiftPatternCount {
    pub pattern: UniqueGiftPattern,
    pub count: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct GiftAttributeId {
    pub value: u64,
    pub type_: GiftAttributeIdType,
}
#[derive(Clone, Debug, Default)]
pub struct MyGiftsDescriptor {
    pub list: Vec<SavedStarGift>,
    pub offset: String,
}
#[derive(Clone, Debug, Default)]
pub struct ResaleGiftsDescriptor {
    pub gift_id: u64,
    pub title: String,
    pub offset: String,
    pub list: Vec<StarGift>,
    pub models: Vec<UniqueGiftModelCount>,
    pub backdrops: Vec<UniqueGiftBackdropCount>,
    pub patterns: Vec<UniqueGiftPatternCount>,
    pub attributes_hash: u64,
    pub count: i32,
    pub sort: ResaleGiftsSort,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResaleGiftsFilter {
    pub attributes_hash: u64,
    pub attributes: BTreeSet<GiftAttributeId>,
    pub sort: ResaleGiftsSort,
    pub for_craft: bool,
    pub stars_only: bool,
}
#[derive(Clone, Debug, Default)]
pub struct CraftGiftsDescriptor {
    pub gift_id: u64,
    pub offset: String,
    pub list: Vec<SavedStarGift>,
    pub count: i32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SavedStarGiftId {
    peer: Option<PeerKey>,
    entity_id: u64,
}
impl SavedStarGiftId {
    pub fn user(message_id: MsgId) -> Self {
        Self {
            peer: None,
            entity_id: message_id.bare as u64,
        }
    }
    pub fn chat(peer: PeerKey, saved_id: u64) -> Self {
        Self {
            peer: Some(peer),
            entity_id: saved_id,
        }
    }
    pub fn is_user(self) -> bool {
        self.peer.is_none()
    }
    pub fn is_chat(self) -> bool {
        self.peer.is_some()
    }
    pub fn user_message_id(self) -> MsgId {
        MsgId::new(if self.peer.is_none() {
            self.entity_id as i64
        } else {
            0
        })
    }
    pub fn chat_peer(self) -> Option<PeerKey> {
        self.peer
    }
    pub fn chat_saved_id(self) -> u64 {
        if self.peer.is_some() {
            self.entity_id
        } else {
            0
        }
    }
    pub fn is_present(self) -> bool {
        self.entity_id != 0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResaleGiftsSort {
    #[default]
    Date,
    Price,
    Number,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum GiftAttributeIdType {
    #[default]
    Model,
    Pattern,
    Backdrop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyUniqueType {
    OwnedAndHosted,
    OnlyOwned,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GiftUpgradeState {
    #[default]
    Initial,
    Preparing,
    Loading,
    Prepared,
    Started,
    FinishBackdrop,
    FinishedBackdrop,
    FinishPattern,
    FinishedPattern,
    FinishModel,
    FinishedModel,
    Finished,
    Timeout,
}
pub struct GiftUpgradeSpinner {
    pub attributes: UniqueGiftAttributes,
    pub target: Option<Rc<UniqueGift>>,
    pub state: tg_rpl::Variable<GiftUpgradeState>,
}
impl Default for GiftUpgradeSpinner {
    fn default() -> Self {
        Self {
            attributes: Default::default(),
            target: None,
            state: tg_rpl::Variable::new(GiftUpgradeState::Initial),
        }
    }
}
impl StarGiftBackground {
    pub fn backdrop(&self) -> UniqueGiftBackdrop {
        UniqueGiftBackdrop {
            center_color: self.center,
            edge_color: self.edge,
            pattern_color: self.edge,
            text_color: self.text,
            ..Default::default()
        }
    }
}
pub fn unique_gift_attribute_has_special_rarity(a: &UniqueGiftAttribute) -> bool {
    a.rarity_type() != UniqueGiftRarity::DEFAULT
}
pub fn attribute_to_tl(id: GiftAttributeId) -> tg_scheme::StarGiftAttributeId {
    match id.type_ {
        GiftAttributeIdType::Backdrop => {
            tg_scheme::mtp_star_gift_attribute_id_backdrop(id.value as u32 as i32)
        }
        GiftAttributeIdType::Model => tg_scheme::mtp_star_gift_attribute_id_model(id.value),
        GiftAttributeIdType::Pattern => tg_scheme::mtp_star_gift_attribute_id_pattern(id.value),
    }
}
pub fn attribute_from_tl(id: &tg_scheme::StarGiftAttributeId) -> GiftAttributeId {
    use tg_scheme::StarGiftAttributeId::*;
    match id {
        StarGiftAttributeIdBackdrop(d) => GiftAttributeId {
            value: d.backdrop_id as u32 as u64,
            type_: GiftAttributeIdType::Backdrop,
        },
        StarGiftAttributeIdModel(d) => GiftAttributeId {
            value: d.document_id,
            type_: GiftAttributeIdType::Model,
        },
        StarGiftAttributeIdPattern(d) => GiftAttributeId {
            value: d.document_id,
            type_: GiftAttributeIdType::Pattern,
        },
    }
}
pub fn id_for_backdrop(v: &UniqueGiftBackdrop) -> GiftAttributeId {
    GiftAttributeId {
        value: v.id as u32 as u64,
        type_: GiftAttributeIdType::Backdrop,
    }
}
pub fn id_for_model(
    v: &UniqueGiftModel,
    wire_id: impl FnOnce(DocumentRecordKey) -> u64,
) -> GiftAttributeId {
    GiftAttributeId {
        value: wire_id(v.document),
        type_: GiftAttributeIdType::Model,
    }
}
pub fn id_for_pattern(
    v: &UniqueGiftPattern,
    wire_id: impl FnOnce(DocumentRecordKey) -> u64,
) -> GiftAttributeId {
    GiftAttributeId {
        value: wire_id(v.document),
        type_: GiftAttributeIdType::Pattern,
    }
}
impl Default for ResaleGiftsFilter {
    fn default() -> Self {
        Self {
            attributes_hash: 0,
            attributes: Default::default(),
            sort: ResaleGiftsSort::Price,
            for_craft: false,
            stars_only: false,
        }
    }
}
/// Pure Api::InputSavedStarGiftId conversion; no CreditsAmount dependency.
pub fn input_saved_star_gift_id(
    id: SavedStarGiftId,
    input: impl FnOnce(PeerKey) -> tg_scheme::InputPeer,
) -> tg_scheme::InputSavedStarGift {
    if let Some(p) = id.chat_peer() {
        tg_scheme::mtp_input_saved_star_gift_chat(input(p), id.chat_saved_id())
    } else {
        tg_scheme::mtp_input_saved_star_gift_user(id.user_message_id().bare as i32)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UniqueGiftRarityColors {
    pub bg: Color,
    pub fg: Color,
}
pub fn unique_gift_rarity_badge_colors(rarity: UniqueGiftRarity) -> UniqueGiftRarityColors {
    let rgb = match rarity.0 {
        -1 => 0x40a920,
        -2 => 0x11aabe,
        -3 => 0x955cdb,
        -4 => 0xbf7600,
        _ => panic!("Invalid rarity type"),
    };
    UniqueGiftRarityColors {
        fg: Color::rgb(rgb),
        bg: Color::from_rgba((rgb << 8) | 38),
    }
}
/// Lang::FormatCountDecimal is supplied by its genuine owner01.
pub fn unique_gift_name(
    title: &str,
    number: i32,
    format_count: impl FnOnce(i32) -> String,
) -> String {
    format!("{title} #{}", format_count(number))
}
/// Request-value phases of the three slices. Actual Api::FromTL(saved gifts)
/// requires unresolved internal text/credits and remains24/01, without a fake parser.
pub fn my_unique_gifts_request(
    user: tg_scheme::InputPeer,
    kind: MyUniqueType,
    offset: String,
) -> tg_scheme::PaymentsGetSavedStarGifts {
    use tg_scheme::PaymentsGetSavedStarGifts as R;
    let mut flags = R::F_EXCLUDE_UPGRADABLE | R::F_EXCLUDE_UNUPGRADABLE | R::F_EXCLUDE_UNLIMITED;
    if kind == MyUniqueType::OnlyOwned {
        flags |= R::F_EXCLUDE_HOSTED
    }
    R::new(flags, user, None, offset.into_bytes(), 50)
}
pub fn resale_gifts_request(
    id: u64,
    filter: &ResaleGiftsFilter,
    offset: String,
) -> tg_scheme::PaymentsGetResaleStarGifts {
    use tg_scheme::PaymentsGetResaleStarGifts as R;
    let mut flags = R::F_ATTRIBUTES_HASH;
    if filter.sort == ResaleGiftsSort::Price {
        flags |= R::F_SORT_BY_PRICE
    } else if filter.sort == ResaleGiftsSort::Number {
        flags |= R::F_SORT_BY_NUM
    }
    if !filter.attributes.is_empty() {
        flags |= R::F_ATTRIBUTES
    }
    if filter.for_craft {
        flags |= R::F_FOR_CRAFT
    }
    if filter.stars_only {
        flags |= R::F_STARS_ONLY
    }
    R::new(
        flags,
        Some(filter.attributes_hash),
        id,
        (!filter.attributes.is_empty()).then(|| {
            filter
                .attributes
                .iter()
                .cloned()
                .map(attribute_to_tl)
                .collect()
        }),
        offset.into_bytes(),
        50,
    )
}
pub fn craft_gifts_request(id: u64, offset: String) -> tg_scheme::PaymentsGetCraftStarGifts {
    tg_scheme::PaymentsGetCraftStarGifts::new(id, offset.into_bytes(), 50)
}

/// Pure counter merge from ResaleGiftsSlice. Parsing attribute bodies still
/// belongs to Api::FromTL24 (credits/internal entities); duplicate ids keep first.
pub fn resale_attribute_counts(
    counters: &[tg_scheme::StarGiftAttributeCounter],
) -> std::collections::BTreeMap<GiftAttributeId, i32> {
    let mut result = std::collections::BTreeMap::new();
    for tg_scheme::StarGiftAttributeCounter::StarGiftAttributeCounter(c) in counters {
        result
            .entry(attribute_from_tl(&c.attribute))
            .or_insert(c.count);
    }
    result
}
impl ResaleGiftsDescriptor {
    /// Receives actual parsed attribute values, never placeholder documents.
    pub fn set_attributes(
        &mut self,
        attributes: UniqueGiftAttributes,
        counters: &[tg_scheme::StarGiftAttributeCounter],
        document_id: impl Fn(DocumentRecordKey) -> u64,
    ) {
        let counts = resale_attribute_counts(counters);
        let count = |id: GiftAttributeId| counts.get(&id).copied().unwrap_or_default();
        self.models = attributes
            .models
            .into_iter()
            .map(|value| {
                let count = count(id_for_model(&value, &document_id));
                UniqueGiftModelCount {
                    model: value,
                    count,
                }
            })
            .collect();
        self.patterns = attributes
            .patterns
            .into_iter()
            .map(|value| {
                let count = count(id_for_pattern(&value, &document_id));
                UniqueGiftPatternCount {
                    pattern: value,
                    count,
                }
            })
            .collect();
        self.backdrops = attributes
            .backdrops
            .into_iter()
            .map(|value| {
                let count = count(id_for_backdrop(&value));
                UniqueGiftBackdropCount {
                    backdrop: value,
                    count,
                }
            })
            .collect();
    }
}
