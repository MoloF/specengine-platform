//! One item with a parse error of another shape (macro punctuation, trap 2 of
//! 05 §5.2); the items around it hash normally because recovery is local.

pub fn intact_before() -> u8 {
    1
}

macro_rules! tilde {
    ($value:expr) => {
        $value ~ 1
    };
}

pub fn intact_after() -> u8 {
    2
}
