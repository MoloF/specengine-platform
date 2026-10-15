//! Declarative macros of the arena.
macro_rules! square {
    ($x:expr) => {
        $x * $x
    };
}

#[macro_export]
macro_rules! clamp_to {
    ($v:expr, $hi:expr) => {
        if $v > $hi { $hi } else { $v }
    };
    ($v:expr) => {
        clamp_to!($v, 100)
    };
}
