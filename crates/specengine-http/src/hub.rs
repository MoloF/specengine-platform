//! Hub mode (`docs/canon/hub.md`, ADR-0058, ADR-0059, task specs
//! `hub-identity`, `hub-team`): a second fence, not a second daemon.
//! `--public-origin` is the whole switch; without it nothing here runs,
//! `Remote-User` is never read and `[team]` is never judged.
//!
//! [`Hub::parse`] judges the flag before anything is created. [`Hub::fenced`]
//! judges every request before routing, failing closed: the public `Host`
//! byte for byte, `Origin`, `Sec-Fetch-Site` (but for the landing), one
//! well-formed `Remote-User`, then the method and path. Any method but GET
//! is refused unless [`class`] names it, so a route added later stays local
//! until it is listed here. A [`Class::Confirm`] route takes no query and
//! needs `Origin`: the fence reads the path alone otherwise, and a proxy's
//! rule anchored on the path's end must see the one byte form. `/mcp/<slug>`
//! (docs/features/hub-mcp.md) takes no query on any method, its POST and
//! DELETE are [`Class::Mcp`], and it is the one path `Authorization` may
//! reach: anywhere else the header is refused before the class, by its
//! presence, since the proxy's MCP credential opens that route alone.
//!
//! The handlers do the rest, in order, nothing read or stored on a refusal:
//! [`member`] on a team or confirm route, the `[team]` of the root the
//! route's slug names, read per request and held nowhere, before a body is
//! read and again once it is in; [`acting`], which
//! binds a body's `as` to the login, on a plain or confirm route; [`needs`] on
//! a confirm route. A confirm of a staged choice has no `as` and no optional
//! key, and adds [`confirms`] ([`crate::confirm`]). A plain write never reads
//! `[team]`, so a bad one stops none (ADR-0012), and a role is authority over
//! the queue, no trade (ADR-0008).
//!
//! The daemon cannot tell its proxy's header from anything else reaching its
//! socket: that is the deployment's guarantee (`docs/canon/hub.md` "Trust").

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::header::{AUTHORIZATION, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, Method, StatusCode};
use futures_util::future::Either;
use futures_util::{Stream, StreamExt as _};
use specengine_cli::{TeamConfig, TeamEntry, hub_login_problem, team_read};

use crate::answer::{self, Refusal};
use crate::app::{SEC_FETCH_SITE, landing, single};
use crate::start::Project;

/// The proxy's one login header: a constant, so no flag can name a header
/// the proxy does not scrub.
pub(crate) const REMOTE_USER: HeaderName = HeaderName::from_static("remote-user");

/// A hub's streams' maximum life: the events tail and `/mcp`'s GET (ADR-0068,
/// RM-0075). A constant, so no flag, key or variable can lift the bound a
/// disable relies on; not below rmcp's idle end, 300 s, so a GET-only
/// client's reconnects keep no idle session (`docs/canon/hub-mcp.md`
/// "Sessions").
pub(crate) const STREAM_LIFE: Duration = Duration::from_secs(300);

/// `stream`, ending once `life` has passed since this call, on tokio's
/// clock, between two items, sending nothing more: an item taken from
/// `stream` is always yielded whole, so a client's last event ID is the last
/// whole event's (`docs/canon/serve.md` "Live tail"). `None`: `stream` as
/// given, never ended for its age (a local daemon).
pub(crate) fn lived<S: Stream>(stream: S, life: Option<Duration>) -> impl Stream<Item = S::Item> {
    match life {
        Some(life) => Either::Left(stream.take_until(tokio::time::sleep(life))),
        None => Either::Right(stream),
    }
}

/// The last segment of a card's plain writes.
const WRITES: [&str; 4] = ["move", "edit", "assign", "cancel"];

/// The last segment of a card's confirm routes: each takes a `Consent`.
const CONFIRMS: [&str; 2] = ["accept", "clear"];

/// How a write is judged on a hub (task spec `hub-team`, "Classes"). The
/// confirm class is the acts a person confirms, each behind the proxy's
/// step-up; a stage is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Any login: the handler binds `as` ([`acting`]).
    Plain,
    /// A `[team]` login: the handler reads it ([`member`]).
    Team,
    /// A `[team]` login, with no query and a required `Origin` here, and in
    /// the handler [`member`], [`acting`] and [`needs`]. RM-0064 adds the
    /// proxy's step-up on this class, which the daemon cannot see.
    Confirm,
    /// POST and DELETE on `/mcp/<slug>` (docs/features/hub-mcp.md "Data"):
    /// any login, `[team]` never read, and no query on any method of the
    /// route. The board's same-origin door is not asked of it: no MCP
    /// client sends `Sec-Fetch-Site`, and rmcp judges JSON and `Accept`.
    Mcp,
}

/// The fence's line for `Authorization` outside `/mcp/<slug>` (Q5): that
/// header is the MCP credential's, and every other route takes the login
/// door's browser session alone.
const AUTHORIZATION_REFUSED: &str = "refused: the Authorization header is for /mcp alone: this \
                                     route takes the login door's session";

/// A hub's public origin, normalised.
#[derive(Debug)]
pub(crate) struct Hub {
    /// `https://` and [`Self::authority`]: what `Origin` must be.
    origin: String,
    /// `host[:port]`, with no `:443`: what `Host` must be.
    authority: String,
}

/// The request's person: a request extension, set by the fence alone, so no
/// client can supply it.
#[derive(Debug, Clone)]
pub(crate) struct Login(pub String);

