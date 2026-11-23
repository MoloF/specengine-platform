//! A reference's file over HTTP (task spec `work-references`, "HTTP",
//! "Serving"; canon `work-references.md`; ADR-0088): the one binary door,
//! `PUT …/blobs`, and the read of a stored file, `GET …/blobs/<b3>`.
//!
//! - **The door**, in order, nothing read or stored on a refusal: the served
//!   slug (404); `Sec-Fetch-Site: same-origin` (403); `Origin` present and
//!   the origin byte for byte (403); `Content-Type` exactly
//!   `application/octet-stream` (415), never multipart; `Content-Encoding`
//!   absent or `identity` (415); `Content-Length` present (411); the file
//!   cap on it (413); the slots (429, `Retry-After: 5`); then the body.
//! - **The stream**: the store's write begins outside the turn
//!   ([`specengine_cli::blob_begin`]: `[ids]`, `[references]`, the quota and
//!   the floor, the `.part`); the body's pieces pass through a two-piece
//!   channel to one blocking writer, so nothing holds the body; a body idle
//!   [`UPLOAD_IDLE`] is cut. Only the rename (and a due sweep) takes the
//!   project's turn ([`specengine_cli::blob_commit`]). A cut body, a dropped
//!   connection or a refusal removes the `.part`, and frees the slots.
//! - **Early answers**: a refusal given while the body is still coming (the
//!   door, the slots, the store's caps, the type) closes the connection
//!   (`Connection: close`). Asked with `Expect: 100-continue` and the body
//!   not read yet, it is the final status at once, sent instead of `100`
//!   (RFC 9110). Otherwise at most 1 MiB of the body is read and discarded,
//!   for at most 2 s, before the answer: that spares the client a reset only
//!   when no more than 1 MiB is still coming; a larger refused body on a
//!   direct connection may still be reset (the page retries a status-0 PUT
//!   once; Traefik on a hub likely masks it). No lingering close is kept.
//! - **Slots** ([`Uploads`]): [`UPLOADS_PER_HUB`] a daemon, and on a hub
//!   [`UPLOADS_PER_LOGIN`] a login, taken before the body and held until the
//!   `.part` is renamed or removed; a login's entry leaves at zero.
//! - **Serving**: no turn; a read-only lookup names the file, which streams
//!   in 64 KiB pieces through a blocking reader, one `bytes=` range at most,
//!   sandboxed: `nosniff`, `Content-Security-Policy: sandbox`, `attachment`
//!   for a model or a PDF.

use std::collections::HashMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use axum::body::{Body, BodyDataStream, Bytes};
use axum::extract::{Request, State};
use axum::http::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONNECTION, CONTENT_DISPOSITION, CONTENT_ENCODING,
    CONTENT_LENGTH, CONTENT_RANGE, CONTENT_SECURITY_POLICY, CONTENT_TYPE, ETAG, EXPECT,
    IF_NONE_MATCH, IF_RANGE, ORIGIN, RANGE, RETRY_AFTER, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse as _, Response};
use futures_util::StreamExt as _;
use specengine_cli::{
    BLOB_BUFFER, BlobBegun, BlobCaps, BlobFailure, BlobFlight, BlobOutcome, BlobPutRequest,
    BlobRead, BlobSeams, Outcome, ReferenceCause, UPLOAD_IDLE, UPLOAD_NAME, UPLOADS_PER_HUB,
    UPLOADS_PER_LOGIN, blob_failed,
};

use crate::answer::{self, Refusal, error, run};
use crate::app::{App, Shared, same_origin, single, unknown_route_of};
use crate::hub::Login;

/// The caps the daemon read at its start (`SPECENGINE_BLOB_*`); unset, the
/// defaults.
static CAPS: OnceLock<BlobCaps> = OnceLock::new();

/// The daemon's caps, set once at its start.
pub(crate) fn set_caps(caps: BlobCaps) {
    let _ = CAPS.set(caps);
}

fn caps() -> BlobCaps {
    CAPS.get().copied().unwrap_or_default()
}

/// The one media type the door takes.
const OCTET_STREAM: &str = "application/octet-stream";

