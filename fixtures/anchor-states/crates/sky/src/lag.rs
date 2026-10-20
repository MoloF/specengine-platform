//! Reel speed: its marker lags the node's revision.

// @implements KITE-LAG@1
pub fn reel_speed(length: f32, wind: f32) -> f32 {
    let base = length * 0.125 + 0.5;
    let boost = (wind * 0.25).min(4.0) * base;
    let speed = base + boost * 0.5;
    let eased = speed - base * 0.0625;
    if eased > 12.0 { 12.0 } else { eased }
}
