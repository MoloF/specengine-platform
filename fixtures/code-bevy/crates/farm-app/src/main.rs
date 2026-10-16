//! The farm's binary: the default plugins, then the farm wired by hand.

use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    configure(&mut app);
    app.run();
}

/// Adds the default plugins and wires the farm.
fn configure(app: &mut App) {
    app.add_plugins(DefaultPlugins);
    farm::FarmPlugin.build(app);
    farm::fields::wire(app);
}
