//! `spec verify --node REF [--symbol QPATH]` (task spec `verify-node`;
//! canon `spec-verify-scope`, "`--node`"): plain verify's rows kept to one
//! node's bindings, judged only over the files the code index picks.
//!
//! In order: discovery (its failure: exit 2, stdout empty); REF resolved as
//! `spec show` resolves it, over the spec index refreshed (a REF naming
//! nothing, a look-alike, `project:`, `HOME` or the data directory: exit 2,
//! stdout empty, `--json` too; several holders: one `warning:`, each
//! verified); then plain's run causes in plain's order, the `cannot:` lines
//! of the printed report (exit 2): (1) also an unknown `[code] frameworks`
//! name (the code index's one reader of the key refuses it: plain never
//! reads it), (3) the refreshed code index's first gap anywhere in the
//! tree, its text the walk's, (8) the code index's refresh failing.
//!
//! With no cause, the node-set judge ([`crate::bindings::judge_set`], the
//! one a read's bindings take) over the holders: the selection, from one
//! read snapshot of the refreshed code index: the files of the stored
//! markers whose ID `spec refs` lists under a holder, and of the lock
//! entries of the rows that can be the holders' (the lock is cut to them,
//! so no other node's entry is judged `file_gone`). Each selected file is
//! read again, its BLAKE3 compared with the stored one (a file edited since
//! the refresh: the code index is refreshed once more and the selection
//! read again; edited again: (3)); then bound or anchored afresh, its Rust
//! names the stored ones (`partial_tree`), and judged by plain's judge
//! (`judge_walk`: levels hashed afresh, never the index's columns; the
//! pairing per node, so the node's own files suffice). The rows kept are
//! "A row in a set" over the holders, both halves, never their sections or
//! parents; `--symbol` keeps those whose key's symbol is QPATH byte for
//! byte (a RON row's `<file>#<symbol>` too), or whose moved anchor's is.
//! Counts, cascade, `delta`, hints, verdict and exit are the kept rows';
//! `skipped` 0; no row: `clean`, with a note. Nothing is written but the
//! data directory (the index refreshes, as `spec refs`).

use std::collections::BTreeSet;

use specengine_core::lock::LockKind;
use specengine_core::verify::RowBinding;

use crate::bindings::{SetScope, judge_set, refreshed_code};
use crate::code_index::known_frameworks;
use crate::corpus::indexed_in;
use crate::location::open_index;
use crate::node_set::NodeSet;
use crate::project::discover_with_text;
use crate::proposals::escaped_error;
use crate::refs::{Corpus, NodeKey};
use crate::show::classify;
use crate::verify::{
    Gathered, PlainReads, VerifyNode, VerifyOutcome, VerifyRequest, concluded, plain_reads,
    unprefixed,
};
use crate::verify_rows::{Anchor, CAUSE_CONFIG, CAUSE_GIT, HeadMode, Row, RunCause};
use crate::{CliError, Env, Globals, Message, one_line};

/// Why a written `@rev` is ignored.
const REV_IGNORED: &str = "`spec verify --node` judges each binding against the node as it is now";

