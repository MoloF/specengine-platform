//! The count and its cap.
//! @implements PIECE-COUNTER@2

// @implements GOAL-01 see LAW-STEP
use std::fmt;

/// The largest count; see LAW-BOUND.
pub const CAP: u32 = 10; // @configures LAW-BOUND@2

/// Shown next to a count.
pub const LABEL: &str = "GOAL-01 // @implements LAW-STEP";

// @implements PIECE-COUNTER@3 [sig, body]

/// A count that never passes the cap (AIM-01).
#[derive(Debug, Clone, Default)]
pub struct Tally {
    // @implements LAW-STEP
    count: u32,
}

impl Tally { // @implements LAW-BOUND@3
    /* @implements LAW-STEP@1 */ pub fn step(&mut self) {
        // keeps LAW-BOUND
        let next = self.count + 1; // @implements GOAL-01
        self.count = next.min(CAP);
    }

    // @implements AIM-01@1 [body] mutation: return the cap
    pub fn value(&self) -> u32 {
        self.count
    }

    // @implements GOAL-01@
    pub const ZERO: u32 = 0;

    // @assumes PREM-001
}

impl Tally {
    // @implements GOAL-01@1234567890
    pub fn reset(&mut self) {
        self.count = 0;
    }
} // @implements LAW-BOUND@3a

impl fmt::Display for Tally {
    // @implements PIECE-COUNTER#LAW-BOUND@2
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.count)
    }
}

/// A bound on counts.
pub trait Limit {
    // @implements LAW-ODD@1
    fn max(&self) -> u32;
}

pub enum Size {
    // @implements LAW-BOUND
    One,
    Many(u32),
}

macro_rules! twice {
    // @implements GOAL-01
    ($e:expr) => {
        $e + $e
    };
}

pub mod inner {
    //! @implements LAW-STEP@2
    use super::CAP;

    // @verifies GOAL-01 [path, path]
    pub fn limit() -> u32 {
        CAP
    }
}

pub fn total(items: &[u32]) -> u32 {
    // @implements PIECE-COUNTER@3
    fn add(a: u32, b: u32) -> u32 {
        a + b
    }
    items.iter().copied().fold(0, add)
}

// The end of the count; GOAL-01 again.