/// The uploads running now, per login (a hub's) and in all, and the bytes
/// each project's uploads in flight declared (the quota counts them).
#[derive(Debug, Default)]
pub(crate) struct Uploads {
    running: Arc<Mutex<Running>>,
    flights: Mutex<HashMap<String, BlobFlight>>,
}

#[derive(Debug, Default)]
struct Running {
    total: usize,
    logins: HashMap<String, usize>,
}

/// An upload's slots, given back when dropped.
#[derive(Debug)]
pub(crate) struct Slot {
    running: Arc<Mutex<Running>>,
    login: Option<String>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running.total = running.total.saturating_sub(1);
        if let Some(login) = &self.login
            && let Some(count) = running.logins.get_mut(login)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                running.logins.remove(login);
            }
        }
    }
}

impl Uploads {
    /// The flight of the project `slug`: one per project served, for the
    /// daemon's life.
    fn flight(&self, slug: &str) -> BlobFlight {
        let mut flights = self.flights.lock().unwrap_or_else(PoisonError::into_inner);
        flights.entry(slug.to_owned()).or_default().clone()
    }

    /// A slot of the daemon's and, on a hub, one of `login`'s; else the
    /// 429's line.
    fn take(&self, login: Option<&str>, hub: bool) -> Result<Slot, String> {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(login) = login
            && running.logins.get(login).copied().unwrap_or(0) >= UPLOADS_PER_LOGIN
        {
            return Err(format!(
                "refused: {login} runs {UPLOADS_PER_LOGIN} uploads; one must end first"
            ));
        }
        if running.total >= UPLOADS_PER_HUB {
            let who = if hub { "this hub" } else { "this daemon" };
            return Err(format!(
                "refused: {who} runs {UPLOADS_PER_HUB} uploads; try again in a moment"
            ));
        }
        running.total += 1;
        if let Some(login) = login {
            *running.logins.entry(login.to_owned()).or_default() += 1;
        }
        Ok(Slot {
            running: Arc::clone(&self.running),
            login: login.map(str::to_owned),
        })
    }
}

/// A refusal's response: its status and line as the error body.
fn refused(status: StatusCode, line: &str) -> Response {
    error(status, line)
}

/// The status of a store's refusal.
fn refusal_status(cause: Option<ReferenceCause>) -> StatusCode {
    match cause {
        Some(ReferenceCause::TooLarge) => StatusCode::PAYLOAD_TOO_LARGE,
        Some(ReferenceCause::Type) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        Some(ReferenceCause::Unknown) => StatusCode::NOT_FOUND,
        Some(ReferenceCause::Forbidden) => StatusCode::FORBIDDEN,
        Some(ReferenceCause::Refused) => StatusCode::CONFLICT,
        Some(ReferenceCause::Body) | None => StatusCode::BAD_REQUEST,
    }
}

/// A store's refusal outcome as its response.
fn outcome_refused(outcome: &BlobOutcome) -> Response {
    let line = outcome
        .refused
        .as_ref()
        .map(|refused| refused.reason.clone())
        .unwrap_or_default();
    refused(refusal_status(outcome.cause()), &line)
}

/// The origins an upload may come from: a hub's public one, else this
/// daemon's two.
fn origins(app: &App) -> Vec<String> {
    match &app.hub {
        Some(hub) => vec![hub.origin().to_owned()],
        None => vec![
            format!("http://127.0.0.1:{}", app.port),
            format!("http://localhost:{}", app.port),
        ],
    }
}

