//! The web UI (docs/features/ui-embed.md "Data", "Rules and edge cases"):
//! the Vite build `ui/dist`, compiled into the daemon by `rust-embed` with
//! `debug-embed` (debug and test builds embed too: files only as
//! compile-time keys, no file read at run time), answered on two read
//! routes inside the fence: `/` the page, `/assets/<file>` its files.
//! Built = the embed holds the key `index.html`; a daemon compiled without
//! it answers `/` with a 503 (the not-built line) and every asset with the
//! unknown-route 404.
//!
//! A UI route calls no CLI function, reads no config, takes no project
//! turn and writes nothing; the query is ignored. `<file>` is the raw last
//! path segment, never percent-decoded: empty, holding `/`, `\` or `%`, or
//! starting with `.`, it is the 404 without a lookup; else one lookup of
//! `assets/<file>` by exact key. No SPA fallback: the page is served at `/`
//! only, and nothing else of the dist's root. Serving is generic over
//! [`RustEmbed`], so a test can embed a fixture, or a folder that never
//! exists.

use std::borrow::Cow;

use axum::body::Body;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse as _, Response};
use rust_embed::RustEmbed;

use crate::answer::{NO_STORE, error};
use crate::app::unknown_route_of;

/// `ui/dist` as it was when the crate was compiled (the crate's own
/// relative path: `publish = false`, so `cargo install --path` reads the
/// clone's); a missing folder compiles to an empty embed.
#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
#[allow_missing = true]
pub(crate) struct Dist;

/// The page's key, which makes an embed built.
const PAGE: &str = "index.html";

/// The route of the assets, and their keys' folder.
const ASSETS_ROUTE: &str = "/assets/";
const ASSETS: &str = "assets/";

/// The message of `/` on a daemon compiled without the page.
const NOT_BUILT: &str = "this specengine-http holds no web UI (ui/dist had no index.html when it \
                         was compiled): run pnpm --dir ui install --frozen-lockfile && pnpm --dir \
                         ui build, then cargo install --locked --force --path \
                         <clone>/crates/specengine-http, then restart spec serve";

/// `X-Content-Type-Options` of every UI 200.
const NOSNIFF: HeaderValue = HeaderValue::from_static("nosniff");

/// `Content-Security-Policy` of the page.
const CSP: HeaderValue = HeaderValue::from_static(
    "default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; \
     frame-ancestors 'none'",
);

/// `GET /`: the page of the compiled-in dist.
pub(crate) async fn page() -> Response {
    page_of::<Dist>()
}

/// `GET /assets/<file>`: a file of the compiled-in dist.
pub(crate) async fn asset(uri: Uri) -> Response {
    asset_of::<Dist>(&uri)
}

/// Whether `E` holds the page.
pub(crate) fn built<E: RustEmbed>() -> bool {
    E::get(PAGE).is_some()
}

/// `/` over `E`: 200 with the page's bytes, or the 503 when `E` is not
/// built.
pub(crate) fn page_of<E: RustEmbed>() -> Response {
    let Some(file) = E::get(PAGE) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, NOT_BUILT);
    };
    let mut response = served(file.data, PAGE);
    response.headers_mut().insert(CONTENT_SECURITY_POLICY, CSP);
    response
}

/// `/assets/<file>` over `E` (`uri` the request's, its path raw): 200 with
/// the file's bytes, or the unknown-route 404 (not built, a name refused,
/// a name not embedded).
pub(crate) fn asset_of<E: RustEmbed>(uri: &Uri) -> Response {
    let found = uri
        .path()
        .strip_prefix(ASSETS_ROUTE)
        .filter(|name| plain(name) && built::<E>())
        .and_then(|name| E::get(&format!("{ASSETS}{name}")).map(|file| (name, file)));
    match found {
        Some((name, file)) => served(file.data, name),
        None => unknown_route_of(uri).into_response(),
    }
}

/// Whether `name` may be looked up: not empty, no `/`, `\` or `%` (no
/// path, no decoding), not starting with `.`.
fn plain(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '%'])
}

/// A UI 200: `data`, the type of `name`, `nosniff`, `no-store`.
fn served(data: Cow<'static, [u8]>, name: &str) -> Response {
    let mut response = Response::new(Body::from(data));
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(type_of(name)));
    headers.insert(X_CONTENT_TYPE_OPTIONS, NOSNIFF);
    headers.insert(CACHE_CONTROL, NO_STORE);
    response
}

