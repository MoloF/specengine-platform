//! Fixture corpus for the `ast-hash` measurement (recipe: 05 §5.2; AC-03, AC-04).
//! Parsed only, never built.

pub mod blocks;
pub mod broken;
pub mod comments;
pub mod macro_error;
pub mod trailing;
pub mod unformatted;

#[path = "vendored/relocated.rs"]
pub mod relocated;

pub const ANSWER: u32 = 42;

pub static GREETING: &str = "// not a comment, a string";

pub type Pair = (u32, u32);

pub fn entry(a: u32, b: u32) -> u32 {
    a + b
}

pub mod nested {
    pub fn inner() -> u8 {
        1
    }

    pub mod deeper {
        pub fn deepest() -> u8 {
            2
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::entry(1, 2), 3);
    }
}
