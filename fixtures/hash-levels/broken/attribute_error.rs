//! An item whose only parse error sits in its attached attribute, and an intact neighbour.

#[derive(Debug,]
struct Torn;

pub fn intact_after() -> u8 {
    3
}
