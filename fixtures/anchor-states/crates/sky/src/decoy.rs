//! Echo: the lift's body under a node never accepted, so a pairing that
//! forgot the node would see two candidates.

use crate::rename::LIFT_SCALE;

// @implements KITE-DECOY@1
/// Lift from the wind.
pub fn lift_echo(wind: f32, area: f32, angle: f32) -> f32 {
    let pressure = LIFT_SCALE * wind * wind * area;
    let factor = (angle * 2.0).sin().abs() + 0.25;
    let raw = pressure * factor;
    if raw > 400.0 { 400.0 } else { raw }
}