/// The door's checks after the slug, in order (see the module
/// documentation); the declared length.
fn door(app: &App, headers: &HeaderMap) -> Result<u64, Refusal> {
    same_origin(headers)?;
    let origin = single(headers, &ORIGIN, "Origin")
        .map_err(|line| Refusal::new(StatusCode::FORBIDDEN, line))?;
    if !origin.is_some_and(|origin| origins(app).iter().any(|own| own.as_bytes() == origin)) {
        return Err(Refusal::new(
            StatusCode::FORBIDDEN,
            "refused: an upload needs the Origin header",
        ));
    }
    let typed = single(headers, &CONTENT_TYPE, "Content-Type")
        .ok()
        .flatten()
        .is_some_and(|value| value.eq_ignore_ascii_case(OCTET_STREAM.as_bytes()));
    if !typed {
        return Err(Refusal::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "refused: an upload is application/octet-stream, never multipart",
        ));
    }
    let encoding = single(headers, &CONTENT_ENCODING, "Content-Encoding")
        .map_err(|line| Refusal::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, line))?;
    if encoding.is_some_and(|value| !value.eq_ignore_ascii_case(b"identity")) {
        return Err(Refusal::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "refused: an upload is sent as it is: no Content-Encoding but identity",
        ));
    }
    let Some(length) = single(headers, &CONTENT_LENGTH, "Content-Length")
        .map_err(|line| Refusal::new(StatusCode::BAD_REQUEST, line))?
    else {
        return Err(Refusal::new(
            StatusCode::LENGTH_REQUIRED,
            "refused: an upload needs its Content-Length",
        ));
    };
    let declared = std::str::from_utf8(length)
        .ok()
        .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|text| text.parse::<u64>().ok())
        .ok_or_else(|| {
            Refusal::new(
                StatusCode::BAD_REQUEST,
                "refused: the Content-Length is no number of bytes",
            )
        })?;
    if let Some(refusal) = caps().file_refusal(UPLOAD_NAME, declared) {
        return Err(Refusal::new(StatusCode::PAYLOAD_TOO_LARGE, refusal.line));
    }
    Ok(declared)
}

/// The most bytes of a refused upload's body read and discarded before its
/// answer ([`drained`]): a client with no more than this still to send then
/// reads the answer; one with more may still see a reset.
const DRAIN_BYTES: usize = 1_048_576;

/// The longest a refused upload's body is read and discarded.
const DRAIN_TIME: Duration = Duration::from_secs(2);

/// `response` to an upload refused while its body is still coming: at most
/// [`DRAIN_BYTES`] of it read and discarded, for at most [`DRAIN_TIME`],
/// never the whole declared body, then the answer with `Connection: close`
/// (the rest is never read on this connection).
async fn drained(mut stream: BodyDataStream, response: Response) -> Response {
    let _ = tokio::time::timeout(DRAIN_TIME, async {
        let mut read = 0usize;
        while read < DRAIN_BYTES {
            match stream.next().await {
                Some(Ok(chunk)) => read += chunk.len(),
                _ => break,
            }
        }
    })
    .await;
    closing(response)
}

/// `response` with `Connection: close`: the rest of the body is never read
/// on this connection.
fn closing(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    response
}

/// An early refusal, before the body is read: with `Expect: 100-continue`
/// (`expecting`) the final status at once, sent instead of `100`, and the
/// connection closed; else [`drained`].
async fn refused_early(stream: BodyDataStream, response: Response, expecting: bool) -> Response {
    if expecting {
        closing(response)
    } else {
        drained(stream, response).await
    }
}

/// The request asks `Expect: 100-continue`.
fn expects_continue(headers: &HeaderMap) -> bool {
    headers
        .get(EXPECT)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"100-continue"))
}

/// What the writer gave back: the staged file, or why not (`None`: the
/// body was cut, nothing to answer for).
type Written = Result<specengine_cli::Spooled, Option<BlobFailure>>;

