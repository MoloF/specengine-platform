//! Hub mode (`docs/canon/hub.md`, ADR-0058, ADR-0059, task spec
//! `hub-identity`): a second fence, not a second daemon. `--public-origin`
//! is the whole switch; without it nothing here runs and `Remote-User` is
//! never read.
//!
//! [`Hub::parse`] judges the flag before anything is created. [`Hub::fenced`]
//! judges every request before routing, failing closed: the public `Host`
//! byte for byte, `Origin`, `Sec-Fetch-Site` (but for the landing), one
//! well-formed `Remote-User`, then the method and path. Any method but GET
//! is refused unless [`listed`] names it, so a route added later stays local
//! until it is listed here. [`acting`] binds a body's `as` to the login.
//!
//! The daemon cannot tell its proxy's header from anything else reaching its
//! socket: that is the deployment's guarantee (`docs/canon/hub.md` "Trust").

use std::net::Ipv4Addr;

use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, Method, StatusCode};
use specengine_cli::hub_login_problem;

use crate::answer::Refusal;
use crate::app::single;

/// The proxy's one login header: a constant, so no flag can name a header
/// the proxy does not scrub.
pub(crate) const REMOTE_USER: HeaderName = HeaderName::from_static("remote-user");

const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const SEC_FETCH_MODE: HeaderName = HeaderName::from_static("sec-fetch-mode");
const SEC_FETCH_DEST: HeaderName = HeaderName::from_static("sec-fetch-dest");

/// The last segment of a card's plain writes. Accept and clear stay off this
/// list until `hub-confirm`: each takes a consent no login gives yet.
const WRITES: [&str; 4] = ["move", "edit", "assign", "cancel"];

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

    /// The request's login, else why it is refused. In order: `Host`,
    /// `Origin`, `Sec-Fetch-Site`, `Remote-User`, the method and path.
    pub(crate) fn fenced(
        &self,
        method: &Method,
        path: &str,
        headers: &HeaderMap,
    ) -> Result<Login, String> {
        let host = single(headers, &HOST, "Host")?;
        if host != Some(self.authority.as_bytes()) {
            return Err(format!(
                "refused: the Host header must be {}: this hub serves its public origin only",
                self.authority
            ));
        }
        if let Some(origin) = single(headers, &ORIGIN, "Origin")?
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
        if !(method == Method::GET || (method == Method::POST && listed(path))) {
            return Err(
                "refused: this hub serves GET and five card writes: this route stays local"
                    .to_owned(),
            );
        }
        Ok(login)
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

/// The one exception to `Sec-Fetch-Site`: a top-level navigation to `/`, as
/// the login door and a chat link arrive. Static bytes, and `/` still takes
/// the rest of the fence; an asset, a frame, any API route or method never
/// qualifies.
fn landing(method: &Method, path: &str, headers: &HeaderMap) -> Result<bool, String> {
    Ok(method == Method::GET
        && path == "/"
        && single(headers, &SEC_FETCH_MODE, "Sec-Fetch-Mode")? == Some(b"navigate".as_slice())
        && single(headers, &SEC_FETCH_DEST, "Sec-Fetch-Dest")? == Some(b"document".as_slice()))
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

/// `path`, raw, is one of a hub's five writes: POST on `.../cards` or
/// `.../cards/:id/{move,edit,assign,cancel}`. Exact segments, so an encoded
/// or trailing form of any other route is not listed.
fn listed(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    match segments.as_slice() {
        ["", "api", "projects", project, "cards"] => !project.is_empty(),
        ["", "api", "projects", project, "cards", id, verb] => {
            !project.is_empty() && !id.is_empty() && WRITES.contains(verb)
        }
        _ => false,
    }
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
            return Err(Refusal::new(
                StatusCode::FORBIDDEN,
                format!("refused: `as` names {named}; this request's login is {login}"),
            ));
        }
    }
    Ok(())
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
        hub().fenced(&Method::GET, path, headers)
    }

    fn post(path: &str, headers: &HeaderMap) -> Result<Login, String> {
        hub().fenced(&Method::POST, path, headers)
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
                hub().fenced(&Method::HEAD, "/", &headers).is_err(),
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

    #[test]
    fn a_write_is_refused_until_listed() {
        for path in [
            "/api/projects/p/cards",
            "/api/projects/p/cards/C-0001/move",
            "/api/projects/p/cards/C-0001/edit",
            "/api/projects/p/cards/C-0001/assign",
            "/api/projects/p/cards/C-0001/cancel",
        ] {
            assert!(post(path, &proxied(&[])).is_ok(), "{path}");
            for method in [Method::PUT, Method::DELETE, Method::PATCH, Method::HEAD] {
                assert!(
                    hub().fenced(&method, path, &proxied(&[])).is_err(),
                    "{method} {path}"
                );
            }
        }
        let line = "refused: this hub serves GET and five card writes: this route stays local";
        for path in [
            "/",
            "/api/identity",
            "/api/projects",
            "/api/projects/p/tree",
            "/api/projects/p/proposals/PR-0001/decision",
            "/api/projects/p/round/answers",
            "/api/projects/p/trace/clear",
            "/api/projects/p/cards/C-0001",
            "/api/projects/p/cards/C-0001/accept",
            "/api/projects/p/cards/C-0001/clear",
            "/api/projects/p/cards/C-0001/%6Dove",
            "/api/projects/p/cards/C-0001/Move",
            "/api/projects/p/cards/C-0001/move/",
            "/api/projects/p/cards/",
            "/api/projects/p/cards/C-0001/move/x",
            "/api/projects//cards",
            "/api/projects//cards/C-0001/move",
            "/api/projects/p/cards//move",
            "//api/projects/p/cards",
            "/API/projects/p/cards",
            "api/projects/p/cards",
            "/api/projects/p/nothing",
            "*",
        ] {
            assert_eq!(
                post(path, &proxied(&[])).err().as_deref(),
                Some(line),
                "{path}"
            );
        }
        let unknown = Method::from_bytes(b"post").expect("an extension method");
        for method in [Method::OPTIONS, Method::CONNECT, Method::TRACE, unknown] {
            assert_eq!(
                hub()
                    .fenced(&method, "/api/projects/p/cards", &proxied(&[]))
                    .err()
                    .as_deref(),
                Some(line),
                "{method}"
            );
        }
        assert_eq!(
            hub()
                .fenced(
                    &Method::DELETE,
                    "/api/projects/p/proposals/PR-0001/decision",
                    &proxied(&[])
                )
                .err()
                .as_deref(),
            Some(line)
        );
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
}
