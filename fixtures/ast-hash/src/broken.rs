//! Two items whose parse errors differ only inside their `ERROR` regions
//! (trap 1 of 05 §5.2): tree-sitter wraps the stray literals in one `ERROR`
//! node each, and a naive `is_extra()` filter drops both regions and gives the
//! two functions one hash. Each must be `cannot_verify`, never hashed.

pub fn broken_one() {
    let x = 1 2;
}

pub fn broken_two() {
    let x = 1 2 3;
}

pub fn intact_neighbour() -> u8 {
    7
}