/// `PUT …/blobs` (see the module documentation): 200 `BlobStored`; 400 a
/// query, a cut or short body; 403 the door; 404 the slug; 411, 413, 415,
/// 429 the door's and the store's; 503 cannot run (`[ids]`, a bad
/// `[references]`, an unwritable store). Every answer given while the body
/// is still coming closes the connection, at once when `Expect:
/// 100-continue` left it unread, else after a bounded drain
/// ([`refused_early`], [`drained`]).
pub(crate) async fn put_blob(State(app): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let stream = body.into_data_stream();
    let expecting = expects_continue(&parts.headers);
    let admitted = (|| {
        let (project, rest) = app.project_of(&parts.uri)?;
        if rest != "blobs" {
            return Err(unknown_route_of(&parts.uri));
        }
        // No query: the door takes the bytes alone.
        crate::app::args("blobs", &[], &parts.uri)?;
        let declared = door(&app, &parts.headers)?;
        Ok((project, declared))
    })();
    let (project, declared) = match admitted {
        Ok(admitted) => admitted,
        Err(refusal) => return refused_early(stream, refusal.into_response(), expecting).await,
    };
    let login = parts
        .extensions
        .get::<Login>()
        .map(|Login(login)| login.clone());
    let slot = match app.uploads.take(login.as_deref(), app.hub.is_some()) {
        Ok(slot) => slot,
        Err(line) => {
            let mut response = refused(StatusCode::TOO_MANY_REQUESTS, &line);
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from_static("5"));
            return refused_early(stream, response, expecting).await;
        }
    };
    let put = BlobPutRequest {
        name: None,
        declared,
        caps: caps(),
        seams: BlobSeams::default(),
        flight: app.uploads.flight(&project.slug),
    };
    // Outside the turn: the walk, the caps, the `.part`.
    let begun = answer::read(Arc::clone(&project), move |env, globals, _| {
        specengine_cli::blob_begin(env, globals, &put)
    })
    .await;
    let upload = match begun {
        Ok(BlobBegun::Upload(upload)) => upload,
        Ok(BlobBegun::Refused(outcome)) => {
            drop(slot);
            return refused_early(stream, outcome_refused(&outcome), expecting).await;
        }
        Err(refusal) => {
            drop(slot);
            return refused_early(stream, refusal.into_response(), expecting).await;
        }
    };
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Option<Bytes>>(2);
    let writer = tokio::task::spawn_blocking(move || -> Written {
        let mut upload = *upload;
        loop {
            match receiver.blocking_recv() {
                Some(Some(chunk)) => upload.write(&chunk).map_err(Some)?,
                Some(None) => return upload.finish().map_err(Some),
                // Dropped without its end: the body was cut.
                None => return Err(None),
            }
        }
    });
    let mut stream = stream;
    // `true` while the body may still be coming: the writer refused it.
    let mut unread = false;
    let cut = loop {
        match tokio::time::timeout(UPLOAD_IDLE, stream.next()).await {
            Err(_) => {
                break Some(format!(
                    "refused: the upload stalled for {} s and was cut",
                    UPLOAD_IDLE.as_secs()
                ));
            }
            Ok(None) => {
                let _ = sender.send(None).await;
                break None;
            }
            Ok(Some(Err(_))) => break Some("refused: the upload broke off".to_owned()),
            Ok(Some(Ok(chunk))) => {
                // The writer stopped: its refusal answers.
                if sender.send(Some(chunk)).await.is_err() {
                    unread = true;
                    break None;
                }
            }
        }
    };
    drop(sender);
    // A writer that panicked: its upload dropped, its `.part` removed.
    let Ok(written) = writer.await else {
        return answer::internal();
    };
    let staged = match (written, cut) {
        (Ok(staged), None) => staged,
        (Err(Some(failure)), _) => {
            let response = match blob_failed(failure) {
                Ok(outcome) => outcome_refused(&outcome),
                Err(cannot) => refused(StatusCode::SERVICE_UNAVAILABLE, &cannot.message),
            };
            // The slots freed before a drain's wait.
            drop(slot);
            return if unread {
                drained(stream, response).await
            } else {
                response
            };
        }
        (_, Some(line)) => return refused(StatusCode::BAD_REQUEST, &line),
        (Err(None), None) => {
            return refused(StatusCode::BAD_REQUEST, "refused: the upload broke off");
        }
    };
    let caps = caps();
    // In the turn: the rename (and a due sweep) alone; the slots held to
    // its end.
    run(project, move |env, globals| {
        let _slot = slot;
        specengine_cli::blob_commit(env, globals, staged, caps, BlobSeams::default())
            .map(|outcome| Outcome::Blob(Box::new(outcome)))
    })
    .await
}

/// `Cache-Control` of a served file: kept privately, checked each time.
const PRIVATE_NO_CACHE: HeaderValue = HeaderValue::from_static("private, no-cache");

