//! Spool: where the tether's binding lands.

/// How much line a spool holds.
pub fn spool_hold(turns: u32) -> f32 {
    turns as f32 * 0.5
}
