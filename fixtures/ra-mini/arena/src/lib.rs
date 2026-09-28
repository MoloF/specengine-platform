//! A tiny ECS-flavoured crate: a component, a trait its systems use, and one
//! item of every kind the `ra` scan counts.

include!(concat!(env!("OUT_DIR"), "/tuning.rs"));

use arena_derive::Label;

pub mod systems;

/// Hit points of an entity.
#[derive(Label, Clone, Copy)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    pub fn fraction(&self) -> f32 {
        self.current as f32 / self.max.max(1) as f32
    }
}

impl Health {
    pub fn full(max: u32) -> Health {
        Health { current: max, max }
    }
}

pub trait Tick {
    fn tick(&mut self, dt: f32);
    fn tick_twice(&mut self, dt: f32) {
        self.tick(dt);
        self.tick(dt);
    }
}

pub enum Phase {
    Setup,
    Running,
}

pub const MAX_ENTITIES: usize = 64;
pub static TITLE: &str = "arena";
pub type Roster = Vec<Health>;

macro_rules! doubled {
    ($e:expr) => {
        $e * 2
    };
}

pub fn doubled_regen() -> u32 {
    doubled!(regen_rate())
}

mod internal {
    pub fn seed() -> u64 {
        fn mix() -> u64 {
            17
        }
        mix()
    }
}

pub fn spawn_seed() -> u64 {
    internal::seed()
}

#[cfg(target_os = "windows")]
pub fn windows_overlay() {}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_every_component() {
        assert_eq!(super::Health::full(3).label(), "Health");
    }
}
