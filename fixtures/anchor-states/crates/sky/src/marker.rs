//! Spin: a marker moved to a neighbour of the same body.

// @implements KITE-MARK@1
pub fn spin_rate(turns: f32, time: f32) -> f32 {
    let per_second = turns / (time + 0.5);
    let wobble = (per_second * 0.25).sin() * 0.125;
    let steady = per_second - wobble;
    if steady < 0.0 { 0.0 } else { steady * 0.875 }
}

pub fn spin_decay(turns: f32, time: f32) -> f32 {
    let per_second = turns / (time + 0.5);
    let wobble = (per_second * 0.25).sin() * 0.125;
    let steady = per_second - wobble;
    if steady < 0.0 { 0.0 } else { steady * 0.875 }
}
