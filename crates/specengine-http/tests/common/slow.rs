//! The daemon's slow lines (docs/features/slow-requests.md "Data",
//! **Lines**), read off its stderr and parsed against the grammar: the keys
//! in order, each value in its charset, `at` to the millisecond, no byte
//! outside 0x20-0x7E, never two lines run together. docs/features/
//! hub-turn-fairness.md (RM-0119) appends `class=` to every line and
//! `line_ms=`, `ahead=` to a request line; a line is taken with or without
//! them, but never a mix in one daemon's run. docs/features/write-cost.md
//! (RM-0125) "Attribution: five fields" appends ` check_ms= index_ms= git_ms=
//! git= write_ms=` after `ahead` (each `<n|->`) to every request line of a
//! build that has them, never to a turn line: a request line is taken with
//! RM-0119's keys alone or with those and the five, again never a mix.
//! Shared by `tests/slow.rs`, `tests/hub_fairness.rs` and
//! `tests/write_cost_slow.rs`.

use std::ffi::OsStr;
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;

use super::Server;

/// How long a line may take to reach stderr after its cause.
pub const LINE_WAIT: Duration = Duration::from_secs(60);

/// A quiet spell after the last expected line, for a line too many to show.
pub const SETTLE: Duration = Duration::from_millis(500);

// ------------------------------------------------------------ the lines

/// A request line's keys, "Data", in order.
const REQUEST_KEYS: [&str; 11] = [
    "at", "method", "route", "project", "id", "tool", "status", "ms", "wait_ms", "run_ms", "index",
];

/// A turn line's keys, "Data", in order.
const TURN_KEYS: [&str; 7] = ["at", "project", "by", "id", "tool", "state", "held_ms"];

/// Appended to a request line, and `class` alone to a turn line:
/// docs/features/hub-turn-fairness.md "The Rust half: a line per login".
const REQUEST_FAIR_KEYS: [&str; 3] = ["class", "line_ms", "ahead"];

/// Appended after RM-0119's keys to a request line: docs/features/
/// write-cost.md "Attribution: five fields".
const REQUEST_COST_KEYS: [&str; 5] = ["check_ms", "index_ms", "git_ms", "git", "write_ms"];

static AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$").unwrap());
static METHOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z]{1,16}|\?)$").unwrap());
/// A route template or `-`: never a `%`, a `?` or a query.
static ROUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(/[A-Za-z0-9/{}*._-]*|-)$").unwrap());
static SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z0-9._-]{1,64}|\?|-)$").unwrap());
static TOOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z0-9._/-]{1,64}|\?|-)$").unwrap());
static STATUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{3}|gone)$").unwrap());
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static COUNT_OR_DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+|-)$").unwrap());

/// RM-0119's appended keys, when the build has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fair {
    pub class: String,
    pub line_ms: Option<u64>,
    pub ahead: Option<u64>,
}

/// One `slow request` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub line: String,
    pub at: String,
    pub method: String,
    pub route: String,
    pub project: String,
    pub id: String,
    pub tool: String,
    pub status: String,
    pub ms: u64,
    pub wait_ms: Option<u64>,
    pub run_ms: Option<u64>,
    pub index: String,
    pub fair: Option<Fair>,
    /// RM-0125's five fields, when the build has them.
    pub cost: Option<Cost>,
}

/// RM-0125's five fields of a request line, `None` for `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cost {
    pub check_ms: Option<u64>,
    pub index_ms: Option<u64>,
    pub git_ms: Option<u64>,
    pub git: Option<u64>,
    pub write_ms: Option<u64>,
}

/// One `slow turn` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub line: String,
    pub at: String,
    pub project: String,
    pub by: String,
    pub id: String,
    pub tool: String,
    pub state: String,
    pub held_ms: u64,
    pub class: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slow {
    Request(Request),
    Turn(Turn),
}

fn count(value: &str) -> u64 {
    value.parse().expect("digits")
}

fn count_or_dash(value: &str) -> Option<u64> {
    (value != "-").then(|| count(value))
}

