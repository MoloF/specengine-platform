//! docs/features/write-cost.md (RM-0125), the check memo through the
//! library, in-process, as a daemon reaches it: a `CheckMemo` per project,
//! its turn held (`CheckMemo::hold_turn`), each call inside
//! `check_memo::scoped(memo, login, …)`; `specengine_core::check::counts()`
//! read around a call; each call inside `memo.hold_turn().within(…)`, the
//! turn's mark on the calling thread. Over `fixtures/spec-a` (its `[decision_records]`)
//! opted in by a committed `[team]` of ten logins, every one of a role that
//! confirms. The daemon's own wiring (the turn, the per-project memo) is
//! `crates/specengine-http/tests/hub_concurrency.rs` (WC28).
//!
//! - WC5 `wc5_*`: a POST after its GET, nothing moved: 0 engine runs and 0
//!   parses, a commit, the record its question showed.
//! - WC6 `wc6_*`: ten logins' reads, read again in another order, all hit
//!   (the daemon's "ten interleaved pairs": a POST commits, which moves every
//!   other key, so the pairs that can all be live at once are reads); C and D
//!   share a budget in bytes, D evicted least recently used and an entry
//!   over the budget not kept; with the budget 0 every answer equals the
//!   memo-on run (the goldens' half is `write_cost_golden.rs`).
//! - WC26 `wc26_*`: a daemon's memo reached outside its turn is a debug
//!   assertion; a call outside any scope leaves the memo alone.
//! - WC27 `wc27_*`: no D hit for a login other than the one that stored the
//!   entry.
//! - WC30 `wc30_*`: a doubt empties D; after a doubt and a hand edit the next
//!   read equals a cold one, byte for byte.

#![cfg(unix)]

mod write_cost_common;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use serde_json::{Value, json};
use specengine_cli::{
    ConfirmBody, ConfirmRequest, Globals, Outcome, ProposeRequest, ProposedText, QuestionRequest,
    ShowRequest, StageBody, StageRequest, confirm_question, confirm_staged, propose,
    propose_question, show, stage, team_read,
};
use specengine_core::check::counts;
use specengine_store::CheckMemo;
use specengine_store::check_memo::{BUDGET, scoped};
use write_cost_common::{NOW, Repo, printed};

/// Ten logins: `ann` the owner, nine leads over `docs/**`.
const LOGINS: [&str; 10] = [
    "ann", "bea", "cal", "dan", "eva", "fin", "gil", "hana", "ivo", "jae",
];

/// Ten question targets, distinct enough that none meets another.
const QUESTIONS: [(&str, &str); 10] = [
    ("DOM-GAME", "Does the lantern burn through a whole night?"),
    (
        "MEC-SPRINT",
        "May a sprint start while carrying the lantern?",
    ),
    ("RULE-MOVE-SPEEDS", "Is walking slower on the outer walls?"),
    ("DOM-MOVEMENT", "Can the keeper climb the outer walls?"),
    (
        "RULE-CORE-LOOP",
        "Does a failed night restart the whole loop?",
    ),
    (
        "EDGE-SPRINT-EMPTY",
        "Does an empty bar end a sprint at once?",
    ),
    ("DOM-GAME", "Is there a map of the keep?"),
    ("MEC-SPRINT", "Can a sprint jump a gap?"),
    ("DOM-MOVEMENT", "Do ladders cost stamina?"),
    ("RULE-CORE-LOOP", "Is the loop the same on every night?"),
];

fn team() -> String {
    let mut text = String::from("\n[zones]\nall = [\"docs/**\"]\n");
    for (at, login) in LOGINS.iter().enumerate() {
        if at == 0 {
            text.push_str(&format!(
                "\n[team.{login}]\nrole = \"owner\"\nemail = \"{login}@example.org\"\n"
            ));
        } else {
            text.push_str(&format!(
                "\n[team.{login}]\nrole = \"lead\"\nzones = [\"all\"]\nemail = \"{login}@example.org\"\n"
            ));
        }
    }
    text
}

/// spec-a with the team committed.
fn opted_in(label: &str) -> Repo {
    let repo = Repo::new(label, "spec-a");
    let config = repo.root.join("specengine.toml");
    let mut text = fs::read_to_string(&config).expect("a config");
    text.push_str(&team());
    fs::write(&config, text).expect("the config");
    repo.commit_all("the team");
    repo
}

