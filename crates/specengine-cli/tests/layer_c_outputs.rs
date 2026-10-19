//! docs/features/layer-c-resolve.md R5 and R12 over a locked game
//! repository: `fixtures/verify-ecs` (an ECS-style crate: systems, a
//! plugin, RON tuning data), its four nodes accepted through
//! `verify_common::ecs`, every git process in the sandbox of
//! `common::git`, a scratch `HOME`.
//!
//! - R5 (`reads_mcp_check_gate_verify_never_spawn_layer_c`): a logging fake
//!   beside the copy of `spec` and another on `PATH` stay unstarted across
//!   every read (`health`, `unmapped`, `symbols`, `refs`, `bundle`, `show`,
//!   `search`, `tree`), `index` (and `--full`), `check`, the gate,
//!   `verify` (plain, `--staged`, `--changed`, `--tests` on a terminal),
//!   `lock accept` (on a terminal, answered no) and `propose drift`; then
//!   `spec resolve` starts the one beside, once. The MCP code tools:
//!   `crates/specengine-mcp/tests/mcp_layer_c.rs`; the daemon route and the
//!   one caller: `layer_c_scan.rs`.
//! - R12 (`outputs_equal_with_and_without_a_snapshot`): the same reads,
//!   `lock accept` and `verify` give exit, stdout and stderr byte for byte
//!   before and after a stored snapshot whose every row joins, and no
//!   output shows a moniker. (`verify_tests_*` green and `daemon_keys` at
//!   56 are their own binaries.)
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;
mod layer_c_common;
mod lock_common;
mod verify_common;

use std::path::PathBuf;

use common::Run;
use layer_c_common::{Fake, GENERATED, Lab, Reply, counts};
use lock_common::LockRepo;
use verify_common::{STAMINA, ecs};

/// verify-ecs's `[project] slug`.
const ECS_SLUG: &str = "verify-ecs";

/// The commands of both criteria run with stdin null from the root:
/// every read of the code and the spec, `check`, `verify` in its modes.
const READS: &[&[&str]] = &[
    &["health"],
    &["--json", "health"],
    &["unmapped"],
    &["--json", "unmapped"],
    &["symbols", "regen"],
    &["--json", "symbols", "regen"],
    &["symbols", "--node", "PERK-REGEN"],
    &["--json", "symbols", "--node", "PERK-REGEN"],
    &["symbols", "--file", STAMINA],
    &["refs", "PERK-REGEN"],
    &["--json", "refs", "PERK-REGEN"],
    &["bundle", "PERK-REGEN"],
    &["--json", "bundle", "PERK-REGEN"],
    &["show", "PERK-REGEN"],
    &["search", "stamina"],
    &["tree"],
    &["check"],
    &["--json", "check"],
    &["verify"],
    &["--json", "verify"],
    &["verify", "--staged"],
    &["verify", "--changed"],
];

/// R5's commands that write (the index, the queue) or gate, stdin null.
const OTHERS: &[&[&str]] = &[
    &["index"],
    &["index", "--full"],
    &["gate", "--worktree", ".", "--path", STAMINA],
    &["propose", "drift"],
];

/// The commands run on a terminal, answered no.
const ON_A_TERMINAL: &[&[&str]] = &[&["lock", "accept", "PERK-REGEN"], &["verify", "--tests"]];

/// A lab over `repo`: the copy of `spec` with its fake beside it, the
/// sandbox's variables and `PATH`, a second logging fake first on `PATH`.
fn lab_over(repo: &LockRepo) -> (Lab, Fake) {
    let path: Vec<PathBuf> =
        std::env::split_paths(repo.git.var("PATH").expect("the sandbox's PATH")).collect();
    let mut lab = Lab::over(
        &repo.scratch.join("lcr"),
        &repo.root,
        &repo.home,
        ECS_SLUG,
        repo.git.vars(),
        path,
    );
    let on_path = lab.extra_fake("on-path");
    let dir = on_path
        .program
        .parent()
        .expect("the fake's directory")
        .to_path_buf();
    lab.path.insert(1, dir);
    (lab, on_path)
}

