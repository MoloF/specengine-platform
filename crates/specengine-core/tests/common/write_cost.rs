//! docs/features/write-cost.md (RM-0125) WC1's catalogue, shared by the
//! core oracle (`tests/introduced_oracle.rs`) and the store's rows tests
//! (`crates/specengine-store/tests/introduced_rows.rs`, included by path):
//! for a corpus's documents, the edits a write may make, by family, each a
//! whole new text for a path (an existing document's, or a new file's).
//! Text only, std only: the families are stated in model terms (an ID, an
//! alias, a section, a link, a status, a `depends_on`, a `parent:`), never a
//! project's prefix, kind or directory (ADR-0008).

#![allow(dead_code)]

/// One document of a corpus: its root-relative path and its text.
#[derive(Debug, Clone)]
pub struct Doc {
    pub path: String,
    pub text: String,
}

/// One edit: its family, the path written and the new text.
#[derive(Debug, Clone)]
pub struct Edit {
    pub family: &'static str,
    pub path: String,
    pub text: String,
}

/// The families, each of which the catalogue gives where the corpus allows.
pub const FAMILIES: [&str; 20] = [
    "id-added",
    "id-removed",
    "id-defined-elsewhere",
    "alias",
    "section-id",
    "superseded-dated",
    "depends-closed",
    "depends-opened",
    "parent-closed",
    "parent-opened",
    "heading-renamed",
    "file-at-linked-path",
    "index-field",
    "new-slug",
    "front-matter-broken",
    "cap-passed",
    "process-rule",
    "create-in-directory",
    "mention",
    "file-link",
];

/// The lines between a text's opening `---` and its closing one, and the
/// byte where the body starts; `None` without front matter.
pub fn front(text: &str) -> Option<(&str, usize)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some((&rest[..=end], 4 + end + 5))
}

/// The document `id:` of a text.
pub fn own_id(text: &str) -> Option<String> {
    let (front, _) = front(text)?;
    front
        .lines()
        .find_map(|line| line.strip_prefix("id: "))
        .map(|id| id.trim().to_owned())
}

/// Every `{#ID}` a heading of `text` gives.
pub fn section_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.starts_with('#'))
        .filter_map(|line| line.split_once("{#"))
        .map(|(_, rest)| {
            rest.chars()
                .take_while(|c| !c.is_whitespace() && *c != '}')
                .collect::<String>()
        })
        .filter(|id| !id.is_empty())
        .collect()
}

/// Every ID `docs` give, with the path giving it, in path order.
pub fn ids(docs: &[Doc]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for doc in docs {
        if let Some(id) = own_id(&doc.text) {
            out.push((id, doc.path.clone()));
        }
        for id in section_ids(&doc.text) {
            out.push((id, doc.path.clone()));
        }
    }
    out
}

/// An ID of the same prefix as `like` that no document gives: a number of
/// the same width made of nines, else a name with `ZZPROBE`.
pub fn fresh_id(like: &str, taken: &[(String, String)]) -> String {
    let (prefix, rest) = like.rsplit_once('-').unwrap_or((like, ""));
    let candidate = if !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{prefix}-{}", "9".repeat(rest.len()))
    } else {
        format!("{prefix}-ZZPROBE")
    };
    if taken.iter().any(|(id, _)| *id == candidate) {
        format!("{candidate}X")
    } else {
        candidate
    }
}

/// `text` with `line` put as the first line of its front matter, or a front
/// matter of that line when it has none.
fn with_key(text: &str, line: &str) -> String {
    match text.strip_prefix("---\n") {
        Some(rest) => format!("---\n{line}\n{rest}"),
        None => format!("---\n{line}\n---\n\n{text}"),
    }
}

/// `text` with every front-matter line starting `key:` (and its indented
/// continuation) left out.
fn without_key(text: &str, key: &str) -> String {
    let Some((front_lines, body_at)) = front(text) else {
        return text.to_owned();
    };
    let mut kept = String::new();
    let mut dropping = false;
    for line in front_lines.split_inclusive('\n') {
        if !line.starts_with([' ', '\t', '-']) {
            dropping = line.starts_with(&format!("{key}:"));
        }
        if !dropping {
            kept.push_str(line);
        }
    }
    format!("---\n{kept}---\n{}", &text[body_at..])
}

/// `text` with `body` appended as its own paragraph.
fn appended(text: &str, body: &str) -> String {
    let mut out = text.to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(body);
    out
}

