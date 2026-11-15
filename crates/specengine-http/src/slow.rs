//! The daemon's slow lines (task spec `slow-requests`, RM-0118): a request
//! whose whole time passes `--slow-ms` writes one stderr line when it
//! completes, or when its client leaves first (`status=gone`); the turn's
//! own lines (`held`, `released`) are [`crate::turn`]'s.
//!
//! - **The threshold**: `--slow-ms N`, 0 to [`SLOW_MS_MAX`], [`SLOW_MS`] by
//!   default; passed when the measured time is at least `N` ms.
//! - **The record** ([`Exchange`]): made by [`watch`], the outermost layer,
//!   before the fence, at the request's first sight; its route template and
//!   the raw `{p}` and `{id}` segments noted past routing, by each route's
//!   own layer ([`routed`]); an
//!   MCP call's tool by [`crate::mcp`]; its turn's wait, run and walk by
//!   [`crate::turn`], which finds it as [`current`] (or, for an MCP call
//!   running in rmcp's own task, in the request's parts).
//! - **When**: when the handler's answer is returned; an answer whose body
//!   is a stream (an MCP call's SSE answer) at its body's end, as the call's
//!   answer travels in it; a drop before then is `status=gone`. The live
//!   tail and `/mcp`'s GET stream write none.
//! - **Hygiene**: no login, body, header, query string, raw path, cookie or
//!   argument; `project` and `id` only as 1 to 64 bytes of
//!   `[A-Za-z0-9._-]`, `tool` of `[A-Za-z0-9._/-]`, the method as 1 to 16
//!   ASCII uppercase letters, else `?`; an absent value `-`.
//! - **Never a change**: no answer, status, header or order changes; a
//!   failed write is ignored; `ms` is measured before the write.
//! - **Appended** (task spec `hub-turn-fairness`): `class=<page|mcp>` by
//!   route, `line_ms` the wait in a hub MCP call's login line (`-` with
//!   none), `ahead` the turn's holders and waiters when the request began
//!   waiting for it (`-` with no turn).
//! - **Then** (task spec `write-cost`): the call's own time by kind,
//!   measured on its thread in its turn, each charged to the innermost
//!   kind: `check_ms` a write's findings, `index_ms` walks and decodes,
//!   `git_ms` and `git` git's processes, `write_ms` synced writes; `-`
//!   where nothing of the kind ran.
//!
//! ```text
//! specengine-http: slow request at=<T> method=<M> route=<R> project=<P> id=<I> tool=<N> status=<S> ms=<n> wait_ms=<n|-> run_ms=<n|-> index=<walked|trusted|-> class=<page|mcp> line_ms=<n|-> ahead=<n|-> check_ms=<n|-> index_ms=<n|-> git_ms=<n|-> git=<n|-> write_ms=<n|->
//! ```

use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Write as _;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes, HttpBody as _};
use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use futures_util::{StreamExt as _, stream};
use specengine_cli::IndexTrust;
use specengine_cli::cost::CallCost;

/// `--slow-ms`'s default.
pub(crate) const SLOW_MS: u64 = 2_000;

/// `--slow-ms`'s largest value: an hour.
pub(crate) const SLOW_MS_MAX: u64 = 3_600_000;

/// The live tail's route: a stream, no request line.
const EVENTS: &str = "/api/projects/{p}/events";

/// A hub's MCP route: its GET is a stream, no request line.
pub(crate) const MCP: &str = "/mcp/{p}";

/// `--slow-ms`, set once at the start.
static THRESHOLD: OnceLock<Duration> = OnceLock::new();

/// Sets `--slow-ms` for the process, once, before serving.
pub(crate) fn set_threshold(ms: u64) {
    let _ = THRESHOLD.set(Duration::from_millis(ms));
}

/// `--slow-ms`; its default when none was set.
pub(crate) fn threshold() -> Duration {
    THRESHOLD
        .get()
        .copied()
        .unwrap_or(Duration::from_millis(SLOW_MS))
}

/// `elapsed` passes the threshold.
pub(crate) fn passed(elapsed: Duration) -> bool {
    elapsed >= threshold()
}

/// Whole milliseconds of `elapsed`.
pub(crate) fn millis(elapsed: Duration) -> u128 {
    elapsed.as_millis()
}

