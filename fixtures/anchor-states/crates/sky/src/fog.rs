//! Fog: renamed beside a parse error outside any item.

// @implements KITE-FOG@1
pub fn fog_density(depth: f32, light: f32) -> f32 {
    let haze = depth * 0.0625 + 0.25;
    let dimmed = light / (haze + 1.0);
    let seen = dimmed * 0.875 - haze * 0.125;
    if seen < 0.0 { 0.0 } else { seen }
}
