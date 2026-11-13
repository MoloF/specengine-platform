//! A no verdict by deadline, asked again: docs/features/release-hardening.md
//! "Acceptance criteria" RH-T3. `spec gate` answers within `GATE_DEADLINE`
//! of wall clock; a run a loaded machine starves past it says only that it
//! gave no answer, so a test that expects a verdict asks once more, at most
//! [`VERDICT_TRIES`] times. Any other answer, a refusal or another no
//! verdict included, is taken at once; a test that asserts a no verdict
//! never asks through here.

/// The most asks of one verdict.
pub const VERDICT_TRIES: usize = 5;

/// A no verdict by deadline: stdout holds `"reason":"deadline"` (the JSON
/// document) or stderr `no answer within` (the binary's line and the
/// wrapper's); no other no verdict.
pub fn starved(stdout: &str, stderr: &str) -> bool {
    stdout.contains("\"reason\":\"deadline\"") || stderr.contains("no answer within")
}

/// Calls `ask` until its answer is not [`starved`] (its streams read by
/// `streams`), at most [`VERDICT_TRIES`] times: the last answer.
pub fn until_verdict<R>(mut ask: impl FnMut() -> R, streams: impl Fn(&R) -> (&str, &str)) -> R {
    let mut answer = ask();
    for _ in 1..VERDICT_TRIES {
        let (stdout, stderr) = streams(&answer);
        if !starved(stdout, stderr) {
            break;
        }
        answer = ask();
    }
    answer
}