impl Hub {
    /// `given`, parsed by hand: `https` only, the host lower-cased, labels
    /// of `[a-z0-9-]` joined by `.`, an optional port, `:443` dropped. A
    /// path (even `/`), query, fragment, userinfo, loopback host or numeric
    /// host other than a dotted quad is refused with
    /// `--public-origin <given>: <reason>`.
    ///
    /// The loopback and numeric checks guard against an operator's mistake
    /// and are no guarantee: they read the text given, so a name that
    /// resolves to loopback, as `127.0.0.1.nip.io` does, cannot be caught.
    pub(crate) fn parse(given: &str) -> Result<Self, String> {
        let refused = |reason: &str| format!("--public-origin {given}: {reason}");
        let rest = given
            .strip_prefix("https://")
            .ok_or_else(|| refused("the origin must be https://host[:port]"))?;
        for (mark, reason) in [
            ('#', "a fragment is no part of an origin"),
            ('?', "a query is no part of an origin"),
            ('/', "a path, even `/`, is no part of an origin"),
            ('@', "userinfo is no part of an origin"),
        ] {
            if rest.contains(mark) {
                return Err(refused(reason));
            }
        }
        // ASCII only: a Unicode lower-casing folds U+212A into `k`.
        let rest = rest.to_ascii_lowercase();
        let (host, port) = match rest.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (rest.as_str(), None),
        };
        if host.is_empty() {
            return Err(refused("the host is empty"));
        }
        if !host.split('.').all(host_label) {
            return Err(refused(
                "a host is labels of a-z, 0-9 and `-` joined by `.`",
            ));
        }
        if ends_in_number(host) && host.parse::<Ipv4Addr>().is_err() {
            return Err(refused("a numeric host must be a dotted quad"));
        }
        if loopback(host) {
            return Err(refused("a loopback host is no public origin"));
        }
        let port = port
            .map(|port| {
                // Text compared back: `+443` and `0443` parse, and no
                // browser sends them, so the fence would refuse every page.
                port.parse::<u16>()
                    .ok()
                    .filter(|number| *number != 0 && number.to_string() == port)
                    .ok_or_else(|| refused("the port is 1 to 65535, no leading zero"))
            })
            .transpose()?;
        let authority = match port {
            None | Some(443) => host.to_owned(),
            Some(port) => format!("{host}:{port}"),
        };
        Ok(Self {
            origin: format!("https://{authority}"),
            authority,
        })
    }

    /// The origin served: `https://host[:port]`, normalised.
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    /// The authority served, `host[:port]` with no `:443`: what `Host` must
    /// be, and the one host rmcp's service admits ([`crate::mcp`]).
    pub(crate) fn authority(&self) -> &str {
        &self.authority
    }

    /// The origin with its port always written, `:443` when the origin
    /// omits it: rmcp's `Origin` allow-list entry ([`crate::mcp`]), which
    /// would match any port without one.
    pub(crate) fn origin_with_port(&self) -> String {
        if self.authority.contains(':') {
            self.origin.clone()
        } else {
            format!("{}:443", self.origin)
        }
    }

    /// The request's login, else why it is refused. In order: `Host`,
    /// `Origin`, `Sec-Fetch-Site`, `Remote-User`, then step 5: `Authorization`
    /// on any path but `/mcp/<slug>`, a query on `/mcp/<slug>` whatever the
    /// method, the method and path, then a [`Class::Confirm`] route's `query`
    /// and `Origin`.
    pub(crate) fn fenced(
        &self,
        method: &Method,
        path: &str,
        query: Option<&str>,
        headers: &HeaderMap,
    ) -> Result<Login, String> {
        let host = single(headers, &HOST, "Host")?;
        if host != Some(self.authority.as_bytes()) {
            return Err(format!(
                "refused: the Host header must be {}: this hub serves its public origin only",
                self.authority
            ));
        }
        let origin = single(headers, &ORIGIN, "Origin")?;
        if let Some(origin) = origin
            && origin != self.origin.as_bytes()
        {
            return Err(format!(
                "refused: the Origin header, when sent, must be {}",
                self.origin
            ));
        }
        if let Some(site) = single(headers, &SEC_FETCH_SITE, "Sec-Fetch-Site")?
            && site != b"same-origin"
            && site != b"none"
            && !landing(method, path, headers)?
        {
            return Err(
                "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`".to_owned(),
            );
        }
        let login = judged_login(headers)?;
        // Step 5, before the class: `Authorization`, once or more, any value,
        // is refused by its presence outside `/mcp/<slug>`, nothing read; on
        // it the header passes, never read (docs/features/hub-mcp.md Q5).
        let mcp = mcp_route(path);
        if !mcp && headers.contains_key(AUTHORIZATION) {
            return Err(AUTHORIZATION_REFUSED.to_owned());
        }
        // `/mcp/<slug>` takes no query on any method, GET's included.
        if mcp && query.is_some() {
            return Err("refused: this route takes no query".to_owned());
        }
        if method == Method::GET {
            return Ok(login);
        }
        match class(method, path) {
            None => Err("refused: this hub does not serve this route".to_owned()),
            Some(Class::Confirm) if query.is_some() => {
                Err("refused: this route takes no query".to_owned())
            }
            Some(Class::Confirm) if origin.is_none() => {
                Err("refused: this route needs the Origin header".to_owned())
            }
            Some(_) => Ok(login),
        }
    }
}

/// One label of a host, lower-cased already.
fn host_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// `host`, lower-cased, ends in what a browser expands into an address:
/// `127.1`, `2130706433`, `0` and `0x7f.0.0.1` all hide `loopback`'s text.
fn ends_in_number(host: &str) -> bool {
    let last = host.rsplit('.').next().unwrap_or(host);
    last.starts_with("0x") || last.bytes().all(|byte| byte.is_ascii_digit())
}

/// `host`, lower-cased, shows in its text that it is this machine.
fn loopback(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host
            .parse::<Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback() || address.is_unspecified())
}

/// The one `Remote-User`, judged: UTF-8, then the library's
/// [`hub_login_problem`]. An empty value is no identity. A Latin look-alike
/// (a Cyrillic `o`) and NFC against NFD pass: they stay the login door's duty.
fn judged_login(headers: &HeaderMap) -> Result<Login, String> {
    let Some(value) =
        single(headers, &REMOTE_USER, "Remote-User")?.filter(|value| !value.is_empty())
    else {
        return Err("refused: this hub needs one Remote-User header from its proxy".to_owned());
    };
    let why = |why: &str| format!("refused: Remote-User: {why}");
    let login = std::str::from_utf8(value).map_err(|_| why("not UTF-8"))?;
    if let Some(trouble) = hub_login_problem(login) {
        return Err(why(&trouble));
    }
    Ok(Login(login.to_owned()))
}

/// The class of a write, default deny: `None` for a route this hub does not
/// serve. Exact raw segments, so an encoded or trailing form of a route is
/// not listed. Plain: POST on `.../cards` or
/// `.../cards/:id/{move,edit,assign,cancel}`. Team: POST, DELETE on
/// `.../proposals/:id/decision`, POST on `.../round/answers`. Confirm: POST on
/// `.../cards/:id/{accept,clear}`, `.../trace/clear` and
/// `.../proposals/:id/confirm`. Mcp: POST, DELETE on `/mcp/<slug>`.
fn class(method: &Method, path: &str) -> Option<Class> {
    let post = method == Method::POST;
    if mcp_route(path) {
        return (post || method == Method::DELETE).then_some(Class::Mcp);
    }
    let segments: Vec<&str> = path.split('/').collect();
    let ["", "api", "projects", project, route @ ..] = segments.as_slice() else {
        return None;
    };
    if project.is_empty() {
        return None;
    }
    match route {
        ["cards"] if post => Some(Class::Plain),
        ["cards", id, verb] if post && !id.is_empty() => {
            if WRITES.contains(verb) {
                Some(Class::Plain)
            } else if CONFIRMS.contains(verb) {
                Some(Class::Confirm)
            } else {
                None
            }
        }
        // The last segment read off the raw path: a route path is the one
        // literal that may name these two words (`tests/no_domain.rs`).
        ["proposals", id, _]
            if !id.is_empty()
                && (post || method == Method::DELETE)
                && path.ends_with("/decision") =>
        {
            Some(Class::Team)
        }
        ["proposals", id, "confirm"] if post && !id.is_empty() => Some(Class::Confirm),
        ["round", _] if post && path.ends_with("/round/answers") => Some(Class::Team),
        ["trace", "clear"] if post => Some(Class::Confirm),
        _ => None,
    }
}

