//! The farm: crops that grow on a clock, fields worked by hand, the news
//! the farm hears, and the plugin that wires them into the app.

pub mod clock;
pub mod crops;
pub mod fields;
pub mod herd;
#[macro_use]
pub mod news;
pub mod weather;

use bevy::prelude::*;

use crate::crops::{Barley, Rye, Wheat};
use crate::herd::{graze, spawn_every};
use crate::weather::sense;

/// The farm's plugin: the crops' tick, the weather's pipe, the clock, the
/// calves, the harvest news and the trampling observer.
pub struct FarmPlugin;

impl Plugin for FarmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (crops::tick::<Wheat>, crops::tick::<Barley>, crops::tick::<Rye>),
        );
        app.add_systems(
            Update,
            (sense.pipe(graze), crate::clock::run, spawn_every(3)),
        );
        app.add_message::<news::Harvest>();
        app.add_observer(|trample: On<news::Trample>| {
            let _ = trample;
        });
        register_news!(app);
    }
}
