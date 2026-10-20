//! Gauge: two locked twins of one node, one skeleton, two bodies.

/// A wind gauge.
pub struct Gauge {
    pub reading: f32,
}

// @implements KITE-GAUGE@1
impl Gauge {
    pub fn low(&self) -> f32 {
        let scaled = self.reading * 0.25 + 1.0;
        let floor = scaled.max(0.5) - 0.125;
        floor * 0.75
    }
}

// @implements KITE-GAUGE@1
impl Gauge {
    pub fn high(&self) -> f32 {
        let scaled = self.reading * 0.75 + 2.0;
        let floor = scaled.max(1.5) - 0.375;
        floor * 1.25
    }
}
