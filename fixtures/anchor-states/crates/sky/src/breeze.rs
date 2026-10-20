//! Breeze: bound to a record whose ID becomes an alias.

// @implements WISP-01@1
pub fn breeze_push(mass: f32, breeze: f32) -> f32 {
    let force = breeze * breeze * 0.5;
    let shove = force / (mass + 0.25);
    let damped = shove * 0.875 - mass * 0.0625;
    if damped < 0.0 { 0.0 } else { damped }
}
