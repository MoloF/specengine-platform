//! Helpers of the docs/features/decision-apply.md tests ("Acceptance
//! criteria", Setup): the pairs of `common::proposal` (a scratch git
//! repository of `fixtures/spec-a` or `fixtures/spec-b`, its main
//! worktree on `main`, a linked one on `t1`, a scratch `HOME`), whose
//! fixture carries `[decision_records]` and a committed
//! `templates/decision.md` outside the walked roots; items raised through
//! the library with `distinct_from` naming their hits; the library
//! `approve_with` at the clock [`CLOCK`] with a consent callback; the
//! binary on a pseudo-terminal (`script`) where the terminal is the
//! subject.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use specengine_cli::{
    ApproveFlags, ApproveRequest, CliError, DiscrepancyInput, DiscrepancyRequest, Evidence, Exit,
    GapType, Globals, IntakeOption, IntakeOutcome, IntakeSeverity, ProposalOutcome,
    QuestionRequest, approve_with, propose_discrepancy, propose_question,
};

use super::proposal::{NOW, Pair};
use super::{RUN_TIMEOUT, SPEC, read_text, write};

/// The clock of every decision (AC Setup).
pub const CLOCK: &str = "2026-10-05T12:00:00Z";

/// Its date, as a record's `date:` carries it.
pub const DAY: &str = "2026-10-05";

/// The `[decision_records]` table of a fixture's config, its lines as the
/// fixture writes them.
pub const TABLE_HEADER: &str = "[decision_records]";

/// The flags `--option N`, `--answer T`, `--canon REF`.
pub fn flags(option: Option<u64>, answer: Option<&str>, canon: Option<&str>) -> ApproveFlags {
    ApproveFlags {
        option,
        answer: answer.map(str::to_owned),
        canon: canon.map(str::to_owned),
    }
}

/// `--option N` alone.
pub fn option(index: u64) -> ApproveFlags {
    flags(Some(index), None, None)
}

/// `config` without its `[decision_records]` table (the header and its
/// keys, up to the next table or the end).
pub fn without_records(config: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in config.split_inclusive('\n') {
        if line.trim_start().starts_with('[') {
            inside = line.trim() == TABLE_HEADER;
        }
        if !inside {
            out.push_str(line);
        }
    }
    assert!(!out.contains(TABLE_HEADER), "{out}");
    out
}

/// `config` with its `[decision_records]` table replaced by one of
/// `prefix`, `dir`, `template`.
pub fn with_records(config: &str, prefix: &str, dir: &str, template: &str) -> String {
    let mut out = without_records(config);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!(
        "\n{TABLE_HEADER}\nprefix   = \"{prefix}\"\ndir      = \"{dir}\"\ntemplate = \"{template}\"\n"
    ));
    out
}

/// A discrepancy of a `developer` about `targets`: one piece of evidence,
/// `options` as `(label, effect, price)`, recommendation 0.
pub fn discrepancy(
    targets: &[&str],
    summary: &str,
    options: &[(&str, &str, &str)],
) -> DiscrepancyInput {
    DiscrepancyInput {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        summary: summary.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::Normal,
        evidence: vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: Some("regen_system".to_owned()),
            lines: Some("10-20".to_owned()),
            observed: "regenerates while sprinting".to_owned(),
            documented: "regenerates only at rest".to_owned(),
        }],
        options: options
            .iter()
            .map(|(label, effect, price)| IntakeOption {
                label: (*label).to_owned(),
                effect: (*effect).to_owned(),
                price: (*price).to_owned(),
            })
            .collect(),
        recommendation: 0,
        working_answer: Some("Keep the spec".to_owned()),
        proposed_patch: None,
        distinct_from: None,
    }
}

/// The three options of AC-01's discrepancy.
pub const THREE: [(&str, &str, &str); 3] = [
    (
        "Keep the spec",
        "Fix the code to regenerate at rest",
        "one work item",
    ),
    (
        "Change the spec",
        "Regeneration also runs while sprinting",
        "a new rule case and a test",
    ),
    ("Defer", "Nothing changes now", "the gap stays open"),
];

/// A pseudo-terminal run of `spec`: its exit code and everything it wrote
/// (stdout and stderr share the terminal; CR LF made LF).
#[derive(Debug, Clone)]
pub struct PtyRun {
    pub code: i32,
    pub output: String,
    /// The answer was written (a `[y/N]` prompt was seen).
    pub asked: bool,
}

