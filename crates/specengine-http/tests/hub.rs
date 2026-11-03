//! docs/features/hub-identity.md (RM-0049), the daemon's half: a hub through
//! the real daemon, over git repositories of `fixtures/spec-a` (a game's
//! brief) and `fixtures/card-press` (a print shop), so the mode is tried on two
//! projects of different nature (ADR-0008). The behaviour is
//! docs/canon/hub.md; every expected value below is worked out from it and
//! from docs/canon/cards.md, never read back from a run.
//!
//! - AC-01: the flag starts a hub on `127.0.0.1` alone, `public ...` before
//!   `listening`, five keys in `daemon.json`; a bad origin, `--host`, `--bind`
//!   exit 2 and create nothing; `spec serve` has no such flag. M: taking
//!   `http://`; binding `0.0.0.0` on a hub; parsing after `start::served`.
//! - AC-02: every route of `fence.rs`' `endpoints()`, each with a good
//!   `Remote-User`, refuses a foreign origin, nothing read. M: `Origin`
//!   compared by prefix; admitting loopback too.
//! - AC-03: the landing, `GET /` alone. M: dropping the exception; widening it
//!   to every GET.
//! - AC-04: no identity, every route, nothing read, no card. M: a default
//!   identity; judging writes only.
//! - AC-05: two logins on one card. M: the first login cached in `App`; the
//!   body's `as` winning.
//! - AC-06: `as` on each of the five writes. M: dropping the compare;
//!   overwriting silently.
//! - AC-07: a flow's `by` stops a login. M: skipping the actor in `move_card`'s
//!   handler.
//! - AC-08, its form in docs/features/hub-team.md: writes stay default deny, the
//!   team routes open to `[team]` alone, so a project with none refuses them.
//!   M: defaulting to allow; `accept` listed as a plain write.
//! - AC-09: `hub_trusts_the_header_on_its_socket`, then what the daemon can
//!   judge of a login. M: taking the first of two; a shared secret.
//! - AC-10, its new half: a local daemon never reads `Remote-User`. M: reading
//!   the header with no hub. The other half is commands, not a test: `git diff`
//!   on the listed files and on `app.rs`' `fn fenced` and `mod tests`.
//! - AC-11, its first half: a hub's five writes touch only the queue. Its
//!   second half is `door.rs` and `no_domain.rs`, unedited, by their named
//!   mutations.
//!
//! Beyond the criteria, from the canon: the fence's order, a header sent twice,
//! `X-Forwarded-*` and the other `Remote-*` headers unread, a listed write
//! keeping the board's door, `GET /api/identity` as a plain read, every login
//! reading every served project.
//!
//! From the security review: the router's GET routes pinned
//! ([`GET_ROUTES`]), and the raw-socket forms of a local route as a table. M: a
//! route added under GET that the pin does not list, kept as a test on a copy
//! in memory of the live router.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::ffi::OsStr;
use std::fs;
use std::process::Command;
use std::time::Duration;

use common::hub::{
    AUTHORITY, ORIGIN, P, Proxied, Setup, assert_plain, assert_refused, events_of, five_writes,
    hub_args, through,
};
use common::{
    Reply, Scratch, Server, Stream, ask, beyond_daemon, beyond_hub, data_dir, files_ending,
    read_text, replace, repository_root, request_bytes, snapshot, spec, spec_json,
};
use serde_json::{Value, json};

/// The refusal lines of docs/canon/hub.md "Fence".
const HOST_LINE: &str =
    "refused: the Host header must be hub.example.test: this hub serves its public origin only";
const ORIGIN_LINE: &str = "refused: the Origin header, when sent, must be https://hub.example.test";
const SITE_LINE: &str = "refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`";
const NO_LOGIN_LINE: &str = "refused: this hub needs one Remote-User header from its proxy";
const TWO_LOGINS_LINE: &str = "refused: the Remote-User header is sent more than once";
const OFF_LIST_LINE: &str = "refused: this hub does not serve this route";

/// The fence's lines of a confirm route (docs/features/hub-team.md "Data").
const NO_QUERY_LINE: &str = "refused: this route takes no query";

/// A project with no `[team]` (docs/features/hub-team.md "Data").
const NO_TEAM_LINE: &str = "refused: this project has no `[team]`";

/// `same_origin()`'s line: a listed write keeps the board's door.
const PAGE: &str = "a write needs a same-origin page (not authentication: ADR-0034)";

/// A login in another script, spoken with a space.
const FULL_NAME: &str =
    "\u{0410}\u{043d}\u{043d}\u{0430} \u{041f}\u{0435}\u{0442}\u{0440}\u{043e}\u{0432}\u{0430}";

/// Where a `Remote-User` value goes that a `&str` cannot hold ([`spliced`]).
const MARKER: &str = "@LOGIN@";

/// `bytes` with [`MARKER`] replaced by `value`.
fn spliced(bytes: &[u8], value: &[u8]) -> Vec<u8> {
    let at = bytes
        .windows(MARKER.len())
        .position(|window| window == MARKER.as_bytes())
        .expect("the marker");
    [&bytes[..at], value, &bytes[at + MARKER.len()..]].concat()
}

/// `endpoints()` of `tests/fence.rs`, read when the test runs and not copied:
/// a route that file gains is a route the hub's fence is tried on.
fn endpoints() -> Vec<(String, String)> {
    let source = read_text(&repository_root(), "crates/specengine-http/tests/fence.rs");
    let body = source
        .split_once("fn endpoints(port: u16)")
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .expect("fence.rs holds `endpoints`")
        .0;
    let literals: Vec<&str> = body.split('"').skip(1).step_by(2).collect();
    let routes: Vec<(String, String)> = literals
        .chunks(2)
        .map(|pair| (pair[0].to_owned(), pair[1].to_owned()))
        .collect();
    assert!(routes.len() >= 24, "fence.rs' routes: {routes:?}");
    for (method, path) in &routes {
        assert!(
            matches!(method.as_str(), "GET" | "POST" | "OPTIONS") && path.starts_with('/'),
            "not a route of fence.rs: {method} {path}"
        );
    }
    routes
}

/// The daemon's listening sockets as `lsof` names them (`127.0.0.1:N`, `*:N`,
/// `[::1]:N`), as `fence.rs`.
fn listening(server: &Server) -> Vec<String> {
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-nP",
            "-a",
            "-p",
            &server.pid().to_string(),
            "-iTCP",
            "-sTCP:LISTEN",
            "-Fn",
        ])
        .output()
        .expect("run lsof");
    String::from_utf8(output.stdout)
        .expect("lsof output")
        .lines()
        .filter_map(|line| line.strip_prefix('n').map(str::to_owned))
        .collect()
}

/// What `GET /` answers on this tree's daemon: the page, or the 503 of a
/// daemon compiled without `ui/dist` (`fence.rs`, `tests/ui.rs`' branch).
fn page_status() -> u16 {
    if repository_root().join("ui/dist/index.html").is_file() {
        200
    } else {
        503
    }
}

/// The page, or the not-built line: past the fence either way.
fn assert_page(reply: &Reply, context: &str) {
    assert_eq!(
        reply.status,
        page_status(),
        "{context}: {}",
        String::from_utf8_lossy(&reply.body)
    );
    assert_plain(reply, context);
    if reply.status == 503 {
        let message = reply.error_message();
        assert!(
            message.starts_with("this specengine-http holds no web UI"),
            "{context}: {message}"
        );
    } else {
        let policy = reply.header("content-security-policy").unwrap_or_default();
        assert!(
            policy.contains("frame-ancestors 'none'"),
            "{context}: the landing keeps the page's policy: {policy}"
        );
    }
}

/// A 403 of the fence that names `header`, whichever line of "Fence" it took.
fn assert_refused_naming(reply: &Reply, header: &str, context: &str) {
    assert_eq!(
        reply.status,
        403,
        "{context}: {}",
        String::from_utf8_lossy(&reply.body)
    );
    let message = reply.error_message();
    assert!(
        message.starts_with("refused: ") && message.contains(header),
        "{context}: {message}"
    );
    assert_plain(reply, context);
    assert_eq!(
        reply.header("allow"),
        None,
        "{context}: {:?}",
        reply.headers
    );
}

// --------------------------------------------------------------- AC-01

/// AC-01: the flag is the whole switch. The start's lines, the one socket on
/// `127.0.0.1` and `daemon.json`'s five keys with its loopback `url`.
#[test]
fn ac01_a_hub_starts_on_loopback_and_prints_its_origin_before_listening() {
    let hub = Setup::new("hub-start", "card-press").start();
    let port = hub.server.port;
    assert_eq!(
        hub.server.lines,
        [
            format!("serving card-press {}", hub.setup.root.display()),
            format!("public {ORIGIN}"),
            format!("listening http://127.0.0.1:{port}"),
        ]
    );
    assert_eq!(
        listening(&hub.server),
        [format!("127.0.0.1:{port}")],
        "the one listening socket is 127.0.0.1:<port>"
    );
    let text =
        fs::read_to_string(data_dir(&hub.setup.home).join("daemon.json")).expect("daemon.json");
    let file: Value = serde_json::from_str(&text).expect("daemon.json is JSON");
    let mut keys: Vec<&str> = file
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["pid", "port", "projects", "started", "url"],
        "{text}"
    );
    assert_eq!(
        file["url"],
        json!(format!("http://127.0.0.1:{port}")),
        "{text}"
    );
    assert!(
        !text.contains(AUTHORITY),
        "no origin in daemon.json: {text}"
    );
}

