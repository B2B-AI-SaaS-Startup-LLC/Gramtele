//! Source SegmentTree including its 30-item split and pending-value helpers.
#[derive(Clone, Debug, Default)]
pub struct SegmentTree {
    array: Vec<i64>,
    heap: Vec<Node>,
}
#[derive(Clone, Debug, Default)]
struct Node {
    from: i32,
    to: i32,
    sum: i64,
    min: i64,
    max: i64,
    pending: Option<i64>,
}
impl Node {
    fn size(&self) -> i32 {
        self.to - self.from + 1
    }
}
impl SegmentTree {
    pub fn new(array: Vec<i64>) -> Self {
        let mut s = Self {
            array,
            heap: vec![],
        };
        if s.array.len() >= 30 {
            let size = 2. * 2f64.powf(((s.array.len() as f64).ln() / 2f64.ln() + 1.).floor());
            s.heap.resize(size as usize, Node::default());
            s.build(1, 0, s.array.len() as i32);
        }
        s
    }
    pub fn empty(&self) -> bool {
        self.array.is_empty()
    }
    fn build(&mut self, v: usize, from: i32, size: i32) {
        self.heap[v].from = from;
        self.heap[v].to = from + size - 1;
        if size == 1 {
            self.heap[v].min = self.array[from as usize];
            self.heap[v].max = self.array[from as usize];
            self.heap[v].sum = self.array[from as usize];
        } else {
            self.build(v * 2, from, size / 2);
            self.build(v * 2 + 1, from + size / 2, size - size / 2);
            self.heap[v].min = self.heap[v * 2].min.min(self.heap[v * 2 + 1].min);
            self.heap[v].max = self.heap[v * 2].max.max(self.heap[v * 2 + 1].max);
            self.heap[v].sum = self.heap[v * 2].sum.wrapping_add(self.heap[v * 2 + 1].sum);
        }
    }
    fn contains(from1: i32, to1: i32, from2: i32, to2: i32) -> bool {
        from2 >= from1 && to2 <= to1
    }
    fn intersects(from1: i32, to1: i32, from2: i32, to2: i32) -> bool {
        from1 <= from2 && to1 >= from2 || from1 >= from2 && from1 <= to2
    }
    fn query(&mut self, v: usize, from: i32, to: i32, min: bool) -> i64 {
        let n = &self.heap[v];
        if let Some(pending) = n.pending.filter(|_| Self::contains(n.from, n.to, from, to)) {
            return pending;
        }
        if Self::contains(from, to, n.from, n.to) {
            return if min { n.min } else { n.max };
        }
        if Self::intersects(from, to, n.from, n.to) {
            self.propagate(v);
            let a = self.query(v * 2, from, to, min);
            let b = self.query(v * 2 + 1, from, to, min);
            if min {
                a.min(b)
            } else {
                a.max(b)
            }
        } else if min {
            i64::MAX
        } else {
            0
        }
    }
    pub fn r_max_q(&mut self, from: i32, to: i32) -> i64 {
        if self.array.len() >= 30 {
            return self.query(1, from, to, false);
        }
        let mut r = 0;
        for i in from.max(0)..=to.min(self.array.len() as i32 - 1) {
            r = r.max(self.array[i as usize]);
        }
        r
    }
    pub fn r_min_q(&mut self, from: i32, to: i32) -> i64 {
        if self.array.len() >= 30 {
            return self.query(1, from, to, true);
        }
        let mut r = i64::MAX;
        for i in from.max(0)..=to.min(self.array.len() as i32 - 1) {
            r = r.min(self.array[i as usize]);
        }
        r
    }
    fn propagate(&mut self, v: usize) {
        if let Some(value) = self.heap[v].pending.take() {
            self.change(v * 2, value);
            self.change(v * 2 + 1, value);
        }
    }
    fn change(&mut self, v: usize, value: i64) {
        let n = &mut self.heap[v];
        n.pending = Some(value);
        n.sum = i64::from(n.size()).wrapping_mul(value);
        n.min = value;
        n.max = value;
        self.array[n.from as usize] = value;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_values_cover_propagate_and_preserve_source_first_slot_write() {
        let mut t = SegmentTree::new((0..30).collect());
        t.change(1, 7);
        assert_eq!(t.r_min_q(0, 29), 7);
        assert_eq!(t.r_max_q(10, 20), 7);
        assert_eq!(t.r_max_q(-1, 15), 7);
        assert!(t.heap[1].pending.is_none());
        assert_eq!(t.array[0], 7);
        assert_eq!(t.array[29], 29);
        assert!(!t.empty());
        assert!(SelfTest::bounds());
    }
    struct SelfTest;
    impl SelfTest {
        fn bounds() -> bool {
            SegmentTree::contains(0, 5, 1, 3)
                && SegmentTree::intersects(0, 3, 2, 5)
                && !SegmentTree::intersects(0, 1, 2, 3)
        }
    }
}
