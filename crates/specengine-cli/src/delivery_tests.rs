//! The delivery-analytics task spec, DA-10, the CLI unit half: one read per
//! call, counted behind the `#[cfg(test)]` counters of
//! `crate::subset::seams` (`delivery_cards`, `delivery_events`,
//! `delivery_items`, `delivery_plans`).
//!
//! - `spec delivery` over five cards (moves, a return, an accept, a cancel,
//!   a re-estimate), then over 200 cards: one card
//!   list, one `events_of_types`, one item list, one plan build each,
//!   capped and browser views alike.
//! - No queue yet: one plan build, nothing read from the queue; no
//!   `[roadmap]`: no plan build.
//!
//! M: `card_items` per card (the item read inside a loop over the cards)
//! turns the 200 cards' item count to 200.
//!
//! The delivery-timeline task spec, DT3 and DT4, the same counters and the
//! `delivery_timelines` seam:
//!
//! - DT3: with the timeline asked, still one card list, one
//!   `events_of_types`, one item list and one plan build, capped and
//!   browser views alike, over five cards and over 200. M: a second
//!   `events_of_types` call for the timeline.
//! - DT4: not asked, no timeline built (no `delivery_timelines` entry) and
//!   `timeline` `None`; asked, one built with every task `total.tasks`
//!   counts. M: the timeline built whatever the flag.
//!
//! Every case works on a scratch corpus written here under
//! `std::env::temp_dir()`, with a scratch `HOME`, removed on drop; no git is
//! run. This file is a CLI source, so the genre rule (`tests/genre.rs`)
//! holds for it: the corpus's words are invented here and its root is
//! `spec`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::View;
use crate::subset::seams;
use crate::{
    CardAcceptRequest, CardCancelRequest, CardEditRequest, CardMoveRequest, CardNewRequest,
    DeliveryOutcome, DeliveryRequest, Env, Exit, Globals, card_accept, card_cancel, card_edit,
    card_move, card_new, delivery, delivery_with_view,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const HEAD: &str = r#"[project]
slug = "tide-count"

[paths]
roots = ["spec"]

[ids]
TD = { kind = "plan-item", width = 4 }

[[card.flow]]
name = "copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "done"]
accept = { from = "review", to = "done" }
return = "returned"

[[workspace]]
name = "words"
title = "Words"
flows = ["copy"]
"#;

const ROADMAP: &str = r#"
[roadmap]
prefix = "TD"
dir = "spec/plan"
releases = ["r1", "r2"]
priorities = ["must"]
"#;

const AS_OF: &str = "2026-11-20";

fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
}

