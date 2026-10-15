---
class: canon
tier: 2
scope: [crates/specengine-eval, crates/specengine-code]
owner: owner
reviewed: 2026-10-15
---

# Layer A targets: cargo runner, layout fallback

Where a file's Cargo targets come from before its unit is judged (`docs/canon/code-identity.md` "Units and `qpath`"): the eval's `ast-hash` asks Cargo, hardened, and falls back to Cargo's layout; the CLI's code walk takes the layout alone (`docs/canon/code-bindings.md` "The walk"). Code: `specengine-eval` `ast_hash/targets.rs`, `ast_hash/fmt.rs`; `specengine-code` `qpath.rs` (`layout_targets`). The runner's limits: `code-identity.md` "Known limits", "Open".

## Target table

`ast-hash` runs, per package dir (a `Cargo.toml` dir owning a listed `.rs` file), shortest first, skipping dirs an earlier run covered (its packages, its `workspace_root`):

    ${SPECENGINE_CARGO:-cargo} metadata --format-version 1 --no-deps --offline --color never --manifest-path <dir>/Cargo.toml

An answer gives `PackageTargets { source: Metadata, targets: [Target { kind, name, root }] }`: Cargo's first kind and name, the root relative to the canonical package dir (outside it: dropped); a covered virtual root has no targets. A dir no run covers (cargo missing, a broken manifest, a nested workspace, no answer) gets `layout_targets(package, files)`, Cargo's auto-discovery (`src/lib.rs` → `lib` `<package>` with `-`→`_`, `src/main.rs` → `bin` `<package>`, `src/bin/<n>.rs`, `src/bin/<n>/main.rs` → `bin` `<n>`, alike under `examples`, `tests`, `benches`, `build.rs` → `custom-build` `build-script-build`), `<package>` = `[package] name` if the manifest parses, else the dir name. Exit 0 either way, `detail.targets_from` tells which. `spec refs` (no cargo): layout for all; a manifest parsing without `[package]` is virtual (no targets: `Unrooted`), one unreadable, unparsable or nameless takes the dir name, `.` for the root (`.::bin:.::main`, deterministic).

- **Never builds, patches or runs a corpus**: `--no-deps` writes no lock file, `--offline` stays off the network, stdin null.
- **cwd always `/`**, root-owned, for cargo and rustfmt (`ast_hash/fmt.rs`): no corpus `rust-toolchain` or `.cargo/config.toml` is read via the cwd; never a world-writable dir (a planted `rust-toolchain.toml` makes the rustup proxy run a foreign toolchain). rustfmt is picked by `RUSTUP_TOOLCHAIN` or a `SPECENGINE_RUSTFMT` wrapper, a relative path (`bin/fmt`) resolved against the harness's cwd. A corpus at `/` → no cargo call, layout for all; rustfmt not run, its rows `null`, one stderr line.
- **Budget** per call: min(60 s, the remaining `--timeout` − 1 s); nothing left → no call. An overrun is killed and reaped and, like a cargo that cannot start, ends the run's cargo calls: layout for the rest. No cargo child outlives the run.
- **stderr** captured, never on stdout; one harness line per failure: cargo's first `error…` line (else first non-blank, else the exit status), the corpus root as a whole path replaced by the label, cut to 200 chars.
