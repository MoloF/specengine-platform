//! Parsed only, never built: the four hand-listed registrations of AC-10 (05 §5.1).

use bevy::prelude::*;

macro_rules! register_tick {
    ($app:expr) => {
        $app.add_systems(Update, tick);
    };
}

fn tick() {}

fn on_spawn(_add: On<Add, Name>) {}

fn setup_plugin(app: &mut App) {
    register_tick!(app);
    app.add_observer(on_spawn);
}

struct MiniPlugin;

impl Plugin for MiniPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(setup_plugin);
    }
}

fn main() {
    App::new().add_plugins((DefaultPlugins, MiniPlugin)).run();
}