/// The `Content-Type` of `name`, by the extension after its last `.`, as
/// written; another extension or none is `application/octet-stream`.
pub(crate) fn type_of(name: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    //! docs/features/ui-embed.md T2: the serving, generic over the embed,
    //! run over a committed fixture (`fixtures/ui-dist`: the page, a `.js`,
    //! a `.css`, an `.svg`, an unknown extension, a dotfile, a nested file
    //! and a root file but the page), over an embed without the page
    //! (`fixtures/ui-dist-pageless`) and over a folder that never exists.
    //! M: `.js` as `application/octet-stream`; `built` inverted; the CSP
    //! dropped; `nosniff` dropped.

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use axum::http::{HeaderMap, StatusCode, Uri};
    use axum::response::Response;
    use rust_embed::RustEmbed;

    use super::{asset_of, built, page_of, plain, type_of};

    /// The committed fixture: a built dist.
    #[derive(RustEmbed)]
    #[folder = "../../fixtures/ui-dist"]
    struct Fixture;

    /// Assets without the page: a failed or partial Vite run.
    #[derive(RustEmbed)]
    #[folder = "../../fixtures/ui-dist-pageless"]
    struct Pageless;

    /// A folder that never exists: the empty embed.
    #[derive(RustEmbed)]
    #[folder = "../../fixtures/ui-dist-absent-by-design"]
    #[allow_missing = true]
    struct Missing;

    /// The not-built line, as the spec writes it ("Data", WA 8).
    const NOT_BUILT_LINE: &str = "this specengine-http holds no web UI (ui/dist had no \
                                  index.html when it was compiled): run pnpm --dir ui install \
                                  --frozen-lockfile && pnpm --dir ui build, then cargo install \
                                  --locked --force --path <clone>/crates/specengine-http, then \
                                  restart spec serve";

    /// The page's policy, as the spec writes it ("Data", WA 11).
    const PAGE_POLICY: &str = "default-src 'self'; img-src 'self' data:; object-src 'none'; \
                               base-uri 'none'; frame-ancestors 'none'";

    /// Headers no UI answer carries ("Data", WA 9).
    const NEVER: [&str; 7] = [
        "etag",
        "last-modified",
        "content-encoding",
        "accept-ranges",
        "access-control-allow-origin",
        "access-control-allow-methods",
        "access-control-allow-headers",
    ];

    fn fixture_dir(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    /// Every file under `dir`, keyed by its `/`-joined relative path.
    fn files_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in fs::read_dir(&at).expect("a fixture directory") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let key = path
                    .strip_prefix(dir)
                    .expect("under the fixture")
                    .components()
                    .map(|part| part.as_os_str().to_str().expect("a UTF-8 name"))
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(key, fs::read(&path).expect("a fixture file"));
            }
        }
        out
    }

    /// Status, headers and body bytes of a handler's answer.
    fn answered(response: Response) -> (StatusCode, HeaderMap, Vec<u8>) {
        let (parts, body) = response.into_parts();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime");
        let bytes = runtime
            .block_on(axum::body::to_bytes(body, usize::MAX))
            .expect("the body");
        (parts.status, parts.headers, bytes.to_vec())
    }

    fn header<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
        headers
            .get(name)
            .map(|value| value.to_str().expect("an ASCII header"))
    }

    /// The error body's message, the body exactly `{"status", "message"}`
    /// with the answer's status.
    fn message_of(status: StatusCode, body: &[u8]) -> String {
        let value: serde_json::Value = serde_json::from_slice(body).expect("a JSON error body");
        let object = value.as_object().expect("an object");
        assert_eq!(object.len(), 2, "two keys: {value}");
        assert_eq!(object["status"], status.as_u16(), "{value}");
        object["message"].as_str().expect("a text").to_owned()
    }

    fn uri(path: &str) -> Uri {
        path.parse()
            .unwrap_or_else(|failure| panic!("{path}: {failure}"))
    }

    /// The unknown-route 404 of `path`, `answer` being it.
    fn assert_unknown(answer: Response, path: &str) {
        let (status, headers, body) = answered(answer);
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            header(&headers, "cache-control"),
            Some("no-store"),
            "{path}"
        );
        let message = message_of(status, &body);
        let route = path.split_once('?').map_or(path, |(route, _)| route);
        assert!(
            message.starts_with(&format!("no route {route}: the routes are /api/projects ")),
            "{path}: {message}"
        );
        assert!(
            message.ends_with("; the web UI: / and /assets/<file>"),
            "{path}: {message}"
        );
    }

    /// The headers every UI 200 carries, and none it never does.
    fn assert_ui_headers(headers: &HeaderMap, wanted_type: &str, context: &str) {
        assert_eq!(
            header(headers, "content-type"),
            Some(wanted_type),
            "{context}"
        );
        assert_eq!(
            header(headers, "x-content-type-options"),
            Some("nosniff"),
            "{context}"
        );
        assert_eq!(
            header(headers, "cache-control"),
            Some("no-store"),
            "{context}"
        );
        for name in NEVER {
            assert_eq!(header(headers, name), None, "{context}: {name}");
        }
    }

    #[test]
    fn t2_the_fixture_is_built_and_embeds_every_file_of_its_folder_alone() {
        let on_disk = files_under(&fixture_dir("ui-dist"));
        for wanted in [
            "index.html",
            "robots.txt",
            "assets/app-Fx1a2B3c.js",
            "assets/app-Dd4e5F6g.css",
            "assets/mark-Hh7i8J9k.svg",
            "assets/blob-Kk0l1M2n.dat",
            "assets/.hidden.js",
            "assets/nested/deep.js",
        ] {
            assert!(on_disk.contains_key(wanted), "the fixture holds {wanted}");
        }
        let mut keys: Vec<String> = Fixture::iter().map(|key| key.into_owned()).collect();
        keys.sort();
        assert_eq!(
            keys,
            on_disk.keys().cloned().collect::<Vec<_>>(),
            "the embed's keys are the fixture's files, `/`-joined"
        );
        for (key, bytes) in &on_disk {
            let file = Fixture::get(key).unwrap_or_else(|| panic!("{key} embedded"));
            assert_eq!(file.data.as_ref(), bytes.as_slice(), "{key}: byte-equal");
        }
        assert!(built::<Fixture>(), "the page embedded: built");

        let pageless = files_under(&fixture_dir("ui-dist-pageless"));
        assert!(!pageless.contains_key("index.html"));
        assert!(Pageless::get("assets/app-Pp3q4R5s.js").is_some());
        assert!(!built::<Pageless>(), "assets without the page: not built");

        assert!(
            !fixture_dir("ui-dist-absent-by-design").exists(),
            "the folder that never exists"
        );
        assert_eq!(Missing::iter().count(), 0, "a missing folder: empty");
        assert!(!built::<Missing>(), "a missing folder: not built");
    }

    #[test]
    fn t2_slash_is_the_page_byte_equal_with_its_type_policy_and_nosniff() {
        let page = fs::read(fixture_dir("ui-dist").join("index.html")).expect("the page");
        let (status, headers, body) = answered(page_of::<Fixture>());
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, page, "the page's bytes");
        assert_ui_headers(&headers, "text/html; charset=utf-8", "/");
        assert_eq!(
            header(&headers, "content-security-policy"),
            Some(PAGE_POLICY),
            "the page's policy, exactly"
        );
        assert_eq!(
            headers.get_all("content-security-policy").iter().count(),
            1,
            "one policy"
        );
    }

    #[test]
    fn t2_each_asset_is_byte_equal_at_its_url_with_the_tables_type() {
        let on_disk = files_under(&fixture_dir("ui-dist"));
        for (name, wanted_type) in [
            ("app-Fx1a2B3c.js", "text/javascript; charset=utf-8"),
            ("app-Dd4e5F6g.css", "text/css; charset=utf-8"),
            ("mark-Hh7i8J9k.svg", "image/svg+xml"),
            ("blob-Kk0l1M2n.dat", "application/octet-stream"),
        ] {
            let bytes = &on_disk[&format!("assets/{name}")];
            for path in [format!("/assets/{name}"), format!("/assets/{name}?v=1&x")] {
                let (status, headers, body) = answered(asset_of::<Fixture>(&uri(&path)));
                assert_eq!(status, StatusCode::OK, "{path}");
                assert_eq!(&body, bytes, "{path}: byte-equal");
                assert_ui_headers(&headers, wanted_type, &path);
            }
        }
    }

    #[test]
    fn t2_a_dotfile_a_dot_segment_a_percent_a_slash_or_a_backslash_is_the_404() {
        // Embedded, so a 404 is the refusal, not a missing key.
        for key in ["assets/.hidden.js", "assets/nested/deep.js", "robots.txt"] {
            assert!(Fixture::get(key).is_some(), "{key} embedded");
        }
        for path in [
            "/assets/.hidden.js",
            "/assets/.",
            "/assets/..",
            "/assets/..%2Findex.html",
            "/assets/..%2frobots.txt",
            "/assets/%2e%2e%2findex.html",
            "/assets/%2E%2E",
            "/assets/app-Fx1a2B3c%2Ejs",
            "/assets/%61pp-Fx1a2B3c.js",
            "/assets/nested%2Fdeep.js",
            "/assets/nested%5Cdeep.js",
            "/assets/nested/deep.js",
            "/assets/../index.html",
            "/assets//index.html",
            "/assets/",
            "/assets",
            "/assets/index.html",
            "/assets/no-such-0000.js",
            "/index.html",
            "/robots.txt",
            "/",
        ] {
            assert_unknown(asset_of::<Fixture>(&uri(path)), path);
        }
        // A backslash: a URI may carry it raw; the name is refused before
        // any lookup (the embed's lookup would read it as a `/`).
        let backslash = "/assets/nested\\deep.js";
        assert_unknown(asset_of::<Fixture>(&uri(backslash)), backslash);
        for name in [
            "",
            ".hidden.js",
            "..",
            "nested/deep.js",
            "nested\\deep.js",
            "a%2Fb.js",
        ] {
            assert!(!plain(name), "{name:?} is refused");
        }
        for name in ["app-Fx1a2B3c.js", "blob-Kk0l1M2n.dat", "a..b.js", "a."] {
            assert!(plain(name), "{name:?} is looked up");
        }
    }

    #[test]
    fn t2_not_built_slash_is_the_503_line_and_every_asset_the_404() {
        for (label, answer) in [
            ("missing", page_of::<Missing>()),
            ("pageless", page_of::<Pageless>()),
        ] {
            let (status, headers, body) = answered(answer);
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{label}");
            assert_eq!(message_of(status, &body), NOT_BUILT_LINE, "{label}");
            assert_eq!(
                header(&headers, "content-type"),
                Some("application/json; charset=utf-8"),
                "{label}"
            );
            assert_eq!(
                header(&headers, "cache-control"),
                Some("no-store"),
                "{label}"
            );
            assert_eq!(header(&headers, "content-security-policy"), None, "{label}");
        }
        for path in ["/assets/app-Fx1a2B3c.js", "/assets/x.js"] {
            assert_unknown(asset_of::<Missing>(&uri(path)), path);
        }
        // Embedded, yet not served: no page, no assets.
        let path = "/assets/app-Pp3q4R5s.js";
        assert_unknown(asset_of::<Pageless>(&uri(path)), path);
    }

    #[test]
    fn t2_the_type_is_the_tables_by_the_last_extension_as_written() {
        for (name, wanted) in [
            ("index.html", "text/html; charset=utf-8"),
            ("a.js", "text/javascript; charset=utf-8"),
            ("a.min.js", "text/javascript; charset=utf-8"),
            ("a.mjs", "text/javascript; charset=utf-8"),
            ("a.css", "text/css; charset=utf-8"),
            ("a.svg", "image/svg+xml"),
            ("a.png", "image/png"),
            ("a.ico", "image/x-icon"),
            ("a.woff2", "font/woff2"),
            ("a.json", "application/json"),
            ("a.js.map", "application/json"),
            ("a.txt", "text/plain; charset=utf-8"),
            ("a.JS", "application/octet-stream"),
            ("a.Html", "application/octet-stream"),
            ("a.tar.gz", "application/octet-stream"),
            ("a.woff", "application/octet-stream"),
            ("a.wasm", "application/octet-stream"),
            ("a.dat", "application/octet-stream"),
            ("a.", "application/octet-stream"),
            ("plain", "application/octet-stream"),
            ("", "application/octet-stream"),
        ] {
            assert_eq!(type_of(name), wanted, "{name:?}");
        }
    }
}
