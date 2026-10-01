//! AC-15 of docs/features/spec-cli-introduced.md: no `cat-file --batch`
//! deadlock with a base. At least 3 000 committed documents (at least
//! 3 MiB in all) plus one of 1 MiB at `HEAD`, generated in a scratch
//! repository; then 10 of them changed and staged (5 with a new error),
//! and, so that `HEAD`'s side alone would overflow both pipes if its
//! requests were written before its replies were read, every one changed.
//! `--staged` exits within 60 s under a watchdog that kills it and fails
//! the test, with the expected report.

#![cfg(unix)]

mod common;

use std::time::Duration;

use common::staged::{Repo, spec_timed};

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

fn path(number: usize) -> String {
    format!("docs/spec/bulk/{:02}/doc-{number:05}.md", number % 50)
}

/// spec-a in `enforce-introduced` with the bulk and the big document,
/// committed.
fn committed_bulk(label: &str) -> Repo {
    let repo = Repo::of(label, "spec-a");
    let config = common::read_text(&repo.top, "specengine.toml");
    common::write(
        &repo.top,
        "specengine.toml",
        format!("{config}\n[check]\nmode = \"enforce-introduced\"\n"),
    );
    let mut total = 0;
    for number in 0..SMALL {
        let text = document(number, SMALL_BYTES);
        total += text.len();
        common::write(&repo.top, &path(number), text);
    }
    assert!(total >= 3 << 20, "{total} bytes of small documents");
    let big = document(SMALL, BIG_BYTES);
    assert!(big.len() >= BIG_BYTES);
    common::write(&repo.top, "docs/spec/bulk/big.md", big);
    repo.add_all();
    repo.git.commit(&repo.top, "the bulk");
    repo
}

fn staged_run(repo: &Repo, json: bool) -> common::Run {
    let args: &[&str] = if json {
        &["--json", "check", "--staged"]
    } else {
        &["check", "--staged"]
    };
    spec_timed(&repo.git, &repo.top, args, &[], &[], WATCHDOG)
        .unwrap_or_else(|problem| panic!("a deadlock: {problem}"))
}

/// Ten documents changed and staged, the big one among them; five gain a
/// dangling reference: exit 1, exactly those five `error` lines,
/// `counts.introduced` 5, every document walked.
#[test]
fn ten_changed_documents_over_a_large_head_never_deadlock() {
    let repo = committed_bulk("intro-load-ten");
    let mut expected = Vec::new();
    for number in (0..SMALL).step_by(SMALL / 9).take(9) {
        let mut text = document(number, SMALL_BYTES);
        if expected.len() < 5 {
            text = text.replacen(
                "scope: [docs/spec]\n",
                "scope: [docs/spec]\nrefs: [R-77]\n",
                1,
            );
            expected.push(format!(
                "error  {}:5: ref-dangling: `refs`: `R-77` resolves to no ID and no alias",
                path(number)
            ));
        } else {
            text.push_str("Changed.\n");
        }
        common::write(&repo.top, &path(number), text);
    }
    let mut big = document(SMALL, BIG_BYTES);
    big.push_str("Changed.\n");
    common::write(&repo.top, "docs/spec/bulk/big.md", big);
    repo.add_all();
    let changed = String::from_utf8(repo.git(&["diff", "--cached", "--name-only"])).unwrap();
    assert_eq!(changed.lines().count(), 10, "{changed}");

    let run = staged_run(&repo, false);
    run.code(1);
    let errors: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.starts_with("error  "))
        .collect();
    expected.sort();
    assert_eq!(errors, expected, "{}", run.show());
    let json = staged_run(&repo, true);
    json.code(1);
    let value = json.json();
    assert_eq!(value["counts"]["introduced"], 5);
    assert!(value["counts"]["documents"].as_u64().unwrap() > SMALL as u64);
}

/// Every bulk document changed (a line appended): `HEAD`'s side reads
/// about 3 000 blobs the index lacks (over 3 MiB of replies, over 100 KiB
/// of requests): exit 0 (only the fixture's pre-existing error), within
/// the watchdog.
#[test]
fn every_document_changed_never_deadlocks() {
    let repo = committed_bulk("intro-load-all");
    for number in 0..SMALL {
        let mut text = document(number, SMALL_BYTES);
        text.push_str("Changed.\n");
        common::write(&repo.top, &path(number), text);
    }
    repo.add_all();
    let run = staged_run(&repo, false);
    run.code(0);
    assert!(
        run.stdout
            .lines()
            .last()
            .unwrap_or_default()
            .ends_with(" — observed"),
        "{}",
        run.show()
    );
    let json = staged_run(&repo, true);
    json.code(0);
    assert_eq!(json.json()["counts"]["introduced"], 0);
}
