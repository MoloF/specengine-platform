//! Lift: renamed within its file.

/// The rename scenario's neighbour.
pub const LIFT_SCALE: f32 = 0.5;

// @implements KITE-RENAME@1
/// Lift from the wind.
pub fn lift_force(wind: f32, area: f32, angle: f32) -> f32 {
    let pressure = LIFT_SCALE * wind * wind * area;
    let factor = (angle * 2.0).sin().abs() + 0.25;
    let raw = pressure * factor;
    if raw > 400.0 { 400.0 } else { raw }
}
