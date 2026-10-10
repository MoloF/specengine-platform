//! docs/features/ui-embed.md T3: the web UI on a spawned daemon. The
//! branch is chosen by `ui/dist/index.html` on disk when the test runs (the
//! daemon under test is compiled from this tree, and its build script
//! reruns on any change of `ui/dist`):
//!
//! - present: `/` is the page's exact bytes, `text/html; charset=utf-8`,
//!   the exact `Content-Security-Policy` and `nosniff`; every file of
//!   `ui/dist/assets/` is byte-equal at `/assets/<file>` with the table's
//!   type; every `/assets/…` the page loads answers 200;
//! - absent: `/` is the 503 with the not-built line; `/assets/x.js` the
//!   unknown-route 404.
//!
//! Both: `Cache-Control: no-store` and no `Access-Control-*` on every
//! answer, no `ETag`, `Last-Modified`, `max-age`, `Content-Encoding`,
//! `Accept-Ranges`; `/?scenario=normal` (and any query) equals `/`; HEAD,
//! POST, PUT, DELETE, PATCH, OPTIONS, TRACE on `/` and an asset are the
//! read routes' 405 `Allow: GET`; no traversal, encoded or not, reaches a
//! file (404 or 400, never 200, never `Cargo.toml`'s bytes); nothing is
//! written beyond the daemon's own two files; `/` answers alike while a
//! served root's `specengine.toml` is broken and `/api/projects` is 503.
//!
//! M: an SPA fallback; `get(page)` for `read(page)`; a disk read of
//! `ui/dist/assets/<decoded name>`; `max-age` on assets in the fence; the
//! page handler reading configs; strict query parsing on `/`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Reply, Scratch, Server, beyond_daemon, repository_root, snapshot};

/// The not-built message, as the spec writes it ("Data", WA 8).
const NOT_BUILT: &str = "this specengine-http holds no web UI (ui/dist had no index.html when it \
                         was compiled): run pnpm --dir ui install --frozen-lockfile && pnpm --dir \
                         ui build, then cargo install --locked --force --path \
                         <clone>/crates/specengine-http, then restart spec serve";

/// The page's policy, as the spec writes it ("Data", WA 11).
const PAGE_POLICY: &str = "default-src 'self'; img-src 'self' data:; object-src 'none'; \
                           base-uri 'none'; frame-ancestors 'none'";

/// Every method but GET.
const NOT_GET: [&str; 7] = ["HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "TRACE"];

/// Headers no UI answer carries ("Data", WA 9).
const NEVER: [&str; 4] = ["etag", "last-modified", "content-encoding", "accept-ranges"];

/// The unknown-route 404's message ends with the web UI's routes.
const UI_ROUTES: &str = "; the web UI: / and /assets/<file>";

fn dist() -> PathBuf {
    repository_root().join("ui/dist")
}

/// Whether the tree holds a built UI (the daemon compiled from it does).
fn built() -> bool {
    dist().join("index.html").is_file()
}

/// The table's `Content-Type` of `name` ("Data", WA 10), written again
/// here rather than read from the daemon.
fn table_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("json" | "map") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// `(name, bytes)` of every entry directly under `ui/dist/assets`; a
/// directory there is a failure (no route serves a nested file).
fn dist_assets() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dist().join("assets")).expect("ui/dist/assets") {
        let path = entry.expect("an entry").path();
        assert!(
            path.is_file(),
            "{}: ui/dist/assets holds only files (Vite's hashed names)",
            path.display()
        );
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 name")
            .to_owned();
        out.push((name, fs::read(&path).expect("an asset")));
    }
    out.sort();
    out
}

/// No-store, no CORS, none of [`NEVER`], no `max-age` anywhere.
fn assert_plain(reply: &Reply, context: &str) {
    assert_eq!(
        reply.header("cache-control"),
        Some("no-store"),
        "{context}: {:?}",
        reply.headers
    );
    assert_eq!(
        reply
            .headers
            .iter()
            .filter(|(name, _)| name == "cache-control")
            .count(),
        1,
        "{context}: one Cache-Control: {:?}",
        reply.headers
    );
    assert!(
        reply.cors_headers().is_empty(),
        "{context}: an Access-Control-* header: {:?}",
        reply.headers
    );
    for name in NEVER {
        assert_eq!(reply.header(name), None, "{context}: {name}");
    }
    assert!(
        reply
            .headers
            .iter()
            .all(|(_, value)| !value.to_ascii_lowercase().contains("max-age")),
        "{context}: max-age: {:?}",
        reply.headers
    );
}

