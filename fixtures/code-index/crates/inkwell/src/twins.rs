//! Twins: items of one name in one file (TENET-ORDER).

// @implements TENET-ORDER@1
#[cfg(unix)]
pub fn separator() -> char {
    '/'
}

/// The separator elsewhere.
#[cfg(not(unix))]
pub fn separator() -> char {
    '\\'
}

/// A sheet with two inherent impls.
pub struct Sheet;

impl Sheet {
    pub fn width(&self) -> u32 {
        80
    }
}

// @implements TENET-WIDTH@2
impl Sheet {
    pub fn height(&self) -> u32 {
        24
    }
}

#[cfg(test)]
mod probe {
    //! The probe under test (SLAB-PROBE).

    // @implements SLAB-PROBE@1
    pub fn run() -> u32 {
        1
    }
}

#[cfg(not(test))]
mod probe {
    //! The probe outside tests.
    //! @implements SLAB-PROBE@1
    pub fn run() -> u32 {
        0
    }
}
