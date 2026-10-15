//! Grammar-only forms: tree-sitter-rust parses them cleanly, rustc and rustfmt refuse them, so no rustfmt variant exists.
pub trait Tick {}

pub struct Clock;

impl Tick for Clock;

impl Clock where Clock: Sized;
