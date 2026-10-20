//! Rig: one locked impl, later given an unmarked twin.

/// A kite's rig.
pub struct Rig {
    pub lines: u32,
    pub span: f32,
}

// @implements KITE-RIG@1
impl Rig {
    pub fn reach(&self) -> f32 {
        self.span * self.lines as f32 * 0.5 + 1.0
    }

    pub fn load(&self, wind: f32) -> f32 {
        let per_line = wind * self.span / (self.lines as f32 + 1.0);
        per_line.min(80.0)
    }
}
