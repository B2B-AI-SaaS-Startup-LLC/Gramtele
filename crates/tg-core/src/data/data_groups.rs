//! Album membership with stable message identities. History facts are supplied
//! at the call boundary; refresh effects are applied by the real owner (18–20).
use super::data_store_keys::{MessageKey, ScopedKey, SessionScopeId};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub items: Vec<MessageKey>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupEffect {
    RequestItemViewRefresh(MessageKey),
    InvalidateChatListEntry(MessageKey),
}
pub struct Groups {
    scope: SessionScopeId,
    groups: BTreeMap<u64, Group>,
}
impl Groups {
    pub fn new(scope: SessionScopeId) -> Self {
        Self {
            scope,
            groups: BTreeMap::new(),
        }
    }
    pub fn is_grouped(group_id: u64, media_can_be_grouped: bool) -> bool {
        group_id != 0 && media_can_be_grouped
    }
    pub fn is_group_of_one(&self, group_id: u64) -> bool {
        group_id != 0
            && self
                .groups
                .get(&group_id)
                .is_some_and(|g| g.items.len() == 1)
    }
    pub fn register_message(
        &mut self,
        item: MessageKey,
        group_id: u64,
        can_group: bool,
        id: impl Fn(MessageKey) -> i64,
    ) -> Vec<GroupEffect> {
        assert_eq!(item.scope(), self.scope);
        if !Self::is_grouped(group_id, can_group) {
            return vec![];
        }
        let items = &mut self.groups.entry(group_id).or_default().items;
        if items.len() >= 10 {
            return vec![];
        }
        let p = Self::find_position_for_item(items, item, id);
        items.insert(p, item);
        if items.len() > 1 {
            Self::refresh_views(items)
        } else {
            vec![]
        }
    }
    pub fn unregister_message(&mut self, item: MessageKey, group_id: u64) -> Vec<GroupEffect> {
        assert_eq!(item.scope(), self.scope);
        if group_id == 0 {
            return vec![];
        }
        let Some(g) = self.groups.get_mut(&group_id) else {
            return vec![];
        };
        let n = g.items.len();
        g.items.retain(|i| *i != item);
        if n == g.items.len() {
            return vec![];
        }
        if g.items.is_empty() {
            self.groups.remove(&group_id);
            vec![]
        } else {
            Self::refresh_views(&g.items)
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn refresh_message(
        &mut self,
        item: MessageKey,
        group_id: u64,
        can_group: bool,
        regular_or_scheduled_or_uploading: bool,
        just_refresh_views: bool,
        id: impl Fn(MessageKey) -> i64,
    ) -> Vec<GroupEffect> {
        assert_eq!(item.scope(), self.scope);
        if !Self::is_grouped(group_id, can_group) {
            let mut e = self.unregister_message(item, group_id);
            e.push(GroupEffect::RequestItemViewRefresh(item));
            return e;
        }
        if !regular_or_scheduled_or_uploading {
            return vec![];
        }
        let Some(g) = self.groups.get_mut(&group_id) else {
            return self.register_message(item, group_id, can_group, id);
        };
        if just_refresh_views {
            return Self::refresh_views(&g.items);
        }
        let position = Self::find_position_for_item(&g.items, item, id);
        if let Some(current) = g.items.iter().position(|i| *i == item) {
            if position == current + 1 {
                return vec![];
            }
            if position > current + 1 {
                g.items[current..position].rotate_left(1)
            } else if position < current {
                g.items[position..=current].rotate_right(1)
            } else {
                panic!("Position of item in Groups::refreshMessage()")
            }
        } else {
            g.items.insert(position, item)
        } // Original refresh insertion has no ten-item cap.
        Self::refresh_views(&g.items)
    }
    pub fn find_position_for_item(
        items: &[MessageKey],
        item: MessageKey,
        id: impl Fn(MessageKey) -> i64,
    ) -> usize {
        let n = id(item);
        items.iter().position(|i| id(*i) > n).unwrap_or(items.len())
    }
    pub fn find(&self, group_id: u64) -> Option<&Group> {
        if group_id == 0 {
            return None;
        }
        self.groups.get(&group_id).filter(|g| g.items.len() > 1)
    }
    pub fn refresh_views(items: &[MessageKey]) -> Vec<GroupEffect> {
        items
            .iter()
            .flat_map(|i| {
                [
                    GroupEffect::RequestItemViewRefresh(*i),
                    GroupEffect::InvalidateChatListEntry(*i),
                ]
            })
            .collect()
    }
    pub fn find_item_to_edit(
        &self,
        item: MessageKey,
        group_id: u64,
        empty_text: impl Fn(MessageKey) -> bool,
    ) -> MessageKey {
        assert_eq!(item.scope(), self.scope);
        self.find(group_id).map_or(item, |g| {
            g.items
                .iter()
                .find(|i| !empty_text(**i))
                .copied()
                .unwrap_or(g.items[0])
        })
    }
}