/// Writes `line` and its line end on a locked stderr, in one write; a
/// failure is ignored.
pub(crate) fn write_line(line: &str) {
    let mut bytes = String::with_capacity(line.len() + 1);
    bytes.push_str(line);
    bytes.push('\n');
    let _ = std::io::stderr().lock().write_all(bytes.as_bytes());
}

/// `time` as UTC `YYYY-MM-DDTHH:MM:SS.mmmZ`.
pub(crate) fn utc(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = since.as_secs();
    let (year, month, day) = civil(seconds / 86_400);
    let rest = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60,
        since.subsec_millis()
    )
}

/// The civil date of a count of days since 1970-01-01 (H. Hinnant's
/// `civil_from_days`, for days on or after the epoch).
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let of_era = shifted % 146_097;
    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// `value` when it is 1 to 64 bytes of `[A-Za-z0-9._-]`, plus `/` when
/// `slash`; else `?`; absent, `-`.
pub(crate) fn clean(value: Option<&str>, slash: bool) -> &str {
    let Some(value) = value else {
        return "-";
    };
    let fits = (1..=64).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'_' | b'-')
                || (slash && byte == b'/')
        });
    if fits { value } else { "?" }
}

/// HTTP's own methods, borrowed as they are: no allocation per request.
const METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT", "PATCH", "TRACE",
];

/// The method as sent, when it is 1 to 16 ASCII uppercase letters; else
/// `?`.
fn method_word(method: &Method) -> Cow<'static, str> {
    let text = method.as_str();
    if let Some(word) = METHODS.iter().find(|word| **word == text) {
        return Cow::Borrowed(word);
    }
    if (1..=16).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(text.to_owned())
    } else {
        Cow::Borrowed("?")
    }
}

/// The raw segments of `path` at the template's `{p}` and `{id}`, never
/// decoded: the template's segments before a wildcard align with the
/// path's.
fn segments(template: &str, path: &str) -> (Option<String>, Option<String>) {
    let mut project = None;
    let mut id = None;
    for (wanted, raw) in template.split('/').zip(path.split('/')) {
        match wanted {
            "{p}" => project = Some(raw.to_owned()),
            "{id}" => id = Some(raw.to_owned()),
            _ => {}
        }
    }
    (project, id)
}

/// What a request's line says of its turn.
#[derive(Debug, Default)]
struct Turned {
    /// It waited for a turn.
    sought: bool,
    /// The turn's holders and waiters when it began waiting.
    ahead: Option<usize>,
    acquired: Option<Instant>,
    released: Option<Instant>,
    /// A hub's trust of the project, whose walks the hold is judged by.
    trust: Option<Arc<IndexTrust>>,
    walks_at: u64,
    walked: bool,
}

/// What is seen of a request as it passes.
#[derive(Debug, Default)]
struct Seen {
    route: Option<String>,
    project: Option<String>,
    id: Option<String>,
    tool: Option<String>,
    /// A hub MCP call's wait in its login's line: joined, headed.
    line: Option<(Instant, Option<Instant>)>,
    turn: Turned,
    /// The call's costs by kind, measured on its thread in its turn
    /// (`write-cost`, the five fields); `None`: no call ran.
    cost: Option<CallCost>,
    /// Its line written, or ruled out.
    done: bool,
}

/// One request's record, from its first sight to its line (see the module
/// documentation): shared by the layers, the handler and its turn.
#[derive(Debug, Clone)]
pub(crate) struct Exchange(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    at: SystemTime,
    start: Instant,
    method: Cow<'static, str>,
    seen: Mutex<Seen>,
}

/// A request line's status.
enum Status {
    Code(u16),
    Gone,
}

impl Exchange {
    fn new(method: &Method) -> Self {
        Self(Arc::new(Inner {
            at: SystemTime::now(),
            start: Instant::now(),
            method: method_word(method),
            seen: Mutex::new(Seen::default()),
        }))
    }

    fn seen(&self) -> MutexGuard<'_, Seen> {
        self.0.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The route matched, as its template, and the raw `{p}` and `{id}` of
    /// `path`.
    fn routed(&self, template: &str, path: &str) {
        let (project, id) = segments(template, path);
        let mut seen = self.seen();
        seen.route = Some(template.to_owned());
        seen.project = project;
        seen.id = id;
    }

    /// An MCP call's tool: a `tools/call`'s name, or `resources/read`.
    pub(crate) fn tool(&self, tool: String) {
        self.seen().tool = Some(tool);
    }