/// The mark a served file's answer (200, 206, 304) carries in its
/// extensions: the fence keeps its own `Cache-Control` for it alone and
/// writes `no-store` on every other answer (`crate::app`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ServedFile;

/// A served range: the whole file, a part, or none that fits.
enum Span {
    Whole,
    Part { first: u64, last: u64 },
    Unsatisfiable,
}

/// The one `bytes=` range of `headers` against a file of `size` bytes, as
/// `If-Range` allows it: none, several, a malformed one or a stale
/// `If-Range` serve the whole file.
fn span(headers: &HeaderMap, size: u64, etag: &str) -> Span {
    let mut ranges = headers.get_all(RANGE).iter();
    let (Some(range), None) = (ranges.next(), ranges.next()) else {
        return Span::Whole;
    };
    if let Some(if_range) = headers.get(IF_RANGE)
        && if_range.as_bytes() != etag.as_bytes()
    {
        return Span::Whole;
    }
    let Some(spec) = range
        .to_str()
        .ok()
        .and_then(|text| text.trim().strip_prefix("bytes="))
    else {
        return Span::Whole;
    };
    if spec.contains(',') {
        return Span::Whole;
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return Span::Whole;
    };
    let number = |text: &str| {
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse::<u64>().ok())
            .flatten()
    };
    match (first.trim(), last.trim()) {
        ("", suffix) => match number(suffix) {
            None => Span::Whole,
            Some(0) => Span::Unsatisfiable,
            Some(_) if size == 0 => Span::Unsatisfiable,
            Some(length) => Span::Part {
                first: size.saturating_sub(length),
                last: size - 1,
            },
        },
        (first, "") => match number(first) {
            None => Span::Whole,
            Some(first) if first >= size => Span::Unsatisfiable,
            Some(first) => Span::Part {
                first,
                last: size - 1,
            },
        },
        (first, last) => match (number(first), number(last)) {
            (Some(first), Some(last)) if first <= last => {
                if first >= size {
                    Span::Unsatisfiable
                } else {
                    Span::Part {
                        first,
                        last: last.min(size - 1),
                    }
                }
            }
            _ => Span::Whole,
        },
    }
}

/// `If-None-Match` names `etag` (or `*`).
fn not_modified(headers: &HeaderMap, etag: &str) -> bool {
    headers.get_all(IF_NONE_MATCH).iter().any(|value| {
        value.to_str().is_ok_and(|text| {
            text.split(',').map(str::trim).any(|tag| {
                tag == "*" || tag == etag || tag.strip_prefix("W/").is_some_and(|tag| tag == etag)
            })
        })
    })
}

