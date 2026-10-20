//! Gusts: twins of one body and one quote.

// @implements KITE-CLONE@1
pub fn gust_left(speed: f32, scale: f32) -> f32 {
    let base = speed * scale + 0.5;
    let swirl = (base * 0.25).cos() * scale;
    let total = base + swirl.abs();
    if total > 10.0 { total - 10.0 } else { total * 2.0 + scale }
}
// The gust ends here.

// @implements KITE-CLONE@1
pub fn gust_right(speed: f32, scale: f32) -> f32 {
    let base = speed * scale + 0.5;
    let swirl = (base * 0.25).cos() * scale;
    let total = base + swirl.abs();
    if total > 10.0 { total - 10.0 } else { total * 2.0 + scale }
}
// The gust ends here.
