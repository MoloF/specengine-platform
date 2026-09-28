//! Inner doc comment on the module; `///`, `//!`, `/** */` and `/* */` in every
//! position the comment-stripping perturbation must survive.

/// Doc comment on a struct.
#[derive(Debug, Default)]
// Line comment between the attribute and the item: the attribute stays attached.
pub struct Counter {
    /// The current value.
    pub value: u64, // trailing line comment
    /* block comment before a field */
    pub step: u64,
}

impl Counter {
    /** Block doc comment on a method. */
    pub fn tick(&mut self) {
        // advance by one step
        self.value += self.step; /* inline block comment */
    }

    pub fn reset(&mut self) {
        self.value = /* zero */ 0; // back to zero
    }
}

/// A trait with a doc comment.
pub trait Named {
    /// Required method.
    fn name(&self) -> &str;

    /// Provided method.
    fn shout(&self) -> String {
        // uppercase
        self.name().to_uppercase()
    }
}

impl Named for Counter {
    fn name(&self) -> &str {
        "counter" // the name
    }
}

/* A block comment
   spanning several lines
   before a constant. */
pub const COMMENTED: &str = "/* not a comment */ // also not a comment";
