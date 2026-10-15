//! Tuning constants and statics of the arena, two of them without a value (rustc refuses them later).
pub const MAX_HP: u32 = 100;
pub const LIMIT: usize = compute_limit(4);
pub const HELP: &str = "move: arrows
fire: space";
pub const WAVE_SIZES: [u32; 6] =
    [compute_limit(1), compute_limit(2), 30, 40, 50, 60];
static GREETING: &str = "ready";
pub static TABLE: [u8; 4] = make_table();
static mut COUNTER: u32 = 0;
pub const UNSET: u8;
pub static PENDING: u16;
