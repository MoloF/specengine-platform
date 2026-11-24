//! docs/features/mixed-load-memory.md (RM-0134) MX2, MX5: the oracles, copied
//! verbatim from `queue.rs` at 540db8ff (0.49.2) before any change of the
//! slice: `raw_rows_in`, `readable` and `RawRow` (with its `decode`), the
//! inbox's read as it collected every row and column before decoding; and
//! `queue_matches` with `MatchRow`, the dedup as it read every row of the
//! family. Nothing here may change with the store: a rewrite is compared
//! against these. The helpers they call (`columns_of`, `index_only`,
//! `corrupt`, the column decoders) are the store's own. The copies are
//! byte for byte; [`list`] and [`matches`], at the end, are their entry points.
//! One addition since: queue schema 15's last column, `comment_id` (task spec
//! `comments`, "Queue schema 15"), taken and judged as `card_id` is, so the
//! copy still builds a [`Proposal`] and reads the rows the store reads; and
//! queue schema 17's `ref_id` (task spec `reference-promotion`), taken last.

use super::super::*;

// ------------------------------------------------- queue.rs:2424-2452 at 540db8ff

/// The project's proposal rows as stored, by ID number, of `filter`'s
/// repository when it names one; none in an index-only DB. Read inside
/// `conn`'s open transaction: [`SqliteQueue`]'s own, or a
/// [`QueueSnapshot`]'s (task spec `release-hardening` RH-R5).
fn raw_rows_in(
    conn: &Connection,
    project: &str,
    filter: &ProposalFilter,
) -> Result<Vec<RawRow>, QueueError> {
    if index_only(conn)? {
        return Ok(Vec::new());
    }
    let columns = columns_of(conn)?;
    let mut statement = conn
        .prepare(&format!(
            "SELECT {columns} FROM main.proposals \
             WHERE project = ?1 AND (?2 IS NULL OR git_common_dir = ?2) {ID_ORDER}"
        ))
        .db()?;
    let raws = statement
        .query_map(
            rusqlite::params![project, filter.git_common_dir],
            RawRow::read,
        )
        .db()?
        .collect::<rusqlite::Result<_>>()
        .db()?;
    Ok(raws)
}

// ------------------------------------------------- queue.rs:2454-2470 at 540db8ff

/// `raws` within `filter`'s statuses, each decoded, else named as a row that
/// does not decode: [`ProposalQueue::list_readable`]'s list.
fn readable(raws: Vec<RawRow>, filter: &ProposalFilter) -> ProposalList {
    let admits =
        |status: ProposalStatus| filter.statuses.is_empty() || filter.statuses.contains(&status);
    let mut list = ProposalList::default();
    for raw in raws {
        if raw.status().is_some_and(|status| !admits(status)) {
            continue;
        }
        match raw.decode() {
            Ok(proposal) => list.proposals.push(proposal),
            Err(row) => list.unreadable.push(row),
        }
    }
    list
}

// ------------------------------------------------- queue.rs:2525-2783 at 540db8ff

/// The row's columns, in [`COLUMNS`] order, before decoding.
struct RawRow {
    text: Vec<Option<String>>,
}

