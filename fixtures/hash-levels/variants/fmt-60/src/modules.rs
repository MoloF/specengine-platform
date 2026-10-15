//! Module structure of the arena: inline modules, `mod` declarations, `extern crate` runs.
pub mod scoring {
    #![allow(dead_code)]
    pub fn bonus() -> u32 {
        base_bonus() + 5
    }
}

mod hub {
    mod alpha;
    mod zeta;
    #[macro_use]
    mod gamma;
    mod beta;
    extern crate core as aa_core;
    extern crate std as zz_std;
    pub fn wire() {
        connect_all();
    }
}

pub mod plain;
mod quiet;
