mod common;

// A `#[path]` aimed into the shared dir: the target is named by its
// location (`shared:tests/common`), never flagged `path_attribute`.
#[path = "common/extra.rs"]
mod extra;

fn setup() {}

#[test]
fn single_file_test() {
    setup();
    common::shared_setup();
}
