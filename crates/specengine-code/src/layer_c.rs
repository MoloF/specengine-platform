//! Layer C's words as data (`docs/features/layer-c.md` "Kind map"): which
//! layer A node kinds ([`crate::items::ITEM_KINDS`],
//! [`crate::items::MEMBER_KINDS`]) an item kind of `specengine-ra` (its
//! `ItemKind::as_str`, protocol v1's `kind`) stands for. Layer C's answers
//! join layer A's rows on `(start, end)` and this map, never on a line or a
//! name.
//!
//! Nothing here reads a moniker; the words are data of this crate and
//! `specengine-ra` only (the genre rule, `docs/canon/architecture.md#universal`).

/// Every layer C item kind with the layer A kinds it stands for, in
/// `specengine-ra`'s `ItemKind::ALL` order: `method` stands for a member
/// with a body or without one, `type_alias`, `mod` and both macro kinds are
/// named apart, every other kind `<kind>_item`.
pub const KIND_MAP: [(&str, &[&str]); 13] = [
    ("function", &["function_item"]),
    ("method", &["function_item", "function_signature_item"]),
    ("struct", &["struct_item"]),
    ("enum", &["enum_item"]),
    ("union", &["union_item"]),
    ("trait", &["trait_item"]),
    ("impl", &["impl_item"]),
    ("const", &["const_item"]),
    ("static", &["static_item"]),
    ("type_alias", &["type_item"]),
    ("mod", &["mod_item"]),
    ("macro_rules", &["macro_definition"]),
    ("macro_def", &["macro_definition"]),
];

/// The layer A kinds a layer C kind stands for: its [`KIND_MAP`] row, or
/// `None` for a word the map does not hold (see [`kind_matches`] for the
/// rule such a word follows).
pub fn layer_a_kinds(layer_c_kind: &str) -> Option<&'static [&'static str]> {
    KIND_MAP
        .iter()
        .find(|(kind, _)| *kind == layer_c_kind)
        .map(|(_, kinds)| *kinds)
}

/// Whether layer A's node kind `layer_a_kind` is one the layer C item kind
/// `layer_c_kind` stands for: a [`KIND_MAP`] row, else `<kind>_item`.
pub fn kind_matches(layer_c_kind: &str, layer_a_kind: &str) -> bool {
    match layer_a_kinds(layer_c_kind) {
        Some(kinds) => kinds.contains(&layer_a_kind),
        None => layer_a_kind.strip_suffix("_item") == Some(layer_c_kind),
    }
}
