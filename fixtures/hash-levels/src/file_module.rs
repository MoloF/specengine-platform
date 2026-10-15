#![allow(dead_code)]
// The arena's entry module: its own levels are the file module's.
#![warn(missing_docs)]
//! Entry point of the arena.

extern crate core;
extern crate alloc;
mod zeta_systems;
mod alpha_systems;
#[macro_use]
mod gamma_macros;
mod beta_systems;

use std::fmt;
use std::collections::BTreeMap;

pub const START_LEVEL: u32 = first_level();

pub fn run() { boot_engine(); fmt::write_marker(); }

pub fn levels() -> BTreeMap<u32, u32> { BTreeMap::new() }
