//! The build script of `specengine-http` (docs/features/ui-embed.md "Data"):
//! std only, it writes no file and runs no process.
//!
//! `rust-embed` makes cargo track only the files it embedded, and builds,
//! tests and `cargo install --path` reuse one target directory, so a file
//! added to or removed from `ui/dist` would keep a stale web UI: cargo scans
//! the whole directory instead (while it is missing, every build reruns this
//! and recompiles the crate). A daemon compiled without the page warns once,
//! here; it answers `/` with the not-built line at run time.

use std::path::Path;

/// The one warning of a build without the page.
const NOT_BUILT: &str = "ui/dist holds no index.html: this specengine-http serves no web UI; \
                         build it first: pnpm --dir ui install --frozen-lockfile && pnpm --dir \
                         ui build, then build this crate again";

fn main() {
    println!("cargo::rerun-if-changed=../../ui/dist");
    // Cargo sets it for every build script; unset, the path is the package
    // directory's own (a build script runs there).
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default();
    if !Path::new(&manifest)
        .join("../../ui/dist/index.html")
        .is_file()
    {
        println!("cargo::warning={NOT_BUILT}");
    }
}
