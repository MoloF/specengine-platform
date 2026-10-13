//! The game's systems and the plugin that registers them.
//! @implements PERK-STAMINA@2

pub mod stamina;

/// The app the plugin registers its systems in.
#[derive(Default)]
pub struct App {
    pub systems: Vec<&'static str>,
}

/// Registers the stamina systems.
pub struct StaminaPlugin;

impl StaminaPlugin {
    // @implements PERK-STAMINA@2
    pub fn build(&self, app: &mut App) {
        app.systems.push("regen_system");
        app.systems.push("drain_system");
    }
}
