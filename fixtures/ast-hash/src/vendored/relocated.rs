//! Reached only through `#[path = "vendored/relocated.rs"]` in `lib.rs`: the
//! file path says nothing about the module path (`Ambiguity::PathAttribute`).

pub fn relocated_helper() -> &'static str {
    "relocated"
}

pub struct Relocated {
    pub id: u32,
}
