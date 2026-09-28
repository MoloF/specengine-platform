//! Generates `tuning.rs` into `OUT_DIR`: the `with` load of `ra` runs this
//! script (into the scratch target directory), the `without` load does not.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out = env::var("OUT_DIR").expect("OUT_DIR is set for build scripts");
    fs::write(
        Path::new(&out).join("tuning.rs"),
        "pub fn regen_rate() -> u32 { 3 }\n",
    )
    .expect("write tuning.rs");
    println!("cargo::rerun-if-changed=build.rs");
}
