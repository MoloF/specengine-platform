//! Use lists of a small arena game, members out of rustfmt's order at file level, in function bodies and in inline modules (N6).
use crate::arena::rules::{RULE, Rule, Rule2, Rule10, rule};
use crate::arena::tiles::{
    self, Border as Edge, Tile,
    grid::*,
    path::{Hop, Jump, Step},
};
use crate::arena::{
    /* hangs high */ Anvil,
    Banner,
    Torch, // lights the hall
};
use crate::arena::{Floor, Gate, Wall};

/// Places one tile; its imports are local.
pub fn place_tile(slot: usize) -> usize {
    use crate::arena::tiles::{self, Corner as Rim, Slab, Tile};
    use crate::arena::{
        Floor::{Moss, Stone},
        Gate, Wall,
    };
    let width = tiles::width(slot);
    width + Rim::size() + Stone::cost() + Moss::cost() + Slab::cost()
}

pub fn short_local() -> u8 {
    use crate::arena::{Floor, Wall};
    Wall::height()
}

pub mod hall {
    use super::{Edge, Hop};
    use crate::arena::{
        Anvil, Banner, Torch,
        forge::{Bellows, Coal, Hammer},
    };

    pub fn light() {
        Torch::burn();
    }

    pub mod depths {
        use crate::arena::{
            Cave,
            depths::{
                lower::{Bat, Rat},
                upper::{Crow, Owl},
            },
        };
        pub fn descend() -> u8 {
            Bat::level()
        }
    }
}

impl Board {
    pub fn reset(&mut self) {
        use crate::arena::{
            Gate, Wall,
            tiles::{self, Tile},
        };
        self.clear();
    }
}
