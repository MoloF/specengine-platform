//! The tally library: a count with a cap (GOAL-01).
//! @implements PIECE-COUNTER@3

// @implements GOAL-01
#![forbid(unsafe_code)]

// @implements LAW-STEP
#[path = "extra/moved.rs"]
mod moved;

// @implements other:GOAL-01
pub mod broken;
// @implements PR-7
pub mod counter;

// @implements ZZZ-1
// @implements GOAL-99@2
// @implements
pub use counter::Tally;

// @implements GOAL-01@1