/// The directory of `path`, `""` at the root.
pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn joined(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// The edits of document `at` of `docs`, each family where it applies.
pub fn edits_of(docs: &[Doc], at: usize) -> Vec<Edit> {
    let doc = &docs[at];
    let text = doc.text.as_str();
    let taken = ids(docs);
    let own = own_id(text);
    let others: Vec<&(String, String)> =
        taken.iter().filter(|(_, path)| *path != doc.path).collect();
    let other = others.first().map(|(id, _)| id.clone());
    let like = own
        .clone()
        .or_else(|| taken.first().map(|(id, _)| id.clone()));
    let mut out = Vec::new();
    let mut push = |family: &'static str, path: &str, new: String| {
        if new != text || path != doc.path {
            out.push(Edit {
                family,
                path: path.to_owned(),
                text: new,
            });
        }
    };
    let here = doc.path.as_str();
    if let Some(like) = &like {
        let fresh = fresh_id(like, &taken);
        push(
            "id-added",
            here,
            appended(text, &format!("## Probe {{#{fresh}}}\n\nA new section.\n")),
        );
        if own.is_none() {
            push("id-added", here, with_key(text, &format!("id: {fresh}")));
        }
    }
    if let Some(own) = &own {
        push(
            "id-removed",
            here,
            text.replacen(&format!("id: {own}\n"), "", 1),
        );
    }
    if let Some(section) = section_ids(text).first() {
        push(
            "id-removed",
            here,
            text.replacen(&format!("{{#{section}"), "{#zz-plain", 1),
        );
    }
    if let Some(other) = &other {
        if let Some(own) = &own {
            push(
                "id-defined-elsewhere",
                here,
                text.replacen(&format!("id: {own}\n"), &format!("id: {other}\n"), 1),
            );
        }
        push(
            "alias",
            here,
            with_key(
                &without_key(text, "aliases"),
                &format!("aliases: [{other}]"),
            ),
        );
        push(
            "section-id",
            here,
            appended(text, &format!("## Again {{#{other}}}\n\nOnce more.\n")),
        );
        let dated = without_key(&without_key(text, "status"), "date");
        push(
            "superseded-dated",
            here,
            with_key(
                &with_key(&dated, &format!("status: superseded-by {other}")),
                "date: 2099-12-31",
            ),
        );
        push(
            "superseded-dated",
            here,
            with_key(
                &with_key(&dated, &format!("status: superseded-by {other}")),
                "date: 2001-01-01",
            ),
        );
        // A `depends_on` on every other document's ID: closes a cycle where
        // any of them depends on this one.
        let all: Vec<&str> = others.iter().map(|(id, _)| id.as_str()).take(12).collect();
        push(
            "depends-closed",
            here,
            with_key(
                &without_key(text, "links"),
                &format!("links:\n  depends_on: [{}]", all.join(", ")),
            ),
        );
        push(
            "parent-closed",
            here,
            with_key(&without_key(text, "parent"), &format!("parent: {other}")),
        );
        push(
            "mention",
            here,
            appended(
                text,
                &format!("As {other} says, and ZZ-404 and [[{other}]].\n"),
            ),
        );
    }
    if text.contains("depends_on") {
        let opened: String = text
            .split_inclusive('\n')
            .filter(|line| !line.trim_start().starts_with("depends_on"))
            .collect();
        push("depends-opened", here, opened);
    }
    push("parent-opened", here, without_key(text, "parent"));
    if let Some(heading) = text.lines().find(|line| line.starts_with("## ")) {
        push(
            "heading-renamed",
            here,
            text.replacen(heading, &format!("{heading} Renamed"), 1),
        );
    }
    if let Some(title) = text.lines().find(|line| line.starts_with("title:")) {
        push(
            "index-field",
            here,
            text.replacen(title, "title: A renamed title", 1),
        );
    }
    if let Some(status) = text.lines().find(|line| line.starts_with("status:")) {
        push(
            "index-field",
            here,
            text.replacen(status, "status: draft", 1),
        );
    }
    push(
        "front-matter-broken",
        here,
        match text.strip_prefix("---\n") {
            Some(rest) => format!("---\nbroken: [\n{rest}"),
            None => format!("---\nbroken: [\n---\n\n{text}"),
        },
    );
    push(
        "cap-passed",
        here,
        appended(
            text,
            &"A long line of words to pass a size cap. ".repeat(2_000),
        ),
    );
    push("process-rule", here, without_key(text, "class"));
    push(
        "process-rule",
        here,
        with_key(&without_key(text, "class"), "class: decision"),
    );
    let other_path = docs
        .iter()
        .find(|other| other.path != doc.path)
        .map(|other| other.path.clone());
    if let Some(other_path) = &other_path {
        let target = relative_from(dir_of(here), other_path);
        push(
            "file-link",
            here,
            appended(
                text,
                &format!("See [it]({target}), [there]({target}#nowhere) and [none](zz-none.md).\n"),
            ),
        );
    }
    out
}

/// `to` (root-relative) as a link from directory `from`.
pub fn relative_from(from: &str, to: &str) -> String {
    let from: Vec<&str> = from.split('/').filter(|part| !part.is_empty()).collect();
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<String> = vec!["..".to_owned(); from.len() - common];
    out.extend(to_parts[common..].iter().map(|part| (*part).to_owned()));
    out.join("/")
}

/// Every link destination ending in `.md` a text writes, as written.
fn md_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let end = rest.find([')', ' ', '\n']).unwrap_or(rest.len());
        let target = rest[..end].split('#').next().unwrap_or("");
        if target.ends_with(".md") && !target.contains("://") {
            out.push(target.to_owned());
        }
    }
    out
}

/// `link` written in a file of directory `dir`, as a root-relative path;
/// `None` when it leaves the root.
fn resolved(dir: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = if link.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|part| !part.is_empty()).collect()
    };
    for part in link.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// The corpus's new files: one at each path a document links to that holds
