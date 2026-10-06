//! The queue's `events` after a `seq` (task spec `daemon-read`, "Data"):
//! what the daemon's live tail polls. No command prints it.
//!
//! One call is one short read transaction on the project's database,
//! ended before it returns: the project's events with a `seq` above the
//! one given, by `seq`, at most [`EVENTS_PAGE_MAX`], their `type` and
//! `payload` as stored; none when no `seq` is given (a tail starts after
//! the current highest, no replay). No database, or one without the
//! queue's tables yet, reads as no event: nothing is created, no schema
//! step runs, the data directory is not made. A queue of a newer build
//! (its `user_version` above this build's) exits 2.
//!
//! [`events_after`] opens the database for its one call; an [`EventsTail`]
//! keeps the connection from one call to the next (a tail polls every
//! 250 ms: no connection, settings and WAL files made and dropped per
//! poll), and opens it again only when the database file is replaced or
//! goes away.

use std::fs;
use std::io;
use std::path::PathBuf;

use specengine_store::SqliteQueue;

use crate::export::identity;
use crate::location::checked_data_dir;
use crate::project::{ProjectRoot, discover};
use crate::proposals::queue_cannot;
use crate::{CliError, Env, Globals};

/// The most events one call gives; the next call goes on from the last.
pub const EVENTS_PAGE_MAX: usize = 512;

/// One event as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLine {
    pub seq: i64,
    /// `proposal.created`, …
    pub event_type: String,
    /// The stored JSON text, untouched.
    pub payload: String,
}

/// What [`events_after`] read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventsPage {
    /// The project's events, by `seq`.
    pub events: Vec<EventLine>,
    /// The `after` of the next call: the last row read when the page is
    /// full, else the table's highest `seq` (any project's), never below
    /// the `after` given; `None` only when none was given and the queue
    /// has no event.
    pub last_seq: Option<i64>,
    /// [`EVENTS_PAGE_MAX`] rows were read, skipped ones counted (a row
    /// whose `type` or `payload` is NULL): the next call may find more at
    /// once.
    pub full: bool,
}

impl EventsPage {
    /// No event: no database, or none yet.
    fn nothing(after: Option<i64>) -> Self {
        Self {
            events: Vec::new(),
            last_seq: after,
            full: false,
        }
    }
}

/// The project's events after `after` (see the module documentation).
pub fn events_after(
    env: &Env,
    globals: &Globals,
    after: Option<i64>,
) -> Result<EventsPage, CliError> {
    let project = discover(env, globals)?;
    EventsTail::new().after(env, &project, after)
}

/// [`events_after`] for a live tail: the project found by the caller (its
/// config read once per call), the database connection kept between
/// calls. Each call is still one short read transaction, none held across
/// calls. The connection is opened again when the database's path (data
/// directory, slug) or its file (device and inode, where the platform
/// gives them) differs from the one it reads, and dropped when the file is
/// gone; nothing is ever created.
#[derive(Default)]
pub struct EventsTail {
    open: Option<OpenQueue>,
}

/// A kept connection and the file it reads.
struct OpenQueue {
    db: PathBuf,
    /// Device and inode when opened; `None` where the platform gives none
    /// (the connection is then kept while the path names a file).
    identity: Option<(u64, u64)>,
    queue: SqliteQueue,
}

impl std::fmt::Debug for EventsTail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventsTail")
            .field("db", &self.open.as_ref().map(|open| &open.db))
            .finish()
    }
}

impl EventsTail {
    /// No connection yet: the first call opens one if the database exists.
    pub fn new() -> Self {
        Self::default()
    }

    /// `project`'s events after `after`, as [`events_after`] gives them.
    pub fn after(
        &mut self,
        env: &Env,
        project: &ProjectRoot,
        after: Option<i64>,
    ) -> Result<EventsPage, CliError> {
        let slug = project.slug()?;
        let db = checked_data_dir(env, project)?.join(format!("{slug}.db"));
        let identity = match fs::metadata(&db) {
            Ok(metadata) => identity(&metadata),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.open = None;
                return Ok(EventsPage::nothing(after));
            }
            Err(_) => {
                // Opened again below, which names the failure.
                self.open = None;
                None
            }
        };
        let kept = self
            .open
            .take()
            .filter(|open| open.db == db && open.identity == identity);
        let open = match kept {
            Some(open) => open,
            None => match SqliteQueue::open_existing(&db, slug).map_err(queue_cannot)? {
                Some(queue) => OpenQueue {
                    db,
                    identity,
                    queue,
                },
                None => return Ok(EventsPage::nothing(after)),
            },
        };
        let read = open
            .queue
            .events_after(after, EVENTS_PAGE_MAX)
            .map_err(queue_cannot)?;
        self.open = Some(open);
        Ok(EventsPage {
            events: read
                .events
                .into_iter()
                .map(|event| EventLine {
                    seq: event.seq,
                    event_type: event.event_type,
                    payload: event.payload,
                })
                .collect(),
            last_seq: read.last_seq,
            full: read.full,
        })
    }
}
