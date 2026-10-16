//! docs/features/code-tools.md (6b-1), the developer's iteration 1 and 2
//! (K1, `CODE_INDEX_FORMAT` 2): `file_place`'s package label rewrites `\`
//! to `/` only where `\` separates path components; on Unix it is a
//! character of a directory name and the label keeps it, as the CLI's
//! `dir_text` does. M: `package_label` rewriting `\` whatever
//! `MAIN_SEPARATOR` is.

#![cfg(unix)]

use std::path::Path;

use specengine_code::{FileRole, NamedFile, RustParser, analyze_file, file_place};

#[test]
fn file_place_keeps_a_backslash_of_a_unix_package_directory() {
    let mut parser = RustParser::new().expect("the pinned grammar loads");
    let analysis = analyze_file(&mut parser, "pub fn a() -> u32 {\n    1\n}\n")
        .expect("the parser gives a tree");
    for (package, path) in [
        ("crates/back\\slash", "crates/back\\slash/src/lib.rs"),
        ("crates/plain", "crates/plain/src/lib.rs"),
        ("we\\ird", "we\\ird/src/lib.rs"),
    ] {
        let file = NamedFile {
            path: Path::new(path),
            package: Some(Path::new(package)),
            analysis: &analysis,
        };
        let (label, role) = file_place(&file, None);
        assert_eq!(label, package, "{path}: the directory as written");
        assert_eq!(role, FileRole::Unrooted, "{path}: no target table given");
    }
}