/// Question `n` raised (its hits named when it met any) and an approve
/// staged by `ann`: its ID.
fn staged_question(repo: &Repo, n: usize) -> String {
    let (target, text) = QUESTIONS[n];
    let mut request = QuestionRequest {
        node_ids: vec![target.to_owned()],
        text: text.to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
        severity: None,
        distinct_from: Vec::new(),
        card: None,
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let ask = |request: &QuestionRequest| {
        propose_question(&repo.env(), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("{text}: {error}"))
    };
    let mut outcome = ask(&request);
    if !outcome.document.created {
        request.distinct_from = outcome
            .document
            .hits
            .iter()
            .map(|hit| hit.name().to_owned())
            .collect();
        outcome = ask(&request);
    }
    let id = outcome.document.id.clone().expect("stored");
    let staged = stage(
        &repo.env(),
        &Globals::default(),
        &StageRequest {
            id: id.clone(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                editorial: None,
                note: None,
                updated_at: NOW.to_owned(),
                actor: Some("ann".to_owned()),
            },
            now: NOW.to_owned(),
            git: repo.git_env(),
            actor: Some("ann".to_owned()),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"));
    assert!(staged.cause.is_none(), "stage {id}: {staged:?}");
    id
}

/// A daemon's memo of `budget` bytes.
fn memo(budget: usize) -> Arc<CheckMemo> {
    Arc::new(CheckMemo::turned(budget))
}

/// The confirm's request of `login` for `id`.
fn request(repo: &Repo, login: &str, id: &str) -> ConfirmRequest {
    let team = team_read(&repo.env(), &Globals::default())
        .expect("[team] reads")
        .expect("a [team]");
    ConfirmRequest {
        id: id.to_owned(),
        entry: team.entry(login).cloned().expect("an entry"),
        now: NOW.to_owned(),
        git: repo.git_env(),
    }
}

/// The confirm's GET of `login` for `id` in `memo`'s turn and scope: its
/// answer as the daemon sends it.
fn read(repo: &Repo, memo: &Arc<CheckMemo>, login: &str, id: &str) -> Value {
    let request = request(repo, login, id);
    let outcome = memo.hold_turn().within(|| {
        scoped(memo, login, || {
            confirm_question(&repo.env(), &Globals::default(), &request)
        })
    });
    printed(outcome.map(|outcome| Outcome::Confirm(Box::new(outcome))))
}

/// The confirm's POST of `login` for `id` of what `read` gave.
fn write(repo: &Repo, memo: &Arc<CheckMemo>, login: &str, id: &str, read: &Value) -> Value {
    let request = request(repo, login, id);
    let document: Value = serde_json::from_str(read["json"].as_str().expect("JSON")).expect("JSON");
    let sent = ConfirmBody {
        question: document["question"]
            .as_str()
            .expect("a question")
            .to_owned(),
        updated_at: document["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned(),
    };
    let outcome = memo.hold_turn().within(|| {
        scoped(memo, login, || {
            confirm_staged(&repo.env(), &Globals::default(), &request, &sent)
        })
    });
    printed(outcome.map(|outcome| Outcome::Confirm(Box::new(outcome))))
}

fn document(answer: &Value) -> Value {
    serde_json::from_str(answer["json"].as_str().unwrap_or("null")).unwrap_or(Value::Null)
}

// ------------------------------------------------------------------- WC5

/// WC5: nothing moved between a GET and its POST: the POST runs the engine
/// 0 times and parses 0 files, commits, and its record is the one its
/// question showed.
#[test]
fn wc5_a_post_after_its_get_runs_no_check_and_parses_nothing() {
    let repo = opted_in("wc5-counts");
    let id = staged_question(&repo, 0);
    let memo = memo(BUDGET);
    let got = read(&repo, &memo, "bea", &id);
    assert_eq!(got["exit"], json!(0), "{got}");
    let before = counts();
    let hits = memo.hits();
    let posted = write(&repo, &memo, "bea", &id, &got);
    let after = counts();
    assert_eq!(posted["exit"], json!(0), "{posted}");
    assert_eq!(
        (after.runs - before.runs, after.parses - before.parses),
        (0, 0),
        "the POST's engine runs and parses"
    );
    assert_eq!(memo.hits() - hits, 1, "the POST took its findings from D");
    let applied = document(&posted);
    assert_eq!(applied["status"], json!("applied"), "{applied}");
    let question = document(&got)["question"]
        .as_str()
        .expect("a question")
        .to_owned();
    let path = applied["record_path"].as_str().expect("a record path");
    let stored = repo.git(&["show", &format!("HEAD:{path}")]);
    let head = format!(
        "record {} at {path}:\n",
        applied["record_id"].as_str().expect("an ID")
    );
    let shown: String = question
        .split_once(&head)
        .expect("the record in the question")
        .1
        .split('\n')
        .take_while(|line| line.is_empty() || line.starts_with("  "))
        .map(|line| format!("{}\n", line.strip_prefix("  ").unwrap_or(line)))
        .collect();
    assert_eq!(stored, shown.trim_end_matches('\n').to_owned() + "\n");
}

// ------------------------------------------------------------------- WC6

/// WC6: ten logins read ten items, then read them again in another order:
/// every second read hits (ten D entries live at once, the budget in bytes).
/// M: a count bound of 8 (the second round misses).
#[test]
fn wc6_ten_logins_reads_all_hit() {
    let repo = opted_in("wc6-ten");
    let ids: Vec<String> = (0..10).map(|n| staged_question(&repo, n)).collect();
    let memo = memo(BUDGET);
    let mut first = BTreeMap::new();
    for (login, id) in LOGINS.iter().zip(&ids) {
        first.insert(*login, read(&repo, &memo, login, id));
    }
    let (hits, misses) = (memo.hits(), memo.misses());
    for (login, id) in LOGINS.iter().zip(&ids).rev() {
        let again = read(&repo, &memo, login, id);
        assert_eq!(
            again, first[login],
            "{login}: the hit's answer is the miss's"
        );
    }
    assert_eq!(
        (memo.hits() - hits, memo.misses() - misses),
        (10, 0),
        "the second round all hit: {:?}",
        memo.lookups_by_login()
    );
    // One POST still hits its own GET.
    let hits = memo.hits();
    let posted = write(&repo, &memo, "jae", &ids[9], &first["jae"]);
    assert_eq!(document(&posted)["status"], json!("applied"), "{posted}");
    assert_eq!(memo.hits() - hits, 1);
}

/// WC6: C and D share one budget in bytes. Measured with room to spare: a
/// cold memo's first read takes the whole check and keeps its D entry alone;
/// the second, warm, keeps C (the tree) and its own D. A memo of exactly
/// those bytes holds the three; a third read evicts the least recently used
/// D entry, not the one read again; nothing ever passes the budget. A budget
/// below the smallest entry keeps nothing; the answers are the same
/// throughout.
#[test]
fn wc6_the_budget_is_bytes_and_d_goes_least_recently_used() {
    let repo = opted_in("wc6-budget");
    let ids: Vec<String> = (0..3).map(|n| staged_question(&repo, n)).collect();
    let roomy = memo(BUDGET);
    let mut sizes = Vec::new();
    let mut answers = Vec::new();
    for id in &ids {
        answers.push(read(&repo, &roomy, "ann", id));
        sizes.push(roomy.bytes());
    }
    assert_eq!(roomy.entries(), 4, "C and three D entries");
    let tree_and_two = sizes[1];
    assert!(
        sizes[0] > 0 && sizes[1] > sizes[0] && sizes[2] > sizes[1],
        "{sizes:?}"
    );

    let tight = memo(tree_and_two);
    assert_eq!(read(&repo, &tight, "ann", &ids[0]), answers[0]);
    assert_eq!(read(&repo, &tight, "ann", &ids[1]), answers[1]);
    assert_eq!(tight.entries(), 3, "C and two D entries fit");
    // ids[0] read again: now the most recent.
    let hits = tight.hits();
    assert_eq!(read(&repo, &tight, "ann", &ids[0]), answers[0]);
    assert_eq!(tight.hits() - hits, 1);
    // A third: the least recently used D entry (ids[1]) goes.
    assert_eq!(read(&repo, &tight, "ann", &ids[2]), answers[2]);
    assert!(
        tight.bytes() <= tree_and_two,
        "{} > {tree_and_two}",
        tight.bytes()
    );
    let hits = tight.hits();
    assert_eq!(read(&repo, &tight, "ann", &ids[0]), answers[0]);
    assert_eq!(tight.hits() - hits, 1, "the entry read again stayed");
    let misses = tight.misses();
    assert_eq!(read(&repo, &tight, "ann", &ids[1]), answers[1]);
    assert_eq!(tight.misses() - misses, 1, "the least recently used went");
    assert!(tight.bytes() <= tree_and_two);

    // A budget below the smallest entry: nothing kept, the same answers.
    let small = memo(sizes[0] / 2);
    for (id, answer) in ids.iter().zip(&answers) {
        assert_eq!(&read(&repo, &small, "ann", id), answer);
    }
    assert_eq!(small.bytes(), 0, "an entry over the budget is not kept");
    // Budget 0: the same answers again.
    let none = memo(0);
    for (id, answer) in ids.iter().zip(&answers) {
        assert_eq!(&read(&repo, &none, "ann", id), answer);
    }
    assert_eq!((none.entries(), none.bytes()), (0, 0));
}

// ------------------------------------------------------------------ WC26

/// WC26: a daemon's memo reached with its turn not held is a debug
/// assertion, so a lookup before `turn::take` fails every test build.
#[test]
#[should_panic(expected = "outside its project's turn")]
fn wc26_a_daemons_memo_outside_its_turn_is_asserted() {
    let memo = memo(BUDGET);
    scoped(&memo, "ann", || ());
}

/// WC26: the in-turn flag is the calling thread's: while another call holds
/// the project's turn on its own thread, a scope entered on this thread
/// with no turn held is still caught. M: a process-wide flag (any thread's
/// turn lets every thread in).
#[test]
fn wc26_another_threads_turn_lets_no_lookup_in() {
    use std::sync::Barrier;
    let memo = memo(BUDGET);
    let held = Arc::new(Barrier::new(2));
    let done = Arc::new(Barrier::new(2));
    let holder = {
        let (memo, held, done) = (Arc::clone(&memo), Arc::clone(&held), Arc::clone(&done));
        std::thread::spawn(move || {
            memo.hold_turn().within(|| {
                held.wait();
                done.wait();
            });
        })
    };
    held.wait();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scoped(&memo, "ann", || ());
    }));
    done.wait();
    holder.join().expect("the holder");
    let message = caught
        .err()
        .and_then(|panic| {
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
        })
        .unwrap_or_else(|| {
            panic!("a scope with no turn on its thread went in while another thread held one")
        });
    assert!(
        message.contains("outside its project's turn"),
        "the in-turn assertion: {message}"
    );
}

/// WC26: a call outside any scope (a terminal's) reaches no memo: after a
/// read in the scope, the same read outside it changes no count.
#[test]
fn wc26_a_call_outside_a_scope_leaves_the_memo_alone() {
    let repo = opted_in("wc26-none");
    let id = staged_question(&repo, 0);
    let memo = memo(BUDGET);
    let inside = read(&repo, &memo, "ann", &id);
    let seen = (memo.entries(), memo.bytes(), memo.hits(), memo.misses());
    assert!(seen.0 > 0, "the scoped read stored");
    let request = request(&repo, "ann", &id);
    let outside = printed(
        confirm_question(&repo.env(), &Globals::default(), &request)
            .map(|outcome| Outcome::Confirm(Box::new(outcome))),
    );
    assert_eq!(outside, inside, "the same answer");
    assert_eq!(
        (memo.entries(), memo.bytes(), memo.hits(), memo.misses()),
        seen,
        "untouched"
    );
}

// ------------------------------------------------------------------ WC27

/// WC27: D's key holds the login. `bea` reads (a miss, stored); `cal` reads
/// the same item on the same tree: a miss, never `bea`'s entry; each then
/// hits only its own. M: the login dropped from the key (`cal`'s first read
/// hits).
#[test]
fn wc27_no_d_hit_crosses_logins() {
    let repo = opted_in("wc27");
    let id = staged_question(&repo, 0);
    let memo = memo(BUDGET);
    read(&repo, &memo, "bea", &id);
    read(&repo, &memo, "cal", &id);
    read(&repo, &memo, "bea", &id);
    read(&repo, &memo, "cal", &id);
    let by_login = memo.lookups_by_login();
    assert_eq!(by_login.get("bea"), Some(&(1, 1)), "{by_login:?}");
    assert_eq!(by_login.get("cal"), Some(&(1, 1)), "{by_login:?}");
}

// ------------------------------------------------------------------ WC30

/// WC30: a doubt empties D (C stays, used again only under the digest a
/// walk gives); after a doubt and a hand edit the next read equals a cold
/// one, byte for byte. M: C served without re-validation (the edit unseen).
#[test]
fn wc30_a_doubt_empties_d_and_the_next_read_is_a_cold_ones() {
    let repo = opted_in("wc30");
    let id = staged_question(&repo, 0);
    let memo = memo(BUDGET);
    // A cold memo's first read takes the whole check and keeps D alone; the
    // first warm read keeps C (the tree) and its D.
    let second = staged_question(&repo, 1);
    read(&repo, &memo, "ann", &second);
    read(&repo, &memo, "ann", &id);
    assert_eq!(memo.entries(), 3, "C and two D entries");
    memo.doubt();
    assert_eq!(memo.entries(), 1, "D emptied, C kept");
    // A hand edit the check reads: a document loses the ID another file
    // cites (`[[DOM-MOVEMENT]]` in `game.md`), so the as-is side gains a
    // finding; a C not judged again would show it as introduced by the
    // record.
    let movement = repo.root.join("docs/spec/movement/README.md");
    let text = fs::read_to_string(&movement).expect("README.md");
    let edited = text.replacen("id: DOM-MOVEMENT\n", "", 1);
    assert_ne!(edited, text, "the document's ID is there");
    fs::write(&movement, edited).expect("a hand edit");
    let warm = read(&repo, &memo, "ann", &id);
    let cold = read(&repo, &self::memo(0), "ann", &id);
    assert_eq!(
        warm, cold,
        "after a doubt and a hand edit, a cold read's bytes"
    );
}

/// The diagnostics `spec propose update` gives `target` with `tag` appended
/// to its span, inside `memo`'s scope: as JSON.
fn proposed(repo: &Repo, memo: &Arc<CheckMemo>, target: &str, tag: &str) -> String {
    let shown = show(
        &repo.env(),
        &Globals::default(),
        &ShowRequest {
            reference: target.to_owned(),
            links: false,
            archive: false,
            bindings: false,
            history: false,
            git: repo.git_env(),
        },
    )
    .expect("show");
    let node = shown.nodes.into_iter().next().expect("the node");
    let request = ProposeRequest {
        target: target.to_owned(),
        base: node.span_hash,
        text: ProposedText::Given(format!("{}\nA probe line, {tag}.\n", node.text).into_bytes()),
        rationale: format!("Probe {tag}."),
        author_role: Some("spec-writer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let outcome = scoped(memo, "-", || {
        propose(&repo.env(), &Globals::default(), &request)
    })
    .expect("propose");
    serde_json::to_string(&outcome.document.diagnostics).expect("JSON")
}

/// WC30, the hand edit on a file C already holds: proposes of
/// `RULE-STAM-REGEN` fill C with its set's as-is findings (the feature
/// `stamina-tuning.md` among them, a reader of `MEC-STAMINA`); a hand edit
/// gives that file a dangling mention (`R-77`). The next propose over the
/// warm memo equals one over a cold memo, byte for byte, and names no `R-77`
/// (it is the corpus's own, never introduced). M: C reused across digests
/// (the stale as-is side makes `R-77` introduced).
#[test]
fn wc30_a_hand_edit_to_a_file_c_holds_is_judged_again() {
    let repo = opted_in("wc30-c");
    let memo = Arc::new(CheckMemo::new());
    proposed(&repo, &memo, "RULE-STAM-REGEN", "one");
    proposed(&repo, &memo, "RULE-STAM-REGEN", "two");
    assert!(
        memo.entries() >= 2,
        "C holds the set's as-is findings: {} entries",
        memo.entries()
    );
    let feature = repo.root.join("docs/features/stamina-tuning.md");
    let mut text = fs::read_to_string(&feature).expect("stamina-tuning.md");
    text.push_str("\nAs R-77 says.\n");
    fs::write(&feature, text).expect("a hand edit");
    let warm = proposed(&repo, &memo, "RULE-STAM-REGEN", "three");
    let cold = proposed(
        &repo,
        &Arc::new(CheckMemo::with_budget(0)),
        "RULE-STAM-REGEN",
        "three",
    );
    assert_eq!(warm, cold, "the warm memo's findings are a cold one's");
    assert!(
        !warm.contains("R-77"),
        "the corpus's own finding, not introduced: {warm}"
    );
}