/// AC-01 and "Flag": what is no `https://host[:port]` exits 2 through
/// `cannot_start`, before `start::served`, so a fresh `HOME` holds nothing: the
/// data directory `served` prepares is the mutation's mark.
#[test]
fn ac01_a_bad_origin_exits_2_and_creates_nothing() {
    let scratch = Scratch::new("hub-bad-origin");
    let root = scratch.repo("card-press", "served", "main");
    let cwd = scratch.dir("cwd");
    let tree = snapshot(&root);
    let bad = [
        "http://hub.example.test",
        "hub.example.test",
        "ftp://hub.example.test",
        "https://hub.example.test/",
        "https://hub.example.test/path",
        "https://hub.example.test?q",
        "https://hub.example.test#f",
        "https://a@hub.example.test",
        "https://a:b@hub.example.test",
        "https://",
        "https://:8443",
        "https://hub_example.test",
        "https://hub..test",
        "https://hub.example.test:port",
        "",
        // The scheme is exactly `https://`.
        "HTTPS://hub.example.test",
        "Https://hub.example.test",
        "https:/hub.example.test",
        // A port is canonical: 1 to 65535, no leading zero, no sign.
        "https://hub.example.test:0",
        "https://hub.example.test:65536",
        "https://hub.example.test:08443",
        "https://hub.example.test:+8443",
        "https://hub.example.test:-1",
        "https://hub.example.test:",
        // A public origin is not loopback: the fence would admit a local page.
        "https://localhost",
        "https://LOCALHOST",
        "https://localhost:8443",
        "https://evil.localhost",
        "https://a.b.localhost",
        "https://127.0.0.1",
        "https://127.1.2.3",
        "https://127.0.0.1:8443",
        "https://0.0.0.0",
        // The same addresses as a resolver reads them, short, decimal, hex.
        "https://127.1",
        "https://2130706433",
        "https://0",
        "https://0x7f.0.0.1",
    ];
    for (index, given) in bad.iter().enumerate() {
        let home = scratch.home(&format!("h{index}"));
        let refused = Server::try_start(&home, &cwd, &hub_args(&[&root], given))
            .err()
            .unwrap_or_else(|| panic!("{given:?} started a hub"));
        assert_eq!(refused.code, Some(2), "{given:?}: {refused:?}");
        assert!(refused.stdout.is_empty(), "{given:?}: {refused:?}");
        assert!(
            refused
                .stderr
                .starts_with(&format!("specengine-http: --public-origin {given}: ")),
            "{given:?}: {refused:?}"
        );
        assert!(
            snapshot(&home).is_empty(),
            "{given:?}: a refusal created {:?}",
            snapshot(&home)
        );
        assert!(
            snapshot(&cwd).is_empty(),
            "{given:?}: the working directory"
        );
        assert!(snapshot(&root) == tree, "{given:?}: the served root");
    }
}

/// AC-01: the bind does not change, and `spec serve` passes no such flag: a
/// launched daemon is never a hub.
#[test]
fn ac01_no_address_flag_and_no_spec_serve_flag() {
    let scratch = Scratch::new("hub-no-flag");
    let root = scratch.repo("card-press", "served", "main");
    let cwd = scratch.dir("cwd");
    for (index, flag) in ["--host", "--bind"].iter().enumerate() {
        let home = scratch.home(&format!("h{index}"));
        let mut args = hub_args(&[&root], ORIGIN);
        args.push(OsStr::new(flag));
        args.push(OsStr::new("0.0.0.0"));
        let refused = Server::try_start(&home, &cwd, &args)
            .err()
            .unwrap_or_else(|| panic!("{flag} 0.0.0.0 started a hub"));
        assert_eq!(refused.code, Some(2), "{flag}: {refused:?}");
        assert!(refused.stdout.is_empty(), "{flag}: {refused:?}");
        assert!(
            refused.stderr.starts_with("specengine-http: ") && refused.stderr.contains(flag),
            "{flag}: {refused:?}"
        );
        assert!(snapshot(&home).is_empty(), "{flag}: {:?}", snapshot(&home));
    }

    let home = scratch.home("serve");
    let root = root.to_str().expect("UTF-8 root");
    let run = spec(
        &home,
        &cwd,
        &["serve", root, "--public-origin", ORIGIN, "--port", "0"],
    );
    run.code(2);
    assert!(run.stderr.contains("--public-origin"), "{}", run.show());
    assert!(snapshot(&home).is_empty(), "{:?}", snapshot(&home));
}

/// "Flag": `:443` is dropped and the host lower-cased in both the origin and
/// the authority, `:8443` stays in both, and the fence compares what was
/// stored, byte for byte.
#[test]
fn ac01_the_origin_is_stored_lower_case_and_443_is_dropped() {
    for (given, origin, authority, other_host, other_origin) in [
        (
            "https://Hub.Example.test:443",
            "https://hub.example.test",
            "hub.example.test",
            "hub.example.test:443",
            "https://hub.example.test:443",
        ),
        (
            "https://hub.example.test:8443",
            "https://hub.example.test:8443",
            "hub.example.test:8443",
            "hub.example.test",
            "https://hub.example.test",
        ),
    ] {
        let setup = Setup::new("hub-origin-form", "card-press");
        let args = hub_args(&[&setup.root], given);
        let server = Server::start(&setup.home, &setup.cwd, &args);
        assert!(
            server.lines.contains(&format!("public {origin}")),
            "{given}: {:?}",
            server.lines
        );
        let person = Proxied::fetch("mira")
            .set("Host", authority)
            .plus("Origin", origin);
        through(&server, "GET", "/api/identity", &person, b"").status(200);
        assert_refused(
            &through(
                &server,
                "GET",
                "/api/identity",
                &person.clone().set("Host", other_host),
                b"",
            ),
            &format!(
                "refused: the Host header must be {authority}: this hub serves its public origin \
                 only"
            ),
            given,
        );
        assert_refused(
            &through(
                &server,
                "GET",
                "/api/identity",
                &person.clone().set("Origin", other_origin),
                b"",
            ),
            &format!("refused: the Origin header, when sent, must be {origin}"),
            given,
        );
    }
}

/// The security review's four: an address a resolver would read in a host
/// that is no dotted quad, each refused with its one reason, so the loopback
/// rule cannot be walked round by another spelling of `127.0.0.1`.
#[test]
fn ac01_a_numeric_host_must_be_a_dotted_quad() {
    let scratch = Scratch::new("hub-numeric-host");
    let root = scratch.repo("card-press", "served", "main");
    let cwd = scratch.dir("cwd");
    for (index, given) in [
        "https://127.1",
        "https://2130706433",
        "https://0",
        "https://0x7f.0.0.1",
    ]
    .iter()
    .enumerate()
    {
        let home = scratch.home(&format!("h{index}"));
        let refused = Server::try_start(&home, &cwd, &hub_args(&[&root], given))
            .err()
            .unwrap_or_else(|| panic!("{given} started a hub"));
        assert_eq!(refused.code, Some(2), "{given}: {refused:?}");
        assert_eq!(
            refused.stderr,
            format!(
                "specengine-http: --public-origin {given}: a numeric host must be a dotted quad\n"
            ),
            "{given}"
        );
        assert!(snapshot(&home).is_empty(), "{given}: {:?}", snapshot(&home));
    }
}

/// The other side of the stricter parse: a port at either end of the range
/// stands, and `localhost` or an IPv4 shape inside a longer name is no loopback.
#[test]
fn ac01_the_ports_at_the_ends_and_names_that_only_look_local_start() {
    for (given, authority) in [
        ("https://hub.example.test:1", "hub.example.test:1"),
        ("https://hub.example.test:65535", "hub.example.test:65535"),
        ("https://localhost.example.test", "localhost.example.test"),
        ("https://notlocalhost", "notlocalhost"),
        ("https://127.0.0.1.example.test", "127.0.0.1.example.test"),
        // Read as text: that this name resolves to loopback cannot be seen.
        ("https://127.0.0.1.nip.io", "127.0.0.1.nip.io"),
        ("https://203.0.113.7", "203.0.113.7"),
    ] {
        let setup = Setup::new("hub-origin-ends", "card-press");
        let args = hub_args(&[&setup.root], given);
        let server = Server::start(&setup.home, &setup.cwd, &args);
        assert!(
            server.lines.contains(&format!("public {given}")),
            "{given}: {:?}",
            server.lines
        );
        let person = Proxied::fetch("mira").set("Host", authority);
        through(&server, "GET", "/api/identity", &person, b"").status(200);
    }
}

// --------------------------------------------------------------- AC-02

