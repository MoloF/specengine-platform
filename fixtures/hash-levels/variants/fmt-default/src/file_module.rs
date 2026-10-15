#![allow(dead_code)]
// The arena's entry module: its own levels are the file module's.
#![warn(missing_docs)]
//! Entry point of the arena.

extern crate alloc;
extern crate core;
mod alpha_systems;
mod zeta_systems;
#[macro_use]
mod gamma_macros;
mod beta_systems;

use std::collections::BTreeMap;
use std::fmt;

pub const START_LEVEL: u32 = first_level();

pub fn run() {
    boot_engine();
    fmt::write_marker();
}

pub fn levels() -> BTreeMap<u32, u32> {
    BTreeMap::new()
}