/// `spec verify --node REF [--symbol QPATH]`: `reference` is REF as
/// written; the request's other forms were refused before.
pub(crate) fn verify_node(
    env: &Env,
    globals: &Globals,
    reference: &str,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let written = reference.trim();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();

    // REF before any run cause: its refusal exits 2, stdout empty.
    let target = classify(written, scheme, &mut messages, REV_IGNORED)
        .map_err(escaped_error)?
        .map_err(refused)?;
    let mut open = open_index(env, &project).map_err(escaped_error)?;
    let input =
        indexed_in(&mut open.index, &project, &mut messages, true).map_err(escaped_error)?;
    let (holders, ids) = {
        let corpus = Corpus::new(&input, scheme, &project.config.paths);
        // In holder order: (path, position).
        let holders: Vec<NodeKey> = corpus
            .target_nodes(&target, written)
            .map_err(escaped_error)?
            .map_err(refused)?
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let set = NodeSet::of(&corpus, scheme, holders.iter().copied());
        if holders.len() > 1 {
            let cited: Vec<String> = holders.iter().map(|&key| corpus.citation(key)).collect();
            messages.push(Message::Warning(format!(
                "`{written}` has {} holders, all verified: {}",
                holders.len(),
                cited.join(", ")
            )));
        }
        let ids: Vec<String> = holders
            .iter()
            .map(|&key| {
                set.canonical(key)
                    .map_or_else(|| corpus.citation(key), str::to_owned)
            })
            .collect();
        (holders, ids)
    };
    let node = VerifyNode {
        reference: written.to_owned(),
        ids,
        symbol: request.symbol.clone(),
    };

    // Plain's run causes; (1) also an unknown `[code] frameworks` name.
    let mut causes: Vec<RunCause> = Vec::new();
    let label = &project.config_label;
    let PlainReads {
        code,
        configured,
        git,
        lock,
    } = plain_reads(&project, &text, &request.git, &mut causes);
    let code = code.and_then(|code| match known_frameworks(&code, label) {
        Ok(()) => Some(code),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, unprefixed(&error)));
            None
        }
    });

    // (3), (8): the code index refreshed, its gap anywhere a cause.
    let notes_at = messages.len();
    let refreshed = match &code {
        Some(code) => refreshed_code(&mut open.index, &project, code, &mut messages)
            .map_err(|cause| causes.push(cause))
            .is_ok(),
        None => false,
    };

    let mut judged = None;
    match (causes.is_empty() && refreshed, code, lock, git) {
        (true, Some(code), Some(lock), Some(git)) => {
            let scope = SetScope {
                project: &project,
                code: &code,
                input: &input,
                lock: &lock,
                keys: &holders,
            };
            match judge_set(&scope, &mut open.index, &git, &mut messages, notes_at) {
                Ok(set) => judged = Some(set.judged),
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = git.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some(git)) => {
            let _ = git.finish();
        }
        _ => {}
    }
    if let Some(judged) = judged.as_mut().filter(|_| causes.is_empty()) {
        if let Some(symbol) = &node.symbol {
            judged.rows.retain(|row| on_symbol(row, symbol));
        }
        judged.skipped = 0;
        if judged.rows.is_empty() {
            let on = node
                .symbol
                .as_deref()
                .map(|symbol| format!(" on {symbol}"))
                .unwrap_or_default();
            messages.push(Message::Note(format!(
                "verify: {written} has no binding{on}"
            )));
        }
    }
    Ok(concluded(Gathered {
        configured,
        has_base: false,
        base: None,
        task: None,
        node: Some(node),
        head_mode: HeadMode::Absent,
        judged,
        causes,
        messages,
        label,
        tests: None,
        symbols: None,
    }))
}

/// A REF that names nothing: exit 2, its reason on one line.
fn refused(reason: String) -> CliError {
    escaped_error(CliError::spec(one_line(&reason)))
}

/// Whether `--symbol`'s QPATH names a row: its key's symbol, byte for byte
/// (a RON row's `<file>#<symbol>` too), or its moved anchor's old one.
fn on_symbol(row: &Row, qpath: &str) -> bool {
    let names = |file: &str, kind: LockKind, symbol: &str| {
        symbol == qpath
            || (kind == LockKind::Ron
                && qpath
                    .strip_prefix(file)
                    .and_then(|rest| rest.strip_prefix('#'))
                    == Some(symbol))
    };
    let own = match &row.key.binding {
        RowBinding::Keyed { kind, symbol, .. } => names(&row.key.file, *kind, symbol),
        RowBinding::Keyless { .. } => false,
    };
    own || matches!(&row.anchor, Some(Anchor::Moved(from)) if names(&from.file, from.kind, &from.symbol))
}
