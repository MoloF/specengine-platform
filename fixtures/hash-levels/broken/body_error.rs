//! Two functions whose parse errors differ only inside their `ERROR` regions (trap 1:
//! an `is_extra()` filter gives both one digest), and an intact neighbour.

pub fn broken_one() {
    let x = 1 2;
}

pub fn broken_two() {
    let x = 1 2 3;
}

pub fn intact_neighbour() -> u8 {
    7
}
