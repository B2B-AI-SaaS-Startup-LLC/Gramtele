//! Generic body of LocationPickers' weak registry. Actual Api::SendAction23 and
//! Ui::LocationPicker5 binding is explicitly partial; neither type is fabricated.
use std::rc::{Rc, Weak};
pub struct LocationPickers<A, P> {
    pickers: Vec<(A, Weak<P>)>,
}
impl<A, P> Default for LocationPickers<A, P> {
    fn default() -> Self {
        Self { pickers: vec![] }
    }
}
impl<A: PartialEq, P> LocationPickers<A, P> {
    pub fn lookup(&mut self, action: &A) -> Option<Rc<P>> {
        let mut i = 0;
        while i < self.pickers.len() {
            if let Some(p) = self.pickers[i].1.upgrade() {
                if &self.pickers[i].0 == action {
                    return Some(p);
                }
                i += 1
            } else {
                self.pickers.remove(i);
            }
        }
        None
    }
    pub fn emplace(&mut self, action: A, picker: &Rc<P>) {
        self.pickers.push((action, Rc::downgrade(picker)))
    }
}