/// `filename="<ASCII, others _>"; filename*=UTF-8''<percent-encoded>`.
fn disposition(kind: &str, name: Option<&str>) -> HeaderValue {
    let Some(name) = name else {
        return HeaderValue::from_str(kind).unwrap_or_else(|_| HeaderValue::from_static("inline"));
    };
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::with_capacity(name.len() * 3);
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    HeaderValue::from_str(&format!(
        "{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// `GET …/blobs/<b3>` (see the module documentation): 200 the file, 206 one
/// range, 304 `If-None-Match`, 404 `{id, reason}` a hash no live reference
/// of the project names (or the store does not hold), 416 an unsatisfiable
/// range, 503 cannot run. No turn.
pub(crate) async fn get_blob(
    State(app): State<Shared>,
    request: Request,
) -> Result<Response, Refusal> {
    let (project, rest) = app.project_of(request.uri())?;
    let Some(b3) = rest
        .strip_prefix("blobs/")
        .filter(|raw| !raw.is_empty() && !raw.contains('/'))
    else {
        return Err(unknown_route_of(request.uri()));
    };
    crate::app::args("blobs", &[], request.uri())?;
    let b3 = b3.to_owned();
    let unnamed = |b3: &str| {
        let reason =
            format!("blob {b3} is not served here: no live reference of this project names it");
        let body = serde_json::json!({"id": b3, "reason": reason});
        answer::json(StatusCode::NOT_FOUND, body.to_string())
    };
    if !specengine_cli::is_blob_name(&b3) {
        return Ok(unnamed(&b3));
    }
    let asked = b3.clone();
    let Some(read) = answer::read(project, move |env, globals, _| {
        specengine_cli::blob_read(env, globals, &asked)
    })
    .await?
    else {
        return Ok(unnamed(&b3));
    };
    let headers = request.headers();
    let etag = format!("\"{b3}\"");
    let etag_value =
        HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("\"\""));
    if not_modified(headers, &etag) {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        let out = response.headers_mut();
        out.insert(ETAG, etag_value);
        out.insert(CACHE_CONTROL, PRIVATE_NO_CACHE);
        response.extensions_mut().insert(ServedFile);
        return Ok(response);
    }
    let BlobRead {
        file,
        size,
        media,
        attachment,
        name,
    } = read;
    let (status, first, length) = match span(headers, size, &etag) {
        Span::Whole => (StatusCode::OK, 0, size),
        Span::Part { first, last } => (StatusCode::PARTIAL_CONTENT, first, last - first + 1),
        Span::Unsatisfiable => {
            let mut response = error(
                StatusCode::RANGE_NOT_SATISFIABLE,
                &format!("the range asks past the file's {size} bytes"),
            );
            if let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}")) {
                response.headers_mut().insert(CONTENT_RANGE, value);
            }
            response
                .headers_mut()
                .insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            return Ok(response);
        }
    };
    let mut response = Response::new(Body::from_stream(pieces(file, first, length)));
    *response.status_mut() = status;
    let out = response.headers_mut();
    out.insert(CONTENT_TYPE, HeaderValue::from_static(media));
    out.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    out.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
    out.insert(
        CONTENT_DISPOSITION,
        disposition(
            if attachment { "attachment" } else { "inline" },
            name.as_deref(),
        ),
    );
    out.insert(CACHE_CONTROL, PRIVATE_NO_CACHE);
    out.insert(ETAG, etag_value);
    out.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    out.insert(CONTENT_LENGTH, HeaderValue::from(length));
    if status == StatusCode::PARTIAL_CONTENT
        && let Ok(value) =
            HeaderValue::from_str(&format!("bytes {first}-{}/{size}", first + length - 1))
    {
        out.insert(CONTENT_RANGE, value);
    }
    response.extensions_mut().insert(ServedFile);
    Ok(response)
}

/// `length` bytes of `file` from `first`, in [`BLOB_BUFFER`] pieces, each
/// read by a short blocking task when the answer asks for it: a stream its
/// client reads slowly, or not at all, holds no thread of the blocking pool
/// between pieces, and a dropped answer reads nothing more.
fn pieces(
    file: std::fs::File,
    first: u64,
    length: u64,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static {
    /// The file, where the next piece starts, and what is left.
    struct Reading {
        file: std::fs::File,
        at: u64,
        left: u64,
    }
    let reading = Reading {
        file,
        at: first,
        left: length,
    };
    futures_util::stream::unfold(Some(reading), |state| async move {
        let mut reading = state?;
        if reading.left == 0 {
            return None;
        }
        let read = tokio::task::spawn_blocking(move || {
            let want = usize::try_from(reading.left.min(BLOB_BUFFER as u64)).unwrap_or(BLOB_BUFFER);
            let mut buffer = vec![0u8; want];
            let piece = reading
                .file
                .seek(SeekFrom::Start(reading.at))
                .and_then(|_| {
                    loop {
                        match reading.file.read(&mut buffer) {
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                            other => break other,
                        }
                    }
                })
                .and_then(|read| {
                    if read == 0 {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "the file is shorter than it was",
                        ))
                    } else {
                        buffer.truncate(read);
                        Ok(buffer)
                    }
                });
            (reading, piece)
        })
        .await;
        match read {
            Ok((mut reading, Ok(buffer))) => {
                reading.at += buffer.len() as u64;
                reading.left -= buffer.len() as u64;
                Some((Ok(Bytes::from(buffer)), Some(reading)))
            }
            // An error ends the stream after it is sent.
            Ok((_, Err(error))) => Some((Err(error), None)),
            Err(_) => Some((
                Err(std::io::Error::other("the file could not be read")),
                None,
            )),
        }
    })
}