/// AC-02: every route of `fence.rs`' `endpoints()`, each request with a good
/// `Remote-User`, so the origin alone is judged: each line of "Fence" names its
/// own cause, and the same requests with nothing foreign pass. A fresh `HOME`
/// stays empty through the refusals.
#[test]
fn ac02_a_foreign_origin_is_refused_on_every_route_with_a_good_login() {
    let hub = Setup::new("hub-origin", "spec-a").start();
    let loopback = format!("127.0.0.1:{}", hub.server.port);
    let localhost = format!("localhost:{}", hub.server.port);
    let loopback_origin = format!("http://{loopback}");
    let foreign = [
        ("Host", "evil.example.test", HOST_LINE),
        ("Host", "hub.example.test.evil.test", HOST_LINE),
        ("Host", loopback.as_str(), HOST_LINE),
        ("Host", localhost.as_str(), HOST_LINE),
        ("Origin", "https://evil.example.test", ORIGIN_LINE),
        ("Origin", "https://hub.example.test.evil.test", ORIGIN_LINE),
        ("Origin", "http://hub.example.test", ORIGIN_LINE),
        ("Origin", "null", ORIGIN_LINE),
        ("Origin", loopback_origin.as_str(), ORIGIN_LINE),
        ("Sec-Fetch-Site", "same-site", SITE_LINE),
        ("Sec-Fetch-Site", "cross-site", SITE_LINE),
    ];
    let routes = endpoints();
    for (name, value, line) in foreign {
        let proxied = Proxied::fetch("mira").set(name, value);
        for (method, path) in &routes {
            let reply = through(&hub.server, method, path, &proxied, b"");
            assert_refused(&reply, line, &format!("{method} {path} {name}: {value}"));
        }
    }
    assert!(
        beyond_hub(&hub.setup.home).is_empty(),
        "a refused request read nothing: the fresh HOME holds {:?}",
        beyond_hub(&hub.setup.home)
    );

    // The control: nothing foreign, the same login, and the reads pass with
    // the answers of `fence.rs`.
    let person = Proxied::fetch("mira");
    for (method, path) in routes
        .iter()
        .filter(|(method, path)| method == "GET" && !path.ends_with("/events"))
    {
        let reply = through(&hub.server, method, path, &person, b"");
        let want = match path.as_str() {
            "/api/projects/lantern-keep/proposals/PR-0001"
            | "/api/projects/lantern-keep/cards/C-0001"
            | "/no/such/route"
            | "/assets/x.js" => 404,
            "/" => page_status(),
            _ => 200,
        };
        assert_eq!(
            reply.status,
            want,
            "{method} {path}: {}",
            String::from_utf8_lossy(&reply.body)
        );
        assert_plain(&reply, &format!("{method} {path}"));
    }
    let stream = Stream::open_raw(
        hub.server.port,
        "/api/projects/lantern-keep/events",
        &person.pairs(),
    );
    assert_eq!(stream.status, 200);
    assert!(
        stream
            .headers
            .iter()
            .all(|(name, _)| !name.starts_with("access-control-")),
        "events: {:?}",
        stream.headers
    );
}

/// "Fence": "in this order", each step's own line when several fail.
#[test]
fn the_fence_judges_host_origin_site_login_route_in_that_order() {
    let hub = Setup::new("hub-order", "spec-a").start();
    let path = "/api/projects/lantern-keep/cards/C-0001/archive";
    let person = Proxied::fetch("mira");
    let no_login = person.clone().without("Remote-User");
    let cases = [
        (
            no_login
                .clone()
                .set("Sec-Fetch-Site", "cross-site")
                .plus("Origin", "https://evil.example.test")
                .set("Host", "evil.example.test"),
            HOST_LINE,
        ),
        (
            no_login
                .clone()
                .set("Sec-Fetch-Site", "cross-site")
                .plus("Origin", "https://evil.example.test"),
            ORIGIN_LINE,
        ),
        (
            no_login.clone().set("Sec-Fetch-Site", "cross-site"),
            SITE_LINE,
        ),
        (no_login, NO_LOGIN_LINE),
        (person, OFF_LIST_LINE),
    ];
    for (proxied, line) in &cases {
        assert_refused(
            &through(&hub.server, "POST", path, proxied, b"{}"),
            line,
            line,
        );
    }
    let reply = through(&hub.server, "POST", path, &Proxied::fetch("local"), b"{}");
    assert_refused_naming(&reply, "Remote-User", "a bad login before the route");
}

/// "Fence": "Any of these headers sent twice is refused", an equal copy too.
#[test]
fn a_header_sent_twice_is_refused() {
    let hub = Setup::new("hub-twice", "spec-a").start();
    for (name, value) in [
        ("Host", AUTHORITY),
        ("Origin", ORIGIN),
        ("Sec-Fetch-Site", "same-origin"),
        ("Remote-User", "mira"),
    ] {
        let twice = Proxied::fetch("mira").set(name, value).plus(name, value);
        for path in ["/api/identity", "/api/projects", "/"] {
            let reply = through(&hub.server, "GET", path, &twice, b"");
            let context = format!("{name} twice on {path}");
            assert_refused_naming(&reply, name, &context);
            if name == "Remote-User" {
                assert_eq!(reply.error_message(), TWO_LOGINS_LINE, "{context}");
            }
        }
    }
    assert!(
        beyond_hub(&hub.setup.home).is_empty(),
        "{:?}",
        beyond_hub(&hub.setup.home)
    );
}

/// "Fence", "Identity": `X-Forwarded-*` is never read, in either direction,
/// and neither are `Remote-Name`, `Remote-Email` and `Remote-Groups`, so the
/// deployment guards one header.
#[test]
fn x_forwarded_and_the_other_remote_headers_are_never_read() {
    let hub = Setup::new("hub-unread", "card-press").start();
    let login = |proxied: &Proxied| {
        let reply = through(&hub.server, "GET", "/api/identity", proxied, b"");
        reply.status(200);
        assert_eq!(reply.text(), "{\"login\":\"mira\"}");
    };
    login(
        &Proxied::fetch("mira")
            .set("X-Forwarded-Host", "evil.example.test")
            .set("X-Forwarded-Proto", "http")
            .set("X-Forwarded-Port", "80"),
    );
    login(
        &Proxied::fetch("mira")
            .plus("Remote-Name", "Eve Stone")
            .plus("Remote-Name", "Ada Stone")
            .plus("Remote-Email", "eve@example.test")
            .plus("Remote-Groups", "admins, editors"),
    );
    let reply = through(
        &hub.server,
        "GET",
        "/api/identity",
        &Proxied::fetch("mira").set("Host", "evil.example.test"),
        b"",
    );
    assert_refused(&reply, HOST_LINE, "X-Forwarded-Host rescues no Host");
    let reply = through(
        &hub.server,
        "GET",
        "/api/identity",
        &Proxied::fetch("mira")
            .without("Remote-User")
            .plus("Remote-Name", "mira")
            .plus("Remote-Email", "mira@example.test"),
        b"",
    );
    assert_refused(&reply, NO_LOGIN_LINE, "Remote-Name is no login");
}

// --------------------------------------------------------------- AC-03

/// AC-03 (D3): `GET /` with `navigate`, `document` passes step 3 whatever
/// `Sec-Fetch-Site` says; the same headers anywhere else, or one of them
/// changed, are refused at step 3; `/` still takes the rest of the fence. The
/// proxy's `Sec-Fetch-*` are what makes this visible: curl sends none.
#[test]
fn ac03_a_navigation_from_the_login_door_lands_on_the_page_and_nothing_else_does() {
    let hub = Setup::new("hub-landing", "spec-a").start();
    for site in ["same-site", "cross-site"] {
        let landing = Proxied::navigation(site, "mira");
        assert_page(
            &through(&hub.server, "GET", "/", &landing, b""),
            &format!("the landing from {site}"),
        );
        let not_the_landing = [
            ("GET", "/api/projects", landing.clone()),
            ("GET", "/assets/x.js", landing.clone()),
            ("GET", "/", landing.clone().set("Sec-Fetch-Dest", "iframe")),
            ("GET", "/", landing.clone().set("Sec-Fetch-Dest", "frame")),
            ("GET", "/", landing.clone().set("Sec-Fetch-Mode", "cors")),
            ("GET", "/", landing.clone().without("Sec-Fetch-Mode")),
            ("GET", "/", landing.clone().without("Sec-Fetch-Dest")),
            ("POST", "/", landing.clone()),
        ];
        for (method, path, proxied) in &not_the_landing {
            let reply = through(&hub.server, method, path, proxied, b"");
            assert_refused(&reply, SITE_LINE, &format!("{method} {path} {proxied:?}"));
        }
        let rest_of_the_fence = [
            (landing.clone().without("Remote-User"), NO_LOGIN_LINE),
            (landing.clone().set("Host", "evil.example.test"), HOST_LINE),
            (
                landing.clone().plus("Origin", "https://evil.example.test"),
                ORIGIN_LINE,
            ),
        ];
        for (proxied, line) in &rest_of_the_fence {
            let reply = through(&hub.server, "GET", "/", proxied, b"");
            assert_refused(&reply, line, &format!("the landing, {proxied:?}"));
        }
    }
    // A typed URL and the page's own reload need no exception.
    for site in ["none", "same-origin"] {
        let reply = through(
            &hub.server,
            "GET",
            "/",
            &Proxied::navigation(site, "mira"),
            b"",
        );
        assert_page(&reply, site);
    }
}