/// A UI 200: `body`, `wanted_type`, `nosniff`, plain.
fn assert_served(reply: &Reply, body: &[u8], wanted_type: &str, context: &str) {
    assert_eq!(reply.status, 200, "{context}: {:?}", reply.headers);
    assert!(
        reply.body == body,
        "{context}: not the dist's bytes ({} B served, {} B on disk)",
        reply.body.len(),
        body.len()
    );
    assert_eq!(reply.header("content-type"), Some(wanted_type), "{context}");
    assert_eq!(
        reply.header("x-content-type-options"),
        Some("nosniff"),
        "{context}"
    );
    assert_plain(reply, context);
}

/// The unknown-route 404 of `path` (its query dropped).
fn assert_unknown(reply: &Reply, path: &str) {
    assert_eq!(
        reply.status,
        404,
        "{path}: {}",
        String::from_utf8_lossy(&reply.body)
    );
    assert_eq!(reply.header("allow"), None, "{path}: no Allow on a 404");
    assert_plain(reply, path);
    let route = path.split_once('?').map_or(path, |(route, _)| route);
    let message = reply.error_message();
    assert!(
        message.starts_with(&format!("no route {route}: the routes are /api/projects ")),
        "{path}: {message}"
    );
    assert!(message.ends_with(UI_ROUTES), "{path}: {message}");
}

/// Every `/assets/…` URL the page names in a `src` or `href`.
fn page_loads(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attribute in ["src=\"", "href=\""] {
        for (at, _) in page.match_indices(attribute) {
            let value = &page[at + attribute.len()..];
            let value = &value[..value.find('"').expect("a closing quote")];
            if value.starts_with("/assets/") {
                out.push(value.to_owned());
            }
        }
    }
    out
}

/// One daemon on a git copy of `fixtures/spec-a` with a fresh `HOME`.
struct Daemon {
    // Dropped first: the daemon is killed before its scratch goes.
    server: Server,
    home: PathBuf,
    root: PathBuf,
    _scratch: Scratch,
}

impl Daemon {
    fn start(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let root = scratch.repo("spec-a", "a", "main");
        let home = scratch.home("fresh");
        let server = Server::serve(&home, scratch.path(), &[&root]);
        Self {
            server,
            home,
            root,
            _scratch: scratch,
        }
    }

    fn home(&self) -> &Path {
        &self.home
    }
}

#[test]
fn t3_slash_and_every_asset_are_the_dists_bytes_or_slash_is_the_not_built_line() {
    let daemon = Daemon::start("ui-bytes");
    let server = &daemon.server;
    let repository = snapshot(&daemon.root);
    if built() {
        let page = fs::read(dist().join("index.html")).expect("ui/dist/index.html");
        let reply = server.get("/");
        assert_served(&reply, &page, "text/html; charset=utf-8", "/");
        assert_eq!(
            reply.header("content-security-policy"),
            Some(PAGE_POLICY),
            "/: the policy, exactly"
        );
        assert_eq!(
            reply
                .headers
                .iter()
                .filter(|(name, _)| name == "content-security-policy")
                .count(),
            1,
            "/: one policy"
        );
        let assets = dist_assets();
        assert!(!assets.is_empty(), "a built dist holds assets");
        for (name, bytes) in &assets {
            let path = format!("/assets/{name}");
            assert_served(&server.get(&path), bytes, table_type(name), &path);
        }
        // What the page loads resolves (base `/`, no `public/`).
        let loads = page_loads(std::str::from_utf8(&page).expect("a UTF-8 page"));
        assert!(!loads.is_empty(), "the page loads its assets");
        for path in loads {
            assert_eq!(server.get(&path).status, 200, "the page loads {path}");
        }
        // Nothing else of the dist's root is served (no SPA fallback,
        // no `/index.html` alias, no `public/`).
        for entry in fs::read_dir(dist()).expect("ui/dist") {
            let name = entry.expect("an entry").file_name();
            let name = name.to_str().expect("a UTF-8 name");
            if name != "assets" {
                let path = format!("/{name}");
                assert_unknown(&server.get(&path), &path);
            }
        }
    } else {
        let reply = server.get("/");
        assert_eq!(reply.status, 503, "/ without ui/dist: {}", reply.text());
        assert_eq!(reply.error_message(), NOT_BUILT);
        assert_plain(&reply, "/ not built");
        assert_eq!(reply.header("content-security-policy"), None);
    }
    for path in ["/assets/x.js", "/assets/x.js?v=1"] {
        assert_unknown(&server.get(path), path);
    }
    assert!(
        beyond_daemon(daemon.home()).is_empty(),
        "the UI routes wrote under HOME: {:?}",
        beyond_daemon(daemon.home())
    );
    assert!(
        snapshot(&daemon.root) == repository,
        "the UI routes changed the served root"
    );
}

