//! The crops and the tick that grows them.

use bevy::prelude::*;

/// What a crop has grown to.
// @implements CHORE-01@1
#[derive(Component, Reflect, Debug)]
pub struct Crop {
    pub growth: u32,
}

impl Crop {
    /// Ready to harvest.
    pub fn ripe(&self) -> bool {
        self.growth >= 3
    }
}

/// Wheat.
pub struct Wheat;

/// Barley.
pub struct Barley;

/// Rye.
pub struct Rye;

/// Grows every crop of kind `K` by one.
// @implements ALM-GROWTH@1
pub fn tick<K: Send + Sync + 'static>(mut crops: Query<&mut Crop>) {
    for mut crop in &mut crops {
        crop.growth += 1;
    }
}
