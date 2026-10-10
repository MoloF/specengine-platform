//! An item with a parse error beside an intact one.
use std::convert::identity;

// @implements GOAL-01@1
pub fn broken() -> u32 {
    let x = 1 2;
    identity(x)
}

// GOAL-77 names no node.
// @implements LAW-STEP@1
pub fn intact() -> u32 {
    7
}