impl RawRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        let mut text = Vec::with_capacity(PROPOSAL_COLUMNS.len());
        for column in 0..PROPOSAL_COLUMNS.len() {
            text.push(row.get::<_, Option<String>>(column)?);
        }
        Ok(Self { text })
    }

    /// The stored state, when it reads: `list_readable`'s filter before
    /// decoding.
    fn status(&self) -> Option<ProposalStatus> {
        self.text[3].as_deref().and_then(ProposalStatus::parse)
    }

    fn decode(self) -> Result<Proposal, UnreadableRow> {
        let mut values = self.text.into_iter();
        let mut take = || values.next().flatten();
        let id = take().unwrap_or_default();
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(&id, column, "it is NULL"))
        };
        let project = required(take(), "project")?;
        let kind_text = required(take(), "kind")?;
        let kind = ProposalKind::parse(&kind_text)
            .ok_or_else(|| corrupt(&id, "kind", format!("`{kind_text}` is no kind")))?;
        let status_text = required(take(), "status")?;
        let status = ProposalStatus::parse(&status_text)
            .ok_or_else(|| corrupt(&id, "status", format!("`{status_text}` is no state")))?;
        let target_id = required(take(), "target_id")?;
        let target_path = required(take(), "target_path")?;
        let place = Place {
            git_common_dir: required(take(), "git_common_dir")?,
            worktree: required(take(), "worktree")?,
            root_rel: required(take(), "root_rel")?,
            branch: required(take(), "branch")?,
            base_commit: required(take(), "base_commit")?,
        };
        if !is_branch_name(&place.branch) {
            return Err(corrupt(
                &id,
                "branch",
                format!("{:?} is no branch name", place.branch),
            ));
        }
        if !is_oid(place.base_commit.as_bytes()) {
            return Err(corrupt(
                &id,
                "base_commit",
                format!("{:?} is no object ID", place.base_commit),
            ));
        }
        // An update's texts; the intake kinds store them NULL, a new file
        // its base.
        let base_columns = [take(), take()];
        let applies = kind.family() == KindFamily::Applies;
        let mut update_text = |column: &str| {
            let value = take();
            if applies {
                required(value, column)
            } else {
                Ok(String::new())
            }
        };
        let new_text = update_text("new_text")?;
        let patch_hash = update_text("patch_hash")?;
        let rationale = update_text("rationale")?;
        // A kind adding nodes: a new file has no base; one that amends is
        // only a new file.
        let [base_hash, base_text] = match base_columns {
            [None, None] if kind.adds_nodes() => [String::new(), String::new()],
            [hash, _] if kind.amends() => {
                let column = if hash.is_some() {
                    "base_hash"
                } else {
                    "base_text"
                };
                return Err(corrupt(
                    &id,
                    column,
                    format!("{} is a new file: it holds no base", kind.with_article()),
                ));
            }
            [Some(hash), _] if kind.adds_nodes() && hash.is_empty() => {
                return Err(corrupt(
                    &id,
                    "base_hash",
                    format!("it is empty on {}", kind.with_article()),
                ));
            }
            [Some(hash), Some(text)] if kind.adds_nodes() => [hash, text],
            [hash, _] if kind.adds_nodes() => {
                let column = if hash.is_none() {
                    "base_hash"
                } else {
                    "base_text"
                };
                return Err(corrupt(
                    &id,
                    column,
                    "it is NULL while the base's other column is set",
                ));
            }
            [hash, text] if applies => [required(hash, "base_hash")?, required(text, "base_text")?],
            _ => [String::new(), String::new()],
        };
        let author_text = required(take(), "author")?;
        let author: Author =
            serde_json::from_str(&author_text).map_err(|error| corrupt(&id, "author", error))?;
        for field in [&author.role, &author.model, &author.run] {
            if let Some(problem) = field.as_deref().and_then(author_field_problem) {
                return Err(corrupt(&id, "author", problem));
            }
        }
        // A hub's login is a person's, never an author field: non-ASCII and
        // inner spaces are valid logins, so it takes the hub's own grammar; a
        // row with no `login` key reads as before:
        // docs/features/hub-deploy.md "Store: `Author.login` on decode".
        if let Some(problem) = author.login.as_deref().and_then(hub_login_problem) {
            return Err(corrupt(&id, "author", problem));
        }
        let diagnostics_text = required(take(), "diagnostics")?;
        let diagnostics: Vec<ProposalFinding> = serde_json::from_str(&diagnostics_text)
            .map_err(|error| corrupt(&id, "diagnostics", error))?;
        let decided_by = take();
        let decided_at = take();
        let decision_note = take();
        let applied_commit = take();
        let created_at = required(take(), "created_at")?;
        let updated_at = required(take(), "updated_at")?;
        let mut columns = IntakeColumns {
            target_ids: take(),
            severity: take(),
            gap_type: take(),
            summary: take(),
            working_answer: take(),
            price_of_other: take(),
            evidence: take(),
            options: take(),
            recommendation: take(),
            distinct_from: take(),
            gist: None,
            matters: None,
        };
        let linked = take();
        let record_columns: [Option<String>; 5] = std::array::from_fn(|_| take());
        let task_id = take();
        let staged_columns = [take(), take()];
        let interpretation = decode_interpretation(&id, kind, take())?;
        let drift_columns = [take(), take()];
        let card_id = take();
        let why = take();
        // Step 13's: intake columns of a question.
        columns.gist = take();
        columns.matters = take();
        // Step 15's, last in the table (task spec `comments`): the comment
        // an item was raised from, a deciding kind's alone.
        let comment_id = take();
        if let Some(comment) = &comment_id {
            if kind.family() != KindFamily::Decides {
                return Err(corrupt(
                    &id,
                    "comment_id",
                    format!(
                        "it is set on {}, which names no comment",
                        kind.with_article()
                    ),
                ));
            }
            if specengine_core::comment::comment_number(comment).is_none() {
                return Err(corrupt(
                    &id,
                    "comment_id",
                    format!("`{comment}` is no comment ID"),
                ));
            }
        }
        if let Some(task) = &task_id
            && specengine_core::task::task_number(task).is_none()
        {
            return Err(corrupt(&id, "task_id", format!("{task:?} is no task ID")));
        }
        // A card's items: a deciding kind names a card, a decision alone
        // holds a why; a blank or long `why` reads as stored.
        if let Some(card) = &card_id {
            if kind.family() != KindFamily::Decides {
                return Err(corrupt(
                    &id,
                    "card_id",
                    format!("it is set on {}, which names no card", kind.with_article()),
                ));
            }
            if card_number(card).is_none() {
                return Err(corrupt(&id, "card_id", format!("`{card}` is no card ID")));
            }
        }
        if why.is_some() && !kind.holds_why() {
            return Err(corrupt(
                &id,
                "why",
                format!("it is set on {}, which holds none", kind.with_article()),
            ));
        }
        let new_file = kind.adds_nodes() && base_hash.is_empty();
        let new_ids = if kind.adds_nodes() {
            columns.decode_create(&id, kind, &target_id, new_file, linked.as_deref())?
        } else {
            Vec::new()
        };
        let drift = match decode_drift(&id, kind, drift_columns)? {
            Some((evidence, stored_rows, hash)) => {
                let (severity, summary) = columns.decode_drift(&id, kind, &target_id, &linked)?;
                Some(DriftItem {
                    severity,
                    summary,
                    evidence,
                    stored_rows,
                    hash,
                })
            }
            None => None,
        };
        let intake = match kind.intake_shape() {
            None => None,
            Some(shape) => Some(Intake {
                why,
                ..columns.decode(&id, kind, shape, &target_id)?
            }),
        };
        // The stage once the options it may name are read (task spec
        // `plain-questions` D3: an option by the item's fields).
        let options = intake.as_ref().map_or(0, |intake| intake.options.len());
        let staged = stage::decode(&id, kind, status, new_file, options, staged_columns)?;
        let record = decode_record(&id, kind, status, intake.as_ref(), record_columns)?;
        if let Some(linked) = &linked
            && proposal_number(linked).is_none()
        {
            return Err(corrupt(
                &id,
                "linked",
                format!("{linked:?} is no proposal ID"),
            ));
        }
        Ok(Proposal {
            id,
            project,
            kind,
            status,
            target_id,
            target_path,
            place,
            base_hash,
            base_text,
            new_text,
            patch_hash,
            rationale,
            author,
            diagnostics,
            decided_by,
            decided_at,
            decision_note,
            applied_commit,
            created_at,
            updated_at,
            intake,
            linked,
            record,
            new_ids,
            task_id,
            card_id,
            comment_id,
            // Step 17's, last in the table (task spec `reference-promotion`):
            // taken as written, the store's own decode judges it.
            ref_id: take(),
            staged,
            interpretation,
            drift,
        })
    }
}