    /// The route template, the raw `{id}` and the tool, for a turn's lines.
    pub(crate) fn holder(&self) -> (Option<String>, Option<String>, Option<String>) {
        let seen = self.seen();
        (seen.route.clone(), seen.id.clone(), seen.tool.clone())
    }

    /// It joins its login's line `at`.
    pub(crate) fn lining(&self, at: Instant) {
        let mut seen = self.seen();
        if seen.line.is_none() {
            seen.line = Some((at, None));
        }
    }

    /// It heads its login's line `at`.
    pub(crate) fn lined(&self, at: Instant) {
        if let Some((_, headed)) = &mut self.seen().line
            && headed.is_none()
        {
            *headed = Some(at);
        }
    }

    /// It waits for its project's turn from now, `ahead` holders and
    /// waiters before it.
    pub(crate) fn seeking(&self, ahead: usize) {
        let mut seen = self.seen();
        seen.turn.sought = true;
        seen.turn.ahead.get_or_insert(ahead);
    }

    /// Its turn acquired `at`; `trust` the project's on a hub.
    pub(crate) fn acquired(&self, at: Instant, trust: Option<Arc<IndexTrust>>) {
        let mut seen = self.seen();
        let turn = &mut seen.turn;
        turn.sought = true;
        if turn.acquired.is_none() {
            turn.acquired = Some(at);
        }
        turn.walks_at = trust.as_deref().map_or(0, IndexTrust::walks);
        turn.trust = trust;
        turn.released = None;
    }

    /// Its turn released `at`, still held: whether a walk ran in it.
    pub(crate) fn released(&self, at: Instant) {
        let mut seen = self.seen();
        let turn = &mut seen.turn;
        if turn.acquired.is_none() {
            return;
        }
        turn.released = Some(at);
        if let Some(trust) = &turn.trust {
            turn.walked |= trust.walks() > turn.walks_at;
        }
    }

    /// The call's costs, as its turn's run measured them.
    pub(crate) fn costed(&self, cost: CallCost) {
        self.seen().cost = Some(cost);
    }

    /// No line: a stream's, or one already written.
    fn rule_out(&self) {
        self.seen().done = true;
    }

    /// Its line, once, when its time passes the threshold.
    fn finish(&self, status: Status) {
        let ended = Instant::now();
        let ms = ended.saturating_duration_since(self.0.start);
        let mut seen = self.seen();
        if seen.done {
            return;
        }
        seen.done = true;
        if !passed(ms) {
            return;
        }
        let start = self.0.start;
        let turn = &seen.turn;
        let wait = match (turn.sought, turn.acquired) {
            (false, _) => None,
            (true, Some(acquired)) => Some(acquired.saturating_duration_since(start)),
            (true, None) => Some(ended.saturating_duration_since(start)),
        };
        let run = turn.acquired.map(|acquired| {
            turn.released
                .unwrap_or(ended)
                .saturating_duration_since(acquired)
        });
        let walk = match (&turn.trust, turn.acquired) {
            (Some(trust), Some(_)) => {
                let walked =
                    turn.walked || (turn.released.is_none() && trust.walks() > turn.walks_at);
                if walked { "walked" } else { "trusted" }
            }
            _ => "-",
        };
        let status = match status {
            Status::Code(code) => code.to_string(),
            Status::Gone => "gone".to_owned(),
        };
        // Task spec `hub-turn-fairness`: the class by route, the wait in a
        // login's line, the turn's holders and waiters ahead.
        let class = if seen.route.as_deref() == Some(MCP) {
            "mcp"
        } else {
            "page"
        };
        let line_wait = seen
            .line
            .map(|(joined, headed)| headed.unwrap_or(ended).saturating_duration_since(joined));
        let shown = |value: Option<Duration>| {
            value.map_or_else(|| "-".to_owned(), |value| millis(value).to_string())
        };
        let costs = seen.cost.unwrap_or_default().fields();
        let line = format!(
            "specengine-http: slow request at={} method={} route={} project={} id={} tool={} \
             status={status} ms={} wait_ms={} run_ms={} index={walk} class={class} line_ms={} \
             ahead={}{costs}",
            utc(self.0.at),
            self.0.method,
            seen.route.as_deref().unwrap_or("-"),
            clean(seen.project.as_deref(), false),
            clean(seen.id.as_deref(), false),
            clean(seen.tool.as_deref(), true),
            millis(ms),
            shown(wait),
            shown(run),
            shown(line_wait),
            turn.ahead
                .filter(|_| turn.sought)
                .map_or_else(|| "-".to_owned(), |ahead| ahead.to_string()),
        );
        drop(seen);
        write_line(&line);
    }

