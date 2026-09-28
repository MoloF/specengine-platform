use crate::{Health, Tick};

pub struct Regeneration {
    pub per_second: f32,
}

impl Tick for Health {
    fn tick(&mut self, _dt: f32) {
        if self.current < self.max {
            self.current += 1;
        }
    }
}

pub fn regenerate(roster: &mut [Health]) {
    for health in roster {
        health.tick(1.0);
    }
}