// ------------------------------------------------- queue.rs:4135-4235 at 540db8ff

/// The project's rows of the intake's family (any repository, any state):
/// the kinds whose [`ProposalKind::intake_shape`] is of its
/// [`IntakeShape::family`], a question and a decision one family, a
/// discrepancy another; those that share a target with it, by ID number:
/// hits when the normalised summary is equal, else related. A row whose
/// targets or state do not read shares nothing.
fn queue_matches(
    tx: &Transaction<'_>,
    project: &str,
    intake: &NewIntake,
) -> Result<(Vec<QueueMatch>, Vec<QueueMatch>), QueueError> {
    let wanted = normalized_summary(&intake.intake.summary);
    let family_of = |kind: ProposalKind| kind.intake_shape().map(IntakeShape::family);
    let wanted_family = family_of(intake.kind);
    let family: Vec<&str> = ProposalKind::ALL
        .into_iter()
        .filter(|&kind| wanted_family.is_some() && family_of(kind) == wanted_family)
        .map(ProposalKind::as_str)
        .collect();
    let slots = (0..family.len())
        .map(|at| format!("?{}", at + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = tx
        .prepare(&format!(
            "SELECT id, status, target_ids, summary, decision_note, record_id, record_path, \
             record_title FROM main.proposals WHERE project = ?1 AND kind IN ({slots}) \
             {ID_ORDER}"
        ))
        .db()?;
    let params = std::iter::once(project).chain(family.iter().copied());
    let rows: Vec<MatchRow> = statement
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(MatchRow {
                id: row.get(0)?,
                status: row.get(1)?,
                target_ids: row.get(2)?,
                summary: row.get(3)?,
                decision_note: row.get(4)?,
                record_id: row.get(5)?,
                record_path: row.get(6)?,
                record_title: row.get(7)?,
            })
        })
        .db()?
        .collect::<rusqlite::Result<_>>()
        .db()?;
    let (mut hits, mut related) = (Vec::new(), Vec::new());
    for row in rows {
        let MatchRow {
            id,
            status,
            target_ids,
            summary,
            decision_note,
            record_id,
            record_path,
            record_title,
        } = row;
        let (Some(id), Some(status)) = (id, status.as_deref().and_then(ProposalStatus::parse))
        else {
            continue;
        };
        let targets: Vec<String> = target_ids
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        if !targets
            .iter()
            .any(|target| intake.intake.target_ids.contains(target))
        {
            continue;
        }
        let same = summary
            .as_deref()
            .is_some_and(|summary| normalized_summary(summary) == wanted);
        if same {
            let reason = (status == ProposalStatus::Rejected)
                .then_some(decision_note)
                .flatten();
            hits.push(QueueMatch {
                id,
                status,
                reason,
                record_id,
                record_path,
                record_title,
            });
        } else {
            related.push(QueueMatch {
                id,
                status,
                reason: None,
                record_id,
                record_path,
                record_title,
            });
        }
    }
    Ok((hits, related))
}

// ------------------------------------------------- queue.rs:4237-4247 at 540db8ff

/// A row [`queue_matches`] reads, not decoded.
struct MatchRow {
    id: Option<String>,
    status: Option<String>,
    target_ids: Option<String>,
    summary: Option<String>,
    decision_note: Option<String>,
    record_id: Option<String>,
    record_path: Option<String>,
    record_title: Option<String>,
}

// ------------------------------------------------- the oracles' entry points

/// HEAD's `list_readable` over `conn`: `readable(raw_rows_in(…))`.
pub(super) fn list(
    conn: &Connection,
    project: &str,
    filter: &ProposalFilter,
) -> Result<ProposalList, QueueError> {
    Ok(readable(raw_rows_in(conn, project, filter)?, filter))
}

/// HEAD's dedup read: `queue_matches`.
pub(super) fn matches(
    tx: &Transaction<'_>,
    project: &str,
    intake: &NewIntake,
) -> Result<(Vec<QueueMatch>, Vec<QueueMatch>), QueueError> {
    queue_matches(tx, project, intake)
}