/// `path` is `/mcp/<seg>`, `<seg>` one non-empty raw segment: the MCP route
/// and the one path `Authorization` may reach.
fn mcp_route(path: &str) -> bool {
    path.strip_prefix("/mcp/")
        .is_some_and(|segment| !segment.is_empty() && !segment.contains('/'))
}

/// Binds a card write's `actor` to the login, before any turn: no [`Login`]
/// (a local daemon), nothing changes; `None`, it takes the login; the login
/// byte for byte, nothing changes; another name, a 403, nothing stored.
pub(crate) fn acting(login: Option<&Login>, actor: &mut Option<String>) -> Result<(), Refusal> {
    let Some(Login(login)) = login else {
        return Ok(());
    };
    match actor {
        None => *actor = Some(login.clone()),
        Some(named) if named == login => {}
        Some(named) => {
            return Err(forbidden(format!(
                "refused: `as` names {named}; this request's login is {login}"
            )));
        }
    }
    Ok(())
}

/// The door of a team or confirm route, before the turn: the login's entry
/// in the `[team]` of the root the route's slug names, read now and kept by
/// nobody. No [`Login`] (a local daemon), `None` and nothing read. A bad
/// `[team]` is a 503, no `[team]` and an absent login a 403: an absent login
/// is never a role. A route with a body calls it twice: first, so an outsider
/// is refused with no body read; again once the body is in, so a login taken
/// out of `[team]` while a slow body was open keeps no power.
pub(crate) async fn member(
    project: &Arc<Project>,
    login: Option<&Login>,
) -> Result<Option<TeamEntry>, Refusal> {
    let Some(Login(login)) = login else {
        return Ok(None);
    };
    let team = answer::read(Arc::clone(project), |env, globals, _| {
        team_read(env, globals)
    })
    .await?;
    entry_in(team.as_ref(), login).map(Some).map_err(forbidden)
}

/// `login`'s entry in `team` as read, else the 403's line: no `[team]`, or a
/// login outside it, which is never a role.
fn entry_in(team: Option<&TeamConfig>, login: &str) -> Result<TeamEntry, String> {
    let Some(team) = team else {
        return Err("refused: this project has no `[team]`".to_owned());
    };
    team.entry(login).cloned().ok_or_else(|| {
        format!("refused: {login} is not in this project's `[team]`: reads and plain writes")
    })
}

/// A confirm's entry as read inside the project's turn, the one judged
/// (docs/features/hub-confirm.md "Rules and edge cases"): `login`'s in
/// `team`, of a role that confirms; else the 403's line, as [`member`] and
/// [`confirms`] word it. A login demoted or taken out of `[team]` while its
/// request waited for the turn keeps no power.
pub(crate) fn confirming(team: Option<&TeamConfig>, login: &str) -> Result<TeamEntry, String> {
    let entry = entry_in(team, login)?;
    match confirms_nothing(&entry) {
        Some(line) => Err(line),
        None => Ok(entry),
    }
}

/// A group clear past [`member`]: a role that clears no group is a 403 before
/// the body is read; whether a `lead` covers the group is judged with the set
/// known, by the library. No entry (a local daemon): nothing judged.
pub(crate) fn clears_groups(entry: Option<&TeamEntry>) -> Result<(), Refusal> {
    match entry {
        Some(entry) if !entry.role.clears_groups() => Err(forbidden(format!(
            "refused: {} is a `member`: this act needs a `lead`",
            entry.login
        ))),
        _ => Ok(()),
    }
}

/// A confirm of a staged choice past [`member`], before its body is read; the
/// library judges the cover with the row known. No entry: nothing judged.
pub(crate) fn confirms(entry: Option<&TeamEntry>) -> Result<(), Refusal> {
    match entry.and_then(confirms_nothing) {
        Some(line) => Err(forbidden(line)),
        None => Ok(()),
    }
}

/// The 403's line of `entry` when its role confirms nothing.
fn confirms_nothing(entry: &TeamEntry) -> Option<String> {
    (!entry.role.confirms()).then(|| {
        format!(
            "refused: {} is a `member`: this act needs a `lead`",
            entry.login
        )
    })
}

/// A confirm route on a hub needs `key`, the compare of what the page showed:
/// a 400 when it was not `sent`. No [`Login`] (a local daemon): as built.
pub(crate) fn needs(login: Option<&Login>, key: &str, sent: bool) -> Result<(), Refusal> {
    if login.is_some() && !sent {
        return Err(Refusal::bad_request(format!("a hub needs `{key}`")));
    }
    Ok(())
}

