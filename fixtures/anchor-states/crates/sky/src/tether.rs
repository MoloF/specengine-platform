//! Tether: a binding by its place only (`[path]`).

/// The slack a tether keeps.
pub fn line_slack(length: f32) -> f32 {
    length * 0.0625
}

// @implements KITE-TETHER@1 [path]
pub fn line_tension(length: f32, wind: f32, weight: f32) -> f32 {
    let stretch = length * 0.03125 + weight;
    let pull = wind * wind * 0.0625 + stretch;
    let held = pull - line_slack(length);
    if held < 0.0 { 0.0 } else { held }
}