/// A scratch corpus with its own `HOME`, removed on drop.
struct Place {
    dir: PathBuf,
    env: Env,
    globals: Globals,
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Place {
    fn new(name: &str, roadmap: bool) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-cli-delivery-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("root");
        fs::create_dir_all(root.join("spec").join("plan")).expect("a scratch corpus");
        let config = if roadmap {
            format!("{HEAD}{ROADMAP}")
        } else {
            HEAD.to_owned()
        };
        fs::write(root.join("specengine.toml"), config).expect("the config");
        for (id, release) in [("TD-0001", "r1"), ("TD-0002", "r2")] {
            fs::write(
                root.join("spec").join("plan").join(format!("{id}.md")),
                format!(
                    "---\nclass: spec\nid: {id}\nstatus: draft\nscope: [tides]\nrelease: \
                     {release}\n---\n\n# Tides {id}\n\nThe tides.\n"
                ),
            )
            .expect("an item");
        }
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
                root: Some(root),
                config: None,
            },
            dir,
        }
    }

    fn card(&self, title: &str, target: Option<&str>, parent: Option<&str>) -> String {
        let outcome = card_new(
            &self.env,
            &self.globals,
            &CardNewRequest {
                title: title.to_owned(),
                discipline: Some("copy".to_owned()),
                targets: target.map(str::to_owned).into_iter().collect(),
                reviewer: Some("Bob".to_owned()),
                estimate: Some("3".to_owned()),
                due: Some("2026-11-10".to_owned()),
                parent: parent.map(str::to_owned),
                now: at(1),
                ..CardNewRequest::default()
            },
        )
        .expect("card new");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    fn move_to(&self, id: &str, to: &str, note: Option<&str>, day: u32) {
        let outcome = card_move(
            &self.env,
            &self.globals,
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some(to.to_owned()),
                note: note.map(str::to_owned),
                actor: Some("Ann".to_owned()),
                now: at(day),
                ..CardMoveRequest::default()
            },
        )
        .expect("card move");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    /// Five cards with every kind of fact the read takes.
    fn five(&self) {
        let first = self.card("Tide table", Some("TD-0001"), None);
        let child = self.card("Tide plates", None, Some(&first));
        let _ = self.card("Harbor map", Some("TD-0002"), None);
        let _ = self.card("Index", None, None);
        let _ = self.card("Foghorn", None, None);
        self.move_to(&first, "todo", None, 2);
        self.move_to(&first, "review", None, 3);
        self.move_to(&first, "returned", Some("Too long"), 4);
        self.move_to(&first, "review", None, 5);
        let edited = card_edit(
            &self.env,
            &self.globals,
            &CardEditRequest {
                id: first.clone(),
                title: None,
                goal: None,
                targets: Vec::new(),
                criteria: Vec::new(),
                estimate: Some("5".to_owned()),
                due: None,
                parent: None,
                corrects: None,
                evidence: Vec::new(),
                tasks: Vec::new(),
                mandate: None,
                reason: Some("The tables doubled".to_owned()),
                revision: None,
                actor: Some("Ann".to_owned()),
                now: at(6),
                tasks_here: true,
            },
        )
        .expect("card edit");
        assert_eq!(edited.exit(), Exit::Answered, "{edited:?}");
        let cancelled = card_cancel(
            &self.env,
            &self.globals,
            &CardCancelRequest {
                id: child,
                reason: "Reused".to_owned(),
                actor: Some("Ann".to_owned()),
                now: at(7),
                ..CardCancelRequest::default()
            },
        )
        .expect("card cancel");
        assert_eq!(cancelled.exit(), Exit::Answered, "{cancelled:?}");
        let mut yes = |_: &str| true;
        let accepted = card_accept(
            &self.env,
            &self.globals,
            &CardAcceptRequest {
                id: first,
                actor: "Bob".to_owned(),
                briefed: true,
                now: at(15),
                ..CardAcceptRequest::default()
            },
            &mut yes,
        )
        .expect("card accept");
        assert_eq!(accepted.exit(), Exit::Answered, "{accepted:?}");
    }

    /// One read at `AS_OF` in `view`: the counts it took.
    fn counted(&self, view: View) -> seams::ReadCounts {
        self.counted_with(view, false).1
    }

    /// One read at `AS_OF` in `view`, the timeline asked or not: its
    /// outcome and the counts it took.
    fn counted_with(&self, view: View, timeline: bool) -> (DeliveryOutcome, seams::ReadCounts) {
        let _ = seams::take_read_counts();
        let request = DeliveryRequest {
            as_of: Some(AS_OF.to_owned()),
            timeline,
        };
        let outcome = match view {
            View::Capped => delivery(&self.env, &self.globals, &request),
            View::Browser => delivery_with_view(&self.env, &self.globals, &request, view),
        }
        .expect("delivery");
        assert_eq!(outcome.delivery.as_of, AS_OF);
        (outcome, seams::take_read_counts())
    }
}

/// (cards, events, items, plans) of `counts`.
fn reads(counts: &seams::ReadCounts) -> (usize, usize, usize, usize) {
    (
        counts.delivery_cards,
        counts.delivery_events,
        counts.delivery_items,
        counts.delivery_plans,
    )
}

