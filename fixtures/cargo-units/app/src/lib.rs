//! The library: the primary target of `unit-app`, named without a unit.

pub mod a;

#[path = "vendored/relocated.rs"]
pub mod relocated;

pub struct Pair;

// Two adjacent inherent impls of one type: the one deliberate `duplicate`.
impl Pair {
    pub fn left(&self) {}
}

impl Pair {
    pub fn right(&self) {}
}
