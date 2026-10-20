//! Pull: twins of one body, their quotes apart (their types differ).

// @implements KITE-PAIR@1
pub fn pull_north(line: f32, gust: f32) -> f32 {
    let taut = line * 0.875 + gust;
    let give = (gust - line).abs() * 0.125;
    let net = taut - give;
    let held = net.max(0.0) + give * 0.5;
    if held > line * 2.0 { line * 2.0 } else { held }
}

// @implements KITE-PAIR@1
pub fn pull_south(line: f64, gust: f64) -> f64 {
    let taut = line * 0.875 + gust;
    let give = (gust - line).abs() * 0.125;
    let net = taut - give;
    let held = net.max(0.0) + give * 0.5;
    if held > line * 2.0 { line * 2.0 } else { held }
}
