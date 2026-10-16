//! The news the farm hears: harvests, quakes, rain and trampling.

use bevy::prelude::*;

/// A harvest came in.
#[derive(Message, Debug, Clone)]
pub struct Harvest {
    pub bushels: u32,
}

/// The ground shook.
#[derive(Message)]
pub struct Quake;

/// Rain fell on the farm.
#[derive(Event)]
pub struct Rain;

/// Something trampled an entity.
#[derive(EntityEvent)]
pub struct Trample {
    pub entity: Entity,
}

/// Registers the quake news on an app.
macro_rules! register_news {
    ($app:expr) => {
        $app.add_message::<Quake>();
    };
}
