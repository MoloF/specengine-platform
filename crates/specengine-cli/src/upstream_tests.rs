//! The upstream-signal task spec, US-9, the CLI unit half: one read per
//! call, counted behind the `#[cfg(test)]` counters of `crate::subset::seams`
//! (`upstream_events`, `upstream_items`, and `whole_decodes` for the index's
//! refresh).
//!
//! - `card list --marks` over 40 cards, 30 of them open subtasks: one
//!   events read, one applied-item read, one refresh.
//! - `card show --marks` of an open subtask: 1, 1, 1.
//! - The daemon's card route (`upstream` asked, `marks` not) of a card with
//!   no parent, and of a closed subtask: 0, 0, 0.
//! - A list or a show that asks no marks reads none of them; a marked list
//!   with no open subtask refreshes once and reads no event or item.
//!
//! M: a scan per card (the events or the items read inside the per-subtask
//! loop) turns the list's counts to 30.
//!
//! Every case works on a scratch corpus written here under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; no git is
//! run. This file is a CLI source, so the genre rule (`tests/genre.rs`)
//! holds for it: the corpus's words are invented here and its root is
//! `spec`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_store::GitEnv;

use crate::subset::seams;
use crate::{
    CardEditRequest, CardListRequest, CardNewRequest, CardShowRequest, Env, Exit, Globals,
    card_edit, card_list, card_new, card_show,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const CONFIG: &str = r#"[project]
slug = "tide-count"

[paths]
roots = ["spec"]

[ids]
TIDE = { kind = "tide", shape = "name" }

[[card.flow]]
name = "desk"
columns = ["backlog", "review", "done"]
accept = { from = "review", to = "done" }
"#;

const TIDES: &str = "---\nid: TIDE-TABLE\nclass: canon\ntitle: The tide table\n---\n\n\
                     # The tide table\n\nThe tides of the year.\n\n## Spring {#TIDE-SPRING}\n\n\
                     High at the full moon.\n\n## Neap {#TIDE-NEAP}\n\nLow at the half moon.\n";

const NOW: &str = "2026-11-12T08:00:00Z";
const LATER: &str = "2026-11-12T09:00:00Z";

/// A scratch corpus with its own `HOME`, removed on drop.
struct Place {
    dir: PathBuf,
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Place {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-cli-upstream-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("root");
        fs::create_dir_all(root.join("spec")).expect("a scratch corpus");
        fs::write(root.join("specengine.toml"), CONFIG).expect("the config");
        fs::write(root.join("spec").join("tides.md"), TIDES).expect("the document");
        let home = dir.join("home");
        fs::create_dir_all(&home).expect("a home");
        let root = fs::canonicalize(&root).expect("canonical");
        let dir = fs::canonicalize(&dir).expect("canonical");
        Self {
            env: Env {
                cwd: root.clone(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(root.clone()),
                config: None,
            },
            git: GitEnv::new(root, [("HOME", home.as_os_str().to_owned())]),
            dir,
        }
    }

    /// `card new`: its ID.
    fn card(&self, title: &str, target: &str, parent: Option<&str>) -> String {
        let outcome = card_new(
            &self.env,
            &self.globals,
            &CardNewRequest {
                title: title.to_owned(),
                targets: vec![target.to_owned()],
                parent: parent.map(str::to_owned),
                now: NOW.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .expect("card new");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    /// The parent's goal edited, so its subtasks have something to say.
    fn edit_goal(&self, id: &str) {
        let outcome = card_edit(
            &self.env,
            &self.globals,
            &CardEditRequest {
                id: id.to_owned(),
                title: None,
                goal: Some("Both tides named.".to_owned()),
                targets: Vec::new(),
                criteria: Vec::new(),
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                evidence: Vec::new(),
                tasks: Vec::new(),
                mandate: None,
                reason: None,
                revision: None,
                actor: Some("Ann".to_owned()),
                now: LATER.to_owned(),
                tasks_here: true,
            },
        )
        .expect("card edit");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    /// Ten parents on the table, each with three subtasks on `TIDE-SPRING`:
    /// 40 cards, 30 open subtasks; the first parent's goal edited.
    fn forty(&self) -> Vec<(String, Vec<String>)> {
        let mut families = Vec::new();
        for n in 0..10 {
            let parent = self.card(&format!("Table {n}"), "TIDE-TABLE", None);
            let children = (0..3)
                .map(|m| self.card(&format!("Table {n} part {m}"), "TIDE-SPRING", Some(&parent)))
                .collect();
            families.push((parent, children));
        }
        self.edit_goal(&families[0].0);
        families
    }

    fn list(&self, marks: bool) -> crate::CardListOutcome {
        let outcome = card_list(
            &self.env,
            &self.globals,
            &CardListRequest {
                marks,
                ..CardListRequest::default()
            },
        )
        .expect("card list");
        assert!(outcome.notes.is_empty(), "{:?}", outcome.notes);
        outcome
    }

    fn show(&self, id: &str, marks: bool, upstream: bool, history: bool) -> crate::CardShowOutcome {
        let outcome = card_show(
            &self.env,
            &self.globals,
            &CardShowRequest {
                id: id.to_owned(),
                history,
                marks,
                upstream,
                context: false,
                budget: None,
                git: self.git.clone(),
                tasks_here: true,
            },
        )
        .expect("card show");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// `(events reads, applied-item reads, index refreshes)` since the last
/// take.
fn counts() -> (usize, usize, usize) {
    let taken = seams::take_read_counts();
    (
        taken.upstream_events,
        taken.upstream_items,
        taken.whole_decodes,
    )
}

/// US-9: `card list --marks` over 40 cards, 30 open subtasks: one events
/// read, one applied-item read, one refresh; every subtask of the edited
/// parent flagged, no other card. M: a scan per card.
#[test]
fn a_marked_list_reads_once() {
    let place = Place::new("list");
    let families = place.forty();
    let _ = counts();
    let listed = place.list(true);
    assert_eq!(counts(), (1, 1, 1));
    assert_eq!(listed.cards.len(), 40);
    let flagged: Vec<&String> = listed.upstream.iter().collect();
    let expected: Vec<&String> = families[0].1.iter().collect();
    assert_eq!(flagged, expected);
}

/// US-9: `card show --marks` of an open subtask: 1, 1, 1; the daemon's card
/// route of a card with no parent: 0, 0, 0.
#[test]
fn a_show_reads_once_and_a_parentless_card_reads_nothing() {
    let place = Place::new("show");
    let families = place.forty();
    let child = &families[0].1[0];
    let _ = counts();
    let shown = place.show(child, true, true, false);
    assert_eq!(counts(), (1, 1, 1));
    assert!(shown.upstream.is_some(), "the parent's goal edit");

    let parent = &families[0].0;
    let routed = place.show(parent, false, true, true);
    assert_eq!(counts(), (0, 0, 0), "the card route of a parentless card");
    assert!(routed.upstream.is_none());
    assert!(routed.mark.is_none(), "the route asks no mark");

    let closed = &families[1].1[0];
    let outcome = crate::card_cancel(
        &place.env,
        &place.globals,
        &crate::CardCancelRequest {
            id: closed.clone(),
            reason: "Not needed.".to_owned(),
            now: LATER.to_owned(),
            ..crate::CardCancelRequest::default()
        },
    )
    .expect("card cancel");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let _ = counts();
    let routed = place.show(closed, false, true, true);
    assert_eq!(counts(), (0, 0, 0), "the card route of a closed subtask");
    assert!(routed.upstream.is_none());
}

/// US-9 and D4: a show or a list that asks no marks reads nothing for the
/// signal; a marked list with no open subtask refreshes once and reads no
/// event or item.
#[test]
fn unasked_reads_nothing() {
    let place = Place::new("unasked");
    let families = place.forty();
    let _ = counts();
    let listed = place.list(false);
    assert_eq!(counts(), (0, 0, 0));
    assert!(listed.upstream.is_empty());
    let shown = place.show(&families[0].1[0], false, false, true);
    assert_eq!(counts(), (0, 0, 0));
    assert!(shown.upstream.is_none());

    let lone = Place::new("lone");
    lone.card("Lone table", "TIDE-TABLE", None);
    lone.card("Other table", "TIDE-NEAP", None);
    let _ = counts();
    let listed = lone.list(true);
    assert_eq!(
        counts(),
        (0, 0, 1),
        "no open subtask: no event or item read"
    );
    assert!(listed.upstream.is_empty());
}