/// no file; a file named as another document is, in another directory; and
/// one in every directory, citing the first ID and a dangling one.
pub fn creates(docs: &[Doc]) -> Vec<Edit> {
    let taken = ids(docs);
    let first = taken.first().map(|(id, _)| id.clone());
    let exists = |path: &str| docs.iter().any(|doc| doc.path == path);
    let mut out = Vec::new();
    let mut linked = Vec::new();
    for doc in docs {
        for link in md_links(&doc.text) {
            if let Some(path) = resolved(dir_of(&doc.path), &link)
                && !exists(&path)
                && !linked.contains(&path)
            {
                linked.push(path);
            }
        }
    }
    for path in linked.into_iter().take(6) {
        out.push(Edit {
            family: "file-at-linked-path",
            path,
            text: "---\nclass: spec\nstatus: draft\n---\n\n# A linked file\n\n## Nowhere\n\nNow here.\n"
                .to_owned(),
        });
    }
    let mut dirs: Vec<&str> = docs.iter().map(|doc| dir_of(&doc.path)).collect();
    dirs.sort_unstable();
    dirs.dedup();
    for doc in docs.iter().take(4) {
        let name = doc.path.rsplit('/').next().unwrap_or(&doc.path);
        if let Some(dir) = dirs.iter().find(|dir| **dir != dir_of(&doc.path)) {
            let path = joined(dir, name);
            if !exists(&path) {
                out.push(Edit {
                    family: "new-slug",
                    path,
                    text: format!(
                        "---\nclass: spec\nstatus: draft\n---\n\n# {name}\n\nA namesake.\n"
                    ),
                });
            }
        }
    }
    for dir in dirs {
        let cites = first.as_deref().unwrap_or("ZZ-1");
        out.push(Edit {
            family: "create-in-directory",
            path: joined(dir, "zz-wc-create.md"),
            text: format!(
                "---\nclass: spec\nstatus: draft\n---\n\n# Made here\n\nCites {cites}, [[{cites}]] and ZZ-404.\n"
            ),
        });
        if let Some(id) = &first {
            out.push(Edit {
                family: "id-defined-elsewhere",
                path: joined(dir, "zz-wc-taken.md"),
                text: format!("---\nid: {id}\nclass: decision\n---\n\n# Taken\n\nCites {id}.\n"),
            });
        }
    }
    out
}

/// Every edit of the corpus: each document's, then its creates.
pub fn catalogue(docs: &[Doc]) -> Vec<Edit> {
    let mut out: Vec<Edit> = (0..docs.len()).flat_map(|at| edits_of(docs, at)).collect();
    out.extend(creates(docs));
    out
}

/// A seeded pick of `n` of `0..len`, in order (xorshift64*).
pub fn sample(len: usize, n: usize, seed: u64) -> Vec<usize> {
    let mut state = seed.max(1);
    let mut picked: Vec<usize> = Vec::new();
    while picked.len() < n.min(len) {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let value = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        let at = usize::try_from(value % len as u64).expect("small");
        if !picked.contains(&at) {
            picked.push(at);
        }
    }
    picked.sort_unstable();
    picked
}

// ------------------------------------------------------- WC2: generated

/// The generated corpora's config: `fixtures/spec-a`'s scheme (name and
/// number prefixes, an `aliases_from` legacy prefix, a feature-scoped one)
/// and its records table; the walk's default roots.
pub const GENERATED_CONFIG: &str = "[project]\nname = \"generated\"\nslug = \"generated\"\n\n\
[ids]\n\
RULE = { kind = \"rule\",        shape = \"name\" }\n\
EDGE = { kind = \"edge-case\",   shape = \"name\" }\n\
TERM = { kind = \"term\",        shape = \"name\" }\n\
R    = { kind = \"requirement\", width = 2 }\n\
Q    = { kind = \"question\",    width = 3, aliases_from = [\"QST\"] }\n\
DEC  = { kind = \"decision\",    width = 4 }\n\
AC   = { kind = \"criterion\",   width = 2, scope = \"feature\" }\n\n\
[paths]\nroots = [\"docs\"]\n";

/// A draw of the generator (xorshift64*).
pub struct Draw(u64);

impl Draw {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        usize::try_from(value % n.max(1) as u64).expect("small")
    }

    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