// --------------------------------------------------------------- AC-04

/// AC-04: a good origin and no identity, on every route, reads nothing.
#[test]
fn ac04_no_login_is_refused_on_every_route_and_reads_nothing() {
    let hub = Setup::new("hub-nobody", "spec-a").start();
    let anonymous = Proxied::fetch("mira").without("Remote-User");
    let mut routes = endpoints();
    routes.push(("GET".to_owned(), "/api/identity".to_owned()));
    for named in [
        "/",
        "/assets/x.js",
        "/api/identity",
        "/api/projects/lantern-keep/events",
    ] {
        assert!(
            routes.iter().any(|(_, path)| path == named),
            "the criterion names {named}: {routes:?}"
        );
    }
    for (method, path) in &routes {
        let reply = through(&hub.server, method, path, &anonymous, b"");
        assert_refused(&reply, NO_LOGIN_LINE, &format!("{method} {path}"));
    }
    assert!(
        beyond_hub(&hub.setup.home).is_empty(),
        "a refused request read nothing: the fresh HOME holds {:?}",
        beyond_hub(&hub.setup.home)
    );
}

/// AC-04: a card POST with no `Remote-User`, a typed `as` no way around it,
/// adds no card and no event.
#[test]
fn ac04_a_card_post_with_no_login_adds_no_card_and_no_event() {
    let hub = Setup::new("hub-nobody-post", "card-press").start();
    let anonymous = Proxied::write("mira").without("Remote-User");
    for body in [
        json!({"title": "Side story", "discipline": "copy"}),
        json!({"title": "Side story", "discipline": "copy", "as": "mira"}),
    ] {
        let reply = through(
            &hub.server,
            "POST",
            &format!("{P}/cards"),
            &anonymous,
            body.to_string().as_bytes(),
        );
        assert_refused(&reply, NO_LOGIN_LINE, &body.to_string());
    }
    assert!(
        beyond_hub(&hub.setup.home).is_empty(),
        "{:?}",
        beyond_hub(&hub.setup.home)
    );
    assert!(hub.events().is_empty(), "{:?}", hub.events());
    let board = hub
        .get("mira", &format!("{P}/cards?with_cancelled=true"))
        .status(200)
        .json();
    assert_eq!(board["cards"], json!([]), "{board}");
}

// --------------------------------------------------------------- AC-05

/// The next event on `tail`: its type and payload.
fn next_on_tail(tail: &mut Stream) -> (String, Value) {
    let frame = tail
        .next_event(Duration::from_secs(20))
        .expect("an event on the tail");
    let kind = frame.event.clone().expect("an event type");
    let data = serde_json::from_str(frame.data.as_deref().expect("data")).expect("JSON");
    (kind, data)
}

/// AC-05, required, its step-8 form: two logins, each heard as itself, whoever
/// asked first; `mira` creates and `tomas` moves one card, and the record says
/// so on the card, in its history and on the tail. `tomas` typing `mira` is no
/// way of being her. RM-0050 takes the `Decided-by` form.
#[test]
fn ac05_two_logins_are_two_persons_on_one_card() {
    let hub = Setup::new("hub-two", "card-press").start();
    for login in ["mira", "tomas", "mira"] {
        let reply = hub.get(login, "/api/identity");
        assert_eq!(reply.status, 200, "{login}: {}", reply.text());
        assert_eq!(reply.text(), format!("{{\"login\":\"{login}\"}}"));
    }

    let mut tail = Stream::open_raw(
        hub.server.port,
        &format!("{P}/events"),
        &Proxied::fetch("mira").plus("Last-Event-ID", "0").pairs(),
    );
    assert_eq!(tail.status, 200);

    let made = hub.done(
        "mira",
        "cards",
        &json!({"title": "Back page", "discipline": "plates"}),
    );
    assert_eq!(made["id"], json!("C-0001"), "{made}");

    let before = hub.board();
    let forged = hub.post(
        "tomas",
        "cards/C-0001/move",
        &json!({"to": "checked", "as": "mira"}),
    );
    assert_refused(
        &forged,
        "refused: `as` names mira; this request's login is tomas",
        "tomas typing mira",
    );
    assert_eq!(hub.board(), before, "tomas typing mira moved nothing");

    let moved = hub.done("tomas", "cards/C-0001/move", &json!({"to": "checked"}));
    assert_eq!(moved["column"], json!("checked"), "{moved}");

    let card = hub.show("C-0001");
    assert_eq!(card["created_by"], json!("mira"), "{card}");
    let history: Vec<(&str, &str)> = card["history"]
        .as_array()
        .expect("the daemon's show is `--history`")
        .iter()
        .map(|event| {
            (
                event["type"].as_str().expect("a type"),
                event["payload"]["as"].as_str().expect("an `as`"),
            )
        })
        .collect();
    assert_eq!(
        history,
        [("card.created", "mira"), ("card.moved", "tomas")],
        "{card}"
    );

    let (kind, data) = next_on_tail(&mut tail);
    assert_eq!(kind, "card.created", "{data}");
    assert_eq!(data["as"], json!("mira"), "{data}");
    let (kind, data) = next_on_tail(&mut tail);
    assert_eq!(kind, "card.moved", "{data}");
    assert_eq!(data["as"], json!("tomas"), "{data}");
}

// --------------------------------------------------------------- AC-06

/// AC-06: on each of the five writes `as` null, absent or the login is a 200
/// with the login stored.
#[test]
fn ac06_a_null_or_equal_as_stores_the_login_on_each_of_the_five_writes() {
    let hub = Setup::new("hub-as", "card-press").start();
    let forms = [Some(Value::Null), Some(json!("mira")), None];
    for (index, form) in forms.iter().enumerate() {
        let id = format!("C-{:04}", index + 1);
        for (path, mut body, kind) in five_writes(&id) {
            if let Some(form) = form {
                body["as"] = form.clone();
            }
            let seen = hub.events().len();
            hub.done("mira", &path, &body);
            let events = hub.events();
            assert_eq!(events.len(), seen + 1, "{path} {body}: {events:?}");
            let (stored, payload) = events.last().expect("the write's event");
            assert_eq!(stored.as_str(), kind, "{path} {body}");
            assert_eq!(payload["as"], json!("mira"), "{path} {body}: {payload}");
        }
        let card = hub.show(&id);
        assert_eq!(card["created_by"], json!("mira"), "{form:?}: {card}");
        assert_eq!(
            card["cancellation"]["by"],
            json!("mira"),
            "{form:?}: {card}"
        );
    }
}

/// AC-06: another well-formed name, on each of the five writes, is a 403
/// naming both, and no row or event changes: compared byte for byte, so case
/// and a space count, and the record's own words are no way in. Edge case: off
/// the person grammar `from_json` answers its 400 first.
#[test]
fn ac06_another_name_is_a_403_on_each_of_the_five_writes_and_changes_nothing() {
    let hub = Setup::new("hub-as-other", "card-press").start();
    hub.done(
        "mira",
        "cards",
        &json!({"title": "Open card", "discipline": "plates"}),
    );
    let before = hub.board();
    for (path, body, _) in five_writes("C-0001") {
        let mut named = body.clone();
        named["as"] = json!("tomas");
        assert_refused(
            &hub.post("mira", &path, &named),
            "refused: `as` names tomas; this request's login is mira",
            &format!("{path} {named}"),
        );
        assert_eq!(hub.board(), before, "{path} {named}: nothing changed");

        for other in ["Mira", " mira", "mira ", "local", "agent"] {
            let mut named = body.clone();
            named["as"] = json!(other);
            let reply = hub.post("mira", &path, &named);
            let context = format!("{path} {named}");
            assert_eq!(reply.status, 403, "{context}: {}", reply.text());
            let message = reply.error_message();
            assert!(
                message.starts_with("refused: `as` names "),
                "{context}: {message}"
            );
            assert_plain(&reply, &context);
            assert_eq!(hub.board(), before, "{context}: nothing changed");
        }

        let mut named = body.clone();
        named["as"] = json!("bell\u{7}");
        let reply = hub.post("mira", &path, &named);
        assert_eq!(reply.status, 400, "{path} {named}: {}", reply.text());
        assert!(
            !reply.error_message().starts_with("refused: "),
            "{path} {named}: the grammar's own line: {}",
            reply.text()
        );
        assert_eq!(hub.board(), before, "{path} {named}: nothing changed");
    }
}

/// "Routes": a listed write keeps the door of board-ui.md: a same-origin page,
/// JSON, a body the CLI decodes, a served slug. The hub widens nothing.
#[test]
fn a_listed_write_keeps_the_boards_door() {
    let hub = Setup::new("hub-door-kept", "card-press").start();
    let path = format!("{P}/cards");
    let body = json!({"title": "Back page", "discipline": "plates"}).to_string();
    let page = Proxied::write("mira");
    for proxied in [
        page.clone().without("Sec-Fetch-Site"),
        page.clone().set("Sec-Fetch-Site", "none"),
    ] {
        let reply = through(&hub.server, "POST", &path, &proxied, body.as_bytes());
        assert_eq!(reply.status, 403, "{proxied:?}: {}", reply.text());
        assert_eq!(reply.error_message(), PAGE, "{proxied:?}");
    }
    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page.clone().set("Content-Type", "text/plain"),
        body.as_bytes(),
    );
    assert_eq!(reply.status, 415, "{}", reply.text());
    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page,
        b"{\"title\":\"Back page\",\"no_such_key\":1}",
    );
    assert_eq!(reply.status, 400, "{}", reply.text());
    let reply = through(
        &hub.server,
        "POST",
        "/api/projects/no-such-project/cards",
        &page,
        body.as_bytes(),
    );
    assert_eq!(reply.status, 404, "{}", reply.text());
    assert!(hub.events().is_empty(), "{:?}", hub.events());

    // `Origin` absent passes the fence, as it does for a read.
    let reply = through(
        &hub.server,
        "POST",
        &path,
        &page.without("Origin"),
        body.as_bytes(),
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
}

