//! docs/features/bevy-roles.md RD-7, the index half ("annotation only"; the
//! `symbols` half is `bevy-roles-reads.md`'s): a role never binds, maps a
//! unit, enters `spec.lock` or a verify row (ADR-0020, ADR-0031), so with
//! `frameworks` on and off `refs`, `unmapped`, `symbols` (10a-1: no role
//! line yet), `verify` and a scratch `lock accept`'s lock are byte for
//! byte alike, and `spec index` differs only by its `roles` line and
//! JSON `code.roles`.
//!
//! Two scratch git repositories of `fixtures/code-bevy`, each with a
//! `HOME` of its own: one as the fixture is (the opt-in on), one with a
//! second commit turning it off (`spec lock accept` reads only the root's
//! committed config: no `--config`, no uncommitted edit). What is compared
//! is masked: each scratch root, `HOME`, and each side's commit IDs (full
//! or abbreviated), which differ by that one commit.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;
mod lock_common;

use serde_json::Value;

use bevy_roles_common::{CODE_OFF, expected, mask_db, parse_roles_line};
use common::{read_text, write};
use lock_common::{LockRepo, Prompted};

/// One side of the comparison.
struct Side {
    repo: LockRepo,
    /// Commit IDs to mask (full; prefixes from 7 characters are masked
    /// too).
    commits: Vec<String>,
}

impl Side {
    /// `fixtures/code-bevy` committed once; with `off`, a second commit
    /// replacing its `[code]` by [`CODE_OFF`].
    fn new(label: &str, off: bool) -> Self {
        let repo = LockRepo::of(label, "code-bevy", "repo", &[]);
        let mut commits = vec![repo.head()];
        if off {
            let text = read_text(&repo.root, "specengine.toml");
            let at = text.find("\n[code]\n").expect("a [code] table") + 1;
            write(
                &repo.root,
                "specengine.toml",
                format!("{}{CODE_OFF}", &text[..at]),
            );
            repo.commit_all("frameworks off");
            commits.push(repo.head());
        }
        Self { repo, commits }
    }

    fn mask(&self, text: &str) -> String {
        let root = self.repo.root.to_str().expect("a UTF-8 root");
        let home = self.repo.home.to_str().expect("a UTF-8 home");
        let mut out = text.replace(root, "<root>").replace(home, "<home>");
        for commit in &self.commits {
            for len in (7..=commit.len()).rev() {
                out = out.replace(&commit[..len], "<commit>");
            }
        }
        out
    }

    /// `spec args` at the root (the sandbox's git, this side's `HOME`):
    /// exit, stdout and stderr, masked.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let run = self.repo.spec_piped(&self.repo.root, args);
        (run.code, self.mask(&run.stdout), self.mask(&run.stderr))
    }

    /// The library `lock accept REF` from the root at `lock_common`'s
    /// clock, the owner answering yes: one question, a commit; the
    /// prompts masked. The new commit joins the masked IDs.
    fn accept(&mut self, reference: &str) -> Vec<Prompted> {
        let attempt = self.repo.accept(reference, true);
        attempt.committed(&format!("accept {reference}"));
        let prompts = attempt
            .prompts
            .iter()
            .map(|prompt| Prompted {
                rows: self.mask(&prompt.rows),
                question: self.mask(&prompt.question),
            })
            .collect();
        self.commits.push(self.repo.head());
        prompts
    }
}

/// The reads compared: every `refs` form over the fixture's nodes, both
/// `unmapped` shapes, `symbols` by name and by file (10a-1 adds no role
/// line there), `verify`; text and JSON.
const READS: [&[&str]; 8] = [
    &["refs", "ALM-GROWTH"],
    &["refs", "ALM-FARM"],
    &["refs", "CHORE-01"],
    &["unmapped"],
    &["unmapped", "crates/farm/src"],
    &["symbols", "tick"],
    &["symbols", "--file", "crates/farm/src/lib.rs"],
    &["verify"],
];

fn compare_reads(on: &Side, off: &Side, when: &str) {
    for read_args in READS {
        for json in [false, true] {
            let mut args: Vec<&str> = Vec::new();
            if json {
                args.push("--json");
            }
            args.extend_from_slice(read_args);
            assert_eq!(on.run(&args), off.run(&args), "{when}: {args:?}");
        }
    }
}

/// RD-7. M: a role maps a unit or alters a row. code-bevy with the opt-in
/// on and off: every read of [`READS`] alike before and after a `lock
/// accept`, the two locks byte for byte alike (commit IDs masked), the
/// accepts' prompts alike; `spec index` alike but for the `roles` line
/// (on: `expected.json`'s) and JSON `code.roles` (on: the line's object;
/// off: `null`).
#[test]
fn roles_change_no_refs_unmapped_verify_or_lock_output() {
    let mut on = Side::new("br-out-on", false);
    let mut off = Side::new("br-out-off", true);

    let (code, text_on, stderr_on) = on.run(&["index"]);
    assert_eq!(code, 0, "{text_on}{stderr_on}");
    let (code, text_off, stderr_off) = off.run(&["index"]);
    assert_eq!(code, 0, "{text_off}{stderr_off}");
    let want = expected();
    let roles = want["index_roles_line"].as_str().expect("the line");
    assert!(
        text_on.lines().any(|line| line == roles),
        "on: the fixture's roles line\n{text_on}"
    );
    let without: String = text_on
        .lines()
        .filter(|line| *line != roles)
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(mask_db(&without), mask_db(&text_off), "index text");
    assert_eq!(stderr_on, stderr_off, "index stderr");

    let (_, json_on, _) = on.run(&["--json", "index"]);
    let (_, json_off, _) = off.run(&["--json", "index"]);
    let mut json_on: Value = serde_json::from_str(&json_on).expect("index JSON");
    let mut json_off: Value = serde_json::from_str(&json_off).expect("index JSON");
    assert_eq!(json_on["code"]["roles"], parse_roles_line(roles));
    assert!(json_off["code"]["roles"].is_null(), "{json_off}");
    for json in [&mut json_on, &mut json_off] {
        let object = json.as_object_mut().expect("an object");
        object.remove("db");
        object["code"]
            .as_object_mut()
            .expect("a code object")
            .remove("roles");
    }
    assert_eq!(json_on, json_off, "index JSON but `roles`");

    compare_reads(&on, &off, "before the accept");
    let prompts_on = on.accept("ALM-GROWTH");
    let prompts_off = off.accept("ALM-GROWTH");
    assert_eq!(prompts_on, prompts_off, "the accept's prompts");
    assert_eq!(
        on.mask(&read_text(&on.repo.root, "spec.lock")),
        off.mask(&read_text(&off.repo.root, "spec.lock")),
        "the locks"
    );
    compare_reads(&on, &off, "after the accept");
}
