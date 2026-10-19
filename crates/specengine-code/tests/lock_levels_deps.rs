//! docs/features/lock-levels.md "The empty set's `deps`" (Q24, S1): the
//! one public function this slice adds to `specengine-code`, the empty
//! set's `deps` digest a RON entry's `deps_hash` renders, checked against
//! the independent oracle of `levels_common` (BLAKE3 of `level_header() ‖
//! put("deps")`, no target), and against an item's `deps` with no call.

mod levels_common;

use levels_common::{deps_after_header, one, snapshot};

/// Q24: `empty_deps_digest()` is `level_header() ‖ put("deps")` over no
/// target, computed (the header carries the grammar's version and ABI),
/// never a pinned constant; the same value twice.
#[test]
fn empty_deps_digest_is_the_empty_sets_deps() {
    let digest = specengine_code::empty_deps_digest();
    assert_eq!(*digest.as_bytes(), deps_after_header(&[]));
    assert_eq!(specengine_code::empty_deps_digest(), digest);
}

/// Q24: an item calling nothing has that `deps`; one calling something
/// has another.
#[test]
fn an_item_without_a_call_has_the_empty_sets_deps() {
    let source = "pub fn quiet() -> u32 {\n    7\n}\n\npub fn loud() -> u32 {\n    quiet()\n}\n";
    let snap = snapshot(source);
    let empty = specengine_code::empty_deps_digest();
    assert_eq!(one(&snap, "function_item", "quiet").deps, empty);
    assert_ne!(one(&snap, "function_item", "loud").deps, empty);
}