#[test]
fn t3_the_query_is_ignored_and_no_answer_caches_or_shares() {
    let daemon = Daemon::start("ui-query");
    let server = &daemon.server;
    let own_origin = format!("http://127.0.0.1:{}", server.port);
    let page = server.get("/");
    assert_plain(&page, "/");
    // The API's parser refuses each of these (an unknown key, a repeated
    // key, a bad escape, an empty key, a bare word): `/` ignores them.
    for path in [
        "/?scenario=normal",
        "/?scenario=normal&scenario=empty",
        "/?%zz=1",
        "/?=&&",
        "/?query",
        "/?node_ids=MEC-STAMINA&budget=-1",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, page.status, "{path}");
        assert!(reply.body == page.body, "{path}: the bytes of /");
        for name in [
            "content-type",
            "content-security-policy",
            "x-content-type-options",
        ] {
            assert_eq!(reply.header(name), page.header(name), "{path}: {name}");
        }
        assert_plain(&reply, path);
    }
    // The page's own loads: same-origin, its Origin sent.
    let fetch_site = [
        ("Origin", own_origin.as_str()),
        ("Sec-Fetch-Site", "same-origin"),
    ];
    let reply = server.request("GET", "/", &fetch_site);
    assert!(reply.body == page.body, "/ same-origin");
    assert_plain(&reply, "/ same-origin");
    // The address bar: Sec-Fetch-Site none.
    let reply = server.request("GET", "/", &[("Sec-Fetch-Site", "none")]);
    assert!(reply.body == page.body, "/ none");
    if built() {
        let (name, bytes) = dist_assets().into_iter().next().expect("an asset");
        for path in [
            format!("/assets/{name}?v=1"),
            format!("/assets/{name}?scenario=normal&%zz"),
        ] {
            let reply = server.request("GET", &path, &fetch_site);
            assert_served(&reply, &bytes, table_type(&name), &path);
        }
    }
    assert!(beyond_daemon(daemon.home()).is_empty());
}

#[test]
fn t3_every_method_but_get_is_the_read_routes_405_allow_get() {
    let daemon = Daemon::start("ui-methods");
    let server = &daemon.server;
    let mut paths = vec!["/".to_owned(), "/assets/x.js".to_owned()];
    if built() {
        let (name, _) = dist_assets().into_iter().next().expect("an asset");
        paths.push(format!("/assets/{name}"));
    }
    for path in &paths {
        for method in NOT_GET {
            let reply = server.request(method, path, &[]);
            let context = format!("{method} {path}");
            assert_eq!(
                reply.status,
                405,
                "{context}: {}",
                String::from_utf8_lossy(&reply.body)
            );
            assert_eq!(reply.header("allow"), Some("GET"), "{context}");
            assert_plain(&reply, &context);
            if method == "HEAD" {
                assert!(reply.body.is_empty(), "{context}: no body");
                continue;
            }
            assert_eq!(
                reply.error_message(),
                format!("method {method} is not served on {path}: only GET is served here"),
                "{context}"
            );
        }
    }
    assert!(beyond_daemon(daemon.home()).is_empty());
}

