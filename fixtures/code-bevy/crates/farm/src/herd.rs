//! The herd: its barn, its tractor, a ghost on the network, grazing and
//! the calves.

use bevy::prelude::*;

/// Where the herd sleeps.
#[derive(Debug)]
#[derive(Component)]
pub struct Barn;

/// The farm's tractor.
pub struct Tractor {
    pub fuel: u32,
}

impl Component for Tractor {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Mutable;
}

/// A ghost only the network shows.
#[cfg_attr(feature = "net", derive(Component))]
pub struct Ghost;

/// Lets the herd graze unless the weather is wet.
pub fn graze(In(wet): In<bool>) {
    let _ = wet;
}

/// A system that spawns a calf every `ticks` ticks.
pub fn spawn_every(ticks: u32) -> impl FnMut(Commands) {
    move |mut commands: Commands| {
        let _ = (ticks, &mut commands);
    }
}