/// A generated corpus of `count` documents (20 to 200) for `seed`: every
/// reference form the check resolves, but `link_base` and an index with
/// shards (`[paths]` defaults, no `[[generators]]`): `id:`, `{#ID}`,
/// `aliases:`, `aliases_from` (`QST-` for `Q-`), a non-Latin legacy alias,
/// `slug/ID` and bare feature IDs, `#section` and the name fallback,
/// `[[…]]`, file links with and without anchors, path-form and
/// reference-form `canon:`, `depends_on`, `parent:`, `superseded-by` with
/// `date:`, Tier 3 (`docs/archive`) and `class: generated` documents, failed
/// front matter, CRLF; and a three-document `depends_on` chain and a
/// `parent:` chain the catalogue's `cycle` edits close (the review's shape:
/// the first member by name outside the set).
pub fn generated(seed: u64, count: usize) -> Vec<Doc> {
    let count = count.clamp(20, 200);
    let mut draw = Draw::new(seed);
    let rules = count / 2;
    let reqs = count / 6;
    let questions = count / 10;
    let mut docs = Vec::new();
    let rule = |n: usize| format!("RULE-G{n}");
    let req = |n: usize| format!("R-{:02}", n + 10);
    for n in 0..rules {
        let mut front = format!(
            "id: {}\nclass: canon\nstatus: accepted\ntitle: Rule {n}\n",
            rule(n)
        );
        if n > 0 && draw.chance(40) {
            front.push_str(&format!("parent: {}\n", rule(draw.below(n))));
        }
        if n > 0 && draw.chance(25) {
            front.push_str(&format!(
                "links:\n  depends_on: [{}]\n",
                rule(draw.below(n))
            ));
        }
        if draw.chance(10) {
            front.push_str(&format!("aliases: [TERM-a{n}]\n"));
        }
        let mut body =
            format!("# Rule {n}\n\nThe rule's own words.\n\n## Case {{#EDGE-G{n}}}\n\nAn edge.\n");
        for _ in 0..draw.below(4) {
            let other = draw.below(rules);
            let line = match draw.below(9) {
                0 => format!("As {} says.", rule(other)),
                1 => format!("See [[{}]].", rule(other)),
                2 => format!("See [[{}#EDGE-G{other}|the edge]].", rule(other)),
                3 => format!("See [the rule](n{other}.md)."),
                4 => format!("See [the case](n{other}.md#case)."),
                5 => format!("As {}#EDGE-G{other} says.", rule(other)),
                6 => format!("Under {}.", req(draw.below(reqs.max(1)))),
                7 => format!("Asked in QST-{:03}.", 100 + draw.below(questions.max(1))),
                _ => format!("See [[{}#Case]].", rule(other)),
            };
            body.push_str(&format!("\n{line}\n"));
        }
        let mut text = format!("---\n{front}---\n\n{body}");
        if draw.chance(10) {
            text = text.replace('\n', "\r\n");
        }
        docs.push(Doc {
            path: format!("docs/spec/n{n}.md"),
            text,
        });
    }
    for n in 0..reqs {
        let mut front = format!("id: {}\nclass: canon\ntitle: Requirement {n}\n", req(n));
        if n > 0 && draw.chance(30) {
            front.push_str(&format!(
                "status: superseded-by {}\ndate: 2026-0{}-1{}\n",
                req(draw.below(n)),
                1 + draw.below(9),
                draw.below(10)
            ));
        } else {
            front.push_str("status: accepted\n");
        }
        if draw.chance(20) {
            front.push_str("aliases: [TERM-\u{0442}\u{0440}\u{0435}\u{0431}]\n");
        }
        docs.push(Doc {
            path: format!("docs/records/R/{}.md", req(n)),
            text: format!(
                "---\n{front}---\n\n# Requirement {n}\n\nCovers {}.\n",
                rule(draw.below(rules))
            ),
        });
    }
    for n in 0..questions {
        docs.push(Doc {
            path: format!("docs/records/Q/Q-{:03}.md", 100 + n),
            text: format!(
                "---\nid: Q-{:03}\nclass: canon\nstatus: accepted\ntitle: Question {n}\nrefs: [QST-{:03}]\n---\n\n# Question {n}\n\nAbout {}.\n",
                100 + n,
                100 + draw.below(questions),
                rule(draw.below(rules))
            ),
        });
    }
    // Features: `{#AC-nn}` sections, cited bare inside and as `slug/ID`.
    let features = 2 + count / 40;
    for n in 0..features {
        docs.push(Doc {
            path: format!("docs/features/f{n}.md"),
            text: format!(
                "---\nclass: spec\nstatus: draft\nscope: [g]\nrefs: [{}]\n---\n\n# Feature {n}\n\nAs AC-01 and f{}/AC-02 say.\n\n## Criteria {{#acceptance}}\n\n### First {{#AC-01}}\n\nOne.\n\n### Second {{#AC-02}}\n\nTwo, after {}.\n",
                req(draw.below(reqs.max(1))),
                draw.below(features),
                rule(draw.below(rules))
            ),
        });
    }
    // Decisions: path-form and reference-form `canon:`.
    for n in 0..3 {
        let target = draw.below(rules);
        let canon = if n % 2 == 0 {
            format!("docs/spec/n{target}.md#case")
        } else {
            format!("{}#EDGE-G{target}", rule(target))
        };
        docs.push(Doc {
            path: format!("docs/records/DEC/DEC-{:04}.md", n + 1),
            text: format!(
                "---\nid: DEC-{:04}\nclass: decision\nstatus: accepted\ndate: 2026-08-0{}\ncanon: {canon}\n---\n\n# Decision {n}\n\nSo it is.\n",
                n + 1,
                n + 1
            ),
        });
    }
    // Tier 3, generated, failed front matter.
    docs.push(Doc {
        path: "docs/archive/old.md".to_owned(),
        text: format!(
            "---\nclass: spec\nstatus: superseded-by {}\ndate: 2026-01-01\n---\n\n# Old\n\nCites {} and ZZ-9.\n",
            req(0),
            rule(0)
        ),
    });
    docs.push(Doc {
        path: "docs/spec/made.md".to_owned(),
        text: format!(
            "---\nclass: generated\ngenerator: a script\n---\n\n# Made\n\nLists {}.\n",
            rule(1)
        ),
    });
    docs.push(Doc {
        path: "docs/spec/broken.md".to_owned(),
        text: "---\nid: RULE-BROKEN\ntitle: [unclosed\n---\n\n# Broken\n\nCites RULE-G0.\n"
            .to_owned(),
    });
    // The review's shape: a -> b -> c, by `depends_on` and by `parent:`.
    for (key, prefix) in [("depends_on", "RULE-CHAIND"), ("parent", "RULE-CHAINP")] {
        for (name, next) in [("A", Some("B")), ("B", Some("C")), ("C", None)] {
            let pointer = match (next, key) {
                (Some(next), "depends_on") => format!("links:\n  depends_on: [{prefix}{next}]\n"),
                (Some(next), _) => format!("parent: {prefix}{next}\n"),
                (None, _) => String::new(),
            };
            docs.push(Doc {
                path: format!("docs/spec/{}-{}.md", prefix.to_lowercase(), name.to_lowercase()),
                text: format!(
                    "---\nid: {prefix}{name}\nclass: canon\nstatus: accepted\ntitle: Chain {name}\n{pointer}---\n\n# Chain {name}\n\nA link of the chain.\n"
                ),
            });
        }
    }
    // The analyst's cycle cases, whole.
    docs.extend(cycle_cases().0);
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs
}

