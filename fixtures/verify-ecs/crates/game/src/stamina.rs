//! Stamina: regen at rest, drain while sprinting.

/// A runner's stamina.
pub struct Stamina {
    pub current: f32,
    pub max: f32,
}

// @implements PERK-REGEN@1
pub fn regen_system(stamina: &mut Stamina, rate: f32, dt: f32) {
    // Rest restores a fixed amount per second.
    stamina.current = (stamina.current + rate * dt).min(stamina.max);
}

// @implements PERK-DRAIN
pub fn drain_system(stamina: &mut Stamina, cost: f32, dt: f32) {
    stamina.current = (stamina.current - cost * dt).max(0.0);
}

// @configures TUNE-01@1
pub const BASE_REGEN: f32 = 2.5;

/// Seconds a runner rests before regen starts.
pub const REST_DELAY: f32 = 0.5;