/// `key=value` pairs separated by one space each, their keys exactly `keys`,
/// `keys` then `fair`, or (a request line) `keys`, `fair`, then
/// [`REQUEST_COST_KEYS`]: the values, and whether `fair` came.
fn pairs<'a>(rest: &'a str, keys: &[&str], fair: &[&str], line: &str) -> (Vec<&'a str>, bool) {
    let found: Vec<(&str, &str)> = rest
        .split(' ')
        .map(|pair| {
            pair.split_once('=')
                .unwrap_or_else(|| panic!("a slow line's part with no `=`: {pair:?} in {line:?}"))
        })
        .collect();
    let names: Vec<&str> = found.iter().map(|(name, _)| *name).collect();
    let with_fair: Vec<&str> = keys.iter().chain(fair).copied().collect();
    let with_cost: Vec<&str> = with_fair.iter().copied().chain(REQUEST_COST_KEYS).collect();
    let fair_too = if names == keys {
        false
    } else if names == with_fair || (keys == REQUEST_KEYS && names == with_cost) {
        true
    } else {
        panic!("a slow line's keys {names:?}, not {keys:?} (nor with {fair:?}): {line:?}");
    };
    for (name, value) in &found {
        assert!(
            !value.is_empty(),
            "a slow line's {name} is empty, not `-`: {line:?}"
        );
    }
    (
        found.into_iter().map(|(_, value)| value).collect(),
        fair_too,
    )
}

#[track_caller]
fn matching(regex: &Regex, key: &str, value: &str, line: &str) {
    assert!(
        regex.is_match(value),
        "a slow line's {key}={value:?} is outside its form: {line:?}"
    );
}

/// "Data", **Lines**: one line, parsed and checked.
pub fn parse(line: &str) -> Slow {
    assert!(
        line.bytes().all(|byte| (0x20..=0x7e).contains(&byte)),
        "a slow line holds a byte outside 0x20-0x7E: {line:?}"
    );
    assert!(!line.contains("  "), "two spaces in a slow line: {line:?}");
    if let Some(rest) = line.strip_prefix("specengine-http: slow request ") {
        let (values, fair) = pairs(rest, &REQUEST_KEYS, &REQUEST_FAIR_KEYS, line);
        matching(&AT, "at", values[0], line);
        matching(&METHOD, "method", values[1], line);
        matching(&ROUTE, "route", values[2], line);
        matching(&SEGMENT, "project", values[3], line);
        matching(&SEGMENT, "id", values[4], line);
        matching(&TOOL, "tool", values[5], line);
        matching(&STATUS, "status", values[6], line);
        matching(&COUNT, "ms", values[7], line);
        matching(&COUNT_OR_DASH, "wait_ms", values[8], line);
        matching(&COUNT_OR_DASH, "run_ms", values[9], line);
        assert!(
            ["walked", "trusted", "-"].contains(&values[10]),
            "index={}: {line:?}",
            values[10]
        );
        let fair = fair.then(|| {
            assert!(
                ["page", "mcp", "sync"].contains(&values[11]),
                "class={}: {line:?}",
                values[11]
            );
            matching(&COUNT_OR_DASH, "line_ms", values[12], line);
            matching(&COUNT_OR_DASH, "ahead", values[13], line);
            Fair {
                class: values[11].to_owned(),
                line_ms: count_or_dash(values[12]),
                ahead: count_or_dash(values[13]),
            }
        });
        let cost = (values.len() == REQUEST_KEYS.len() + 3 + REQUEST_COST_KEYS.len()).then(|| {
            for (offset, key) in REQUEST_COST_KEYS.iter().enumerate() {
                matching(&COUNT_OR_DASH, key, values[14 + offset], line);
            }
            Cost {
                check_ms: count_or_dash(values[14]),
                index_ms: count_or_dash(values[15]),
                git_ms: count_or_dash(values[16]),
                git: count_or_dash(values[17]),
                write_ms: count_or_dash(values[18]),
            }
        });
        Slow::Request(Request {
            line: line.to_owned(),
            at: values[0].to_owned(),
            method: values[1].to_owned(),
            route: values[2].to_owned(),
            project: values[3].to_owned(),
            id: values[4].to_owned(),
            tool: values[5].to_owned(),
            status: values[6].to_owned(),
            ms: count(values[7]),
            wait_ms: count_or_dash(values[8]),
            run_ms: count_or_dash(values[9]),
            index: values[10].to_owned(),
            fair,
            cost,
        })
    } else if let Some(rest) = line.strip_prefix("specengine-http: slow turn ") {
        let (values, fair) = pairs(rest, &TURN_KEYS, &["class"], line);
        matching(&AT, "at", values[0], line);
        matching(&SEGMENT, "project", values[1], line);
        assert!(
            values[2] == "sync" || (ROUTE.is_match(values[2]) && values[2] != "-"),
            "by={}: a route template or `sync`: {line:?}",
            values[2]
        );
        matching(&SEGMENT, "id", values[3], line);
        matching(&TOOL, "tool", values[4], line);
        assert!(
            ["held", "released"].contains(&values[5]),
            "state={}: {line:?}",
            values[5]
        );
        matching(&COUNT, "held_ms", values[6], line);
        let class = fair.then(|| {
            assert!(
                ["page", "mcp", "sync"].contains(&values[7]),
                "class={}: {line:?}",
                values[7]
            );
            values[7].to_owned()
        });
        Slow::Turn(Turn {
            line: line.to_owned(),
            at: values[0].to_owned(),
            project: values[1].to_owned(),
            by: values[2].to_owned(),
            id: values[3].to_owned(),
            tool: values[4].to_owned(),
            state: values[5].to_owned(),
            held_ms: count(values[6]),
            class,
        })
    } else {
        panic!("a slow line of neither form: {line:?}")
    }
}

