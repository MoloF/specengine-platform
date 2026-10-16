//! The fields, worked by the drip line or by hand.

use bevy::prelude::*;

/// Passes its tokens through unchanged.
macro_rules! quietly {
    ($($body:tt)*) => {
        $($body)*
    };
}

/// Runs the drip line.
#[cfg(feature = "drip")]
pub fn run() {}

/// Runs the furrows by hand.
#[cfg(not(feature = "drip"))]
pub fn run() {}

/// Wires the fields: a lamp that blinks, and the run of the day.
pub fn wire(app: &mut App) {
    fn blink() {}
    quietly! {
        app.add_systems(Update, blink);
    }
    app.add_systems(Update, run);
}