#[test]
fn t3_no_traversal_encoded_or_not_reaches_a_file() {
    let daemon = Daemon::start("ui-probes");
    let server = &daemon.server;
    let cargo_toml = fs::read(repository_root().join("Cargo.toml")).expect("Cargo.toml");
    let page = built().then(|| fs::read(dist().join("index.html")).expect("the page"));
    for path in [
        "/assets/../../../Cargo.toml",
        "/assets/..%2f..%2f..%2fCargo.toml",
        "/assets/..%2F..%2F..%2FCargo.toml",
        "/assets/%2e%2e/%2e%2e/%2e%2e/Cargo.toml",
        "/assets/%2e%2e%2f%2e%2e%2f%2e%2e%2fCargo.toml",
        "/assets/%2E%2E%2F%2E%2E%2F%2E%2E%2FCargo.toml",
        "/assets/..%5c..%5c..%5cCargo.toml",
        "/assets/..\\..\\..\\Cargo.toml",
        "/assets/%2e%2e",
        "/assets/..",
        "/assets/.",
        "/assets/.%2e/.%2e/.%2e/Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/%2e%2e%2fCargo.toml",
        "/..%2fCargo.toml",
        "/../Cargo.toml",
        "/../../Cargo.toml",
        "/Cargo.toml",
        "/assets//etc/passwd",
        "/assets/%2fetc%2fpasswd",
        "//etc/passwd",
        "/assets/..%2f..%2findex.html",
        "/assets/..%2findex.html",
        "/assets/index.html",
        "/ui/dist/index.html",
        "/index.html",
        "/assets",
        "/assets/",
        "/assets/a/b.js",
        "/assets/.vite",
        "/no/such/page",
    ] {
        let reply = server.get(path);
        assert!(
            reply.status == 404 || reply.status == 400,
            "{path}: {} {}",
            reply.status,
            String::from_utf8_lossy(&reply.body[..reply.body.len().min(200)])
        );
        assert!(reply.body != cargo_toml, "{path}: Cargo.toml's bytes");
        let text = String::from_utf8_lossy(&reply.body);
        assert!(
            !text.contains("[workspace]") && !text.contains("root:"),
            "{path}: a file's text: {text}"
        );
        if let Some(page) = &page {
            assert!(&reply.body != page, "{path}: the page");
        }
        assert_plain(&reply, path);
        if reply.status == 404 {
            reply.error_message();
        }
    }
    assert!(beyond_daemon(daemon.home()).is_empty());
}

#[test]
fn t3_slash_answers_alike_while_a_served_config_is_broken() {
    let daemon = Daemon::start("ui-broken");
    let server = &daemon.server;
    let before = server.get("/");
    let asset = built().then(|| {
        let (name, _) = dist_assets().into_iter().next().expect("an asset");
        let path = format!("/assets/{name}");
        let reply = server.get(&path);
        (path, reply)
    });
    server.get("/api/projects").status(200);

    let config = daemon.root.join("specengine.toml");
    fs::write(&config, "[project\nslug = \"lantern-keep\"\n").expect("break the config");
    let projects = server.get("/api/projects");
    assert_eq!(projects.status, 503, "{}", projects.text());
    projects.error_message();

    let home_after_api = snapshot(daemon.home());
    for _ in 0..2 {
        let after = server.get("/");
        assert_eq!(after.status, before.status, "/ while the config is broken");
        assert!(after.body == before.body, "/: the same bytes");
        for name in [
            "content-type",
            "content-security-policy",
            "x-content-type-options",
        ] {
            assert_eq!(after.header(name), before.header(name), "/: {name}");
        }
        assert_plain(&after, "/ broken config");
        if let Some((path, reply)) = &asset {
            let again = server.get(path);
            assert_eq!(again.status, 200, "{path}");
            assert!(again.body == reply.body, "{path}: the same bytes");
        }
    }
    assert!(
        snapshot(daemon.home()) == home_after_api,
        "the UI routes wrote under HOME"
    );
}
