//! Container-parametric sparse snapshot, preserving vector/deque ordering.
use std::collections::VecDeque;
use tg_base::flat_set::FlatSet;
pub trait IdsContainer {
    type Id: Copy + Eq;
    fn size(&self) -> usize;
    fn item(&self, index: usize) -> Option<&Self::Id>;
}
impl<T: Copy + Eq> IdsContainer for Vec<T> {
    type Id = T;
    fn size(&self) -> usize {
        self.len()
    }
    fn item(&self, index: usize) -> Option<&T> {
        self.get(index)
    }
}
impl<T: Copy + Eq> IdsContainer for VecDeque<T> {
    type Id = T;
    fn size(&self) -> usize {
        self.len()
    }
    fn item(&self, index: usize) -> Option<&T> {
        self.get(index)
    }
}
impl<T: Copy + Ord> IdsContainer for FlatSet<T> {
    type Id = T;
    fn size(&self) -> usize {
        self.len()
    }
    fn item(&self, index: usize) -> Option<&T> {
        self.get_at(index)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AbstractSparseIds<C> {
    ids: C,
    full_count: Option<i32>,
    skipped_before: Option<i32>,
    skipped_after: Option<i32>,
}
impl<C: IdsContainer> AbstractSparseIds<C> {
    pub fn new(
        ids: C,
        full_count: Option<i32>,
        skipped_before: Option<i32>,
        skipped_after: Option<i32>,
    ) -> Self {
        Self {
            ids,
            full_count,
            skipped_before,
            skipped_after,
        }
    }
    pub fn full_count(&self) -> Option<i32> {
        self.full_count
    }
    pub fn skipped_before(&self) -> Option<i32> {
        self.skipped_before
    }
    pub fn skipped_after(&self) -> Option<i32> {
        self.skipped_after
    }
    pub fn size(&self) -> usize {
        self.ids.size()
    }
    pub fn index_of(&self, id: C::Id) -> Option<i32> {
        (0..self.size())
            .find(|&i| self.ids.item(i) == Some(&id))
            .map(|i| i as i32)
    }
    pub fn at(&self, index: usize) -> C::Id {
        *self.ids.item(index).expect("sparse index out of bounds")
    }
    pub fn distance(&self, a: C::Id, b: C::Id) -> Option<i32> {
        Some(self.index_of(b)? - self.index_of(a)?)
    }
}
impl<T: Copy + Eq> AbstractSparseIds<Vec<T>> {
    pub fn reverse(&mut self) {
        self.ids.reverse();
        std::mem::swap(&mut self.skipped_before, &mut self.skipped_after);
    }
}
impl<T: Copy + Eq> AbstractSparseIds<VecDeque<T>> {
    pub fn reverse(&mut self) {
        self.ids.make_contiguous().reverse();
        std::mem::swap(&mut self.skipped_before, &mut self.skipped_after);
    }
}
impl<T: Copy + Ord> AbstractSparseIds<FlatSet<T>> {
    pub fn nearest(&self, id: T) -> Option<T> {
        self.ids
            .get_at(self.ids.lower_bound(&id))
            .or_else(|| self.ids.back())
            .copied()
    }
}
