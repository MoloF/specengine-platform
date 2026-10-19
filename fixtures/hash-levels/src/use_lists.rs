//! Use lists of a small arena game, members out of rustfmt's order at file level, in function bodies and in inline modules (N6).
use crate::arena::{Wall, Gate, Floor};
use crate::arena::tiles::{self, Tile, grid::*, Border as Edge, path::{Step, Hop, Jump}};
use crate::arena::{
    Torch, // lights the hall
    Banner, /* hangs high */ Anvil,
};
use crate::arena::rules::{Rule10, Rule2, rule, Rule, RULE};

/// Places one tile; its imports are local.
pub fn place_tile(slot: usize) -> usize {
    use crate::arena::tiles::{Tile, Corner as Rim, self, Slab};
    use crate::arena::{Gate, Floor::{Stone, Moss}, Wall};
    let width = tiles::width(slot);
    width + Rim::size() + Stone::cost() + Moss::cost() + Slab::cost()
}

pub fn short_local() -> u8 { use crate::arena::{Wall, Floor}; Wall::height() }

pub mod hall {
    use crate::arena::{Torch, Banner, Anvil, forge::{Hammer, Coal, Bellows}};
    use super::{Edge, Hop};

    pub fn light() { Torch::burn(); }

    pub mod depths {
        use crate::arena::{depths::{lower::{Rat, Bat}, upper::{Owl, Crow}}, Cave};
        pub fn descend() -> u8 { Bat::level() }
    }
}

impl Board {
    pub fn reset(&mut self) {
        use crate::arena::{Wall, Gate, tiles::{Tile, self}};
        self.clear();
    }
}