    /// A stream that writes no line: the live tail, `/mcp`'s GET.
    fn is_stream(&self) -> bool {
        let seen = self.seen();
        self.0.method == "GET" && matches!(seen.route.as_deref(), Some(EVENTS | MCP))
    }

    /// The handler's `response`: its line now, or at its body's end when
    /// the body is a stream.
    fn answered(self, response: Response) -> Response {
        if self.is_stream() {
            self.rule_out();
            return response;
        }
        let status = response.status().as_u16();
        // A body hyper never reads (a HEAD's, a 1xx, 204 or 304) is no
        // stream to wait for: never `gone` for want of its end.
        let bodiless = self.0.method == "HEAD" || matches!(status, 100..=199 | 204 | 304);
        if bodiless || response.body().size_hint().exact().is_some() {
            self.finish(Status::Code(status));
            return response;
        }
        let (parts, body) = response.into_parts();
        let end = Unanswered {
            exchange: Some(self),
            status: Some(status),
        };
        let tail = stream::once(async move {
            end.answered();
        })
        .filter_map(|()| async { None::<Result<Bytes, axum::Error>> });
        Response::from_parts(
            parts,
            Body::from_stream(body.into_data_stream().chain(tail)),
        )
    }
}

/// A request not yet answered: dropped so, its line reads `status=gone`.
struct Unanswered {
    exchange: Option<Exchange>,
    /// Set once the handler answered: the status a body's end writes.
    status: Option<u16>,
}

impl Unanswered {
    /// The answer reached its end: its line with its status.
    fn answered(mut self) {
        if let (Some(exchange), Some(status)) = (self.exchange.take(), self.status) {
            exchange.finish(Status::Code(status));
        }
    }

    /// No line from here: the handler answered.
    fn disarm(mut self) {
        self.exchange = None;
    }
}

impl Drop for Unanswered {
    fn drop(&mut self) {
        if let Some(exchange) = self.exchange.take() {
            exchange.finish(Status::Gone);
        }
    }
}

std::thread_local! {
    /// The request whose handler this thread is polling ([`Polled`]).
    static POLLED: RefCell<Option<Exchange>> = const { RefCell::new(None) };
}

/// The record of the request whose handler is being polled, if any: a
/// handler runs inline in [`watch`]'s future, so its turn finds it here.
pub(crate) fn current() -> Option<Exchange> {
    POLLED
        .try_with(|polled| polled.borrow().clone())
        .ok()
        .flatten()
}

/// `inner`, polled with `exchange` as [`current`], the one before restored
/// after each poll, a panicking one too.
struct Polled<F> {
    exchange: Exchange,
    inner: Pin<Box<F>>,
}

/// Puts back the thread's [`current`] as it was before a poll.
struct Restore(Option<Exchange>);

impl Drop for Restore {
    fn drop(&mut self) {
        let before = self.0.take();
        let _ = POLLED.try_with(|polled| *polled.borrow_mut() = before);
    }
}

impl<F: Future> Future for Polled<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.get_mut();
        let before = POLLED
            .try_with(|polled| polled.borrow_mut().replace(this.exchange.clone()))
            .ok()
            .flatten();
        let _restore = Restore(before);
        this.inner.as_mut().poll(context)
    }
}

/// The outermost layer, before the fence: a request's record from its
/// first sight, its line when answered or dropped.
pub(crate) async fn watch(mut request: Request, next: Next) -> Response {
    let exchange = Exchange::new(request.method());
    request.extensions_mut().insert(exchange.clone());
    let unanswered = Unanswered {
        exchange: Some(exchange.clone()),
        status: None,
    };
    let response = Polled {
        exchange: exchange.clone(),
        inner: Box::pin(next.run(request)),
    }
    .await;
    unanswered.disarm();
    exchange.answered(response)
}

/// A route's layer, past routing: its `template` (the path the route was
/// added under, as axum's `MatchedPath` would give it; that feature is off
/// in the pinned build) and the raw `{p}` and `{id}` noted on the request's
/// record.
pub(crate) async fn routed(template: &'static str, request: Request, next: Next) -> Response {
    if let Some(exchange) = request.extensions().get::<Exchange>() {
        exchange.routed(template, request.uri().path());
    }
    next.run(request).await
}