impl Pair {
    /// Library `approve_with` from `cwd` at `now`: `note`, `flags`,
    /// `consent` answering every question.
    pub fn decide_with(
        &self,
        cwd: &Path,
        id: &str,
        flags: &ApproveFlags,
        note: Option<&str>,
        now: &str,
        consent: &mut dyn FnMut(&str) -> bool,
    ) -> Result<ProposalOutcome, CliError> {
        approve_with(
            &self.env(cwd),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: note.map(str::to_owned),
                now: now.to_owned(),
                git: self.git_env(cwd),
            },
            flags,
            consent,
        )
    }

    /// [`Pair::decide_with`] at `now`, consent `answer`: the outcome and
    /// the questions asked.
    pub fn decide_at(
        &self,
        cwd: &Path,
        id: &str,
        flags: &ApproveFlags,
        note: Option<&str>,
        answer: bool,
        now: &str,
    ) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            answer
        };
        let outcome = self.decide_with(cwd, id, flags, note, now, &mut consent);
        (outcome, questions)
    }

    /// [`Pair::decide_at`] at [`CLOCK`].
    pub fn decide(
        &self,
        cwd: &Path,
        id: &str,
        flags: &ApproveFlags,
        note: Option<&str>,
        answer: bool,
    ) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
        self.decide_at(cwd, id, flags, note, answer, CLOCK)
    }

    /// A decision that must apply (exit 0, one question): the outcome and
    /// the question.
    pub fn decide_ok(
        &self,
        cwd: &Path,
        id: &str,
        flags: &ApproveFlags,
        note: Option<&str>,
    ) -> (ProposalOutcome, String) {
        let (outcome, questions) = self.decide(cwd, id, flags, note, true);
        let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        assert_eq!(questions.len(), 1, "approve {id}: {questions:?}");
        (outcome, questions.into_iter().next().expect("one question"))
    }

    /// A question of a `developer` from `cwd`, stored: when its first ask
    /// hits, asked again naming every hit in `distinct_from` (AC Setup).
    pub fn raise_question(
        &self,
        cwd: &Path,
        targets: &[&str],
        text: &str,
        working_answer: &str,
        price_of_other: &str,
    ) -> String {
        let mut request = QuestionRequest {
            node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            text: text.to_owned(),
            working_answer: working_answer.to_owned(),
            price_of_other: price_of_other.to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            author_role: Some("developer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: NOW.to_owned(),
            git: self.git_env(cwd),
        };
        let ask = |request: &QuestionRequest| {
            propose_question(&self.env(cwd), &Globals::default(), request)
                .unwrap_or_else(|error| panic!("ask: {error}"))
        };
        let first = ask(&request);
        let outcome = if first.document.created {
            first
        } else {
            request.distinct_from = hit_names(&first);
            ask(&request)
        };
        stored(outcome)
    }

    /// A discrepancy of a `developer` from `cwd`, stored as
    /// [`Pair::raise_question`] stores a question.
    pub fn raise_discrepancy(&self, cwd: &Path, input: DiscrepancyInput) -> String {
        let report = |input: DiscrepancyInput| {
            propose_discrepancy(
                &self.env(cwd),
                &Globals::default(),
                &DiscrepancyRequest {
                    input,
                    author_role: Some("developer".to_owned()),
                    author_model: Some("claude-opus-5-5".to_owned()),
                    run: None,
                    now: NOW.to_owned(),
                    git: self.git_env(cwd),
                },
            )
            .unwrap_or_else(|error| panic!("report: {error}"))
        };
        let first = report(input.clone());
        let outcome = if first.document.created {
            first
        } else {
            let mut named = input;
            named.distinct_from = Some(hit_names(&first));
            report(named)
        };
        stored(outcome)
    }

    /// The worktree `dir`'s `specengine.toml` without `[decision_records]`,
    /// committed there (no hook).
    pub fn drop_records_table(&self, dir: &Path) {
        let config = read_text(dir, "specengine.toml");
        write(dir, "specengine.toml", without_records(&config));
        self.commit_all(dir, "No decision records.");
    }

    /// `git status --porcelain=v1 --untracked-files=all` in `dir`, as
    /// printed (untrimmed).
    pub fn porcelain(&self, dir: &Path) -> String {
        String::from_utf8(
            self.git
                .git(dir, &["status", "--porcelain=v1", "--untracked-files=all"]),
        )
        .expect("UTF-8 status")
    }

    /// `spec args` in `cwd` with stdin on a pseudo-terminal (`script`),
    /// the sandbox's variables and this `HOME`; at the first `[y/N]` the
    /// terminal answers `answer` (none: the run must not ask). The
    /// terminal stays open until `spec` exits; a watchdog kills a run
    /// past [`RUN_TIMEOUT`].
    pub fn spec_pty(&self, cwd: &Path, args: &[&str], answer: Option<&str>) -> PtyRun {
        let script = ["/usr/bin/script", "/bin/script"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a `script` program gives `spec` a terminal");
        let mut command = Command::new(script);
        if cfg!(target_os = "macos") {
            command.arg("-q").arg("/dev/null").arg(SPEC).args(args);
        } else {
            let quoted: Vec<String> = std::iter::once(SPEC)
                .chain(args.iter().copied())
                .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
                .collect();
            command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
        }
        command
            .env_clear()
            .envs(self.git.vars())
            .env("HOME", &self.home)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn script");
        let mut stdin = child.stdin.take().expect("stdin");
        let mut stdout = child.stdout.take().expect("stdout");
        let (sender, chunks) = mpsc::channel::<Vec<u8>>();
        let reader = std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if sender.send(buffer[..read].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut output = Vec::new();
        let mut asked = false;
        let started = Instant::now();
        let status = loop {
            while let Ok(chunk) = chunks.try_recv() {
                output.extend_from_slice(&chunk);
            }
            if !asked && String::from_utf8_lossy(&output).contains("[y/N]") {
                asked = true;
                if let Some(answer) = answer {
                    let _ = stdin.write_all(format!("{answer}\n").as_bytes());
                    let _ = stdin.flush();
                }
            }
            if let Some(status) = child.try_wait().expect("wait on script") {
                break status;
            }
            if started.elapsed() > RUN_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                panic!("spec {args:?} on a terminal ran longer than {RUN_TIMEOUT:?}; killed");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        drop(stdin);
        reader.join().expect("the reader");
        while let Ok(chunk) = chunks.try_recv() {
            output.extend_from_slice(&chunk);
        }
        let output = String::from_utf8(output)
            .expect("UTF-8 terminal output")
            .replace("\r\n", "\n");
        if !asked && output.contains("[y/N]") {
            asked = true;
        }
        PtyRun {
            code: status
                .code()
                .expect("script exited, not killed by a signal"),
            output,
            asked,
        }
    }
}

/// The names `distinct_from` takes for every hit of `outcome`.
pub fn hit_names(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

/// The stored item's ID; it must have been stored.
fn stored(outcome: IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}