/// The review's edits over [`generated`]'s chains: `c` points back at `a`,
/// closing a cycle whose first member by name (`a`) reads `b`'s key, not
/// `c`'s, so it lies outside the set.
pub fn chain_closers(docs: &[Doc]) -> Vec<Edit> {
    let mut out = Vec::new();
    for (key, prefix) in [("depends_on", "RULE-CHAIND"), ("parent", "RULE-CHAINP")] {
        let path = format!("docs/spec/{}-c.md", prefix.to_lowercase());
        let Some(doc) = docs.iter().find(|doc| doc.path == path) else {
            continue;
        };
        let pointer = if key == "depends_on" {
            format!("links:\n  depends_on: [{prefix}A]\n")
        } else {
            format!("parent: {prefix}A\n")
        };
        out.push(Edit {
            family: if key == "depends_on" {
                "depends-closed"
            } else {
                "parent-closed"
            },
            path,
            text: doc
                .text
                .replacen("title: Chain C\n", &format!("title: Chain C\n{pointer}"), 1),
        });
    }
    // And the analyst's cycle cases' writes, where their documents are.
    if docs.iter().any(|doc| doc.path.starts_with("docs/cyc/")) {
        out.extend(cycle_cases().1);
    }
    out
}

// ------------------------------------------- cycles (the analyst's cases)

/// What a cycle edit's introduced findings must hold, beside the oracle: a
/// finding of `code` at `path` and `line` naming `subject` (a `line` of 0 or
/// an empty `subject` not checked), or (`present` false) no finding of
/// `code` at all.
#[derive(Debug, Clone)]
pub struct Expect {
    pub family: &'static str,
    pub code: &'static str,
    pub present: bool,
    pub path: &'static str,
    pub line: usize,
    pub subject: &'static str,
}

/// A document of the cycle corpora: `id` (none: `None`), `extra` front-matter
/// lines after `id`, `class: canon`, `status: accepted`, `title:` (so they
/// start on line 6), and a body.
fn node(path: &str, id: Option<&str>, extra: &str) -> Doc {
    let id_line = id.map(|id| format!("id: {id}\n")).unwrap_or_default();
    let title = id.unwrap_or(path);
    Doc {
        path: path.to_owned(),
        text: format!(
            "---\n{id_line}class: canon\nstatus: accepted\ntitle: {title}\n{extra}---\n\n# {title}\n\nA body.\n"
        ),
    }
}

fn depends(ids: &[&str]) -> String {
    format!("links:\n  depends_on: [{}]\n", ids.join(", "))
}