// --------------------------------------------------------------- AC-07

/// AC-07: `by` compares with the login, so a flow's `by` stops a person: `mira`
/// moves, `tomas` gets the CLI's 409 document naming him. A `by` listing no
/// login refuses everyone with a 409 (docs/canon/hub.md "Identity").
#[test]
fn ac07_a_flows_by_compares_with_the_login() {
    let setup = Setup::new("hub-by", "card-press");
    replace(
        &setup.root,
        "specengine.toml",
        "by = [\"desk\"]",
        "by = [\"mira\"]",
    );
    let git = setup.scratch.git();
    git.run(&setup.root, &["commit", "-qam", "the desk is mira"]);
    let hub = setup.start();
    for (login, title) in [("mira", "Lead story"), ("tomas", "Second story")] {
        hub.done(
            login,
            "cards",
            &json!({"title": title, "discipline": "copy"}),
        );
    }

    let before = hub.board();
    let reply = hub.post("tomas", "cards/C-0002/move", &json!({"to": "edit"}));
    assert_eq!(reply.status, 409, "{}", reply.text());
    let document = reply.json();
    assert_eq!(
        document["notes"].as_array().and_then(|notes| notes.last()),
        Some(&json!(
            "C-0002 is in draft: the move to `edit` is `mira`'s, not `tomas`'s; nothing changed"
        )),
        "{document}"
    );
    assert_eq!(hub.board(), before, "a refused move changed nothing");
    let setup = &hub.setup;
    let run = spec_json(
        &setup.home,
        &setup.cwd,
        &setup.root,
        &["card", "move", "C-0002", "--to", "edit", "--as", "tomas"],
    );
    run.code(1);
    assert_eq!(reply.text(), run.document(), "the CLI's document");

    let moved = hub.done("mira", "cards/C-0001/move", &json!({"to": "edit"}));
    assert_eq!(moved["column"], json!("edit"), "{moved}");
    let (_, last) = hub.events().pop().expect("the move's event");
    assert_eq!(last["as"], json!("mira"), "{last}");

    replace(
        &setup.root,
        "specengine.toml",
        "by = [\"mira\"]",
        "by = [\"lead\"]",
    );
    for login in ["mira", "tomas"] {
        let reply = hub.post(login, "cards/C-0002/move", &json!({"to": "edit"}));
        assert_eq!(reply.status, 409, "{login}: {}", reply.text());
        let document = reply.json();
        assert_eq!(
            document["notes"].as_array().and_then(|notes| notes.last()),
            Some(&json!(format!(
                "C-0002 is in draft: the move to `edit` is `lead`'s, not `{login}`'s; nothing \
                 changed"
            ))),
            "{document}"
        );
    }
}

// --------------------------------------------------------------- AC-08

