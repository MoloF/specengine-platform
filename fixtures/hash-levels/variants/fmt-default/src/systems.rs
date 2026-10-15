//! ECS-style systems and the plugin that registers them; system names passed as values are not calls.
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (move_players, regen_stamina).chain())
            .add_systems(Startup, spawn_arena);
    }
}

pub fn move_players(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation += velocity.0 * time.delta_secs();
    }
}

pub fn regen_stamina(mut query: Query<&mut Stamina>) {
    for mut stamina in &mut query {
        stamina.current = (stamina.current + 1).min(stamina.max);
    }
}

pub fn spawn_arena(mut commands: Commands) {
    commands.spawn((Arena, Name::new("arena")));
}
