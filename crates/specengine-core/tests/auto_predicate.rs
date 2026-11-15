//! docs/features/auto-rule.md RL2, RL4, RL6 and RL8 on the pure predicate
//! (core `auto.rs`; docs/canon/auto-mode.md "Switches", "Judging an item"):
//! the rules are `fixtures/auto-mini`'s (`rules/*.toml` appended to its
//! config and read by `AutoRule::from_toml`), the facts written here.
//!
//! - RL2: one row per reason the predicate judges, each changing one fact
//!   of a row that applies, so it passes every other check, and giving
//!   exactly its reason and detail; plus the row that applies.
//! - RL4: one `[team]` login, on; two, `team`; on a hub one `lead`, `team`.
//! - RL6: an excluded zone or kind beats the cover.
//! - RL8: under a pack, the frozen rule and `HEAD`'s both judge, the one
//!   failing alone named; a changed branch is `off`.

mod auto_common;

use auto_common::{base_config, config_with, with_table};
use specengine_core::auto::{
    PackFacts, ProposalFacts, Reason, StagedAs, Surface, Switches, TaskFacts, TextState, Verdict,
    is_config_target, judge_approve, judge_land_paths, judge_proposal, judge_switches,
};
use specengine_core::auto_toml::AutoRule;
use specengine_core::proposal::AuthorType;
use specengine_core::team_toml::TeamRole;
use specengine_model::{RunOutcome, TaskStatus};

const NOW: &str = "2026-11-16T01:00:00Z";
const PACK: i64 = 812;

fn read(config: &str) -> AutoRule {
    AutoRule::from_toml(config.as_bytes(), "specengine.toml")
        .rule
        .unwrap_or_else(|| panic!("a rule in:\n{config}"))
}

/// `rules/<name>` on the fixture's config.
fn rule(name: &str) -> AutoRule {
    read(&config_with(name))
}

/// A rule of `[auto]` keys `keys` on the fixture's config.
fn rule_of(keys: &str) -> AutoRule {
    read(&with_table(&base_config(), &format!("[auto]\n{keys}")))
}

fn logins(entries: &[(&str, TeamRole)]) -> Vec<(String, TeamRole)> {
    entries
        .iter()
        .map(|(login, role)| ((*login).to_owned(), *role))
        .collect()
}

fn switches<'a>(
    surface: Surface,
    rule: &'a AutoRule,
    team: &'a [(String, TeamRole)],
) -> Switches<'a> {
    Switches {
        surface,
        rule: Ok(rule),
        logins: Ok(team),
        stopped: None,
    }
}

/// An `update` of R-02 in `docs/spec/core.md`, raised over `/mcp` by
/// `ann`, bound to a pack's task: it applies under `rules/on.toml`.
fn update() -> ProposalFacts {
    ProposalFacts {
        id: "PR-0007".to_owned(),
        kind: "update".to_owned(),
        staged: None,
        author_type: AuthorType::Agent,
        author_login: Some("ann".to_owned()),
        pack: Some(PACK),
        paths: vec!["docs/spec/core.md".to_owned()],
        ids: vec!["R-02".to_owned()],
        findings: Vec::new(),
        text: TextState::Applies,
    }
}

/// A pack task touching `src/lib.rs` and R-02.
fn task<'a>(status: TaskStatus, pack: Option<PackFacts<'a>>) -> TaskFacts<'a> {
    TaskFacts {
        id: "T-0042".to_owned(),
        status,
        pack,
        paths: vec!["src/lib.rs".to_owned(), "docs/spec/core.md".to_owned()],
        ids: vec!["R-02".to_owned()],
        unresolved: Vec::new(),
        last_run: Some((1, RunOutcome::Completed)),
    }
}

fn pack(frozen: &AutoRule) -> PackFacts<'_> {
    PackFacts {
        seq: PACK,
        until: None,
        frozen,
    }
}

