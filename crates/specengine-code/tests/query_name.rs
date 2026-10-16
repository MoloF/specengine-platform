//! docs/features/symbol-reads.md RD-1, Q27: `query_name` (`naming.rs`),
//! the one reader of `::` in a query, gives the `name` tier's key: the text
//! after the last `::` outside `<>`, `()`, `[]`; a segment starting `impl `
//! runs to the end; a trailing ` #<value>` (a discriminator) is dropped.
//! M: a string split (the last `::` wherever it sits) reads
//! `<T as a::Tr>`, a `cfg` value holding `::` or ` #`, and an impl of a
//! path type wrong.
//!
//! The fixture oracle (every stored qpath and suffix of
//! `fixtures/code-index` names its stored `name`) is in the CLI's
//! `tests/symbol_reads.rs`: this crate has no JSON reader, and its
//! dependencies stay as they are.
//!
//! The entry: `specengine_code::naming::query_name(&str) -> &str`.

use specengine_code::naming::query_name;

/// `(query, its name)`.
const CASES: [(&str, &str); 16] = [
    ("wide", "wide"),
    ("crates/inkwell::bin:stamp::main", "main"),
    (
        "crates/inkwell::page::<Page as std::fmt::Display>::fmt",
        "fmt",
    ),
    ("<Page as std::fmt::Display>::fmt", "fmt"),
    (
        "elsewhere::impl <Page as std::fmt::Display>",
        "impl <Page as std::fmt::Display>",
    ),
    (
        "impl <Page as std::fmt::Display>",
        "impl <Page as std::fmt::Display>",
    ),
    ("impl Sheet #2", "impl Sheet"),
    ("crates/inkwell::twins::impl Sheet #2", "impl Sheet"),
    ("probe::run #cfg(not(test))", "run"),
    ("x::run #cfg(feature=\"a::b\")", "run"),
    ("x::run #cfg(feature = \"a::b\")", "run"),
    ("x::run #cfg(feature=\"a #b\")", "run"),
    ("a::<[u8] as b::Tr>::f", "f"),
    ("m::impl dyn a::Tr", "impl dyn a::Tr"),
    ("m::impl a::B #1", "impl a::B"),
    ("pkg::a #1::f", "f"),
];

#[test]
fn query_name_reads_brackets_impl_and_discriminator() {
    let mut failures = Vec::new();
    for (query, want) in CASES {
        let got = query_name(query);
        if got != want {
            failures.push(format!("{query:?}: {got:?}, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
