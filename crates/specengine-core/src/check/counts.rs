//! How often the process ran the check's engine and parsed a file
//! (`write-cost`, RM-0125): process-wide, monotonic counters a test reads
//! before and after a call, the deltas its cost. `Relaxed` atomics: a
//! count, never an order.

use std::sync::atomic::{AtomicU64, Ordering};

/// Engine runs, whole ([`super::run`]) or over a set of files.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// Parses: a check file's own `parse` and the store's `parse_file`.
static PARSES: AtomicU64 = AtomicU64::new(0);

/// The two counters, read at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunCounts {
    /// Engine runs since the process started.
    pub runs: u64,
    /// File parses since the process started.
    pub parses: u64,
}

/// The counters now.
pub fn counts() -> RunCounts {
    RunCounts {
        runs: RUNS.load(Ordering::Relaxed),
        parses: PARSES.load(Ordering::Relaxed),
    }
}

/// One engine run more.
pub(crate) fn note_run() {
    RUNS.fetch_add(1, Ordering::Relaxed);
}

/// One parse more: the store's `parse_file` counts its own parses here.
pub fn note_parse() {
    PARSES.fetch_add(1, Ordering::Relaxed);
}