/// Every complete stderr line that is a slow line, parsed; a line holding a
/// slow line's start anywhere but at its own start is two lines run
/// together. All of one form: RM-0118's alone, or each with RM-0119's keys.
pub fn slow_lines(stderr: &str) -> Vec<Slow> {
    let complete = match stderr.rfind('\n') {
        Some(end) => &stderr[..end],
        None => "",
    };
    let mut lines = Vec::new();
    for line in complete.split('\n') {
        if line.starts_with("specengine-http: slow ") {
            lines.push(parse(line));
        } else {
            assert!(
                !line.contains("specengine-http: slow ")
                    && !line.contains(" slow request ")
                    && !line.contains(" slow turn "),
                "a slow line inside another line: {line:?}"
            );
        }
    }
    let fair: Vec<bool> = lines
        .iter()
        .map(|slow| match slow {
            Slow::Request(request) => request.fair.is_some(),
            Slow::Turn(turn) => turn.class.is_some(),
        })
        .collect();
    assert!(
        fair.iter().all(|one| *one) || fair.iter().all(|one| !*one),
        "some slow lines carry hub-turn-fairness' keys and some do not:\n{stderr}"
    );
    let cost: Vec<bool> = requests(&lines)
        .iter()
        .map(|request| request.cost.is_some())
        .collect();
    assert!(
        cost.iter().all(|one| *one) || cost.iter().all(|one| !*one),
        "some request lines carry write-cost's five fields and some do not:\n{stderr}"
    );
    lines
}

pub fn requests(lines: &[Slow]) -> Vec<Request> {
    lines
        .iter()
        .filter_map(|slow| match slow {
            Slow::Request(request) => Some(request.clone()),
            Slow::Turn(_) => None,
        })
        .collect()
}

pub fn turns(lines: &[Slow]) -> Vec<Turn> {
    lines
        .iter()
        .filter_map(|slow| match slow {
            Slow::Turn(turn) => Some(turn.clone()),
            Slow::Request(_) => None,
        })
        .collect()
}

/// The first `found` over the daemon's slow lines so far, within `within`.
#[track_caller]
pub fn wait_for<T>(
    server: &Server,
    within: Duration,
    what: &str,
    mut found: impl FnMut(&[Slow]) -> Option<T>,
) -> T {
    let started = Instant::now();
    loop {
        let stderr = server.stderr();
        if let Some(value) = found(&slow_lines(&stderr)) {
            return value;
        }
        assert!(
            started.elapsed() < within,
            "{what}: not on stderr within {within:?}\n--- the daemon's stderr\n{stderr}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// The request lines of `route`, once `n` of them are on stderr and a quiet
/// [`SETTLE`] has passed.
#[track_caller]
pub fn requests_of(server: &Server, route: &str, n: usize) -> Vec<Request> {
    wait_for(
        server,
        LINE_WAIT,
        &format!("{n} request lines of {route}"),
        |lines| (requests(lines).iter().filter(|r| r.route == route).count() >= n).then_some(()),
    );
    thread::sleep(SETTLE);
    requests(&slow_lines(&server.stderr()))
        .into_iter()
        .filter(|request| request.route == route)
        .collect()
}

/// The turn lines `by` a holder.
pub fn turns_by(server: &Server, by: &str) -> Vec<Turn> {
    turns(&slow_lines(&server.stderr()))
        .into_iter()
        .filter(|turn| turn.by == by)
        .collect()
}

/// `--slow-ms value` after `args`.
pub fn slow_ms<'a>(mut args: Vec<&'a OsStr>, value: &'a str) -> Vec<&'a OsStr> {
    args.push(OsStr::new("--slow-ms"));
    args.push(OsStr::new(value));
    args
}
