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
//! ID and meets the error then). On a hub the stream also ends
//! [`STREAM_LIFE`](crate::hub::STREAM_LIFE) after its 200 ([`answered`]),
//! between two events or comments, sending nothing more: the browser
//! reconnects with its last ID through the fence, which judges the request
//! anew, and resumes after it, losing no event; a local daemon's never ends
//! for its age (`docs/canon/serve.md` "Live tail").

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::BoxError;
use axum::extract::State;
use axum::http::{HeaderMap, Uri};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::Stream;
use specengine_cli::{EventLine, EventsPage, EventsTail};
use tokio::time::{Interval, MissedTickBehavior};

use crate::answer::{self, Refusal};
use crate::app::{App, Shared};
use crate::args::Args;
use crate::hub::lived;
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
    Ok(answered(&app, stream))
}

/// The 200 of a tail: `events` as server-sent events, a `:` comment every
/// 15 s, ending at `app.life` after this call, between two events or
/// comments, with nothing sent at the end; a local daemon's (`None`) never
/// ends for its age (`docs/canon/serve.md` "Live tail").
fn answered<S, E>(app: &App, events: S) -> Response
where
    S: Stream<Item = Result<Event, E>> + Send + 'static,
    E: Into<BoxError>,
{
    Sse::new(lived(events, app.life))
        .keep_alive(KeepAlive::default())
        .into_response()
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

#[cfg(test)]
mod tests {
    use futures_util::StreamExt as _;
    use tokio::time::{Instant, timeout_at};

    use super::*;

    /// SL3's life: short, yet long against a loaded debug build's scheduling.
    const LIFE: Duration = Duration::from_millis(800);

    /// The three events on the wire, each `id`, `event`, `data` and a blank
    /// line (docs/features/stream-life.md "Data").
    const SENT: [&str; 3] = [
        "id: 41\nevent: card.created\ndata: {\"id\":\"C-0001\",\"as\":\"mira\"}\n\n",
        "id: 42\nevent: card.moved\ndata: {\"id\":\"C-0001\",\"from\":\"drafted\",\"to\":\"checked\",\"position\":0,\"as\":\"tomas\"}\n\n",
        "id: 43\nevent: card.edited\ndata: {\"id\":\"C-0001\",\"as\":\"tomas\"}\n\n",
    ];

    /// The three events, then silence.
    fn three_then_silence() -> impl Stream<Item = Result<Event, Infallible>> + Send + 'static {
        let events = [
            ("41", "card.created", r#"{"id":"C-0001","as":"mira"}"#),
            (
                "42",
                "card.moved",
                r#"{"id":"C-0001","from":"drafted","to":"checked","position":0,"as":"tomas"}"#,
            ),
            ("43", "card.edited", r#"{"id":"C-0001","as":"tomas"}"#),
        ]
        .map(|(id, kind, data)| {
            Ok::<_, Infallible>(Event::default().id(id).event(kind).data(data))
        });
        futures_util::stream::iter(events).chain(futures_util::stream::pending())
    }

    /// `response`'s body read until it ends or `by`: its text, and when it
    /// ended since `started` (`None`: still open at `by`).
    async fn read_until(
        response: Response,
        started: Instant,
        by: Instant,
    ) -> (String, Option<Duration>) {
        let mut body = response.into_body().into_data_stream();
        let mut sent = Vec::new();
        let ended = loop {
            match timeout_at(by, body.next()).await {
                Ok(Some(chunk)) => sent.extend_from_slice(&chunk.expect("a body chunk")),
                Ok(None) => break Some(started.elapsed()),
                Err(_) => break None,
            }
        };
        (String::from_utf8(sent).expect("UTF-8"), ended)
    }

    /// `text`'s event blocks, each with its blank line, keep-alive comments
    /// left out; a byte past the last blank line fails.
    fn events_in(text: &str) -> Vec<&str> {
        let mut events = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            let Some(end) = rest.find("\n\n") else {
                panic!("bytes past the last blank line: {rest:?} in {text:?}");
            };
            let (block, after) = rest.split_at(end + 2);
            let comment = block
                .trim_end_matches('\n')
                .lines()
                .all(|line| line.starts_with(':'));
            if !comment {
                events.push(block);
            }
            rest = after;
        }
        events
    }

    /// SL3 of docs/features/stream-life.md, a life: the body is exactly the
    /// three whole events, keep-alive comments allowed between them; it ends
    /// at no less than the life and before a second past it, with no byte
    /// after the last event's blank line. M: `answered` without `lived`; a
    /// frame appended at the end.
    #[tokio::test]
    async fn sl3_a_tail_with_a_life_ends_after_it_with_nothing_more() {
        let mut app = App::new(vec![], 0, None, None);
        app.life = Some(LIFE);
        let started = Instant::now();
        let response = answered(&app, three_then_silence());
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .map(|value| value.as_bytes()),
            Some(&b"text/event-stream"[..])
        );
        let by = started + LIFE + Duration::from_secs(1);
        let (text, ended) = read_until(response, started, by).await;
        let ended = ended.unwrap_or_else(|| panic!("still open a second past its life: {text:?}"));
        assert!(
            ended >= LIFE,
            "ended at {ended:?}, before its life: {text:?}"
        );
        assert_eq!(events_in(&text), SENT, "{text:?}");
        assert!(
            text.ends_with(SENT[2]),
            "a byte after the last event's blank line: {text:?}"
        );
    }

    /// SL3, no life: the three come, and the stream is still open at three
    /// times the life above. M: `answered` capping `None` at 3L or less
    /// (`STREAM_LIFE`'s 300 s is not waited for on a real clock).
    #[tokio::test]
    async fn sl3_a_tail_with_no_life_is_still_open_at_three_lives() {
        let mut app = App::new(vec![], 0, None, None);
        app.life = None;
        let started = Instant::now();
        let response = answered(&app, three_then_silence());
        let (text, ended) = read_until(response, started, started + LIFE * 3).await;
        assert_eq!(ended, None, "no life, yet ended: {text:?}");
        assert_eq!(events_in(&text), SENT, "{text:?}");
    }
}