/// The cycle cases of the analyst's reading of `graph.rs` (`depends-cycle`:
/// strongly connected components over every holder, named and placed by the
/// first member by name; `parent-cycle`: placed by the first member in
/// (path, position) order; introduced by (code, path, subject)): their
/// documents, the writes, and what each write's introduced findings hold.
/// Every document lies under `docs/cyc/` (and one feature under
/// `docs/features/`), every ID `RULE-` (a name), so the cases sit in any
/// corpus of [`GENERATED_CONFIG`]'s scheme.
pub fn cycle_cases() -> (Vec<Doc>, Vec<Edit>, Vec<Expect>) {
    let mut docs = Vec::new();
    let mut edits = Vec::new();
    let mut expect = Vec::new();

    // A split: {X, Y, Z} with X->Y, Y->X, Y->Z, Z->X (Z's item an alias X
    // gives). P = W gains the ID itself, which a lookup takes before an
    // alias: Z->W. {X, Y} is introduced; it holds neither P nor Z, and Y is
    // outside the set (it reads neither W's keys nor the alias).
    docs.push(node(
        "docs/cyc/split/x.md",
        Some("RULE-SX"),
        &format!("aliases: [RULE-SK]\n{}", depends(&["RULE-SY"])),
    ));
    docs.push(node(
        "docs/cyc/split/y.md",
        Some("RULE-SY"),
        &depends(&["RULE-SX", "RULE-SZ"]),
    ));
    docs.push(node(
        "docs/cyc/split/z.md",
        Some("RULE-SZ"),
        &depends(&["RULE-SK"]),
    ));
    let w = node("docs/cyc/split/w.md", Some("RULE-SW"), "");
    edits.push(Edit {
        family: "cycle-split",
        path: w.path.clone(),
        text: format!("{}\n## Kept {{#RULE-SK}}\n\nNow here.\n", w.text),
    });
    docs.push(w);
    expect.push(Expect {
        family: "cycle-split",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/split/x.md",
        line: 8,
        subject: "RULE-SX, RULE-SY",
    });

    // A duplicate-ID swap: A(X) -> Y, held by B and C; B -> A through A's
    // alias K1, C -> K2 (dangling). P = A trades K1 for K2: the cycle {A, B}
    // becomes {A, C}, the same triple (A's path, "X, Y"): not introduced.
    let a = node(
        "docs/cyc/swap/a.md",
        Some("RULE-WX"),
        &format!("aliases: [RULE-WK1]\n{}", depends(&["RULE-WY"])),
    );
    docs.push(node(
        "docs/cyc/swap/b.md",
        Some("RULE-WY"),
        &depends(&["RULE-WK1"]),
    ));
    docs.push(node(
        "docs/cyc/swap/c.md",
        Some("RULE-WY"),
        &depends(&["RULE-WK2"]),
    ));
    edits.push(Edit {
        family: "cycle-swap",
        path: a.path.clone(),
        text: a
            .text
            .replacen("aliases: [RULE-WK1]", "aliases: [RULE-WK2]", 1),
    });
    docs.push(a);
    expect.push(Expect {
        family: "cycle-swap",
        code: "depends-cycle",
        present: false,
        path: "",
        line: 0,
        subject: "",
    });

    // A create closing a cycle through a `slug/ID` lookup that dangled: Q
    // depends on `fzwc/AC-01`, no such feature yet; the new feature gives
    // AC-01 and depends on Q. And the same create superseded (Q's reference
    // then reaches a superseded document).
    docs.push(node(
        "docs/cyc/slug/q.md",
        Some("RULE-FQ"),
        &depends(&["fzwc/AC-01"]),
    ));
    let feature = |status: &str| {
        format!(
            "---\nclass: spec\n{status}scope: [g]\nlinks:\n  depends_on: [RULE-FQ]\n---\n\n# A feature\n\n## Criteria {{#acceptance}}\n\n### One {{#AC-01}}\n\nOne.\n"
        )
    };
    edits.push(Edit {
        family: "cycle-slug",
        path: "docs/features/fzwc.md".to_owned(),
        text: feature("status: draft\n"),
    });
    edits.push(Edit {
        family: "cycle-slug-superseded",
        path: "docs/features/fzwc.md".to_owned(),
        text: feature("status: superseded-by RULE-FQ\ndate: 2026-02-02\n"),
    });
    expect.push(Expect {
        family: "cycle-slug",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/slug/q.md",
        line: 7,
        subject: "RULE-FQ, docs/features/fzwc.md",
    });

    // A new duplicate holder adding an edge to the file written: E -> Y
    // (held by B); the new D gives Y too and depends on E: {E, D}.
    docs.push(node(
        "docs/cyc/dup/e.md",
        Some("RULE-HE"),
        &depends(&["RULE-HY"]),
    ));
    docs.push(node("docs/cyc/dup/b.md", Some("RULE-HY"), ""));
    edits.push(Edit {
        family: "cycle-duplicate-holder",
        path: "docs/cyc/dup/d.md".to_owned(),
        text: node("docs/cyc/dup/d.md", Some("RULE-HY"), &depends(&["RULE-HE"])).text,
    });
    expect.push(Expect {
        family: "cycle-duplicate-holder",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/dup/e.md",
        line: 7,
        subject: "RULE-HE, RULE-HY",
    });

    // P's liveness flip. Breaking: {A, B, P} as is; P made generated (its
    // edges no longer count): {A, B} is introduced. Tier 3 by status alike.
    // Making: P generated as is, {A, B}; P made live: {A, B, P}.
    docs.push(node(
        "docs/cyc/live/a.md",
        Some("RULE-LA"),
        &depends(&["RULE-LB", "RULE-LP"]),
    ));
    docs.push(node(
        "docs/cyc/live/b.md",
        Some("RULE-LB"),
        &depends(&["RULE-LA"]),
    ));
    let p = node(
        "docs/cyc/live/p.md",
        Some("RULE-LP"),
        &depends(&["RULE-LA"]),
    );
    edits.push(Edit {
        family: "cycle-liveness-break",
        path: p.path.clone(),
        text: p.text.replacen(
            "class: canon\n",
            "class: generated\ngenerator: a script\n",
            1,
        ),
    });
    edits.push(Edit {
        family: "cycle-liveness-tier3",
        path: p.path.clone(),
        text: p.text.replacen(
            "status: accepted\n",
            "status: superseded-by RULE-LA\ndate: 2026-02-02\n",
            1,
        ),
    });
    docs.push(p);
    expect.push(Expect {
        family: "cycle-liveness-break",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/live/a.md",
        line: 7,
        subject: "RULE-LA, RULE-LB",
    });
    docs.push(node(
        "docs/cyc/make/a.md",
        Some("RULE-MA"),
        &depends(&["RULE-MB", "RULE-MP"]),
    ));
    docs.push(node(
        "docs/cyc/make/b.md",
        Some("RULE-MB"),
        &depends(&["RULE-MA"]),
    ));
    let made = node(
        "docs/cyc/make/p.md",
        Some("RULE-MP"),
        &depends(&["RULE-MA"]),
    );
    let generated_text = made.text.replacen(
        "class: canon\n",
        "class: generated\ngenerator: a script\n",
        1,
    );
    edits.push(Edit {
        family: "cycle-liveness-make",
        path: made.path.clone(),
        text: made.text.clone(),
    });
    docs.push(Doc {
        path: made.path,
        text: generated_text,
    });
    expect.push(Expect {
        family: "cycle-liveness-make",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/make/a.md",
        line: 7,
        subject: "RULE-MA, RULE-MB, RULE-MP",
    });

    // A parent cycle whose first member in (path, position) order (a.md,
    // RULE-PZ) is not the first by name (RULE-PA, b.md), and lies outside
    // the set: c.md, written, closes a -> b -> c -> a.
    docs.push(node(
        "docs/cyc/porder/a.md",
        Some("RULE-PZ"),
        "parent: RULE-PA\n",
    ));
    docs.push(node(
        "docs/cyc/porder/b.md",
        Some("RULE-PA"),
        "parent: RULE-PM\n",
    ));
    let c = node("docs/cyc/porder/c.md", Some("RULE-PM"), "");
    edits.push(Edit {
        family: "cycle-parent-order",
        path: c.path.clone(),
        text: c
            .text
            .replacen("title: RULE-PM\n", "title: RULE-PM\nparent: RULE-PZ\n", 1),
    });
    docs.push(c);
    expect.push(Expect {
        family: "cycle-parent-order",
        code: "parent-cycle",
        present: true,
        path: "docs/cyc/porder/a.md",
        line: 6,
        subject: "RULE-PA, RULE-PM, RULE-PZ",
    });

    // Longer cycles, the first member by name outside the set: five by
    // `depends_on`, four by `parent:`.
    for n in 1..=5 {
        let next = format!("RULE-LONG{}", n + 1);
        let extra = if n < 5 {
            depends(&[&next])
        } else {
            String::new()
        };
        let doc = node(
            &format!("docs/cyc/long/v{n}.md"),
            Some(&format!("RULE-LONG{n}")),
            &extra,
        );
        if n == 5 {
            edits.push(Edit {
                family: "cycle-long-depends",
                path: doc.path.clone(),
                text: doc.text.replacen(
                    "title: RULE-LONG5\n",
                    &format!("title: RULE-LONG5\n{}", depends(&["RULE-LONG1"])),
                    1,
                ),
            });
        }
        docs.push(doc);
    }
    expect.push(Expect {
        family: "cycle-long-depends",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/long/v1.md",
        line: 7,
        subject: "RULE-LONG1, RULE-LONG2, RULE-LONG3, RULE-LONG4, RULE-LONG5",
    });
    for n in 1..=4 {
        let extra = if n < 4 {
            format!("parent: RULE-LP{}\n", n + 1)
        } else {
            String::new()
        };
        let doc = node(
            &format!("docs/cyc/longp/u{n}.md"),
            Some(&format!("RULE-LP{n}")),
            &extra,
        );
        if n == 4 {
            edits.push(Edit {
                family: "cycle-long-parent",
                path: doc.path.clone(),
                text: doc.text.replacen(
                    "title: RULE-LP4\n",
                    "title: RULE-LP4\nparent: RULE-LP1\n",
                    1,
                ),
            });
        }
        docs.push(doc);
    }
    expect.push(Expect {
        family: "cycle-long-parent",
        code: "parent-cycle",
        present: true,
        path: "docs/cyc/longp/u1.md",
        line: 6,
        subject: "RULE-LP1, RULE-LP2, RULE-LP3, RULE-LP4",
    });
    // Hop 2: Y, an accepted decision dated 2026-03-01, cites X; X is
    // `superseded-by` S, which P gives, dated 2026-05-01, so Y is exempt
    // (it cited X while X was live). P's date moves to 2026-01-01: the
    // exemption flips and Y's `ref-superseded` is introduced. Y reads X's
    // key, not P's: only the second hop (X, a superseded reader of S, then
    // X's readers) brings Y into the set.
    docs.push(node("docs/cyc/hop2/x.md", Some("RULE-H2X"), ""));
    let last = docs.len() - 1;
    docs[last].text =
        docs[last]
            .text
            .replacen("status: accepted\n", "status: superseded-by RULE-H2S\n", 1);
    let successor = node("docs/cyc/hop2/s.md", Some("RULE-H2S"), "date: 2026-05-01\n");
    edits.push(Edit {
        family: "superseded-dated-hop2",
        path: successor.path.clone(),
        text: successor
            .text
            .replacen("date: 2026-05-01\n", "date: 2026-01-01\n", 1),
    });
    docs.push(successor);
    docs.push(Doc {
        path: "docs/cyc/hop2/y.md".to_owned(),
        text: "---\nid: DEC-0901\nclass: decision\nstatus: accepted\ndate: 2026-03-01\ntitle: A decision\nrefs: [RULE-H2X]\n---\n\n# A decision\n\nIt cites RULE-H2X.\n".to_owned(),
    });
    expect.push(Expect {
        family: "superseded-dated-hop2",
        code: "ref-superseded",
        present: true,
        path: "docs/cyc/hop2/y.md",
        line: 0,
        subject: "",
    });

    // A cycle shrunk where the patched side alone cannot see it: {A, B, C, D}
    // as is (A->B, B->C, C->B, C->D, D->A); P = C drops C->D, keeping C->B.
    // D (and A) are reached only through C's old edge: {B, C} is introduced.
    docs.push(node(
        "docs/cyc/shrink/a.md",
        Some("RULE-RA"),
        &depends(&["RULE-RB"]),
    ));
    docs.push(node(
        "docs/cyc/shrink/b.md",
        Some("RULE-RB"),
        &depends(&["RULE-RC"]),
    ));
    let shrunk = node(
        "docs/cyc/shrink/c.md",
        Some("RULE-RC"),
        &depends(&["RULE-RB", "RULE-RD"]),
    );
    edits.push(Edit {
        family: "cycle-region-shrink",
        path: shrunk.path.clone(),
        text: shrunk
            .text
            .replacen("depends_on: [RULE-RB, RULE-RD]", "depends_on: [RULE-RB]", 1),
    });
    docs.push(shrunk);
    docs.push(node(
        "docs/cyc/shrink/d.md",
        Some("RULE-RD"),
        &depends(&["RULE-RA"]),
    ));
    expect.push(Expect {
        family: "cycle-region-shrink",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/shrink/b.md",
        line: 7,
        subject: "RULE-RB, RULE-RC",
    });

    // One subject, two paths: a1 and a2 both give RULE-QX; B gives RULE-QY,
    // depends on RULE-QX (both holders), and has the alias a1's item names.
    // P = B trades that alias for the one a2's item names: {a1, B} becomes
    // {a2, B}, the same subject at another path: introduced.
    docs.push(node(
        "docs/cyc/same/a1.md",
        Some("RULE-QX"),
        &depends(&["RULE-QK1"]),
    ));
    docs.push(node(
        "docs/cyc/same/a2.md",
        Some("RULE-QX"),
        &depends(&["RULE-QK2"]),
    ));
    let b = node(
        "docs/cyc/same/b.md",
        Some("RULE-QY"),
        &format!("aliases: [RULE-QK1]\n{}", depends(&["RULE-QX"])),
    );
    edits.push(Edit {
        family: "cycle-same-subject",
        path: b.path.clone(),
        text: b
            .text
            .replacen("aliases: [RULE-QK1]", "aliases: [RULE-QK2]", 1),
    });
    docs.push(b);
    expect.push(Expect {
        family: "cycle-same-subject",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/same/a2.md",
        line: 7,
        subject: "RULE-QX, RULE-QY",
    });

    // A defined ID (no alias) that dangles today and a create defines: Q
    // depends on RULE-NEWX; the new X is `superseded-by` Z, so Q's reference
    // now reaches a superseded document (Q's `ref-superseded`). Only Q's miss
    // recorded on RULE-NEWX brings Q into the set. And the same shape closing
    // a cycle: Q2 depends on RULE-NEWY; the new Y depends on Q2.
    docs.push(node(
        "docs/cyc/newx/q.md",
        Some("RULE-NQ"),
        &depends(&["RULE-NEWX"]),
    ));
    docs.push(node("docs/cyc/newx/z.md", Some("RULE-NZ"), ""));
    let mut created = node(
        "docs/cyc/newx/x.md",
        Some("RULE-NEWX"),
        "date: 2026-02-02\n",
    );
    created.text =
        created
            .text
            .replacen("status: accepted\n", "status: superseded-by RULE-NZ\n", 1);
    edits.push(Edit {
        family: "defined-dangling-create",
        path: created.path.clone(),
        text: created.text,
    });
    expect.push(Expect {
        family: "defined-dangling-create",
        code: "ref-superseded",
        present: true,
        path: "docs/cyc/newx/q.md",
        line: 0,
        subject: "",
    });
    docs.push(node(
        "docs/cyc/newy/q.md",
        Some("RULE-NQ2"),
        &depends(&["RULE-NEWY"]),
    ));
    edits.push(Edit {
        family: "defined-dangling-cycle",
        path: "docs/cyc/newy/y.md".to_owned(),
        text: node(
            "docs/cyc/newy/y.md",
            Some("RULE-NEWY"),
            &depends(&["RULE-NQ2"]),
        )
        .text,
    });
    expect.push(Expect {
        family: "defined-dangling-cycle",
        code: "depends-cycle",
        present: true,
        path: "docs/cyc/newy/y.md",
        line: 7,
        subject: "RULE-NEWY, RULE-NQ2",
    });
    (docs, edits, expect)
}

