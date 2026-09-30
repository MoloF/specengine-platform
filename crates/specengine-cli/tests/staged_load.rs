//! AC-11 of docs/features/spec-cli-staged.md: no `cat-file --batch`
//! deadlock. At least 3 000 small staged documents (at least 3 MiB in all)
//! plus one of 1 MiB, generated in a scratch repository: `--staged` exits
//! within 60 s under a watchdog that kills it and fails the test, and its
//! report is plain's.

#![cfg(unix)]

mod common;

use std::time::Duration;

use common::staged::{Repo, assert_same, spec_timed};

const SMALL: usize = 3_000;
const SMALL_BYTES: usize = 1_150;
const BIG_BYTES: usize = 1 << 20;
const WATCHDOG: Duration = Duration::from_secs(60);

/// A spec document of about `size` bytes, its body unique to `number`.
fn document(number: usize, size: usize) -> String {
    let mut text = format!(
        "---\nclass: spec\nstatus: draft\nscope: [docs/spec]\n---\n\n# Bulk {number:05}\n\n"
    );
    let line = format!("Line of bulk document {number:05} to fill the blob with text.\n");
    while text.len() + line.len() <= size {
        text.push_str(&line);
    }
    while text.len() < size {
        text.push('.');
    }
    text.push('\n');
    text
}

#[test]
fn thousands_of_documents_and_a_big_one_never_deadlock() {
    let repo = Repo::of("staged-load", "spec-a");
    let mut total = 0;
    for number in 0..SMALL {
        let text = document(number, SMALL_BYTES);
        total += text.len();
        common::write(
            &repo.top,
            &format!("docs/spec/bulk/{:02}/doc-{number:05}.md", number % 50),
            text,
        );
    }
    assert!(total >= 3 << 20, "{total} bytes of small documents");
    let big = document(SMALL, BIG_BYTES);
    assert!(big.len() >= BIG_BYTES);
    common::write(&repo.top, "docs/spec/bulk/big.md", big);
    repo.add_all();

    for args in [&["check", "--staged"][..], &["--json", "check", "--staged"]] {
        let staged = spec_timed(&repo.git, &repo.top, args, &[], &[], WATCHDOG)
            .unwrap_or_else(|problem| panic!("a deadlock: {problem}"));
        let plain_args: Vec<&str> = args
            .iter()
            .copied()
            .filter(|arg| *arg != "--staged")
            .collect();
        let plain = spec_timed(&repo.git, &repo.top, &plain_args, &[], &[], WATCHDOG * 2)
            .expect("plain finishes");
        assert_same(&staged, &plain, "a large staged tree");
    }
    let json = repo.staged_check(&["--json"]);
    let value: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    let documents = value["counts"]["documents"].as_u64().unwrap();
    assert!(documents > SMALL as u64, "{documents} documents walked");
}
