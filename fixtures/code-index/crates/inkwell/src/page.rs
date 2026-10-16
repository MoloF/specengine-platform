//! Pages and how they print (QUILL-01).

use std::fmt;

/// A page of marks; see NIB-01 once nibs have an ID.
#[derive(Debug, Clone, Default)]
pub struct Page {
    marks: Vec<u32>,
}

// @implements QUILL-01@1 [sig, body]
impl std::fmt::Display for Page {
    // @implements TENET-WIDTH@2
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.marks.len())
    }
}

impl Page {
    /// The sum of the marks.
    pub fn total(&self) -> u32 {
        fn add(a: u32, b: u32) -> u32 {
            a + b
        }
        self.marks.iter().copied().fold(0, add)
    }
}

/// Its signature passes the display cap (QUILL-01).
#[allow(clippy::too_many_arguments)]
pub fn wide(
    first_argument_with_a_rather_long_name: u32,
    second_argument_with_a_rather_long_name: u32,
    third_argument_with_a_rather_long_name: u32,
    fourth_argument_with_a_rather_long_name: u32,
    fifth_argument_with_a_rather_long_name: u32,
    sixth_argument_with_a_rather_long_name: u32,
    seventh_argument_with_a_rather_long_name: u32,
    eighth_argument_with_a_rather_long_name: u32,
    ninth_argument_with_a_rather_long_name: u32,
    tenth_argument_with_a_rather_long_name: u32,
    eleventh_argument_with_a_rather_long_name: u32,
    twelfth_argument_with_a_rather_long_name: u32,
) -> u32 {
    first_argument_with_a_rather_long_name + twelfth_argument_with_a_rather_long_name
}