/// The cycle cases' documents in a corpus of [`GENERATED_CONFIG`]'s scheme,
/// path order.
pub fn cycle_corpus() -> Vec<Doc> {
    let (mut docs, _, _) = cycle_cases();
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs
}

/// Whether `findings` (JSON, `ProposalFinding` or core `Finding` shape: `code`,
/// `path`, `line`, `subject`) meet `expect`: the problem, if not.
pub fn unmet(expect: &Expect, findings: &[(String, String, usize, String)]) -> Option<String> {
    let found = findings
        .iter()
        .filter(|(code, _, _, _)| code == expect.code)
        .collect::<Vec<_>>();
    if !expect.present {
        return (!found.is_empty()).then(|| {
            format!(
                "{}: no {} expected, got {found:?}",
                expect.family, expect.code
            )
        });
    }
    // A line of 0 or an empty subject is not checked.
    let hit = found.iter().any(|(_, path, line, subject)| {
        path == expect.path
            && (expect.line == 0 || *line == expect.line)
            && (expect.subject.is_empty() || subject == expect.subject)
    });
    (!hit).then(|| {
        format!(
            "{}: {} at {}:{} naming {:?} expected, got {found:?}",
            expect.family, expect.code, expect.path, expect.line, expect.subject
        )
    })
}
