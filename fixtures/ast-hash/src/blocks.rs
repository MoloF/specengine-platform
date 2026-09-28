//! Closure bodies and match-arm values that rustfmt moves in and out of `{ }`
//! depending on `max_width`: `match_arm_blocks`, closure block wrapping in a
//! chain and inside the arguments of `vec![...]` and `assert!(...)`, and a
//! `return` arm that gains a `;`. The file is rustfmt-clean under the default
//! configuration; the contrasting `max_width = 60` wraps the long arms and
//! closure bodies in blocks, so recipe v2's transparent wrappers are what
//! keeps every hash stable here (`stable_pct.fmt_contrast` = 100).

pub enum Command {
    Move { dx: i64, dy: i64 },
    Rename(String),
    Halt,
}

pub struct Report {
    pub label: String,
    pub weight: u64,
}

fn describe_move_with_full_context(dx: i64, dy: i64, scale: u64) -> Report {
    Report {
        label: format!("move {dx} {dy}"),
        weight: scale,
    }
}

fn rename_with_prefix_and_suffix(name: &str, prefix: &str, suffix: &str) -> Report {
    Report {
        label: format!("{prefix}{name}{suffix}"),
        weight: name.len() as u64,
    }
}

fn halted_weight(fallback: u64, scale: u64) -> u64 {
    fallback.saturating_mul(scale)
}

fn has_weight(command: &Command, scale: u64) -> bool {
    summarize(command, scale).weight > 0
}

fn every(commands: &[Command], accept: impl Fn(&Command) -> bool) -> bool {
    commands.iter().all(accept)
}

pub fn summarize(command: &Command, scale: u64) -> Report {
    match command {
        Command::Move { dx, dy } => describe_move_with_full_context(*dx, *dy, scale),
        Command::Rename(name) => rename_with_prefix_and_suffix(name, "renamed:", "!"),
        Command::Halt => Report {
            label: String::from("halt"),
            weight: 0,
        },
    }
}

pub fn weight_or_default(command: &Command, fallback: u64, scale: u64) -> u64 {
    match command {
        Command::Move { dx, dy } => dx.unsigned_abs().max(dy.unsigned_abs()).max(fallback),
        Command::Rename(name) => name.len() as u64 + fallback,
        Command::Halt => return halted_weight(fallback, scale).max(fallback),
    }
}

pub fn total_weight(commands: &[Command], scale: u64) -> u64 {
    commands
        .iter()
        .map(|command| summarize(command, scale).weight)
        .sum()
}

pub fn labels_with_prefix(commands: &[Command], prefix: &str) -> Vec<String> {
    commands
        .iter()
        .map(|command| format!("{prefix}{}", summarize(command, 1).label))
        .collect()
}

pub fn first_heavy_index(commands: &[Command], threshold: u64) -> Option<usize> {
    commands
        .iter()
        .position(|command| weight_or_default(command, 0, 1) > threshold)
}

pub fn count_until_halt(commands: &[Command]) -> usize {
    let mut seen = 0;
    for command in commands {
        if matches!(command, Command::Halt) {
            break;
        } else {
            seen += 1;
        }
    }
    seen
}

pub fn skip_renames(commands: &[Command]) -> Vec<&Command> {
    let mut kept = Vec::new();
    for command in commands {
        let Command::Rename(_) = command else {
            kept.push(command);
            continue;
        };
    }
    kept
}

pub fn heavy_labels(commands: &[Command], threshold: u64) -> Vec<String> {
    commands
        .iter()
        .filter(|command| weight_or_default(command, 0, 1) > threshold)
        .map(|command| summarize(command, 1).label)
        .collect()
}

pub fn labels_in_macro(commands: &[Command], prefix: &str) -> Vec<String> {
    vec![
        commands
            .iter()
            .map(|command| format!("{prefix}{}", summarize(command, 1).label))
            .collect(),
    ]
}

pub fn all_have_weight(commands: &[Command], scale: u64) -> bool {
    assert!(
        commands
            .iter()
            .all(|command| summarize(command, scale).weight > 0 && scale > 0)
    );
    every(commands, |command| has_weight(command, 2))
}
