//! A macro whose body holds a token tree-sitter-rust cannot lex (`~`); the items around it
//! are intact because recovery is local.

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