/// A 403, the fence's answer.
fn forbidden(message: impl Into<String>) -> Refusal {
    Refusal::new(StatusCode::FORBIDDEN, message)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use axum::response::IntoResponse as _;

    use super::*;

    const AUTHORITY: &str = "hub.example.test";
    const ORIGIN_SERVED: &str = "https://hub.example.test";

    fn hub() -> Hub {
        Hub::parse(ORIGIN_SERVED).expect("a good origin")
    }

    fn map(pairs: &[(&'static str, &[u8])]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                HeaderName::from_static(name),
                HeaderValue::from_bytes(value).expect("a header value"),
            );
        }
        map
    }

    /// The public `Host`, one `Remote-User`, and `extra`.
    fn proxied(extra: &[(&'static str, &[u8])]) -> HeaderMap {
        let mut pairs: Vec<(&'static str, &[u8])> =
            vec![("host", AUTHORITY.as_bytes()), ("remote-user", b"mira")];
        pairs.extend_from_slice(extra);
        map(&pairs)
    }

    fn get(path: &str, headers: &HeaderMap) -> Result<Login, String> {
        hub().fenced(&Method::GET, path, None, headers)
    }

    fn post(path: &str, headers: &HeaderMap) -> Result<Login, String> {
        hub().fenced(&Method::POST, path, None, headers)
    }

    #[test]
    fn the_flag_normalises_a_good_origin() {
        let hub = Hub::parse("https://Hub.Example.test:443").expect("a good origin");
        assert_eq!(hub.origin, ORIGIN_SERVED);
        assert_eq!(hub.authority, AUTHORITY);
        let hub = Hub::parse("https://hub.example.test:8443").expect("a good origin");
        assert_eq!(hub.origin, "https://hub.example.test:8443");
        assert_eq!(hub.authority, "hub.example.test:8443");
        assert_eq!(hub.origin(), "https://hub.example.test:8443");
        assert!(Hub::parse("https://a-b.example.test").is_ok());
        assert!(Hub::parse("https://203.0.113.7").is_ok());
        assert!(Hub::parse("https://1a.example.0a").is_ok());
    }

    #[test]
    fn a_numeric_host_is_a_dotted_quad() {
        for given in [
            "https://127.1",
            "https://2130706433",
            "https://0",
            "https://0x7f.0.0.1",
            "https://0x7f000001",
            "https://0X7F.1",
            "https://127.0.0.0x1",
            "https://0177.0.0.1",
            "https://1.2.3",
            "https://1.2.3.4.5",
            "https://256.0.0.1",
            "https://hub.example.123",
            "https://hub.example.0xa",
        ] {
            assert_eq!(
                Hub::parse(given).err(),
                Some(format!(
                    "--public-origin {given}: a numeric host must be a dotted quad"
                )),
                "{given}"
            );
        }
        // A dotted quad falls under the loopback rule.
        assert_eq!(
            Hub::parse("https://127.0.0.1").err().as_deref(),
            Some("--public-origin https://127.0.0.1: a loopback host is no public origin")
        );
        // No guarantee: the text of a name says nothing of where it resolves.
        assert!(Hub::parse("https://127.0.0.1.nip.io").is_ok());
    }

    #[test]
    fn the_flag_refuses_what_is_no_origin() {
        for given in [
            "",
            "hub.example.test",
            "http://hub.example.test",
            "HTTPS://hub.example.test",
            "https://",
            "https://:8443",
            "https://hub.example.test/",
            "https://hub.example.test/a",
            "https://hub.example.test?q",
            "https://hub.example.test#f",
            "https://a@hub.example.test",
            "https://hub.example.test.",
            "https://.hub.example.test",
            "https://hub..example.test",
            "https://hub_a.example.test",
            "https://hüb.example.test",
            "https://hub.example.test ",
            "https://\u{212a}.example.test",
            "https://hub.example.test:",
            "https://hub.example.test:0",
            "https://hub.example.test:65536",
            "https://hub.example.test:0443",
            "https://hub.example.test:+443",
            "https://hub.example.test:port",
            "https://[::1]",
            "https://a:b:8443",
        ] {
            let refused = Hub::parse(given).err();
            assert!(
                refused
                    .as_deref()
                    .is_some_and(|line| line.starts_with(&format!("--public-origin {given}: "))),
                "{given:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn a_loopback_host_is_no_public_origin() {
        for given in [
            "https://localhost",
            "https://LOCALHOST:8443",
            "https://app.localhost",
            "https://127.0.0.1",
            "https://127.1.2.3:8443",
            "https://0.0.0.0",
        ] {
            assert!(Hub::parse(given).is_err(), "{given}");
        }
    }

    #[test]
    fn the_fence_takes_the_public_origin_alone() {
        let login = get("/api/projects", &proxied(&[])).expect("a good request");
        assert_eq!(login.0, "mira");
        assert!(
            get(
                "/api/projects",
                &proxied(&[
                    ("origin", ORIGIN_SERVED.as_bytes()),
                    ("sec-fetch-site", b"same-origin"),
                ])
            )
            .is_ok()
        );
        assert!(get("/", &proxied(&[("sec-fetch-site", b"none")])).is_ok());

        let line = "refused: the Host header must be hub.example.test: this hub serves its \
                    public origin only";
        for host in [
            "evil.test",
            "127.0.0.1:7777",
            "localhost:7777",
            "hub.example.test:443",
            "Hub.example.test",
        ] {
            let headers = map(&[("host", host.as_bytes()), ("remote-user", b"mira")]);
            assert_eq!(
                get("/api/projects", &headers).err().as_deref(),
                Some(line),
                "{host}"
            );
        }
        let no_host = map(&[("remote-user", b"mira")]);
        assert_eq!(get("/api/projects", &no_host).err().as_deref(), Some(line));

        for origin in [
            "https://evil.test",
            "https://hub.example.test.evil.test",
            "https://hub.example.test:8443",
            "http://hub.example.test",
            "null",
        ] {
            assert_eq!(
                get("/api/projects", &proxied(&[("origin", origin.as_bytes())]))
                    .err()
                    .as_deref(),
                Some("refused: the Origin header, when sent, must be https://hub.example.test"),
                "{origin}"
            );
        }
        for site in ["same-site", "cross-site"] {
            assert_eq!(
                get(
                    "/api/projects",
                    &proxied(&[("sec-fetch-site", site.as_bytes())])
                )
                .err()
                .as_deref(),
                Some("refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`"),
                "{site}"
            );
        }
    }

    #[test]
    fn a_header_sent_twice_is_refused() {
        for (name, value) in [
            ("host", AUTHORITY.as_bytes()),
            ("origin", ORIGIN_SERVED.as_bytes()),
            ("sec-fetch-site", b"none"),
            ("remote-user", b"mira"),
        ] {
            let mut headers = proxied(&[]);
            headers.append(
                HeaderName::from_static(name),
                HeaderValue::from_bytes(value).expect("a header value"),
            );
            if name == "origin" || name == "sec-fetch-site" {
                headers.append(
                    HeaderName::from_static(name),
                    HeaderValue::from_bytes(value).expect("a header value"),
                );
            }
            let refused = get("/api/projects", &headers).err();
            assert!(
                refused
                    .as_deref()
                    .is_some_and(|line| line.ends_with("header is sent more than once")),
                "{name}: {refused:?}"
            );
        }
    }

    #[test]
    fn the_landing_is_a_top_level_navigation_to_the_page_alone() {
        let navigate: [(&'static str, &[u8]); 2] = [
            ("sec-fetch-mode", b"navigate"),
            ("sec-fetch-dest", b"document"),
        ];
        for site in ["same-site", "cross-site"] {
            let mut extra = navigate.to_vec();
            extra.push(("sec-fetch-site", site.as_bytes()));
            let headers = proxied(&extra);
            assert!(get("/", &headers).is_ok(), "{site}");
            assert!(get("/api/projects", &headers).is_err(), "{site}");
            assert!(get("/assets/x.js", &headers).is_err(), "{site}");
            assert!(post("/", &headers).is_err(), "{site}");
            assert!(
                hub().fenced(&Method::HEAD, "/", None, &headers).is_err(),
                "{site}"
            );
        }
        for extra in [
            [
                ("sec-fetch-mode", b"navigate".as_slice()),
                ("sec-fetch-dest", b"iframe".as_slice()),
            ],
            [
                ("sec-fetch-mode", b"cors".as_slice()),
                ("sec-fetch-dest", b"document".as_slice()),
            ],
        ] {
            let mut extra = extra.to_vec();
            extra.push(("sec-fetch-site", b"cross-site"));
            assert!(get("/", &proxied(&extra)).is_err());
        }
        let mut doubled = navigate.to_vec();
        doubled.extend_from_slice(&[
            ("sec-fetch-mode", b"navigate"),
            ("sec-fetch-site", b"cross-site"),
        ]);
        assert!(get("/", &proxied(&doubled)).is_err());
        // The landing takes the rest of the fence.
        let mut no_identity = navigate.to_vec();
        no_identity.push(("host", AUTHORITY.as_bytes()));
        no_identity.push(("sec-fetch-site", b"same-site"));
        assert_eq!(
            get("/", &map(&no_identity)).err().as_deref(),
            Some("refused: this hub needs one Remote-User header from its proxy")
        );
    }

    #[test]
    fn no_route_is_served_without_one_good_identity() {
        let none = "refused: this hub needs one Remote-User header from its proxy";
        for path in [
            "/",
            "/assets/x.js",
            "/api/identity",
            "/api/projects",
            "/api/projects/p/events",
        ] {
            let bare = map(&[("host", AUTHORITY.as_bytes())]);
            assert_eq!(get(path, &bare).err().as_deref(), Some(none), "{path}");
            let empty = map(&[("host", AUTHORITY.as_bytes()), ("remote-user", b"")]);
            assert_eq!(get(path, &empty).err().as_deref(), Some(none), "{path}");
        }
        let long = "x".repeat(129);
        let spaced = "\u{a0}mira";
        let cases: [(&[u8], &str); 11] = [
            (
                b"eve, mira",
                "refused: Remote-User: a comma: one login, not a list",
            ),
            (
                b"mi\tra",
                "refused: Remote-User: U+0009 in a name: one line, no control character or \
                 bidirectional mark",
            ),
            (b"mi\xffra", "refused: Remote-User: not UTF-8"),
            (
                long.as_bytes(),
                "refused: Remote-User: 129 bytes; at most 128",
            ),
            (
                spaced.as_bytes(),
                "refused: Remote-User: white space around the login",
            ),
            (
                b"local",
                "refused: Remote-User: `local` is the record's word for no person",
            ),
            (
                b"agent",
                "refused: Remote-User: `agent` is the record's word for no person",
            ),
            (
                b"Local",
                "refused: Remote-User: `Local` is the record's word for no person",
            ),
            (
                b"AGENT",
                "refused: Remote-User: `AGENT` is the record's word for no person",
            ),
            (
                b"aGeNt",
                "refused: Remote-User: `aGeNt` is the record's word for no person",
            ),
            (
                b"-",
                "refused: Remote-User: `-` is the record's word for clearing a name",
            ),
        ];
        for (value, line) in cases {
            let headers = map(&[("host", AUTHORITY.as_bytes()), ("remote-user", value)]);
            assert_eq!(get("/", &headers).err().as_deref(), Some(line));
        }
    }

    #[test]
    fn a_format_character_is_no_part_of_a_login() {
        // Both ends of each range of category Cf the person's grammar leaves.
        for c in [
            '\u{ad}',
            '\u{600}',
            '\u{605}',
            '\u{6dd}',
            '\u{70f}',
            '\u{890}',
            '\u{891}',
            '\u{8e2}',
            '\u{180e}',
            '\u{200b}',
            '\u{200d}',
            '\u{2060}',
            '\u{2064}',
            '\u{206a}',
            '\u{206f}',
            '\u{feff}',
            '\u{fff9}',
            '\u{fffb}',
            '\u{110bd}',
            '\u{110cd}',
            '\u{13430}',
            '\u{1343f}',
            '\u{1bca0}',
            '\u{1bca3}',
            '\u{1d173}',
            '\u{1d17a}',
            '\u{e0001}',
            '\u{e0020}',
            '\u{e007f}',
        ] {
            for login in [format!("mira{c}"), format!("{c}mira"), format!("mi{c}ra")] {
                let headers = map(&[
                    ("host", AUTHORITY.as_bytes()),
                    ("remote-user", login.as_bytes()),
                ]);
                assert_eq!(
                    get("/", &headers).err(),
                    Some(format!(
                        "refused: Remote-User: U+{:04X} in a name: no format character, which \
                         can read as another person",
                        u32::from(c)
                    )),
                    "{login:?}"
                );
            }
        }
        // The rest of Cf is the grammar's bidirectional marks.
        for c in [
            '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202e}', '\u{2066}', '\u{2069}',
        ] {
            let login = format!("mira{c}");
            let headers = map(&[
                ("host", AUTHORITY.as_bytes()),
                ("remote-user", login.as_bytes()),
            ]);
            assert!(get("/", &headers).is_err(), "{login:?}");
        }
    }

    #[test]
    fn a_login_is_any_script_and_kept_verbatim() {
        for login in [
            "mira",
            "Mira K",
            "\u{041c}\u{0438}\u{0440}\u{0430}",
            "mira@example.test",
            "locals",
            "--",
            // The login door's duty: a Cyrillic look-alike and an NFD form.
            "m\u{456}ra",
            "Jose\u{301}",
        ] {
            let headers = map(&[
                ("host", AUTHORITY.as_bytes()),
                ("remote-user", login.as_bytes()),
            ]);
            assert_eq!(get("/", &headers).expect("a login").0, login);
        }
    }

    /// hub-team, "Classes": default deny by exact raw segments and exact methods,
    /// each class on its own routes; the confirm class's own two refusals in the
    /// fence, a team or a plain route taking neither. M: `fenced` reading the
    /// path alone.
    #[test]
    fn a_write_is_refused_until_listed() {
        let line = "refused: this hub does not serve this route";
        let page = || proxied(&[("origin", ORIGIN_SERVED.as_bytes())]);
        let plain = [
            "/api/projects/p/cards",
            "/api/projects/p/cards/C-0001/move",
            "/api/projects/p/cards/C-0001/edit",
            "/api/projects/p/cards/C-0001/assign",
            "/api/projects/p/cards/C-0001/cancel",
        ];
        let team = [
            "/api/projects/p/proposals/PR-0001/decision",
            "/api/projects/p/round/answers",
        ];
        let confirm = [
            "/api/projects/p/cards/C-0001/accept",
            "/api/projects/p/cards/C-0001/clear",
            "/api/projects/p/trace/clear",
        ];
        for path in plain.iter().chain(&team).chain(&confirm) {
            assert!(post(path, &page()).is_ok(), "{path}");
            for method in [Method::PUT, Method::PATCH, Method::HEAD] {
                assert_eq!(
                    hub().fenced(&method, path, None, &page()).err().as_deref(),
                    Some(line),
                    "{method} {path}"
                );
            }
        }
        // A team route takes DELETE on the stage alone.
        assert!(
            hub()
                .fenced(&Method::DELETE, team[0], None, &page())
                .is_ok()
        );
        for path in plain.iter().chain(&team[1..]).chain(&confirm) {
            assert_eq!(
                hub()
                    .fenced(&Method::DELETE, path, None, &page())
                    .err()
                    .as_deref(),
                Some(line),
                "DELETE {path}"
            );
        }
        // No `Origin` and a query: the confirm class's alone, a bare `?` too.
        for path in plain.iter().chain(&team) {
            assert!(post(path, &proxied(&[])).is_ok(), "{path}");
            assert!(
                hub()
                    .fenced(&Method::POST, path, Some("x=1"), &page())
                    .is_ok(),
                "{path}"
            );
        }
        for path in confirm {
            assert_eq!(
                post(path, &proxied(&[])).err().as_deref(),
                Some("refused: this route needs the Origin header"),
                "{path}"
            );
            for query in ["x=/move", "x=1", ""] {
                assert_eq!(
                    hub()
                        .fenced(&Method::POST, path, Some(query), &page())
                        .err()
                        .as_deref(),
                    Some("refused: this route takes no query"),
                    "{path}?{query}"
                );
            }
        }
        for path in [
            "/",
            "/api/identity",
            "/api/projects",
            "/api/projects/p/tree",
            "/api/projects/p/cards/C-0001",
            "/api/projects/p/cards/C-0001/archive",
            "/api/projects/p/cards/C-0001/%6Dove",
            "/api/projects/p/cards/C-0001/Move",
            "/api/projects/p/cards/C-0001/move/",
            "/api/projects/p/cards/",
            "/api/projects/p/cards/C-0001/move/x",
            "/api/projects/p/cards/C-0001/accept/",
            "/api/projects/p/cards/C-0001/%61ccept",
            "/api/projects/p/cards//accept",
            "/api/projects/p/proposals/PR-0001/decision/",
            "/api/projects/p/proposals//decision",
            "/api/projects/p/proposals/PR-0001/%64ecision",
            "/api/projects/p/proposals/PR-0001/undecision",
            "/api/projects/p/proposals/decision",
            "/api/projects/p/round/answers/",
            "/api/projects/p/round/%61nswers",
            "/api/projects/p/round/xanswers",
            "/api/projects/p/round",
            "/api/projects/p/trace/clear/",
            "/api/projects/p/trace/%63lear",
            "/api/projects/p/trace",
            "/api/projects//cards",
            "/api/projects//cards/C-0001/move",
            "/api/projects//trace/clear",
            "/api/projects//round/answers",
            "/api/projects//proposals/PR-0001/decision",
            "/api/projects/p/cards//move",
            "//api/projects/p/cards",
            "/API/projects/p/cards",
            "api/projects/p/cards",
            "/api/projects/p/nothing",
            "*",
        ] {
            assert_eq!(post(path, &page()).err().as_deref(), Some(line), "{path}");
        }
        let unknown = Method::from_bytes(b"post").expect("an extension method");
        for method in [Method::OPTIONS, Method::CONNECT, Method::TRACE, unknown] {
            assert_eq!(
                hub()
                    .fenced(&method, "/api/projects/p/cards", None, &page())
                    .err()
                    .as_deref(),
                Some(line),
                "{method}"
            );
        }
        // A read of any path past the fence: the routes answer it.
        assert!(get("/api/projects/p/inbox", &proxied(&[])).is_ok());
        assert!(get("/api/projects/p/nothing", &proxied(&[])).is_ok());
    }

    #[test]
    fn the_acting_name_is_the_login() {
        let login = Login("mira".to_owned());

        let mut actor = None;
        assert!(acting(None, &mut actor).is_ok());
        assert_eq!(actor, None, "a local daemon changes nothing");
        let mut actor = Some("Ana".to_owned());
        assert!(acting(None, &mut actor).is_ok());
        assert_eq!(actor.as_deref(), Some("Ana"));

        let mut actor = None;
        assert!(acting(Some(&login), &mut actor).is_ok());
        assert_eq!(actor.as_deref(), Some("mira"));
        let mut actor = Some("mira".to_owned());
        assert!(acting(Some(&login), &mut actor).is_ok());
        assert_eq!(actor.as_deref(), Some("mira"));

        for named in ["tomas", "Mira", " mira", "local"] {
            let mut actor = Some(named.to_owned());
            let refused = acting(Some(&login), &mut actor).expect_err("another name");
            assert_eq!(refused.into_response().status(), StatusCode::FORBIDDEN);
            assert_eq!(actor.as_deref(), Some(named), "nothing overwritten");
        }
    }

    /// The router's call, spelled apart: `tests/hub.rs` reads every `src` file
    /// for it, and this is no route.
    const ROUTER_CALL: &str = concat!(".", "route(");

    /// The four routes of the confirm class, as the router spells them.
    const STEPPED_UP: [&str; 4] = [
        "/api/projects/{p}/cards/{id}/accept",
        "/api/projects/{p}/cards/{id}/clear",
        "/api/projects/{p}/proposals/{id}/confirm",
        "/api/projects/{p}/trace/clear",
    ];

    /// Each line of a fenced block of `text` that opens as a route pattern.
    fn fenced_patterns(text: &str) -> Vec<&str> {
        let mut inside = false;
        let mut found = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with("```") {
                inside = !inside;
            } else if inside && line.starts_with("^/api/projects/") {
                found.push(line);
            }
        }
        found
    }

    /// Every method but GET of every router call in `source`, with its path,
    /// comment lines left out, as `tests/hub.rs`'s `get_routes_in` reads the
    /// GET ones; an `any(..)` route serves each of the four (`/mcp/{p}`,
    /// docs/features/hub-mcp.md S3). A path that is no literal panics: it
    /// could not be judged.
    fn writes_in(source: &str) -> Vec<(Method, String)> {
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| format!("{line}\n"))
            .collect();
        let mut writes = Vec::new();
        for (at, _) in code.match_indices(ROUTER_CALL) {
            let rest = &code[at + ROUTER_CALL.len()..];
            let mut depth = 1;
            let end = rest
                .char_indices()
                .find(|(_, letter)| {
                    match letter {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map(|(end, _)| end)
                .expect("a closed call");
            let (path, methods) = rest[..end]
                .trim_start()
                .strip_prefix('"')
                .and_then(|call| call.split_once('"'))
                .unwrap_or_else(|| panic!("a path that is no literal: {}", &rest[..end]));
            let methods =
                methods.trim_start_matches(|letter: char| letter == ',' || letter.is_whitespace());
            let any = methods.starts_with("any(") || methods.contains(".any(");
            for (word, method) in [
                ("post(", Method::POST),
                ("put(", Method::PUT),
                ("patch(", Method::PATCH),
                ("delete(", Method::DELETE),
            ] {
                if any || methods.contains(word) {
                    writes.push((method, path.to_owned()));
                }
            }
        }
        writes
    }

    /// `route` as a request names it: each `{..}` segment a value.
    fn sampled(route: &str) -> String {
        route
            .split('/')
            .map(|segment| {
                if segment.starts_with('{') {
                    "x1"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// B11 of docs/features/hub-confirm.md: the proxy's step-up regex, read from
    /// its own fenced block of the hub's canon, against the router's every
    /// write, read from `app.rs` as `get_routes_in` reads the GET ones. It
    /// matches the four routes of the confirm class and no other write, and it
    /// agrees with [`class`] on each, so neither the route, the class nor the
    /// regex can move alone. M: renaming a route alone.
    #[test]
    fn the_step_up_regex_is_the_confirm_class() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // Spelled apart: `tests/no_domain.rs` reads this word in any literal.
        let page = manifest
            .join("../../docs")
            .join(concat!("ca", "non"))
            .join("hub.md");
        let text = std::fs::read_to_string(&page)
            .unwrap_or_else(|error| panic!("{}: {error}", page.display()));
        let patterns = fenced_patterns(&text);
        assert_eq!(
            patterns.len(),
            1,
            "one fenced pattern in {}: {patterns:?}",
            page.display()
        );
        let regex = regex::Regex::new(patterns[0]).expect("a pattern");

        let router = manifest.join("src/app.rs");
        let source = std::fs::read_to_string(&router).expect("the router's source");
        let writes = writes_in(&source);
        assert!(writes.len() > STEPPED_UP.len(), "{writes:?}");
        let mut matched: Vec<&str> = writes
            .iter()
            .filter(|(_, route)| regex.is_match(&sampled(route)))
            .map(|(_, route)| route.as_str())
            .collect();
        matched.sort_unstable();
        assert_eq!(matched, STEPPED_UP, "the four, and no other write");
        for (method, route) in &writes {
            let path = sampled(route);
            assert_eq!(
                regex.is_match(&path),
                class(method, &path) == Some(Class::Confirm),
                "{method} {route}: the regex and `class`"
            );
        }

        // The pattern's own controls: one byte form, so a suffix, a query or a
        // sibling route is no match.
        for route in STEPPED_UP {
            let path = sampled(route);
            assert!(regex.is_match(&path), "{path}");
            for form in [
                format!("{path}/"),
                format!("{path}?x=1"),
                format!("{path}/x"),
                format!("/x{path}"),
            ] {
                assert!(!regex.is_match(&form), "{form}");
            }
        }
        for path in [
            "/api/projects/x1/proposals/x1/decision",
            "/api/projects/x1/round/answers",
            "/api/projects/x1/cards",
            "/api/projects/x1/cards/x1/move",
            "/api/projects/x1/trace",
        ] {
            assert!(!regex.is_match(path), "{path}");
        }

        // The reader's controls: what it must see and what it must leave.
        let probe = "let r = R::new()\n    @\"/a\", read(a))\n    \
                     @\"/b\", post(b).fallback(only_post))\n    \
                     // @\"/c\", post(c))\n    \
                     @\n        \"/d\",\n        get(d)\n            .post(e)\n            \
                     .delete(f),\n    );\n"
            .replace('@', ROUTER_CALL);
        assert_eq!(
            writes_in(&probe),
            [
                (Method::POST, "/b".to_owned()),
                (Method::POST, "/d".to_owned()),
                (Method::DELETE, "/d".to_owned()),
            ]
        );
        let block = "text\n```text\n^/api/projects/a$\n```\n^/api/projects/b$\n";
        assert_eq!(fenced_patterns(block), ["^/api/projects/a$"]);
    }

    /// Every path of every router call in `source`, any method, comment lines
    /// left out, each once, as [`writes_in`] reads the writes.
    fn routes_in(source: &str) -> Vec<String> {
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| format!("{line}\n"))
            .collect();
        let mut routes: Vec<String> = Vec::new();
        for (at, _) in code.match_indices(ROUTER_CALL) {
            let rest = &code[at + ROUTER_CALL.len()..];
            let path = rest
                .trim_start()
                .strip_prefix('"')
                .and_then(|call| call.split_once('"'))
                .map(|(path, _)| path.to_owned())
                .unwrap_or_else(|| {
                    panic!("a path that is no literal: {}", &rest[..40.min(rest.len())])
                });
            if !routes.contains(&path) {
                routes.push(path);
            }
        }
        routes
    }

    /// Each line of a fenced block of `text`, trimmed, that opens with
    /// `prefix`: a fence inside a list item is indented, its rule too.
    fn fenced_lines<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
        let mut inside = false;
        let mut found = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with("```") {
                inside = !inside;
            } else if inside && line.starts_with(prefix) {
                found.push(line);
            }
        }
        found
    }

    /// `route` with its slug, its ID, its REF and its file filled in.
    fn filled(route: &str, slug: &str, id: &str) -> String {
        route
            .split('/')
            .map(|segment| match segment {
                "{p}" => slug,
                "{id}" => id,
                "{*ref}" => "MEC-STAMINA",
                "{file}" => "x.js",
                other => other,
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The slugs the hub tests serve, and the IDs of a proposal, a card and a
    /// task; `x1` the sampler's.
    const SLUGS: [&str; 4] = ["lantern-keep", "zerkalo", "card-press", "x1"];
    const IDS: [&str; 5] = ["PR-0001", "PR-100001", "C-0001", "T-0001", "x1"];

    /// What makes `rule` other than item 6's: the router's `/mcp/{p}` not
    /// matched with a slug, or another route matched with one.
    fn mcp_rule_problems(rule: &regex::Regex, router: &str) -> Vec<String> {
        let routes = routes_in(router);
        let mut problems = Vec::new();
        if !routes.iter().any(|route| route == "/mcp/{p}") {
            problems.push("no /mcp/{p} route".to_owned());
        }
        for route in &routes {
            for slug in SLUGS {
                for id in IDS {
                    let path = filled(route, slug, id);
                    if rule.is_match(&path) != (route == "/mcp/{p}") {
                        problems.push(format!("{route} as {path}"));
                    }
                }
            }
        }
        problems
    }

    /// H12 of docs/features/hub-mcp.md: trust item 6's rule, read from the one
    /// fenced `^/` line of `docs/canon/hub-mcp.md`, in its "Trust", against
    /// the router: it matches `/mcp/{p}` with each served slug and no other
    /// route, every route of `app.rs`, `GET_ROUTES`' and `class`'s writes,
    /// with slugs and IDs filled in, nor a suffix, a query or a prefix; `class`
    /// calls POST and DELETE on what it matches `Mcp`; item 5's rule
    /// (`hub.md`) matches no `/mcp` path. M: MCP served at
    /// `/api/projects/{p}/mcp`.
    #[test]
    fn the_mcp_credential_rule_is_the_mcp_route() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // Spelled apart: `tests/no_domain.rs` reads this word in any literal.
        let pages = manifest.join("../../docs").join(concat!("ca", "non"));
        let page = pages.join("hub-mcp.md");
        let text = std::fs::read_to_string(&page)
            .unwrap_or_else(|error| panic!("{}: {error}", page.display()));
        let rules = fenced_lines(&text, "^/mcp/");
        assert_eq!(rules.len(), 1, "item 6's one fenced pattern: {rules:?}");
        assert_eq!(
            fenced_lines(&text, "^/"),
            rules,
            "the page's only fenced line starting `^/`"
        );
        let trust = text
            .split_once("## Trust")
            .map(|(_, rest)| rest.split("\n## ").next().unwrap_or(rest))
            .unwrap_or_else(|| panic!("{}: no \"Trust\"", page.display()));
        assert_eq!(
            fenced_lines(trust, "^/mcp/"),
            rules,
            "the pattern lives in \"Trust\""
        );
        let rule = regex::Regex::new(rules[0]).expect("a pattern");

        let router = std::fs::read_to_string(manifest.join("src/app.rs")).expect("the router");
        assert_eq!(
            mcp_rule_problems(&rule, &router),
            Vec::<String>::new(),
            "item 6's pattern against the router"
        );
        for slug in SLUGS {
            let path = format!("/mcp/{slug}");
            assert!(rule.is_match(&path), "{path}");
            assert_eq!(class(&Method::POST, &path), Some(Class::Mcp), "{path}");
            assert_eq!(class(&Method::DELETE, &path), Some(Class::Mcp), "{path}");
            // Paths alone: a router's path rule never sees the query, and the
            // fence refuses one on every method of `/mcp/<slug>`.
            for form in [
                format!("{path}/"),
                format!("{path}/x"),
                format!("/x{path}"),
                format!("/api/projects/{slug}/mcp"),
                "/mcp".to_owned(),
                "/mcp/".to_owned(),
            ] {
                assert!(!rule.is_match(&form), "{form}");
            }
        }
        // The writes `any(..)` serves are read: `/mcp/{p}`'s four, of which
        // `class` admits POST and DELETE; the pattern and `class` agree on
        // every write the router serves.
        let writes = writes_in(&router);
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(
                writes.contains(&(method.clone(), "/mcp/{p}".to_owned())),
                "{method} /mcp/{{p}} among the writes: {writes:?}"
            );
        }
        for (method, route) in &writes {
            let path = sampled(route);
            let admitted = class(method, &path);
            if admitted == Some(Class::Mcp) {
                assert!(rule.is_match(&path), "{method} {route}: `Mcp`, not matched");
            }
            if rule.is_match(&path) {
                let want =
                    (*method == Method::POST || *method == Method::DELETE).then_some(Class::Mcp);
                assert_eq!(admitted, want, "{method} {route}: the pattern and `class`");
            }
        }

        let hub = std::fs::read_to_string(pages.join("hub.md")).expect("hub.md");
        let step_up = regex::Regex::new(fenced_patterns(&hub)[0]).expect("item 5's pattern");
        for slug in SLUGS {
            for path in [format!("/mcp/{slug}"), format!("/mcp/{slug}/x")] {
                assert!(!step_up.is_match(&path), "item 5's pattern matches {path}");
            }
        }
    }

    /// H12's checker on a copy in memory of the live router, with the rule as
    /// the spec writes it (docs/features/hub-mcp.md "Rules and edge cases"), so
    /// the named mutation repeats on every run and writes nothing under `src`:
    /// the router as it is passes, and MCP moved to `/api/projects/{p}/mcp`
    /// does not.
    #[test]
    fn the_mcp_rule_checker_sees_a_moved_route() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let router = std::fs::read_to_string(manifest.join("src/app.rs")).expect("the router");
        let rule = regex::Regex::new("^/mcp/[^/]+$").expect("the spec's pattern");
        assert_eq!(mcp_rule_problems(&rule, &router), Vec::<String>::new());
        let at = "\"/mcp/{p}\"";
        assert_eq!(router.matches(at).count(), 1, "the route's one literal");
        let moved = router.replacen(at, "\"/api/projects/{p}/mcp\"", 1);
        let problems = mcp_rule_problems(&rule, &moved);
        assert!(
            problems
                .iter()
                .any(|problem| problem == "no /mcp/{p} route"),
            "{problems:?}"
        );
        let everything = regex::Regex::new("^/").expect("every path");
        assert!(!mcp_rule_problems(&everything, &router).is_empty());

        // The route reader's control: `any(..)` serves the four writes.
        let probe = "let r = R::new()\n    @\"/m\", any(m))\n    @\"/n\", read(n))\n"
            .replace('@', ROUTER_CALL);
        assert_eq!(
            writes_in(&probe),
            [
                (Method::POST, "/m".to_owned()),
                (Method::PUT, "/m".to_owned()),
                (Method::PATCH, "/m".to_owned()),
                (Method::DELETE, "/m".to_owned()),
            ]
        );
        assert_eq!(routes_in(&probe), ["/m", "/n"]);

        // The reader's controls: a fence indented in a list item, a rule
        // outside any fence, another fenced rule.
        let page = "1. Item six:\n\n   ```text\n   ^/mcp/[^/]+$\n   ```\n^/mcp/outside\n\
                    ```text\n^/api/projects/a$\n```\n";
        assert_eq!(fenced_lines(page, "^/mcp/"), ["^/mcp/[^/]+$"]);
        assert_eq!(
            fenced_lines(page, "^/"),
            ["^/mcp/[^/]+$", "^/api/projects/a$"]
        );
    }

    // ------------------------------------------------- stream-life SL1

    /// SL1's life: short, yet long against a loaded debug build's scheduling.
    const SL1_LIFE: Duration = Duration::from_millis(800);

    /// Three items at once, then nothing.
    fn three_then_nothing() -> impl Stream<Item = u8> {
        use futures_util::StreamExt as _;
        futures_util::stream::iter([1, 2, 3]).chain(futures_util::stream::pending())
    }

    /// SL1 of docs/features/stream-life.md, a life: the three come at once,
    /// whole and in order, then the stream ends at no less than its life and
    /// before a second past it, nothing yielded in between. M: the stream
    /// passed through; ended at once.
    #[tokio::test]
    async fn sl1_a_lived_stream_ends_after_its_life_between_items() {
        use futures_util::StreamExt as _;
        let started = tokio::time::Instant::now();
        let mut items = std::pin::pin!(lived(three_then_nothing(), Some(SL1_LIFE)));
        let by = started + SL1_LIFE + Duration::from_secs(1);
        let mut got = Vec::new();
        let ended = loop {
            match tokio::time::timeout_at(by, items.next()).await {
                Ok(Some(item)) => got.push((item, started.elapsed())),
                Ok(None) => break started.elapsed(),
                Err(_) => panic!("still open a second past its life, after {got:?}"),
            }
        };
        let order: Vec<u8> = got.iter().map(|(item, _)| *item).collect();
        assert_eq!(order, [1, 2, 3], "the three, whole and in order");
        for (item, at) in &got {
            assert!(*at < SL1_LIFE, "{item} came at {at:?}, not at once");
        }
        assert!(ended >= SL1_LIFE, "ended at {ended:?}, before its life");
        assert!(
            ended < SL1_LIFE + Duration::from_secs(1),
            "ended at {ended:?}"
        );
    }

    /// SL1, no life: the three come, and the stream is still open at three
    /// times the life above. M: `None` read as a life of 3L or less.
    #[tokio::test]
    async fn sl1_a_stream_with_no_life_is_still_open_at_three_lives() {
        use futures_util::StreamExt as _;
        let mut items = std::pin::pin!(lived(three_then_nothing(), None));
        for want in [1, 2, 3] {
            let got = tokio::time::timeout(SL1_LIFE, items.next()).await;
            assert_eq!(got.ok(), Some(Some(want)), "item {want} at once");
        }
        let after = tokio::time::timeout(SL1_LIFE * 3, items.next()).await;
        assert!(after.is_err(), "no life, yet at 3L: {after:?}");
    }

    /// SL1, no life, past 3L: `None` is the stream as given
    /// (docs/features/stream-life.md "Data"), with no timer, so it is polled
    /// outside any runtime. M: `None` read as any life, `STREAM_LIFE`
    /// included, which a real clock cannot wait for.
    #[test]
    fn sl1_no_life_is_the_stream_as_given_with_no_timer() {
        use std::task::{Context, Poll, Waker};
        let mut items = std::pin::pin!(lived(three_then_nothing(), None));
        let mut cx = Context::from_waker(Waker::noop());
        for want in [1, 2, 3] {
            assert_eq!(items.as_mut().poll_next(&mut cx), Poll::Ready(Some(want)));
        }
        for _ in 0..2 {
            assert_eq!(
                items.as_mut().poll_next(&mut cx),
                Poll::Pending,
                "still open"
            );
        }
    }
}
