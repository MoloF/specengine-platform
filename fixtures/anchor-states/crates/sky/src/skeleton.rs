//! Tail: renamed, a local renamed too.

// @implements KITE-SKETCH@1
pub fn tail_angle(length: f32, wind: f32, weight: f32) -> f32 {
    let spread = length * 0.25 + weight;
    let swing = (wind / (spread + 1.0)).atan();
    let settled = swing * 0.75 + spread * 0.0625;
    let damped = settled - (settled * weight * 0.03125).sin();
    damped.clamp(-1.5, 1.5)
}