/// A repository where consent has something to act on, with no daemon left
/// running: two open questions, the second staged by a local daemon so a DELETE
/// has something to clear, and C-0001 walked to `accept.from` with `tomas` its
/// reviewer, so a refused request would land if it reached its handler. The two
/// proposals' IDs.
fn consent_waits(name: &str) -> (Setup, String, String) {
    let setup = Setup::new(name, "card-press");
    let asked = ask(
        &setup.home,
        &setup.root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let staged = ask(
        &setup.home,
        &setup.root,
        &["SHT-INNER"],
        "Which typeface carries the inner column rules?",
    );
    {
        let local = Server::serve(&setup.home, &setup.cwd, &[&setup.root]);
        let at = local
            .get(&format!("{P}/proposals/{staged}"))
            .status(200)
            .json()["updated_at"]
            .as_str()
            .expect("updated_at")
            .to_owned();
        let stage = json!({"decision": "reject", "reason": "Asked before.", "updated_at": at});
        local
            .stage(
                &format!("{P}/proposals/{staged}/decision"),
                &stage.to_string(),
            )
            .status(200);
        for (path, body) in [
            (
                "cards",
                json!({"title": "Back page", "discipline": "plates", "assignee": "mira"}),
            ),
            ("cards/C-0001/assign", json!({"reviewer": "tomas"})),
            ("cards/C-0001/move", json!({"to": "checked"})),
        ] {
            local
                .stage(&format!("{P}/{path}"), &body.to_string())
                .status(200);
        }
    }
    (setup, asked, staged)
}

/// `[team]` for [`raw_forms_of_a_local_route_and_other_methods_stay_refused`]:
/// `tomas`, the card's reviewer in [`consent_waits`], in it, committed.
fn tomas_on_the_team(setup: &Setup) {
    let mut config = read_text(&setup.root, "specengine.toml");
    config.push_str(
        "\n[team.tomas]\nname = \"Tomas B\"\nrole = \"member\"\nemail = \"tomas@example.org\"\n",
    );
    fs::write(setup.root.join("specengine.toml"), config).expect("the config");
    let git = setup.scratch.git();
    git.run(&setup.root, &["commit", "-qam", "tomas joins"]);
}

/// AC-08, its hub-team form: a project opts in by a reviewed commit, so with no
/// `[team]` each act past the plain writes is a 403 naming the table, on every
/// route at once, whoever asks, each request one that would land
/// ([`consent_waits`]); the list of writes stays default deny; nothing stored
/// moves; the reads stay served. M: defaulting to allow; `accept` listed as a
/// plain write.
#[test]
fn ac08_with_no_team_the_acts_are_refused_and_writes_are_default_deny() {
    let (setup, asked, staged) = consent_waits("hub-local-only");
    let hub = setup.start();
    let ids = [asked.as_str(), staged.as_str()];
    let before = hub.stored(&ids);

    let at = hub.proposal(&asked)["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let revision = hub.show("C-0001")["revision"]
        .as_str()
        .expect("a revision")
        .to_owned();
    let stage = json!({"decision": "reject", "reason": "Asked before.", "updated_at": at});
    let answers = json!({"project": "card-press", "answers": [{"id": asked, "stage": stage}]});
    let no_team = [
        (
            "POST",
            format!("{P}/proposals/{asked}/decision"),
            stage.to_string(),
        ),
        (
            "POST",
            format!("{P}/proposals/{staged}/decision"),
            stage.to_string(),
        ),
        (
            "DELETE",
            format!("{P}/proposals/{staged}/decision"),
            String::new(),
        ),
        ("POST", format!("{P}/round/answers"), answers.to_string()),
        (
            "POST",
            format!("{P}/cards/C-0001/accept"),
            json!({"briefed": true, "note": "Read twice.", "revision": revision, "as": "tomas"})
                .to_string(),
        ),
        (
            "POST",
            format!("{P}/cards/C-0001/clear"),
            json!({"outcome": "no_effect", "reason": "A reprint.", "revision": revision,
                "as": "tomas"})
            .to_string(),
        ),
        (
            "POST",
            format!("{P}/trace/clear"),
            json!({"group": "PRN-WEEKLY", "reason": "A reprint.", "count": 1, "as": "tomas"})
                .to_string(),
        ),
    ];
    let page = Proxied::write("tomas");
    for (method, path, body) in &no_team {
        let reply = through(&hub.server, method, path, &page, body.as_bytes());
        assert_refused(&reply, NO_TEAM_LINE, &format!("{method} {path}"));
    }
    assert_eq!(hub.stored(&ids), before, "no act landed with no `[team]`");

    // Every path off the list, exact raw segments, and a read route's 405.
    let body = json!({"title": "Back page", "discipline": "plates"}).to_string();
    let unlisted = [
        "/no/such/route".to_owned(),
        "/".to_owned(),
        "/assets/x.js".to_owned(),
        "/api/identity".to_owned(),
        "/api/projects".to_owned(),
        format!("{P}/tree"),
        format!("{P}/events"),
        format!("{P}/cards/C-0001"),
        format!("{P}/cards/C-0001/archive"),
        format!("{P}/cards/C-0001/MOVE"),
        format!("{P}/cards/C-0001/%6Dove"),
        format!("{P}/cards/C-0001/move/"),
        format!("{P}/cards/C-0001/move/again"),
        format!("{P}/cards/C-0001/accept/../move"),
        format!("{P}/cards/"),
        "/api/projects/card-press/%63ards".to_owned(),
    ];
    for path in &unlisted {
        let reply = through(&hub.server, "POST", path, &page, body.as_bytes());
        assert_refused(&reply, OFF_LIST_LINE, &format!("POST {path}"));
    }

    // Any method off the list, on a listed route too. DELETE stands on the
    // stage route alone.
    for method in ["PUT", "PATCH", "DELETE", "OPTIONS", "TRACE"] {
        for path in [
            format!("{P}/cards"),
            format!("{P}/cards/C-0001/move"),
            format!("{P}/cards/C-0001/accept"),
            format!("{P}/cards/C-0001/clear"),
            format!("{P}/trace/clear"),
            format!("{P}/round/answers"),
            "/api/projects".to_owned(),
        ] {
            let reply = through(&hub.server, method, &path, &page, b"");
            assert_refused(&reply, OFF_LIST_LINE, &format!("{method} {path}"));
        }
    }
    for method in ["PUT", "PATCH", "OPTIONS", "TRACE"] {
        let path = format!("{P}/proposals/{staged}/decision");
        let reply = through(&hub.server, method, &path, &page, b"");
        assert_refused(&reply, OFF_LIST_LINE, &format!("{method} {path}"));
    }

    // A HEAD carries no body to read a line from.
    for path in [
        "/".to_owned(),
        "/api/identity".to_owned(),
        "/api/projects".to_owned(),
        format!("{P}/cards"),
        format!("{P}/round"),
    ] {
        let reply = through(&hub.server, "HEAD", &path, &Proxied::fetch("tomas"), b"");
        assert_eq!(reply.status, 403, "HEAD {path}");
        assert_plain(&reply, &format!("HEAD {path}"));
        assert_eq!(reply.header("allow"), None, "HEAD {path}");
    }
    assert_eq!(hub.stored(&ids), before, "no write off the list landed");

    // The reads a person sees these on stay served.
    let inbox = hub.text("tomas", "inbox");
    assert!(inbox.contains(&asked) && inbox.contains(&staged), "{inbox}");
    assert!(hub.text("tomas", "round").contains(&asked));
    let review = hub
        .get("tomas", &format!("{P}/proposals/{staged}"))
        .status(200)
        .json();
    assert_eq!(
        review["staged"]["decision"],
        json!("reject"),
        "the local stage stands: {review}"
    );
}

/// The security review's raw-socket table, kept: `listed` matches exact raw
/// segments, so no spelling of a route reaches its handler, whatever a router or
/// a proxy would normalise it to, and only the listed methods pass the fence.
/// Each form names a request that would land ([`consent_waits`]): `tomas` is on
/// the project's `[team]`, the card's reviewer, the body holds its `revision`,
/// with a good `Origin` and a same-origin page, so the 403 is the list's alone.
/// `accept?x=/move` is the fence's query refusal (hub-team A5, which holds the
/// rest); the plain form lands, last, so the table is no refusal of everything.
/// M: `Hub::fenced` reading the path alone.
#[test]
fn raw_forms_of_a_local_route_and_other_methods_stay_refused() {
    let (setup, asked, staged) = consent_waits("hub-raw-forms");
    tomas_on_the_team(&setup);
    let hub = setup.start();
    let ids = [asked.as_str(), staged.as_str()];
    let before = hub.stored(&ids);
    let page = Proxied::write("tomas");
    let revision = hub.show("C-0001")["revision"]
        .as_str()
        .expect("a revision")
        .to_owned();
    let accept =
        json!({"briefed": true, "note": "Read twice.", "revision": revision, "as": "tomas"})
            .to_string();
    let card = format!("{P}/cards/C-0001");
    let forms = [
        format!("{card}/accept/"),
        format!("{card}//accept"),
        format!("{card}/%61ccept"),
        format!("{card}/ACCEPT"),
        format!("{card}/move/../accept"),
        format!("{card}/./accept"),
        format!("{card}%2Faccept"),
        format!("{card}/%2Faccept"),
        format!("{card}/move%2Faccept"),
        format!("{card}/accept;x"),
        format!("{card}/accept%00"),
    ];
    for path in &forms {
        let reply = through(&hub.server, "POST", path, &page, accept.as_bytes());
        assert_refused(&reply, OFF_LIST_LINE, &format!("POST {path}"));
    }
    let queried = format!("{card}/accept?x=/move");
    let reply = through(&hub.server, "POST", &queried, &page, accept.as_bytes());
    assert_refused(&reply, NO_QUERY_LINE, &format!("POST {queried}"));

    let listed_and_local = [
        format!("{P}/cards"),
        format!("{card}/move"),
        format!("{card}/accept"),
        "/api/identity".to_owned(),
    ];
    for method in [
        "post", "Post", "get", "Get", "HEAD", "OPTIONS", "TRACE", "CONNECT", "PUT", "PATCH",
        "DELETE", "PROPFIND",
    ] {
        for path in &listed_and_local {
            let body = match method {
                "post" | "Post" | "PUT" | "PATCH" | "PROPFIND" => accept.as_bytes(),
                _ => b"",
            };
            let reply = through(&hub.server, method, path, &page, body);
            let context = format!("{method} {path}");
            if method == "HEAD" {
                assert_eq!(reply.status, 403, "{context}");
                assert_plain(&reply, &context);
            } else {
                assert_refused(&reply, OFF_LIST_LINE, &context);
            }
        }
    }
    assert_eq!(hub.stored(&ids), before, "no form reached a handler");

    let path = format!("{card}/accept");
    let reply = through(&hub.server, "POST", &path, &page, accept.as_bytes());
    assert_eq!(reply.status, 200, "the plain form lands: {}", reply.text());
}

// ----------------------------------------------------------- the GET routes

/// Every path the router serves on GET, sorted.
///
/// This list is a decision, not a record. A hub has a list of writes and no
/// list of reads: past the fence every GET route is served to every login
/// (docs/canon/hub.md "Routes"), so a route added under GET is public to every
/// person the day it lands. [`the_get_routes_are_pinned`] fails until someone
/// has decided the new route is safe on a hub, and writes it here. Do not add a
/// line to make the test pass: add it once the route has been judged, for what
/// it reads and for what a login may learn from it.
const GET_ROUTES: [&str; 25] = [
    "/",
    "/api/identity",
    "/api/projects",
    "/api/projects/{p}/bundle",
    "/api/projects/{p}/cards",
    "/api/projects/{p}/cards/{id}",
    "/api/projects/{p}/check",
    "/api/projects/{p}/events",
    "/api/projects/{p}/flow",
    "/api/projects/{p}/graph",
    "/api/projects/{p}/health",
    "/api/projects/{p}/inbox",
    "/api/projects/{p}/lock-diff",
    "/api/projects/{p}/nodes/{*ref}",
    "/api/projects/{p}/proposals/{id}",
    "/api/projects/{p}/proposals/{id}/confirm",
    "/api/projects/{p}/roadmap",
    "/api/projects/{p}/round",
    "/api/projects/{p}/search",
    "/api/projects/{p}/tasks",
    "/api/projects/{p}/tasks/{id}",
    "/api/projects/{p}/team",
    "/api/projects/{p}/trace",
    "/api/projects/{p}/tree",
    "/assets/{file}",
];

/// Whether the method expression of a route serves GET. Only the shape known
/// not to counts as no: a route that opens on a write and chains no read. A
/// `read(..)`, a `get(..)`, an `any(..)` or anything new counts as yes.
fn serves_get(methods: &str) -> bool {
    let opens_on_a_write = ["post(", "put(", "patch(", "delete("]
        .iter()
        .any(|opening| methods.starts_with(opening));
    !opens_on_a_write
        || [".get(", ".any(", ".on("]
            .iter()
            .any(|chain| methods.contains(chain))
}

/// The paths of every `.route(` in `source` whose methods serve GET, comment
/// lines left out. A path that is no literal panics: it could not be judged.
fn get_routes_in(source: &str) -> Vec<String> {
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| format!("{line}\n"))
        .collect();
    let mut paths = Vec::new();
    for (at, call) in code.match_indices(".route(") {
        let rest = &code[at + call.len()..];
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
            .expect("a closed `.route(` call");
        let (path, methods) = rest[..end]
            .trim_start()
            .strip_prefix('"')
            .and_then(|call| call.split_once('"'))
            .unwrap_or_else(|| panic!("a route with no literal path: {}", &rest[..end]));
        let methods =
            methods.trim_start_matches(|letter: char| letter == ',' || letter.is_whitespace());
        if serves_get(methods) {
            paths.push(path.to_owned());
        }
    }
    paths
}

/// [`GET_ROUTES`] against the source of the router, read when the test runs:
/// a running daemon cannot be asked for its routes. Every `src` file is walked,
/// and a way of adding a route that is no `.route(` call is refused, so nothing
/// reaches a hub round the pin.
#[test]
fn the_get_routes_are_pinned() {
    let src = repository_root().join("crates/specengine-http/src");
    let mut served = Vec::new();
    let mut outer_fallbacks = 0;
    for file in files_ending(&src, ".rs") {
        let source = read_text(&src, &file);
        let code: String = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect();
        for other_way in [".nest(", ".nest_service(", ".merge(", ".route_service("] {
            assert!(
                !code.contains(other_way),
                "{file} adds a route with `{other_way}`, which this pin cannot read: list what \
                 it serves on GET, or teach the pin"
            );
        }
        outer_fallbacks += code.matches(".fallback_service(").count();
        served.extend(get_routes_in(&source));
    }
    assert_eq!(
        outer_fallbacks, 1,
        "the routes are the one fallback service of the outer router"
    );
    served.sort();
    assert_eq!(
        served, GET_ROUTES,
        "a GET route changed: every GET is served to every login on a hub. Decide whether it is \
         safe there, then change GET_ROUTES (read its comment)"
    );
}

/// The pin's positive controls: what it must see and what it must leave.
#[test]
fn the_pin_sees_each_shape_of_a_get_route() {
    let router = |route: &str| format!("    let routes = Router::new()\n{route}\n        ;\n");
    for (route, seen) in [
        (r#"        .route("/new", read(new))"#, true),
        (r#"        .route("/new", get(new))"#, true),
        (r#"        .route("/new", any(new))"#, true),
        (r#"        .route("/new", get(new).post(write))"#, true),
        (r#"        .route("/new", post(write).get(new))"#, true),
        (
            "        .route(\n            \"/new\",\n            get(new)\n                \
             .head(only_get)\n                .fallback(only_get),\n        )",
            true,
        ),
        (
            r#"        .route("/new", post(write).fallback(only_post))"#,
            false,
        ),
        (
            r#"        .route("/new", post(write).delete(drop).fallback(only))"#,
            false,
        ),
        (r#"        // .route("/new", read(new))"#, false),
    ] {
        let want: Vec<String> = seen.then(|| "/new".to_owned()).into_iter().collect();
        assert_eq!(get_routes_in(&router(route)), want, "{route}");
    }
}

/// The security review's named mutation, on a copy in memory of the live
/// router, so it repeats on every run and writes nothing under `src`: a route
/// added under GET that [`GET_ROUTES`] does not list parts from the pin, in the
/// comparison [`the_get_routes_are_pinned`] makes. The unmutated copy agrees, so
/// the difference is that one route.
#[test]
fn a_get_route_the_pin_does_not_list_parts_from_it() {
    let source = read_text(&repository_root(), "crates/specengine-http/src/app.rs");
    let anchor = "let routes = Router::new()\n";
    assert_eq!(
        source.matches(anchor).count(),
        1,
        "the router's one opening"
    );
    let sorted = |text: &str| {
        let mut paths = get_routes_in(text);
        paths.sort();
        paths
    };
    assert_eq!(sorted(&source), GET_ROUTES, "the copy, unmutated");
    let added = format!("{anchor}        .route(\"/api/added\", read(identity))\n");
    let mutated = sorted(&source.replacen(anchor, &added, 1));
    assert_ne!(mutated, GET_ROUTES, "the pin did not see the route");
    assert_eq!(
        mutated.len(),
        GET_ROUTES.len() + 1,
        "exactly that route: {mutated:?}"
    );
    assert!(
        mutated.iter().any(|path| path == "/api/added"),
        "{mutated:?}"
    );
}

// --------------------------------------------------------------- AC-09

/// AC-09, required, its first case, and it is no defect: this is the
/// deployment's guarantee, not the daemon's. A raw client on the socket, with no
/// proxy before it, sends `Remote-User: anyone` and passes as anyone: it gets a
/// 200 and `anyone` is recorded. The daemon cannot tell its proxy's header from
/// any other process reaching `127.0.0.1`, so what keeps a stranger off this
/// socket is the deployment: `docs/canon/hub.md#trust` and RM-0064 (the port
/// never published, nothing but the proxy in the network namespace, the header
/// in `authResponseHeaders` on every router, no untrusted process on the
/// host).
///
/// A shared secret would turn this test red. That is ADR-0034 speaking: it
/// would be the token that decision forbids; do not "fix" this test.
#[test]
fn hub_trusts_the_header_on_its_socket() {
    let hub = Setup::new("hub-forged", "card-press").start();
    let identity = hub.server.raw(&request_bytes(
        "GET",
        "/api/identity",
        &[("Host", AUTHORITY), ("Remote-User", "anyone")],
    ));
    identity.status(200);
    assert_eq!(identity.text(), "{\"login\":\"anyone\"}");

    let body = json!({"title": "Forged", "discipline": "plates"}).to_string();
    let length = body.len().to_string();
    let mut bytes = request_bytes(
        "POST",
        &format!("{P}/cards"),
        &[
            ("Host", AUTHORITY),
            ("Remote-User", "anyone"),
            ("Sec-Fetch-Site", "same-origin"),
            ("Content-Type", "application/json"),
            ("Content-Length", length.as_str()),
        ],
    );
    bytes.extend_from_slice(body.as_bytes());
    let reply = hub.server.raw(&bytes);
    assert_eq!(reply.status, 200, "{}", reply.text());

    assert_eq!(hub.show("C-0001")["created_by"], json!("anyone"));
    let (kind, payload) = hub.events().pop().expect("the card's event");
    assert_eq!(kind, "card.created");
    assert_eq!(payload["as"], json!("anyone"), "{payload}");
}

/// What the daemon can judge of one `Remote-User` value: each of these reaches
/// the fence (httparse passes a tab and `0x80`-`0xFF`) and is a 403, on a read,
/// the page and a write alike; `local` and `agent` are the record's words for
/// no person.
fn bad_logins() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("a proxy's folded `a, b`", b"a, b".to_vec()),
        ("an inner tab", b"mi\tra".to_vec()),
        ("a byte off UTF-8", b"mi\xFFra".to_vec()),
        ("129 bytes", vec![b'n'; 129]),
        (
            "129 bytes in another script",
            format!("{}n", "\u{0430}".repeat(64)).into_bytes(),
        ),
        ("`local`", b"local".to_vec()),
        ("`agent`", b"agent".to_vec()),
        ("a bidirectional mark", "a\u{202e}b".as_bytes().to_vec()),
        ("a line separator", "a\u{2028}b".as_bytes().to_vec()),
        // `.trim()` would change these, so the record could not hold the login
        // verbatim: hyper strips ASCII space, never these.
        ("a leading NBSP", "\u{a0}mira".as_bytes().to_vec()),
        ("a trailing NBSP", "mira\u{a0}".as_bytes().to_vec()),
        (
            "a leading ideographic space",
            "\u{3000}mira".as_bytes().to_vec(),
        ),
        // Reads as `mira` on a screen and is not her.
        ("a zero-width space", "mira\u{200b}".as_bytes().to_vec()),
        ("a soft hyphen", "mi\u{ad}ra".as_bytes().to_vec()),
        ("a word joiner", "mira\u{2060}".as_bytes().to_vec()),
        ("a byte order mark", "\u{feff}mira".as_bytes().to_vec()),
        // The record's words in another case, and no name at all.
        ("`Local`", b"Local".to_vec()),
        ("`AGENT`", b"AGENT".to_vec()),
        ("`LoCaL`", b"LoCaL".to_vec()),
        ("`-`", b"-".to_vec()),
    ]
}

/// AC-09: two values, `a, b`, an inner tab, a byte off UTF-8, 129 bytes,
/// `local`, `agent`, each a 403 that stores nothing, on every kind of route.
/// Edge case: a control byte never reaches the fence, hyper's 400.
#[test]
fn ac09_a_bad_login_is_a_403_on_every_kind_of_route_and_stores_nothing() {
    let hub = Setup::new("hub-bad-login", "card-press").start();
    let card = json!({"title": "Back page", "discipline": "plates"}).to_string();
    let routes = [
        ("GET", "/api/identity".to_owned(), ""),
        ("GET", "/".to_owned(), ""),
        ("GET", format!("{P}/cards"), ""),
        ("POST", format!("{P}/cards"), card.as_str()),
    ];
    for (label, value) in bad_logins() {
        for (method, path, body) in &routes {
            let bytes = Proxied::write(MARKER).bytes(method, path, body.as_bytes());
            let reply = hub.server.raw(&spliced(&bytes, &value));
            assert_refused_naming(&reply, "Remote-User", &format!("{label}: {method} {path}"));
        }
    }
    for (first, second) in [("mira", "eve"), ("eve", "mira"), ("mira", "mira")] {
        let two = Proxied::write(first).plus("Remote-User", second);
        for (method, path, body) in &routes {
            let reply = through(&hub.server, method, path, &two, body.as_bytes());
            let context = format!("{first} and {second}: {method} {path}");
            assert_refused(&reply, TWO_LOGINS_LINE, &context);
        }
    }
    // An empty value is no identity, not a bad one; a blank one is empty by
    // the time hyper hands it on.
    for value in ["", "   "] {
        let blank = Proxied::write(value);
        for (method, path, body) in &routes {
            let reply = through(&hub.server, method, path, &blank, body.as_bytes());
            let context = format!("{value:?}: {method} {path}");
            assert_refused(&reply, NO_LOGIN_LINE, &context);
        }
    }
    let bytes = Proxied::fetch(MARKER).bytes("GET", "/api/identity", b"");
    let reply = hub.server.raw(&spliced(&bytes, b"mi\x01ra"));
    assert_eq!(reply.status, 400, "a control byte: {reply:?}");
    assert!(hub.events().is_empty(), "{:?}", hub.events());
    let board = hub
        .get("mira", &format!("{P}/cards?with_cancelled=true"))
        .status(200)
        .json();
    assert_eq!(board["cards"], json!([]), "{board}");
}

/// The lines the review's logins are refused with: a person reading a 403 is
/// told which word it was. Tried on the identity read, which needs no project.
#[test]
fn ac09_a_login_that_reads_as_another_or_as_no_one_is_told_why() {
    let hub = Setup::new("hub-login-lines", "card-press").start();
    for (login, line) in [
        (
            "mira\u{200b}",
            "refused: Remote-User: U+200B in a name: no format character, which can read as \
             another person",
        ),
        (
            "Local",
            "refused: Remote-User: `Local` is the record's word for no person",
        ),
        (
            "AGENT",
            "refused: Remote-User: `AGENT` is the record's word for no person",
        ),
        (
            "-",
            "refused: Remote-User: `-` is the record's word for clearing a name",
        ),
    ] {
        assert_refused(&hub.get(login, "/api/identity"), line, login);
    }
}

/// AC-09's other side: the longest login, any script and an inner space pass,
/// and the record holds the login verbatim (A4).
#[test]
fn ac09_the_longest_login_and_any_script_are_recorded_verbatim() {
    let hub = Setup::new("hub-good-login", "card-press").start();
    for login in [
        "n".repeat(128),
        "\u{0430}".repeat(64),
        "Mira Ortiz".to_owned(),
        FULL_NAME.to_owned(),
    ] {
        let reply = hub.get(&login, "/api/identity");
        assert_eq!(reply.status, 200, "{login}: {}", reply.text());
        assert_eq!(reply.json(), json!({"login": login}));
        let made = hub.done(
            &login,
            "cards",
            &json!({"title": "Back page", "discipline": "plates"}),
        );
        let id = made["id"].as_str().expect("an ID");
        assert_eq!(hub.show(id)["created_by"], json!(login));
    }
}

// --------------------------------------------------------------- AC-10

/// AC-10, its new half, required: with no flag `Remote-User` is never read. A
/// typed `Ana` stands against `mira`, null is `local`, `GET /api/identity`
/// answers null, and a value a hub would refuse passes untouched. A forgotten
/// flag fails closed: the public `Host` is the local fence's 403.
#[test]
fn ac10_a_local_daemon_never_reads_remote_user() {
    let scratch = Scratch::new("hub-local");
    let root = scratch.repo("card-press", "served", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);

    let long = "n".repeat(129);
    let unread: [&[(&str, &str)]; 6] = [
        &[],
        &[("Remote-User", "mira")],
        &[("Remote-User", "local")],
        &[("Remote-User", "a, b")],
        &[("Remote-User", "mira"), ("Remote-User", "tomas")],
        &[("Remote-User", long.as_str())],
    ];
    for headers in unread {
        let reply = server.request("GET", "/api/identity", headers);
        assert_eq!(reply.status, 200, "{headers:?}: {}", reply.text());
        assert_eq!(reply.text(), "{\"login\":null}", "{headers:?}");
    }
    assert!(
        beyond_daemon(&home).is_empty(),
        "the identity read reads no project: {:?}",
        beyond_daemon(&home)
    );

    let own = format!("http://127.0.0.1:{}", server.port);
    let page = |login: &'static str| {
        [
            ("Sec-Fetch-Site", "same-origin"),
            ("Origin", own.as_str()),
            ("Content-Type", "application/json"),
            ("Remote-User", login),
        ]
    };
    let cases = [
        ("mira", json!("Ana"), "Ana"),
        ("mira", Value::Null, "local"),
        ("local", json!("Ana"), "Ana"),
    ];
    for (index, (login, typed, recorded)) in cases.iter().enumerate() {
        let body = json!({"title": "Lead story", "discipline": "copy", "as": typed});
        let reply = server.send(
            "POST",
            &format!("{P}/cards"),
            &page(login),
            body.to_string().as_bytes(),
        );
        assert_eq!(reply.status, 200, "{body}: {}", reply.text());
        let card = server
            .get(&format!("{P}/cards/C-{:04}", index + 1))
            .status(200)
            .json();
        assert_eq!(card["created_by"], json!(recorded), "{body}: {card}");
    }
    let events = events_of(&home, &root);
    let recorded: Vec<&Value> = events.iter().map(|(_, payload)| &payload["as"]).collect();
    assert_eq!(
        recorded,
        [&json!("Ana"), &json!("local"), &json!("Ana")],
        "{events:?}"
    );

    let reply = server.raw(&request_bytes(
        "GET",
        "/api/identity",
        &[("Host", AUTHORITY), ("Remote-User", "mira")],
    ));
    assert_eq!(reply.status, 403, "{}", reply.text());
    let message = reply.error_message();
    assert!(
        message.starts_with(&format!(
            "refused: the Host header must be 127.0.0.1:{}",
            server.port
        )),
        "the local fence's own line: {message}"
    );
}

// --------------------------------------------------------- the identity read

/// "Route": `read(identity)`, in both modes, no project, no turn: the exact
/// body, a fresh `HOME` left alone, no query name, and the route named just
/// before the web UI's in the unknown route's line, the prefix the other files
/// pin kept with its space.
#[test]
fn the_identity_route_is_a_plain_read_in_both_modes() {
    let hub = Setup::new("hub-identity", "card-press").start();
    let reply = hub.get("mira", "/api/identity");
    reply.status(200);
    assert_eq!(reply.text(), "{\"login\":\"mira\"}");
    assert_eq!(
        reply.header("content-type"),
        Some("application/json; charset=utf-8")
    );
    assert_plain(&reply, "on a hub");
    assert!(
        beyond_hub(&hub.setup.home).is_empty(),
        "no project, no turn: {:?}",
        beyond_hub(&hub.setup.home)
    );
    let reply = hub.get("mira", "/api/identity?login=tomas");
    assert_eq!(reply.status, 400, "{}", reply.text());
    let reply = hub.get("mira", "/no/such/route");
    assert_eq!(reply.status, 404, "{}", reply.text());
    let message = reply.error_message();
    assert!(
        message.starts_with("no route /no/such/route: the routes are /api/projects ")
            && message.ends_with("/api/identity; the web UI: / and /assets/<file>"),
        "{message}"
    );

    let scratch = Scratch::new("hub-identity-local");
    let root = scratch.repo("card-press", "served", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::serve(&home, &cwd, &[&root]);
    let reply = server.get("/api/identity?login=tomas");
    assert_eq!(reply.status, 400, "{}", reply.text());
    let reply = server.send("POST", "/api/identity", &[], b"");
    assert_eq!(reply.status, 405, "{}", reply.text());
    assert_eq!(reply.header("allow"), Some("GET"));
}

// --------------------------------------------------------------- AC-11

/// AC-11, its first half: on a repository with an open question and an
/// identity that could commit, each of a hub's five writes adds exactly its one
/// `card.*` event and leaves `git status`, `HEAD`, the refs, every file and the
/// proposal as they were (`door.rs`, `card_writes_touch_only_the_queue`, on the
/// local daemon).
#[test]
fn ac11_a_hubs_five_writes_touch_only_the_queue() {
    let setup = Setup::new("hub-door", "card-press");
    let question = ask(
        &setup.home,
        &setup.root,
        &["SHT-BACK"],
        "Does the tide table keep its rule?",
    );
    let hub = setup.start();
    let repository = hub.repository();
    assert_eq!(repository[0], "", "the copy is committed");
    let proposal = hub.proposal(&question);
    let mut stored = hub.events();
    for (path, body, kind) in five_writes("C-0001") {
        hub.done("mira", &path, &body);
        assert_eq!(
            hub.repository(),
            repository,
            "{path}: git, the refs, every file"
        );
        assert_eq!(hub.proposal(&question), proposal, "{path}: the proposal");
        let events = hub.events();
        assert_eq!(
            events[..stored.len()],
            stored[..],
            "{path}: the earlier events kept"
        );
        let added: Vec<&str> = events[stored.len()..]
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect();
        assert_eq!(added, [kind], "{path}: one event");
        stored = events;
    }
}

// -------------------------------------------------------------- D1, two roots

/// D1 and "Routes": any identity reads every served project, on two projects of
/// different nature, whoever asks.
#[test]
fn any_login_reads_every_served_project() {
    let scratch = Scratch::new("hub-two-roots");
    let a = scratch.repo("spec-a", "a", "main");
    let b = scratch.repo("card-press", "b", "main");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let server = Server::start(&home, &cwd, &hub_args(&[&a, &b], ORIGIN));
    assert_eq!(
        server.lines[2..],
        [
            format!("public {ORIGIN}"),
            format!("listening http://127.0.0.1:{}", server.port),
        ],
        "after the `serving` lines"
    );
    for login in ["mira", "tomas"] {
        let person = Proxied::fetch(login);
        let list = through(&server, "GET", "/api/projects", &person, b"")
            .status(200)
            .json();
        let slugs: Vec<&str> = list
            .as_array()
            .expect("a list")
            .iter()
            .map(|project| project["slug"].as_str().expect("a slug"))
            .collect();
        assert_eq!(slugs, ["lantern-keep", "card-press"], "{login}");
        for slug in slugs {
            let path = format!("/api/projects/{slug}/tree");
            through(&server, "GET", &path, &person, b"").status(200);
        }
    }
}
