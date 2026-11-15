//! The door timer of the auto-mini fixture.

/// Seconds a door stays open after the player passes it.
pub const DOOR_SECONDS: u32 = 5;

/// Whether a door that opened `elapsed` seconds ago is still open.
pub fn door_open(elapsed: u32) -> bool {
    elapsed < DOOR_SECONDS
}
