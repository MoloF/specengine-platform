//! `GET /api/projects/<slug>/events`: the live tail of the queue's
//! `events` (docs/features/daemon-read.md "Data"), as server-sent events.
//!
//! A stream opens with `id: <start seq>` and a blank line, no `data:`
//! line (it sets the browser's last event ID and dispatches nothing, so a
//! stream that drops before its first event resumes where it started):
//! the start seq is the highest `seq` of the slug's database now (0 when
//! none), or `Last-Event-ID` when sent and not above it. Then per event
//! `id: <seq>`, `event: <type>`, `data: <payload>` (the stored JSON, one
//! line), a blank line; a `:` comment every 15 s. Only the project's
//! events. Without `Last-Event-ID` the tail starts after the highest `seq`
//! now (no replay); `Last-Event-ID: n` gives the events with `seq > n`, at
//! once; an `n` above the highest (the database wiped or made anew) starts
//! from the highest instead, so the new database's events still come;
//! anything but a non-negative decimal integer is a 400. The stream polls
//! at most 250 ms apart (at once after a full page), each poll one CLI
//! library call ([`EventsTail::after`]) on the config read once and the
//! stream's own database connection, kept from poll to poll: one short
//! read transaction, none held across polls, no database created. A poll
//! that cannot run ends the stream (the browser reconnects with its last
//! ID and meets the error then).

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, Uri};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use specengine_cli::{EventLine, EventsPage, EventsTail};
use tokio::time::{Interval, MissedTickBehavior};

use crate::answer::{self, Refusal};
use crate::app::Shared;
use crate::args::Args;
use crate::start::Project;

/// The longest wait between two polls.
const POLL: Duration = Duration::from_millis(250);

/// A tail's state between two events.
struct Tail {
    project: Arc<Project>,
    /// The stream's database connection, kept between polls.
    reader: EventsTail,
    /// The next poll gives the events after it.
    cursor: i64,
    /// Read, not sent yet.
    pending: VecDeque<EventLine>,
    ticks: Interval,
    /// The last poll read a full page (skipped rows counted): poll again
    /// without waiting.
    full: bool,
    /// The opening `id:` line is sent.
    opened: bool,
}

/// The handler (see the module documentation).
pub(crate) async fn events(
    State(app): State<Shared>,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, Refusal> {
    let (project, _) = app.project_of(&uri)?;
    Args::new("events", &[], uri.query()).map_err(Refusal::bad_request)?;
    let resume = last_event_id(&headers).map_err(Refusal::bad_request)?;
    let (reader, page) = poll(Arc::clone(&project), EventsTail::new(), None).await?;
    let highest = page.last_seq.unwrap_or(0);
    let cursor = resume.map_or(highest, |seq| seq.min(highest));
    let mut ticks = tokio::time::interval(POLL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let tail = Tail {
        project,
        reader,
        cursor,
        pending: VecDeque::new(),
        ticks,
        full: false,
        opened: false,
    };
    let stream = futures_util::stream::unfold(tail, next_event);
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

/// `Last-Event-ID`, if sent: a non-negative decimal integer.
fn last_event_id(headers: &HeaderMap) -> Result<Option<i64>, String> {
    let mut values = headers.get_all("last-event-id").iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err("Last-Event-ID is sent more than once".to_owned());
    }
    let text = value.to_str().unwrap_or_default();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "Last-Event-ID `{}`: not a non-negative integer",
            String::from_utf8_lossy(value.as_bytes())
        ));
    }
    text.parse()
        .map(Some)
        .map_err(|_| format!("Last-Event-ID `{text}`: out of range"))
}

/// One poll on the blocking pool with the stream's `reader`, given back
/// with the page; exit 2 as a 503.
async fn poll(
    project: Arc<Project>,
    mut reader: EventsTail,
    after: Option<i64>,
) -> Result<(EventsTail, EventsPage), Refusal> {
    answer::read(project, move |env, _, found| {
        let page = reader.after(env, found, after)?;
        Ok((reader, page))
    })
    .await
}

/// The next event to send, polling until there is one; `None` ends the
/// stream (a poll that could not run).
async fn next_event(mut tail: Tail) -> Option<(Result<Event, Infallible>, Tail)> {
    if !tail.opened {
        tail.opened = true;
        let start = Event::default().id(tail.cursor.to_string());
        return Some((Ok(start), tail));
    }
    loop {
        while let Some(line) = tail.pending.pop_front() {
            // A type with a line break cannot be sent as a field.
            if line.event_type.contains(['\r', '\n']) {
                continue;
            }
            let event = Event::default()
                .id(line.seq.to_string())
                .event(&line.event_type)
                .data(&line.payload);
            return Some((Ok(event), tail));
        }
        if !tail.full {
            tail.ticks.tick().await;
        }
        let (reader, page) = poll(Arc::clone(&tail.project), tail.reader, Some(tail.cursor))
            .await
            .ok()?;
        tail.reader = reader;
        tail.full = page.full;
        if let Some(last) = page.last_seq {
            tail.cursor = tail.cursor.max(last);
        }
        tail.pending.extend(page.events);
    }
}