fn held(verdict: &Verdict) -> (&'static str, String) {
    match verdict {
        Verdict::Hold { reason, detail } => (reason.as_str(), detail.clone()),
        Verdict::Apply => ("apply", String::new()),
    }
}

/// The reasons' strings, the canon's closed set in its order.
#[test]
fn the_reasons_are_the_canons() {
    assert_eq!(
        Reason::ALL.map(Reason::as_str),
        [
            "off",
            "team",
            "stopped",
            "not-in-pack",
            "author",
            "until",
            "status",
            "run-outcome",
            "proposal-kind",
            "staged",
            "config-target",
            "excluded-zone",
            "excluded-kind",
            "uncovered",
            "findings",
            "rebases",
            "cap",
            "place",
            "merge-conflict",
            "check",
            "branch-checked-out",
            "branch-moved",
            "agent-error",
            "denied-tool",
            "no-mcp",
            "max-turns",
            "max-budget",
            "not-planned",
            "not-done",
        ]
    );
    for reason in Reason::ALL {
        assert_eq!(Reason::parse(reason.as_str()), Some(reason));
    }
}

/// RL2: every reason, one row; the base rows apply. M: any one check
/// removed turns its row red.
#[test]
fn rl2_every_reason_one_row() {
    let on = rule("on.toml");
    let hub = rule("hub.toml");
    let off = rule("off.toml");
    let local_only = rule("local.toml");
    let night = rule_of(
        "mode = \"both\"\nzones = [\"code\", \"tests\", \"docs\"]\nexclude_zones = [\"narrative\"]\nexclude_kinds = [\"scenario\"]\nbranch = \"night\"\n",
    );
    let one = logins(&[("ann", TeamRole::Owner)]);
    let two = logins(&[("ann", TeamRole::Owner), ("mira", TeamRole::Lead)]);
    let none = logins(&[]);
    let lead = logins(&[("mira", TeamRole::Lead)]);
    let local = switches(Surface::Local, &on, &one);
    let on_hub = switches(Surface::Hub, &hub, &one);
    let live = pack(&on);
    let base = update();

    // The rows that apply: a pack's update locally, the owner's on a hub
    // under the cap, a pack task's approve and land.
    assert_eq!(
        judge_proposal(&local, Some(&live), &base, None, NOW),
        Verdict::Apply
    );
    assert_eq!(
        judge_proposal(&on_hub, None, &base, Some(19), NOW),
        Verdict::Apply
    );
    let create = ProposalFacts {
        kind: "create".to_owned(),
        ids: vec!["R-02".to_owned(), "R-03".to_owned()],
        ..update()
    };
    assert_eq!(
        judge_proposal(&local, Some(&live), &create, None, NOW),
        Verdict::Apply
    );
    assert_eq!(
        judge_approve(&local, &task(TaskStatus::Review, Some(pack(&on))), NOW),
        Verdict::Apply
    );
    assert_eq!(
        judge_land_paths(
            &local,
            &task(TaskStatus::Done, Some(pack(&on))),
            &["src/lib.rs".to_owned(), "tests/a.rs".to_owned()]
        ),
        Verdict::Apply
    );
    assert_eq!(judge_switches(&local), Ok(()));

    let p = |s: &Switches<'_>, facts: &ProposalFacts| {
        held(&judge_proposal(s, Some(&live), facts, None, NOW))
    };
    let mut rows = Rows::default();

    // off: each why.
    let absent = Switches {
        rule: Err("no [auto] table in specengine.toml@HEAD"),
        ..local
    };
    rows.row(
        "off: no table",
        p(&absent, &base),
        ("off", "no [auto] table in specengine.toml@HEAD"),
    );
    rows.row(
        "off: no commit",
        p(
            &Switches {
                rule: Err("no commit"),
                ..local
            },
            &base,
        ),
        ("off", "no commit"),
    );
    rows.row(
        "off: mode off",
        p(&switches(Surface::Local, &off, &one), &base),
        ("off", "[auto] mode is off"),
    );
    rows.row(
        "off: a hub rule on a terminal",
        p(&switches(Surface::Local, &hub, &one), &base),
        (
            "off",
            "[auto] mode is hub: a terminal acts only under local or both",
        ),
    );
    rows.row(
        "off: a local rule on a hub",
        held(&judge_proposal(
            &switches(Surface::Hub, &local_only, &one),
            None,
            &base,
            Some(0),
            NOW,
        )),
        (
            "off",
            "[auto] mode is local: a hub acts only under hub or both",
        ),
    );
    rows.row(
        "off: the branch changed",
        p(&switches(Surface::Local, &night, &one), &base),
        (
            "off",
            "the rule's branch changed since pack 812 started (`auto` -> `night`)",
        ),
    );

    // team.
    rows.row(
        "team: two logins",
        p(&switches(Surface::Local, &on, &two), &base),
        ("team", "[team] names 2 logins: auto stops at two"),
    );
    rows.row(
        "team: a hub with none",
        held(&judge_proposal(
            &switches(Surface::Hub, &hub, &none),
            None,
            &base,
            Some(0),
            NOW,
        )),
        (
            "team",
            "[team] names no login: a hub delegates only to its sole owner",
        ),
    );
    rows.row(
        "team: a hub's sole lead",
        held(&judge_proposal(
            &switches(Surface::Hub, &hub, &lead),
            None,
            &base,
            Some(0),
            NOW,
        )),
        (
            "team",
            "the sole [team] login `mira` is a lead: a hub delegates only to the owner",
        ),
    );
    rows.row(
        "team: does not read",
        p(
            &Switches {
                logins: Err("`[team]` does not read: specengine.toml:30: bad"),
                ..local
            },
            &base,
        ),
        ("team", "`[team]` does not read: specengine.toml:30: bad"),
    );

    // stopped.
    rows.row(
        "stopped",
        p(
            &Switches {
                stopped: Some("/data/specengine/auto-stop"),
                ..local
            },
            &base,
        ),
        (
            "stopped",
            "the stop file /data/specengine/auto-stop exists: spec auto resume removes it",
        ),
    );

    // not-in-pack: a proposal and a task.
    rows.row(
        "not-in-pack: a proposal",
        held(&judge_proposal(&local, None, &base, None, NOW)),
        ("not-in-pack", "`PR-0007` is bound to no live pack's task"),
    );
    rows.row(
        "not-in-pack: a task",
        held(&judge_approve(&local, &task(TaskStatus::Review, None), NOW)),
        ("not-in-pack", "`T-0042` is in no live pack"),
    );

    // author: a person's, another login's, no login.
    for (name, author_type, login) in [
        ("author: a person", AuthorType::Human, Some("ann")),
        ("author: another login", AuthorType::Agent, Some("mira")),
        ("author: no login", AuthorType::Agent, None),
    ] {
        let facts = ProposalFacts {
            author_type,
            author_login: login.map(str::to_owned),
            ..update()
        };
        rows.row(
            name,
            held(&judge_proposal(&on_hub, None, &facts, Some(0), NOW)),
            ("author", "`PR-0007` was not raised over /mcp by `ann`"),
        );
    }

    // until: an approve after the pack's end; a land still runs.
    let ended = PackFacts {
        until: Some("2026-11-16T00:59:59Z"),
        ..pack(&on)
    };
    rows.row(
        "until",
        held(&judge_approve(
            &local,
            &task(TaskStatus::Review, Some(ended)),
            NOW,
        )),
        ("until", "pack 812 ended at 2026-11-16T00:59:59Z"),
    );
    rows.row(
        "until: a land still runs",
        held(&judge_land_paths(
            &local,
            &task(TaskStatus::Done, Some(ended)),
            &["src/lib.rs".to_owned()],
        )),
        ("apply", ""),
    );

    // status: each act's state.
    rows.row(
        "status: approve",
        held(&judge_approve(
            &local,
            &task(TaskStatus::Draft, Some(pack(&on))),
            NOW,
        )),
        ("status", "`T-0042` is draft: `approve` needs review"),
    );
    rows.row(
        "status: land",
        held(&judge_land_paths(
            &local,
            &task(TaskStatus::Review, Some(pack(&on))),
            &[],
        )),
        ("status", "`T-0042` is review: `land` needs done"),
    );

    // run-outcome.
    let failed = TaskFacts {
        last_run: Some((2, RunOutcome::Failed)),
        ..task(TaskStatus::Done, Some(pack(&on)))
    };
    rows.row(
        "run-outcome: failed",
        held(&judge_land_paths(&local, &failed, &[])),
        (
            "run-outcome",
            "`T-0042`'s run 2 ended failed, not completed",
        ),
    );
    let runless = TaskFacts {
        last_run: None,
        ..task(TaskStatus::Done, Some(pack(&on)))
    };
    rows.row(
        "run-outcome: no run",
        held(&judge_land_paths(&local, &runless, &[])),
        ("run-outcome", "`T-0042` has no run"),
    );

    // proposal-kind: every deciding kind.
    for (kind, detail) in [
        ("question", "a question waits for a person"),
        ("discrepancy", "a discrepancy waits for a person"),
        ("decision", "a decision waits for a person"),
        ("interpretation", "an interpretation waits for a person"),
        ("amendment", "an amendment waits for a person"),
        ("drift", "a drift waits for a person"),
    ] {
        let facts = ProposalFacts {
            kind: kind.to_owned(),
            ..update()
        };
        rows.row(
            "proposal-kind",
            p(&local, &facts),
            ("proposal-kind", detail),
        );
    }

    // staged.
    for (staged, detail) in [
        (
            StagedAs::Reject,
            "`PR-0007` has a reject staged at 2026-11-16T00:30:00Z: a person is deciding",
        ),
        (
            StagedAs::Approve,
            "`PR-0007` has an approve staged at 2026-11-16T00:30:00Z: a person is deciding",
        ),
    ] {
        let facts = ProposalFacts {
            staged: Some((staged, "2026-11-16T00:30:00Z".to_owned())),
            ..update()
        };
        rows.row("staged", p(&local, &facts), ("staged", detail));
    }

    // config-target: a config at any depth and case, the root's lock.
    for path in [
        "specengine.toml",
        "docs/spec/SpecEngine.TOML",
        "docs/narrative/specengine.toml",
        "spec.lock",
    ] {
        let facts = ProposalFacts {
            paths: vec![path.to_owned()],
            ..update()
        };
        let detail = format!("`{path}` is a project config or the lock: never delegated");
        rows.row(
            "config-target",
            p(&local, &facts),
            ("config-target", detail),
        );
    }

    // excluded-zone, excluded-kind, uncovered.
    let narrative = ProposalFacts {
        paths: vec!["docs/narrative/intro.md".to_owned()],
        ids: vec!["R-02".to_owned()],
        ..update()
    };
    rows.row(
        "excluded-zone",
        p(&local, &narrative),
        (
            "excluded-zone",
            "`docs/narrative/intro.md` is in the excluded zone `narrative`",
        ),
    );
    let scene = ProposalFacts {
        kind: "create".to_owned(),
        ids: vec!["R-02".to_owned(), "SC-002".to_owned()],
        ..update()
    };
    rows.row(
        "excluded-kind",
        p(&local, &scene),
        ("excluded-kind", "`SC-002` is a scenario, an excluded kind"),
    );
    let outside = ProposalFacts {
        kind: "create".to_owned(),
        paths: vec!["notes/plan.md".to_owned()],
        ids: vec!["R-03".to_owned()],
        ..update()
    };
    rows.row(
        "uncovered",
        p(&local, &outside),
        (
            "uncovered",
            "`notes/plan.md` is in no zone the rule covers (code, docs, tests)",
        ),
    );
    let unresolved = TaskFacts {
        unresolved: vec!["R-09".to_owned()],
        ..task(TaskStatus::Review, Some(pack(&on)))
    };
    rows.row(
        "uncovered: an ID with no file",
        held(&judge_approve(&local, &unresolved, NOW)),
        ("uncovered", "`R-09` resolves to no file: uncovered"),
    );

    // findings.
    let found = ProposalFacts {
        findings: vec![
            (
                "ref-dangling".to_owned(),
                "docs/spec/core.md".to_owned(),
                19,
            ),
            (
                "mention-dangling".to_owned(),
                "docs/spec/core.md".to_owned(),
                20,
            ),
        ],
        ..update()
    };
    rows.row(
        "findings",
        p(&local, &found),
        (
            "findings",
            "2 finding(s) stored at creation, the first ref-dangling at docs/spec/core.md:19",
        ),
    );

    // rebases: each text state.
    for (text, path, detail) in [
        (
            TextState::Rebases,
            "docs/spec/core.md",
            "its span changed since its base: it would rebase".to_owned(),
        ),
        (
            TextState::Conflict,
            "docs/spec/core.md",
            "it conflicts with the span now".to_owned(),
        ),
        (
            TextState::Gone,
            "docs/spec/core.md",
            "its target is gone".to_owned(),
        ),
        (
            TextState::FileExists,
            "docs/spec/more.md",
            "`docs/spec/more.md` exists: a new file's create needs none".to_owned(),
        ),
    ] {
        let facts = ProposalFacts {
            text,
            paths: vec![path.to_owned()],
            ..update()
        };
        rows.row("rebases", p(&local, &facts), ("rebases", detail));
    }

    // cap: a hub's, the count at the cap; one below applies.
    rows.row(
        "cap",
        held(&judge_proposal(&on_hub, None, &base, Some(20), NOW)),
        (
            "cap",
            "20 delegated applies today (UTC), the cap max_applies 20",
        ),
    );
    let capped = rule_of(
        "mode = \"hub\"\nzones = [\"docs\"]\nexclude_zones = [\"narrative\"]\nmax_applies = 1\n",
    );
    rows.row(
        "cap: max_applies 1",
        held(&judge_proposal(
            &switches(Surface::Hub, &capped, &one),
            None,
            &base,
            Some(1),
            NOW,
        )),
        (
            "cap",
            "1 delegated applies today (UTC), the cap max_applies 1",
        ),
    );
    // The cap is a hub's alone, `author` too: locally neither is judged.
    rows.row(
        "no cap locally",
        held(&judge_proposal(&local, Some(&live), &base, Some(1000), NOW)),
        ("apply", ""),
    );
    let persons = ProposalFacts {
        author_type: AuthorType::Human,
        author_login: None,
        ..update()
    };
    rows.row("no author locally", p(&local, &persons), ("apply", ""));

    let failures: Vec<String> = rows
        .0
        .iter()
        .filter(|(_, got, want)| got.0 != want.0 || got.1 != want.1)
        .map(|(name, got, want)| format!("{name}: got {got:?}, want {want:?}"))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let reasons: std::collections::BTreeSet<&str> =
        rows.0.iter().map(|(_, got, _)| got.0).collect();
    for reason in &Reason::ALL[..17] {
        assert!(
            reasons.contains(reason.as_str()),
            "no row gives {}",
            reason.as_str()
        );
    }
}

