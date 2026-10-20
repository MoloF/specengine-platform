//! Drag: moved to another file, this file kept.

/// Drag's coefficient.
pub const DRAG_SCALE: f32 = 0.75;

// @implements KITE-CARRY@1
pub fn drag_force(wind: f32, area: f32, tail: f32) -> f32 {
    let base = DRAG_SCALE * wind * area;
    let extra = tail * tail * 0.0625 + base * 0.125;
    let total = base + extra;
    if total > 250.0 { 250.0 } else { total.max(0.0) }
}
