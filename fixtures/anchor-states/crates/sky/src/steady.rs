//! Steady: a binding no scenario touches.

// @implements KITE-STEADY@1
pub fn hold_steady(height: f32, wind: f32, line: f32) -> f32 {
    let pull = wind * line * 0.125;
    let sag = (height - pull).max(0.0) * 0.5;
    let held = height - sag + pull * 0.25;
    if held < 0.0 { 0.0 } else { held.min(height * 2.0) }
}
