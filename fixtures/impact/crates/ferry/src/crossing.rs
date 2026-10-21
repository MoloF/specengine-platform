//! Crossings: the tide window and its slack.

/// One crossing of the strait.
pub struct Crossing {
    pub open: u32,
    pub close: u32,
}

// @implements TIDE-WINDOW
impl Crossing {
    pub fn opens(&self) -> u32 {
        self.open
    }

    pub fn closes(&self) -> u32 {
        self.close
    }
}

pub mod slack {
    //! @implements TIDE-SLACK

    pub fn margin() -> u32 {
        5
    }

    pub mod deep {
        pub fn reach() -> u32 {
            super::margin() + 1
        }
    }
}

// Waits at ROUTE-DOCKS between crossings.
pub fn dock_time() -> u32 {
    3
}

// @implements TIDE-NOPE
pub fn drifting() -> u32 {
    0
}
