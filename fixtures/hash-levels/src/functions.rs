//! Free functions of a small arena game; `expected.json` edits them in memory, pair by pair.
use std::fmt::Write as _;
use std::collections::BTreeMap;

/// Adds the round bonus to a score.
pub fn tally_score(base: u32, bonus: u32) -> u32 { let total = base + bonus; clamp_score(total) }

pub fn clamp_score(value: u32) -> u32 { if value > 9_999 { 9_999 } else { value } }

#[inline]
pub fn copy_one(x: u8) -> u8 { x.wrapping_add(1) }
#[inline]
pub fn copy_two(x: u8) -> u8 { x.wrapping_add(1) }

#[doc = "Spawns one wave."]
pub fn spawn_wave(count: usize, board: &mut BTreeMap<u32, String>) -> usize {
    prepare_board(board);
    for slot in 0..count { board.insert(slot as u32, format!("enemy {}", label_of(slot))); }
    let names: Vec<String> = board.values().cloned().collect::<Vec<_>>();
    let _spare = Vec::<u8>::new();
    finish_wave(names.len())
}

pub fn buffer_of(data: [u8; capacity()]) -> usize { data.len() }

pub fn pick<T: Copy>(first: T, _second: T) -> T { first }

pub fn qualified() -> Score { <Score as Default>::default() }

pub fn with_closure(values: &[u32]) -> u32 { values.iter().map(|v| { boost(*v) }).sum() }

pub fn with_nested() -> u32 { fn inner() -> u32 { deep_call() } inner() }

pub fn banner() -> &'static str { "ARENA
GO" }

pub fn trivial_hp(hp: u32) -> u32 { hp.min(100) }

fn report(score: u32) -> String { let mut out = String::new(); write!(out, "{}", fmt_score(score)).unwrap(); out }

pub fn order_calls() { load_level(); start_music(); }

pub fn long_tick(state: &mut Vec<u32>, delta: u32) -> u32 {
    let mut total = 0;
    for value in state.iter_mut() { *value = value.saturating_add(delta); total += *value; }
    if total > 1_000 { state.retain(|v| *v < 500); }
    match state.len() { 0 => { return 0 } n => total / n as u32 }
}
