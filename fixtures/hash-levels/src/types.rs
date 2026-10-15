//! Data types of the arena: structs of three shapes, enums, a union, aliases.
use std::fmt::Debug;

/// Hit points of a fighter.
#[derive(Debug, Clone)]
pub struct Stamina { pub current: u32, pub max: u32 }

pub struct Pair(pub u8, pub u8);

#[derive(Default)]
pub struct Marker;

pub struct Slots<T> where T: Copy { items: [T; slot_count()], used: usize }

#[repr(u8)]
pub enum Phase { Idle, Running(u8), Done { at: u32 }, }

pub enum Code { First = first_code(), Second = 2 }

#[repr(C)]
pub union Bits { int: u32, float: f32, raw: [u8; byte_len()] }

pub type Grid = Vec<Vec<u8>>;

pub type Buffer = [u8; buffer_len()];
