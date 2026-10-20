//! Reel: two entries of one body, then only one binding of it.

// @implements KITE-CONTEST@1
pub fn reel_low(line: f32, drag: f32) -> f32 {
    let pulled = line * 0.375 - drag;
    let slack = (drag * 0.5).max(0.0) + 0.125;
    let rate = pulled / (slack + 1.0);
    if rate < 0.0 { 0.0 } else { rate.min(line) }
}

// @implements KITE-CONTEST@1
pub fn reel_high(line: f64, drag: f64) -> f64 {
    let pulled = line * 0.375 - drag;
    let slack = (drag * 0.5).max(0.0) + 0.125;
    let rate = pulled / (slack + 1.0);
    if rate < 0.0 { 0.0 } else { rate.min(line) }
}

/// The contest ends here.
pub const CONTEST_END: u8 = 1;