/// A verdict as compared: the reason (`apply` for none) and the detail.
type Judged = (&'static str, String);

/// RL2's rows: a name, the verdict got, the one wanted.
#[derive(Default)]
struct Rows(Vec<(&'static str, Judged, Judged)>);

impl Rows {
    fn row(
        &mut self,
        name: &'static str,
        got: (&'static str, String),
        want: (&'static str, impl Into<String>),
    ) {
        self.0.push((name, got, (want.0, want.1.into())));
    }
}

/// `is_config_target`: the gate's controlled-config rule.
#[test]
fn config_targets() {
    for path in [
        "specengine.toml",
        "SPECENGINE.TOML",
        "a/b/specengine.toml",
        "docs/Specengine.Toml",
        "spec.lock",
    ] {
        assert!(is_config_target(path), "{path}");
    }
    for path in [
        "docs/spec/core.md",
        "specengine.toml.bak",
        "docs/spec.lock",
        "my-specengine.toml",
    ] {
        assert!(!is_config_target(path), "{path}");
    }
}

/// RL4: one `[team]` login, on; two, `team`; a hub's sole `lead`, `team`.
/// M: `> 2` (two logins pass).
#[test]
fn rl4_a_team_turns_it_off() {
    let on = rule("on.toml");
    let one = logins(&[("ann", TeamRole::Owner)]);
    let member = logins(&[("ann", TeamRole::Member)]);
    let two = logins(&[("ann", TeamRole::Owner), ("ben", TeamRole::Owner)]);
    let three = logins(&[
        ("ann", TeamRole::Owner),
        ("ben", TeamRole::Member),
        ("cy", TeamRole::Member),
    ]);
    let lead = logins(&[("mira", TeamRole::Lead)]);
    assert_eq!(judge_switches(&switches(Surface::Local, &on, &one)), Ok(()));
    // Locally a sole login's role is not judged: the terminal is the owner's.
    assert_eq!(
        judge_switches(&switches(Surface::Local, &on, &member)),
        Ok(())
    );
    assert_eq!(judge_switches(&switches(Surface::Local, &on, &[])), Ok(()));
    for (team, n) in [(&two, 2), (&three, 3)] {
        for surface in [Surface::Local, Surface::Hub] {
            assert_eq!(
                judge_switches(&switches(surface, &on, team)),
                Err((
                    Reason::Team,
                    format!("[team] names {n} logins: auto stops at two")
                )),
                "{surface:?}"
            );
        }
    }
    assert_eq!(judge_switches(&switches(Surface::Hub, &on, &one)), Ok(()));
    assert_eq!(
        judge_switches(&switches(Surface::Hub, &on, &lead)),
        Err((
            Reason::Team,
            "the sole [team] login `mira` is a lead: a hub delegates only to the owner".to_owned()
        ))
    );
    assert_eq!(
        judge_switches(&switches(Surface::Hub, &on, &member)),
        Err((
            Reason::Team,
            "the sole [team] login `ann` is a member: a hub delegates only to the owner".to_owned()
        ))
    );
    // Every item held `team`: a task's approve too.
    let task = task(TaskStatus::Review, Some(pack(&on)));
    assert_eq!(
        held(&judge_approve(
            &switches(Surface::Local, &on, &two),
            &task,
            NOW
        )),
        (
            "team",
            "[team] names 2 logins: auto stops at two".to_owned()
        )
    );
}

/// The switches' order: `off` before `team` before `stopped`.
#[test]
fn the_switches_judge_in_order() {
    let on = rule("on.toml");
    let off = rule("off.toml");
    let two = logins(&[("ann", TeamRole::Owner), ("ben", TeamRole::Owner)]);
    let stopped = Switches {
        stopped: Some("/x/auto-stop"),
        ..switches(Surface::Local, &off, &two)
    };
    assert_eq!(
        judge_switches(&stopped).unwrap_err().0,
        Reason::Off,
        "off first"
    );
    let stopped = Switches {
        rule: Ok(&on),
        ..stopped
    };
    assert_eq!(
        judge_switches(&stopped).unwrap_err().0,
        Reason::Team,
        "then team"
    );
    let one = logins(&[("ann", TeamRole::Owner)]);
    let stopped = Switches {
        logins: Ok(&one),
        ..stopped
    };
    assert_eq!(judge_switches(&stopped).unwrap_err().0, Reason::Stopped);
}

/// RL6: exclusion beats cover. `docs` covered and `narrative` excluded: a
/// narrative path is `excluded-zone`; SC-001 under `exclude_kinds` alone
/// is `excluded-kind`. M: the cover checked before the exclusions.
#[test]
fn rl6_exclusion_beats_cover() {
    let on = rule("on.toml");
    let one = logins(&[("ann", TeamRole::Owner)]);
    let local = switches(Surface::Local, &on, &one);
    let live = pack(&on);
    let narrative = ProposalFacts {
        paths: vec!["docs/narrative/intro.md".to_owned()],
        ids: vec!["SC-001".to_owned()],
        ..update()
    };
    assert_eq!(
        held(&judge_proposal(&local, Some(&live), &narrative, None, NOW)),
        (
            "excluded-zone",
            "`docs/narrative/intro.md` is in the excluded zone `narrative`".to_owned()
        ),
        "the zone is judged before the kind"
    );
    let kinds_alone = rule_of(
        "mode = \"both\"\nzones = [\"code\", \"tests\", \"docs\"]\nexclude_kinds = [\"scenario\"]\n",
    );
    let local = switches(Surface::Local, &kinds_alone, &one);
    let live = pack(&kinds_alone);
    assert_eq!(
        held(&judge_proposal(&local, Some(&live), &narrative, None, NOW)),
        (
            "excluded-kind",
            "`SC-001` is a scenario, an excluded kind".to_owned()
        )
    );
    // A task affecting SC-001: the same order.
    let affecting = TaskFacts {
        paths: vec![
            "docs/spec/core.md".to_owned(),
            "docs/narrative/intro.md".to_owned(),
        ],
        ids: vec!["R-02".to_owned(), "SC-001".to_owned()],
        ..task(TaskStatus::Review, Some(pack(&kinds_alone)))
    };
    assert_eq!(
        held(&judge_approve(&local, &affecting, NOW)),
        (
            "excluded-kind",
            "`SC-001` is a scenario, an excluded kind".to_owned()
        )
    );
    // A land's changed paths: an excluded zone beats the cover.
    let on_local = switches(Surface::Local, &on, &one);
    assert_eq!(
        held(&judge_land_paths(
            &on_local,
            &task(TaskStatus::Done, Some(pack(&on))),
            &[
                "src/lib.rs".to_owned(),
                "docs/narrative/intro.md".to_owned()
            ],
        )),
        (
            "excluded-zone",
            "`docs/narrative/intro.md` is in the excluded zone `narrative`".to_owned()
        )
    );
    // The first failing path in the facts' order is named.
    assert_eq!(
        held(&judge_land_paths(
            &on_local,
            &task(TaskStatus::Done, Some(pack(&on))),
            &["notes/a.md".to_owned(), "notes/b.md".to_owned()],
        )),
        (
            "uncovered",
            "`notes/a.md` is in no zone the rule covers (code, docs, tests)".to_owned()
        )
    );
}

/// RL8: frozen and now. Frozen `["code", "docs"]`, now `["code"]`: a docs
/// path is uncovered by the rule at HEAD; frozen `["code"]`, now `["code",
/// "docs"]`: uncovered by the rule at the pack's start; a changed branch,
/// `off`. M: judging the frozen rule alone; the rule now alone.
#[test]
fn rl8_frozen_and_now() {
    let one = logins(&[("ann", TeamRole::Owner)]);
    let wide = rule_of("mode = \"both\"\nzones = [\"code\", \"docs\"]\n");
    let narrow = rule_of("mode = \"both\"\nzones = [\"code\"]\n");
    let docs = ProposalFacts {
        paths: vec!["docs/spec/core.md".to_owned()],
        ..update()
    };

    // Narrowed since the start: HEAD's rule fails alone.
    let head_narrow = switches(Surface::Local, &narrow, &one);
    assert_eq!(
        held(&judge_proposal(
            &head_narrow,
            Some(&pack(&wide)),
            &docs,
            None,
            NOW
        )),
        (
            "uncovered",
            "`docs/spec/core.md` is in no zone the rule covers (code) (the rule at HEAD)"
                .to_owned()
        )
    );
    // Widened since the start: the frozen rule fails alone.
    let head_wide = switches(Surface::Local, &wide, &one);
    assert_eq!(
        held(&judge_proposal(&head_wide, Some(&pack(&narrow)), &docs, None, NOW)),
        (
            "uncovered",
            "`docs/spec/core.md` is in no zone the rule covers (code) (the rule at pack 812's start)"
                .to_owned()
        )
    );
    // Both cover: applies; neither: no suffix.
    assert_eq!(
        judge_proposal(&head_wide, Some(&pack(&wide)), &docs, None, NOW),
        Verdict::Apply
    );
    assert_eq!(
        held(&judge_proposal(
            &head_narrow,
            Some(&pack(&narrow)),
            &docs,
            None,
            NOW
        )),
        (
            "uncovered",
            "`docs/spec/core.md` is in no zone the rule covers (code)".to_owned()
        )
    );

    // An exclusion in either: the start's alone, HEAD's alone.
    let excluding = rule_of(
        "mode = \"both\"\nzones = [\"code\", \"docs\"]\nexclude_zones = [\"narrative\"]\nexclude_kinds = [\"scenario\"]\n",
    );
    let narrative = ProposalFacts {
        paths: vec!["docs/narrative/intro.md".to_owned()],
        ids: vec!["R-02".to_owned()],
        ..update()
    };
    assert_eq!(
        held(&judge_proposal(&head_wide, Some(&pack(&excluding)), &narrative, None, NOW)),
        (
            "excluded-zone",
            "`docs/narrative/intro.md` is in the excluded zone `narrative` (the rule at pack 812's start)"
                .to_owned()
        )
    );
    let head_excluding = switches(Surface::Local, &excluding, &one);
    assert_eq!(
        held(&judge_proposal(
            &head_excluding,
            Some(&pack(&wide)),
            &narrative,
            None,
            NOW
        )),
        (
            "excluded-zone",
            "`docs/narrative/intro.md` is in the excluded zone `narrative` (the rule at HEAD)"
                .to_owned()
        )
    );
    let scene = ProposalFacts {
        ids: vec!["SC-002".to_owned()],
        ..update()
    };
    assert_eq!(
        held(&judge_proposal(
            &head_wide,
            Some(&pack(&excluding)),
            &scene,
            None,
            NOW
        )),
        (
            "excluded-kind",
            "`SC-002` is a scenario, an excluded kind (the rule at pack 812's start)".to_owned()
        )
    );

    // The branch changed since the start: off.
    let other_branch =
        rule_of("mode = \"both\"\nzones = [\"code\", \"docs\"]\nbranch = \"nightly\"\n");
    assert_eq!(
        held(&judge_proposal(
            &switches(Surface::Local, &other_branch, &one),
            Some(&pack(&wide)),
            &docs,
            None,
            NOW
        )),
        (
            "off",
            "the rule's branch changed since pack 812 started (`auto` -> `nightly`)".to_owned()
        )
    );
    // A task's approve and land judge the same pair.
    let in_docs = TaskFacts {
        paths: vec!["docs/spec/core.md".to_owned()],
        ..task(TaskStatus::Review, Some(pack(&wide)))
    };
    assert_eq!(
        held(&judge_approve(&head_narrow, &in_docs, NOW)).0,
        "uncovered"
    );
    let done = task(TaskStatus::Done, Some(pack(&narrow)));
    assert_eq!(
        held(&judge_land_paths(&head_wide, &done, &["docs/spec/core.md".to_owned()])),
        (
            "uncovered",
            "`docs/spec/core.md` is in no zone the rule covers (code) (the rule at pack 812's start)"
                .to_owned()
        )
    );
}
