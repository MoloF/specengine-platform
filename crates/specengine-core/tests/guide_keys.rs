//! docs/features/specengine-guide.md (RM-0129) GU8, core's half ("Data",
//! "Front-matter `summarises`"): `TYPED_KEYS` is 34, its last
//! `("summarises", KeyType::List)` after RM-0128's `disciplines`, every
//! earlier entry kept; `CHECK_CODES` is 41, its last `summary-stale` after
//! RM-0128's `practice-discipline` (the whole list is `check_output.rs`'s
//! pin). M: `summarises` untyped; the code missing or not last.

use specengine_core::check::CHECK_CODES;
use specengine_core::{KeyType, TYPED_KEYS};

/// `TYPED_KEYS` 34: `summarises`, a list of strings, appended last;
/// `disciplines` (RM-0128) before it, `tags` (RM-0127) before that.
#[test]
fn gu8_summarises_is_the_thirty_fourth_typed_key() {
    assert_eq!(TYPED_KEYS.len(), 34);
    assert_eq!(TYPED_KEYS[33], ("summarises", KeyType::List));
    let keys: Vec<&str> = TYPED_KEYS.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        keys.iter().filter(|key| **key == "summarises").count(),
        1,
        "once"
    );
    let disciplines = keys
        .iter()
        .position(|key| *key == "disciplines")
        .expect("RM-0128's disciplines");
    assert_eq!(disciplines, 32, "disciplines right before: {keys:?}");
    assert_eq!(&keys[..3], ["id", "kind", "class"], "the head kept");
}

/// `CHECK_CODES` 41: `summary-stale` last, once.
#[test]
fn gu8_summary_stale_is_the_forty_first_check_code() {
    assert_eq!(CHECK_CODES.len(), 41);
    assert_eq!(CHECK_CODES.last(), Some(&"summary-stale"));
    assert_eq!(CHECK_CODES[39], "practice-discipline");
    assert_eq!(
        CHECK_CODES
            .iter()
            .filter(|code| **code == "summary-stale")
            .count(),
        1
    );
}