/// DA-10: five cards, then 200: one card list, one events read, one item
/// list and one plan build per call, capped and browser alike. M:
/// `card_items` per card.
#[test]
fn da10_one_read_per_call() {
    let place = Place::new("one-read", true);
    place.five();
    for view in [View::Capped, View::Browser] {
        assert_eq!(reads(&place.counted(view)), (1, 1, 1, 1), "five cards");
    }
    for n in 0..195 {
        let target = ["TD-0001", "TD-0002"].get(n % 3).copied();
        let _ = place.card(&format!("Tide line {n}"), target, None);
    }
    for view in [View::Capped, View::Browser] {
        assert_eq!(reads(&place.counted(view)), (1, 1, 1, 1), "200 cards");
    }
    let request = DeliveryRequest {
        as_of: Some(AS_OF.to_owned()),
        timeline: false,
    };
    let outcome = delivery(&place.env, &place.globals, &request).expect("delivery");
    assert_eq!(
        outcome.delivery.total.tasks, 199,
        "200 cards, one cancelled"
    );
}

/// DA-10: no queue yet: one plan build, no queue read; no `[roadmap]`: no
/// plan build.
#[test]
fn da10_no_queue_and_no_plan_read_nothing_more() {
    let place = Place::new("no-queue", true);
    assert_eq!(reads(&place.counted(View::Capped)), (0, 0, 0, 1));
    let bare = Place::new("no-plan", false);
    bare.five();
    assert_eq!(reads(&bare.counted(View::Capped)), (1, 1, 1, 0));
}

/// DT3: the timeline asked reads what part a reads, once each: five cards,
/// then 200, capped and browser alike, the same counts as without it. M: a
/// second `events_of_types` call for the timeline.
#[test]
fn dt3_the_timeline_reads_nothing_more() {
    let place = Place::new("timeline-reads", true);
    place.five();
    for view in [View::Capped, View::Browser] {
        let (_, without) = place.counted_with(view, false);
        let (outcome, with) = place.counted_with(view, true);
        assert_eq!(reads(&with), (1, 1, 1, 1), "five cards");
        assert_eq!(reads(&with), reads(&without), "five cards");
        assert_eq!(
            outcome
                .delivery
                .timeline
                .map(|timeline| timeline.tasks.len()),
            Some(4),
            "five cards, one cancelled"
        );
    }
    for n in 0..195 {
        let target = ["TD-0001", "TD-0002"].get(n % 3).copied();
        let _ = place.card(&format!("Tide line {n}"), target, None);
    }
    for view in [View::Capped, View::Browser] {
        let (_, without) = place.counted_with(view, false);
        let (outcome, with) = place.counted_with(view, true);
        assert_eq!(reads(&with), (1, 1, 1, 1), "200 cards");
        assert_eq!(reads(&with), reads(&without), "200 cards");
        assert_eq!(with.delivery_timelines, [199], "200 cards");
        assert_eq!(
            outcome
                .delivery
                .timeline
                .map(|timeline| timeline.tasks.len()),
            Some(199)
        );
    }
}

/// DT4: not asked, nothing of the timeline is built and `timeline` is
/// `None`; asked, one is built, its tasks every one `total.tasks` counts;
/// no queue yet, asked: one built, empty. M: the timeline built whatever
/// the flag.
#[test]
fn dt4_no_timeline_built_unless_asked() {
    let place = Place::new("timeline-off", true);
    place.five();
    for view in [View::Capped, View::Browser] {
        let (outcome, counts) = place.counted_with(view, false);
        assert_eq!(counts.delivery_timelines, Vec::<usize>::new(), "not asked");
        assert_eq!(outcome.delivery.timeline, None);
        let (outcome, counts) = place.counted_with(view, true);
        assert_eq!(counts.delivery_timelines, [4], "asked");
        assert_eq!(outcome.delivery.total.tasks, 4);
        assert!(outcome.delivery.timeline.is_some());
    }
    let empty = Place::new("timeline-empty", true);
    let (outcome, counts) = empty.counted_with(View::Capped, false);
    assert_eq!(counts.delivery_timelines, Vec::<usize>::new());
    assert_eq!(outcome.delivery.timeline, None);
    let (outcome, counts) = empty.counted_with(View::Capped, true);
    assert_eq!(counts.delivery_timelines, [0]);
    assert_eq!(reads(&counts), (0, 0, 0, 1));
    let timeline = outcome.delivery.timeline.expect("asked");
    assert_eq!((timeline.from, timeline.to), (None, None));
}