/// Every command of R12, in order: the reads, then the terminal ones.
fn outputs(lab: &Lab) -> Vec<(String, Run)> {
    let mut out = Vec::new();
    for args in READS {
        out.push((args.join(" "), lab.run(args)));
    }
    for args in ON_A_TERMINAL {
        out.push((
            format!("{} (on a terminal)", args.join(" ")),
            lab.pty(&lab.root, args, "n"),
        ));
    }
    out
}

/// A reply to the tree's requested files from its own layer A rows, stored:
/// every row joins. The number of rows.
fn store_joined(lab: &Lab) -> u64 {
    lab.index();
    let files = lab.requested_files();
    assert!(!files.is_empty(), "verify-ecs has Rust files");
    let reply = Reply::from_index(&lab.db(), &files);
    let rows = u64::try_from(reply.item_count()).expect("a count");
    assert!(rows > 0, "rows to join");
    lab.fake.clear();
    lab.fake.script(&[], &reply.render());
    let run = lab.resolve_json();
    run.code(0);
    let found = counts(&run);
    assert_eq!(
        (found["joined"], found["unmatched"], found["c_only"]),
        (rows, 0, 0),
        "{}",
        run.show()
    );
    let stored = lab.layer_c();
    assert!(
        stored.contains("code_monikers\t") && stored.contains(GENERATED),
        "the snapshot holds the monikers:\n{stored}"
    );
    rows
}

/// R5. M: `symbols` spawns. No command but `resolve` starts
/// `specengine-ra`, beside `spec` or on `PATH`; `resolve` starts the one
/// beside, once.
#[test]
fn reads_mcp_check_gate_verify_never_spawn_layer_c() {
    let repo = ecs("lcr-never");
    let (lab, on_path) = lab_over(&repo);
    let mut ran = Vec::new();
    for args in READS.iter().chain(OTHERS) {
        let run = lab.run(args);
        ran.push(format!("spec {} -> exit {}", args.join(" "), run.code));
    }
    for args in ON_A_TERMINAL {
        let run = lab.pty(&lab.root, args, "n");
        ran.push(format!(
            "spec {} (on a terminal) -> exit {}",
            args.join(" "),
            run.code
        ));
    }
    assert!(
        lab.fake.invocations().is_empty() && on_path.invocations().is_empty(),
        "only `spec resolve` starts specengine-ra: beside {:?}, on PATH {:?}\nran:\n{}",
        lab.fake.invocations(),
        on_path.invocations(),
        ran.join("\n")
    );

    store_joined(&lab);
    assert_eq!(lab.fake.invocations().len(), 1, "resolve starts it once");
    assert!(on_path.invocations().is_empty(), "the one beside spec");
}

/// R12. M: a moniker leaking into a read. Each command's exit, stdout and
/// stderr are the same before and after a stored snapshot whose every
/// row joins; no output shows a moniker of it.
#[test]
fn outputs_equal_with_and_without_a_snapshot() {
    let repo = ecs("lcr-outputs");
    let (lab, _on_path) = lab_over(&repo);
    lab.index();
    let before = outputs(&lab);
    store_joined(&lab);
    let after = outputs(&lab);
    assert_eq!(before.len(), after.len());
    let mut differ = Vec::new();
    for ((args, was), (_, now)) in before.iter().zip(&after) {
        if (was.code, &was.stdout, &was.stderr) != (now.code, &now.stdout, &now.stderr) {
            differ.push(format!(
                "spec {args}:\n--- without\n{}\n--- with\n{}",
                was.show(),
                now.show()
            ));
        }
        assert!(
            !now.stdout.contains(GENERATED) && !now.stderr.contains(GENERATED),
            "spec {args} shows a moniker:\n{}",
            now.show()
        );
    }
    assert!(
        differ.is_empty(),
        "a snapshot changed {} output(s):\n{}",
        differ.len(),
        differ.join("\n")
    );
}
