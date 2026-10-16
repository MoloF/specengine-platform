//! The weather over the farm, and what the farm senses of it.

use bevy::prelude::*;

/// Today's weather.
#[derive(bevy::prelude::Resource, bevy::reflect::Reflect, Default)]
pub struct Weather {
    pub wet: bool,
}

impl Weather {
    /// A dry day.
    pub fn clear() -> Self {
        Self { wet: false }
    }
}

/// Reads the sky: is it wet?
pub fn sense(weather: Res<Weather>) -> bool {
    weather.wet
}

/// How long the herd may graze in this weather: a struct named as the
/// system it limits.
#[allow(non_camel_case_types)]
pub struct graze {
    pub minutes: u32,
}
