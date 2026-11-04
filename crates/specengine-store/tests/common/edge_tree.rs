//! The seeded tree generator of AC-1 and ET-1 in
//! docs/features/edge-table.md "Acceptance criteria": the store's seeded
//! edits, written to disk, and core's key completeness, parsed in memory.
//! A tree is a config and 12 to 20 spec files drawn from a fixed pool of
//! paths; each edit names the listed case it exercises.
//!
//! It knows no SpecEngine type: a tree is its config text and its files'
//! bytes, so the store's tests (`common/mod.rs`) and core's
//! (`tests/edge_keys.rs`, through `#[path]`) share it. The config has `R`
//! project-scoped with the legacy prefix `RQ` (`aliases_from`), `AC`
//! feature-scoped, `TERM` with `shape = "name"`, roots `docs`, and the
//! three `[paths]` keys resolution reads (`features`, `link_base`,
//! `exclude`) each edited by a step.
//!
//! Non-Latin text is built from escapes only (ADR-0024).

use std::collections::{BTreeMap, BTreeSet};

/// A small deterministic generator (xorshift64*), as core's
/// `tests/check_rules.rs`.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % n.max(1)
    }

    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Every case AC-1 lists, as a step names it. A tally of a run counts the
/// cases its steps performed.
pub const CASES: [&str; 58] = [
    "add a file",
    "delete a file",
    "rename a file",
    "rename into features",
    "rename out of features",
    "an upper-case stem",
    "add an ID section",
    "remove an ID section",
    "add an id:",
    "remove an id:",
    "a duplicate ID",
    "add a Latin alias",
    "add a look-alike alias",
    "remove an alias",
    "mention bare",
    "mention slug/ID",
    "mention slug/ID#Y",
    "mention feature-scoped inside its feature",
    "mention feature-scoped outside its feature",
    "mention legacy",
    "mention name-fallback suffix",
    "mention wiki",
    "mention project:",
    "mention look-alike",
    "link relative",
    "link /-led",
    "link through link_base",
    "link #anchor",
    "link %20",
    "canon with an anchor",
    "canon without an anchor",
    "canon target canon",
    "canon target not canon",
    "parent plain",
    "parent scoped",
    "parent sectioned",
    "parent project:",
    "parent escaped",
    "status superseded-by",
    "a front-matter reference",
    "a labelled link",
    "remove a front-matter key",
    "a failed front-matter",
    "invalid UTF-8",
    "chmod 000",
    "repair a file",
    "edit prose",
    "remove a line",
    "edit [paths] features",
    "edit [paths] link_base",
    "edit [paths] exclude",
    "edit [ids]",
    "a scoped parent through its feature",
    "a section of a name-shaped ID",
    "a look-alike slug/ID through its alias",
    "a look-alike feature-scoped ID through its alias",
    "add the Latin twin alias",
    "remove the Latin twin alias",
];

/// The cases that write one line into one file (also the initial content).
const LINE_CASES: [&str; 15] = [
    "mention bare",
    "mention slug/ID",
    "mention slug/ID#Y",
    "mention feature-scoped inside its feature",
    "mention feature-scoped outside its feature",
    "mention legacy",
    "mention name-fallback suffix",
    "mention wiki",
    "mention project:",
    "mention look-alike",
    "link relative",
    "link /-led",
    "link through link_base",
    "link #anchor",
    "link %20",
];

/// The step of seed `seed` at position `step`: every other step walks
/// [`CASES`] in turn (ten seeds of 25 steps meet each case at least twice),
/// the others draw a case.
pub fn scheduled_case(seed: u64, step: usize, rng: &mut Rng) -> &'static str {
    if step.is_multiple_of(2) {
        let slot = seed as usize * 13 + step / 2;
        CASES[slot % CASES.len()]
    } else {
        rng.pick(&CASES)
    }
}

pub const CONFIG_PATH: &str = "specengine.toml";
pub const FEATURE_DIRS: [&str; 2] = ["docs/features", "docs/feat"];
pub const LINK_BASES: [Option<&str>; 3] = [None, Some("docs"), Some("docs/sub")];
const TOP: [&str; 10] = [
    "docs/r01.md",
    "docs/r02.md",
    "docs/r03.md",
    "docs/r04.md",
    "docs/r05.md",
    "docs/r06.md",
    "docs/r07.md",
    "docs/r08.md",
    "docs/r09.md",
    "docs/r10.md",
];
const SUB: [&str; 4] = [
    "docs/sub/s01.md",
    "docs/sub/s02.md",
    "docs/sub/s03.md",
    "docs/sub/two words.md",
];
const SKIP: [&str; 2] = ["docs/skip/k01.md", "docs/skip/k02.md"];
/// Feature slugs; `nowhere` is cited but never a file.
pub const SLUGS: [&str; 4] = ["stamina", "sprint", "glide", "ember"];
/// A stem under the features directory that is no slug.
pub const UPPER: &str = "Upper";
const MIN_FILES: usize = 12;
const MAX_FILES: usize = 20;

const DOC_IDS: [&str; 14] = [
    "R-01",
    "R-02",
    "R-03",
    "R-04",
    "R-05",
    "R-06",
    "R-07",
    "R-08",
    "R-09",
    "R-10",
    "R-11",
    "R-12",
    "TERM-glide",
    "TERM-ember",
];
const SECTION_IDS: [&str; 12] = [
    "R-20",
    "R-21",
    "R-22",
    "R-23",
    "R-24",
    "R-25",
    "R-26",
    "R-27",
    "R-28",
    "R-29",
    "TERM-ember",
    "TERM-glide",
];
const AC_IDS: [&str; 4] = ["AC-01", "AC-02", "AC-03", "AC-04"];
const LATIN_ALIASES: [&str; 5] = ["R-50", "R-51", "RQ-07", "RQ-08", TWIN];
/// The Latin twin of [`look_alike_ac`]: an `aliases:` entry of another
/// file that a one-file lookup's fallback from the ID to the written text
/// depends on.
pub const TWIN: &str = "AC-52";
const TITLES: [&str; 5] = ["Intro", "Notes", "Delay", "Rate", "Limits"];
const ANCHORS: [&str; 7] = [
    "intro",
    "notes",
    "delay",
    "AC-01",
    "R-21",
    "missing",
    "TERM-glide",
];

/// A feature-scoped ID written with a Cyrillic `A` (U+0410, read as Latin
/// `A`): the ID `AC-52`, the written text different; aliased so in a
/// feature document, it resolves there through the written text only while
/// no file has the Latin [`TWIN`] as an alias.
pub fn look_alike_ac() -> String {
    "\u{0410}C-52".to_owned()
}

/// The look-alike alias entries: a Cyrillic `T` in `TERM-x`, a Cyrillic `E`
/// in `TERM-ember` (both map to Latin, so each reads as a `TERM` ID).
pub fn look_alikes() -> [String; 2] {
    ["\u{0422}ERM-x".to_owned(), "T\u{0415}RM-ember".to_owned()]
}

/// The `[paths]` and `[ids]` of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// An index into [`FEATURE_DIRS`].
    pub features: usize,
    /// An index into [`LINK_BASES`].
    pub link_base: usize,
    /// `exclude = ["docs/skip/**"]`.
    pub exclude: bool,
    /// The `[ids]` edit: `Q` configured.
    pub extra_prefix: bool,
}

impl Config {
    pub fn features(&self) -> &'static str {
        FEATURE_DIRS[self.features]
    }

    pub fn link_base(&self) -> Option<&'static str> {
        LINK_BASES[self.link_base]
    }

    pub fn text(&self) -> String {
        let mut text = String::from("[paths]\nroots = [\"docs\"]\n");
        text.push_str(&format!("features = \"{}\"\n", self.features()));
        if let Some(base) = self.link_base() {
            text.push_str(&format!("link_base = \"{base}\"\n"));
        }
        if self.exclude {
            text.push_str("exclude = [\"docs/skip/**\"]\n");
        }
        text.push_str("\n[ids]\n");
        text.push_str("R = { kind = \"requirement\", width = 2, aliases_from = [\"RQ\"] }\n");
        text.push_str("AC = { kind = \"criterion\", width = 2, scope = \"feature\" }\n");
        text.push_str("TERM = { kind = \"term\", shape = \"name\" }\n");
        if self.extra_prefix {
            text.push_str("Q = { kind = \"question\", width = 2 }\n");
        }
        text
    }
}

/// One heading and its paragraphs.
#[derive(Debug, Clone, Default)]
pub struct Section {
    pub level: usize,
    pub title: &'static str,
    pub id: Option<String>,
    pub lines: Vec<String>,
}

/// One file, as a structure rendered to bytes.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub front: bool,
    pub id: Option<String>,
    pub class: Option<&'static str>,
    pub aliases: Vec<String>,
    /// `parent:`'s value as written.
    pub parent: Option<String>,
    pub status: Option<String>,
    pub canon: Option<String>,
    pub refs: Vec<String>,
    pub depends_on: Vec<String>,
    /// `follows:` items as written (a reference or a `{to, kind, when}`).
    pub follows: Vec<String>,
    /// An unclosed flow sequence in the front-matter.
    pub broken_front: bool,
    /// Bytes that are no UTF-8 at the end.
    pub not_utf8: bool,
    /// Mode 000 on disk; unreadable.
    pub locked: bool,
    pub title: String,
    pub body: Vec<String>,
    pub sections: Vec<Section>,
}

impl Doc {
    pub fn bytes(&self) -> Vec<u8> {
        let mut text = String::new();
        if self.front || self.broken_front {
            text.push_str("---\n");
            if let Some(id) = &self.id {
                text.push_str(&format!("id: {id}\n"));
            }
            if let Some(class) = self.class {
                text.push_str(&format!("class: {class}\n"));
            }
            if !self.aliases.is_empty() {
                let quoted: Vec<String> = self
                    .aliases
                    .iter()
                    .map(|alias| format!("\"{alias}\""))
                    .collect();
                text.push_str(&format!("aliases: [{}]\n", quoted.join(", ")));
            }
            if let Some(parent) = &self.parent {
                text.push_str(&format!("parent: {parent}\n"));
            }
            if let Some(status) = &self.status {
                text.push_str(&format!("status: {status}\n"));
            }
            if let Some(canon) = &self.canon {
                text.push_str(&format!("canon: {canon}\n"));
            }
            if !self.refs.is_empty() {
                text.push_str(&format!("refs: [{}]\n", self.refs.join(", ")));
            }
            if !self.depends_on.is_empty() || !self.follows.is_empty() {
                text.push_str("links:\n");
                if !self.depends_on.is_empty() {
                    text.push_str(&format!("  depends_on: [{}]\n", self.depends_on.join(", ")));
                }
                if !self.follows.is_empty() {
                    text.push_str("  follows:\n");
                    for item in &self.follows {
                        text.push_str(&format!("    - {item}\n"));
                    }
                }
            }
            if self.broken_front {
                text.push_str("broken: [unclosed\n");
            }
            text.push_str("---\n\n");
        }
        text.push_str(&format!("# {}\n\n", self.title));
        for line in &self.body {
            text.push_str(line);
            text.push_str("\n\n");
        }
        for section in &self.sections {
            text.push_str(&"#".repeat(section.level));
            text.push(' ');
            text.push_str(section.title);
            if let Some(id) = &section.id {
                text.push_str(&format!(" {{#{id}}}"));
            }
            text.push_str("\n\n");
            for line in &section.lines {
                text.push_str(line);
                text.push_str("\n\n");
            }
        }
        let mut bytes = text.into_bytes();
        if self.not_utf8 {
            bytes.extend_from_slice(b"Broken \xFF\xFE bytes.\n");
        }
        bytes
    }

    fn damaged(&self) -> bool {
        self.broken_front || self.not_utf8 || self.locked
    }

    fn line_count(&self) -> usize {
        self.body.len()
            + self
                .sections
                .iter()
                .map(|section| section.lines.len())
                .sum::<usize>()
    }

    fn has_key(&self) -> bool {
        self.parent.is_some()
            || self.status.is_some()
            || self.canon.is_some()
            || self.class.is_some()
            || !self.refs.is_empty()
            || !self.depends_on.is_empty()
            || !self.follows.is_empty()
    }

    /// Puts `line` in the body or in one of the sections.
    fn push_line(&mut self, rng: &mut Rng, line: String) {
        if self.sections.is_empty() || rng.chance(35) {
            self.body.push(line);
        } else {
            let at = rng.below(self.sections.len());
            self.sections[at].lines.push(line);
        }
    }
}

/// What one step did: the cases performed and the paths it touched.
#[derive(Debug, Clone, Default)]
pub struct Change {
    pub cases: Vec<&'static str>,
    /// Paths whose bytes (or mode) are written now.
    pub written: BTreeSet<String>,
    /// Paths deleted.
    pub removed: BTreeSet<String>,
    /// A rename keeping the bytes, `(from, to)`.
    pub renamed: Option<(String, String)>,
    /// `specengine.toml` is written.
    pub config: bool,
}

impl Change {
    /// Every path the step touched, `specengine.toml` included: what an
    /// `update_paths` names.
    pub fn paths(&self) -> Vec<String> {
        let mut paths: BTreeSet<String> = self.written.union(&self.removed).cloned().collect();
        if let Some((from, to)) = &self.renamed {
            paths.insert(from.clone());
            paths.insert(to.clone());
        }
        if self.config {
            paths.insert(CONFIG_PATH.to_owned());
        }
        paths.into_iter().collect()
    }
}

/// A tree: its config and its files.
#[derive(Debug, Clone)]
pub struct Tree {
    pub config: Config,
    pub files: BTreeMap<String, Doc>,
}

/// The directory of a root-relative path (`""` at the root).
pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `to` as a link written in a file of `from_dir`: `../` up to the common
/// directory, then down; a space as `%20`.
pub fn relative_link(from_dir: &str, to: &str) -> String {
    let from: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };
    let parts: Vec<&str> = to.split('/').collect();
    let (to_dir, name) = parts.split_at(parts.len() - 1);
    let common = from
        .iter()
        .zip(to_dir.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<&str> = vec![".."; from.len() - common];
    out.extend_from_slice(&to_dir[common..]);
    out.push(name[0]);
    out.join("/").replace(' ', "%20")
}

fn pool() -> Vec<String> {
    let mut pool: Vec<String> = TOP
        .iter()
        .chain(SUB.iter())
        .map(|p| (*p).to_owned())
        .collect();
    for dir in FEATURE_DIRS {
        for slug in SLUGS {
            pool.push(format!("{dir}/{slug}.md"));
        }
        pool.push(format!("{dir}/{UPPER}.md"));
    }
    pool.extend(SKIP.iter().map(|p| (*p).to_owned()));
    pool
}

fn any_r(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => format!("R-{:02}", 1 + rng.below(12)),
        1 => format!("R-{}", 20 + rng.below(10)),
        2 => (*rng.pick(&["R-50", "R-51", "R-52", "R-77"])).to_owned(),
        _ => (*rng.pick(&DOC_IDS)).to_owned(),
    }
}

fn any_slug(rng: &mut Rng) -> &'static str {
    if rng.chance(15) {
        "nowhere"
    } else {
        rng.pick(&SLUGS)
    }
}

fn random_parent(rng: &mut Rng) -> String {
    match rng.below(5) {
        0 => (*rng.pick(&DOC_IDS)).to_owned(),
        1 => format!("{}/{}", any_slug(rng), rng.pick(&AC_IDS)),
        2 => format!("{}#{}", rng.pick(&DOC_IDS), rng.pick(&SECTION_IDS)),
        3 => format!("other:{}", rng.pick(&DOC_IDS)),
        _ => format!("\"{}\\/{}\"", any_slug(rng), rng.pick(&AC_IDS)),
    }
}

fn front_reference(rng: &mut Rng) -> String {
    match rng.below(5) {
        0 => format!("{}/{}", any_slug(rng), rng.pick(&AC_IDS)),
        1 => format!("RQ-{:02}", 1 + rng.below(12)),
        _ => any_r(rng),
    }
}

fn flow_item(rng: &mut Rng) -> String {
    let to = any_r(rng);
    match rng.below(4) {
        0 => to,
        1 => format!("{{to: {to}, kind: repeat, when: \"rounds remain\"}}"),
        2 => format!("{{to: {to}, when: \"the runner arrives\"}}"),
        _ => format!("{{to: {to}, kind: alternative}}"),
    }
}

impl Tree {
    /// A seeded tree of 12 to 20 files, two of them feature documents.
    pub fn generate(rng: &mut Rng) -> Self {
        let config = Config {
            features: 0,
            link_base: rng.below(LINK_BASES.len()),
            exclude: rng.chance(50),
            extra_prefix: false,
        };
        let mut tree = Tree {
            config,
            files: BTreeMap::new(),
        };
        let count = MIN_FILES + rng.below(MAX_FILES - MIN_FILES + 1);
        let first = rng.below(SLUGS.len());
        for slug in [SLUGS[first], SLUGS[(first + 1) % SLUGS.len()]] {
            let path = format!("{}/{slug}.md", tree.config.features());
            let doc = random_doc(rng, &path, true);
            tree.files.insert(path, doc);
        }
        while tree.files.len() < count {
            let free = tree.free_paths(|_| true);
            let path = rng.pick(&free).clone();
            let feature = tree.is_feature(&path);
            let doc = random_doc(rng, &path, feature);
            tree.files.insert(path, doc);
        }
        let paths: Vec<String> = tree.files.keys().cloned().collect();
        for path in paths {
            for _ in 0..1 + rng.below(4) {
                let case = *rng.pick(&LINE_CASES);
                let line = tree.line_for(rng, case, &path);
                let doc = tree.files.get_mut(&path).expect("generated path");
                doc.push_line(rng, line);
            }
        }
        tree
    }

    /// Every file as `(path, bytes)`, `None` for a mode-000 one, in path
    /// order (walked or not: the caller applies the walk).
    pub fn contents(&self) -> Vec<(String, Option<Vec<u8>>)> {
        self.files
            .iter()
            .map(|(path, doc)| (path.clone(), (!doc.locked).then(|| doc.bytes())))
            .collect()
    }

    /// `path` is a feature document under the current config: directly in
    /// the features directory, its stem a slug.
    pub fn is_feature(&self, path: &str) -> bool {
        dir_of(path) == self.config.features()
            && SLUGS
                .iter()
                .any(|slug| path.ends_with(&format!("/{slug}.md")))
    }

    fn in_features_dir(&self, path: &str) -> bool {
        dir_of(path) == self.config.features()
    }

    fn free_paths(&self, keep: impl Fn(&str) -> bool) -> Vec<String> {
        pool()
            .into_iter()
            .filter(|path| !self.files.contains_key(path) && keep(path))
            .collect()
    }

    fn paths_where(&self, keep: impl Fn(&str, &Doc) -> bool) -> Vec<String> {
        self.files
            .iter()
            .filter(|(path, doc)| keep(path, doc))
            .map(|(path, _)| path.clone())
            .collect()
    }

    fn any_path(&self, rng: &mut Rng) -> String {
        let paths: Vec<String> = self.files.keys().cloned().collect();
        rng.pick(&paths).clone()
    }

    /// A target path for a link or `canon:`: mostly a file of the tree,
    /// sometimes a free pool path (dangling).
    fn target(&self, rng: &mut Rng) -> String {
        if rng.chance(85) {
            self.any_path(rng)
        } else {
            let free = self.free_paths(|_| true);
            rng.pick(&free).clone()
        }
    }

    /// One line of `case` for the file `from`.
    fn line_for(&self, rng: &mut Rng, case: &'static str, from: &str) -> String {
        let dir = dir_of(from);
        match case {
            "mention bare" => {
                let extra = if rng.chance(20) {
                    "Q-01".to_owned()
                } else {
                    any_r(rng)
                };
                format!("Needs {} and {extra}.", any_r(rng))
            }
            "mention slug/ID" => format!("Per {}/{}.", any_slug(rng), rng.pick(&AC_IDS)),
            "mention slug/ID#Y" => format!(
                "Per {}/{}#{}.",
                any_slug(rng),
                rng.pick(&AC_IDS),
                rng.pick(&AC_IDS)
            ),
            "mention feature-scoped inside its feature"
            | "mention feature-scoped outside its feature" => {
                let own: Vec<String> = self
                    .files
                    .get(from)
                    .map(|doc| {
                        doc.sections
                            .iter()
                            .filter_map(|section| section.id.clone())
                            .filter(|id| id.starts_with("AC-"))
                            .collect()
                    })
                    .unwrap_or_default();
                let id = if !own.is_empty() && rng.chance(70) {
                    rng.pick(&own).clone()
                } else {
                    (*rng.pick(&AC_IDS)).to_owned()
                };
                format!("Own {id} here.")
            }
            "mention legacy" => {
                let id = if rng.chance(50) {
                    (*rng.pick(&["RQ-07", "RQ-08"])).to_owned()
                } else {
                    format!("RQ-{:02}", 1 + rng.below(12))
                };
                format!("Old {id} still.")
            }
            "mention name-fallback suffix" => format!(
                "Term {} used.",
                rng.pick(&["TERM-glide-fast", "TERM-ember-x-y", "TERM-nothing-z"])
            ),
            "mention wiki" => match rng.below(4) {
                0 => format!("Wiki [[{}]] here.", any_r(rng)),
                1 => format!("Wiki [[{}|the label]] here.", any_r(rng)),
                2 => format!("Wiki [[{}/{}]] here.", any_slug(rng), rng.pick(&AC_IDS)),
                _ => format!("Wiki [[{}]] here.", rng.pick(&look_alikes())),
            },
            "mention project:" => format!("From other:{} too.", rng.pick(&DOC_IDS)),
            "mention look-alike" => {
                let id = match rng.below(4) {
                    0 => "\u{0422}ERM-glide".to_owned(),
                    1 => format!("{}/{}", any_slug(rng), look_alike_ac()),
                    _ => rng.pick(&look_alikes()).clone(),
                };
                format!("Odd {id} spelled.")
            }
            "link relative" => {
                let target = self.target(rng);
                format!("See [rel]({}).", relative_link(dir, &target))
            }
            "link /-led" => {
                let target = self.target(rng);
                format!("See [root](/{}).", target.replace(' ', "%20"))
            }
            "link through link_base" => {
                let base = self.config.link_base().unwrap_or("docs");
                let under: Vec<String> = self
                    .files
                    .keys()
                    .filter(|path| dir_of(path) == base)
                    .cloned()
                    .collect();
                let target = if under.is_empty() {
                    format!("{base}/r01.md")
                } else {
                    rng.pick(&under).clone()
                };
                let name = &target[base.len() + 1..];
                format!("See [base]({}).", name.replace(' ', "%20"))
            }
            "link #anchor" => {
                let anchor = rng.pick(&ANCHORS);
                if rng.chance(25) {
                    format!("See [own](#{anchor}).")
                } else {
                    let target = self.target(rng);
                    format!("See [anchor]({}#{anchor}).", relative_link(dir, &target))
                }
            }
            "link %20" => format!(
                "See [space]({}).",
                relative_link(dir, "docs/sub/two words.md")
            ),
            other => panic!("no line case {other}"),
        }
    }

    /// Applies `case`; the change names what it did.
    pub fn step(&mut self, rng: &mut Rng, case: &'static str) -> Change {
        let mut change = Change::default();
        self.apply(rng, case, &mut change);
        change
    }

    fn apply(&mut self, rng: &mut Rng, case: &'static str, change: &mut Change) {
        match case {
            "add a file" => {
                if self.files.len() >= MAX_FILES {
                    self.apply(rng, "delete a file", change);
                }
                let free = self.free_paths(|_| true);
                let path = rng.pick(&free).clone();
                let feature = self.is_feature(&path);
                let doc = random_doc(rng, &path, feature);
                self.files.insert(path.clone(), doc);
                for _ in 0..1 + rng.below(3) {
                    let line_case = *rng.pick(&LINE_CASES);
                    let line = self.line_for(rng, line_case, &path);
                    self.files
                        .get_mut(&path)
                        .expect("added")
                        .push_line(rng, line);
                }
                change.written.insert(path);
                change.cases.push(case);
            }
            "delete a file" => {
                if self.files.len() <= MIN_FILES {
                    self.apply(rng, "add a file", change);
                }
                let candidates = self.paths_where(|path, _| !change.written.contains(path));
                let path = rng.pick(&candidates).clone();
                self.files.remove(&path);
                change.removed.insert(path);
                change.cases.push(case);
            }
            "rename a file" => {
                let from = self.any_path(rng);
                let free = self.free_paths(|_| true);
                let to = rng.pick(&free).clone();
                self.rename(from, to, change, case);
            }
            "rename into features" => {
                let outside = self.paths_where(|path, _| !self.in_features_dir(path));
                let features = self.config.features();
                let free = self.free_paths(|path| {
                    dir_of(path) == features && !path.ends_with(&format!("/{UPPER}.md"))
                });
                if outside.is_empty() || free.is_empty() {
                    return self.apply(rng, "rename a file", change);
                }
                let from = rng.pick(&outside).clone();
                let to = rng.pick(&free).clone();
                self.rename(from, to, change, case);
            }
            "rename out of features" => {
                let inside = self.paths_where(|path, _| self.in_features_dir(path));
                if inside.is_empty() {
                    return self.apply(rng, "rename into features", change);
                }
                let free = self.free_paths(|path| TOP.contains(&path) || SUB.contains(&path));
                if free.is_empty() {
                    return self.apply(rng, "rename a file", change);
                }
                let from = rng.pick(&inside).clone();
                let to = rng.pick(&free).clone();
                self.rename(from, to, change, case);
            }
            "an upper-case stem" => {
                let path = format!("{}/{UPPER}.md", self.config.features());
                if self.files.contains_key(&path) {
                    let line = format!("Upper words {}.", rng.below(1000));
                    let doc = self.files.get_mut(&path).expect("present");
                    doc.push_line(rng, line);
                    change.written.insert(path);
                    change.cases.push(case);
                } else if rng.chance(50) || self.files.len() >= MAX_FILES {
                    let from = self.any_path(rng);
                    self.rename(from, path, change, case);
                } else {
                    let mut doc = random_doc(rng, &path, false);
                    doc.body
                        .push(format!("Per {}/{}.", any_slug(rng), rng.pick(&AC_IDS)));
                    self.files.insert(path.clone(), doc);
                    change.written.insert(path);
                    change.cases.push(case);
                }
            }
            "add an ID section" => {
                let path = self.any_path(rng);
                let feature = self.is_feature(&path);
                let id = if feature && rng.chance(60) || rng.chance(10) {
                    (*rng.pick(&AC_IDS)).to_owned()
                } else {
                    (*rng.pick(&SECTION_IDS)).to_owned()
                };
                let section = Section {
                    level: 2 + rng.below(2),
                    title: rng.pick(&TITLES),
                    id: Some(id),
                    lines: vec![format!("Section words {}.", rng.below(1000))],
                };
                self.edit(&path, change, case, |doc| doc.sections.push(section));
            }
            "remove an ID section" => {
                let with = self.paths_where(|_, doc| doc.sections.iter().any(|s| s.id.is_some()));
                if with.is_empty() {
                    return self.apply(rng, "add an ID section", change);
                }
                let path = rng.pick(&with).clone();
                let drop_whole = rng.chance(50);
                let at = {
                    let doc = &self.files[&path];
                    let ids: Vec<usize> = (0..doc.sections.len())
                        .filter(|&at| doc.sections[at].id.is_some())
                        .collect();
                    *rng.pick(&ids)
                };
                self.edit(&path, change, case, |doc| {
                    if drop_whole {
                        doc.sections.remove(at);
                    } else {
                        doc.sections[at].id = None;
                    }
                });
            }
            "add an id:" => {
                let without = self.paths_where(|_, doc| doc.id.is_none());
                if without.is_empty() {
                    return self.apply(rng, "a duplicate ID", change);
                }
                let path = rng.pick(&without).clone();
                let id = (*rng.pick(&DOC_IDS)).to_owned();
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.id = Some(id);
                });
            }
            "remove an id:" => {
                let with = self.paths_where(|_, doc| doc.id.is_some());
                if with.is_empty() {
                    return self.apply(rng, "add an id:", change);
                }
                let path = rng.pick(&with).clone();
                self.edit(&path, change, case, |doc| doc.id = None);
            }
            "a duplicate ID" => {
                let first = self.any_path(rng);
                let second = self.any_path(rng);
                if first == second {
                    return self.apply(rng, "add an ID section", change);
                }
                let id = match &self.files[&first].id {
                    Some(id) => id.clone(),
                    None => (*rng.pick(&DOC_IDS)).to_owned(),
                };
                if self.files[&first].id.is_none() {
                    let copy = id.clone();
                    self.edit(&first, change, case, |doc| {
                        doc.front = true;
                        doc.id = Some(copy);
                    });
                }
                if rng.chance(50) {
                    self.edit(&second, change, case, |doc| {
                        doc.front = true;
                        doc.id = Some(id);
                    });
                } else {
                    let title = *rng.pick(&TITLES);
                    self.edit(&second, change, case, |doc| {
                        doc.sections.push(Section {
                            level: 2,
                            title,
                            id: Some(id),
                            lines: Vec::new(),
                        });
                    });
                }
            }
            "add a Latin alias" | "add a look-alike alias" => {
                let path = self.any_path(rng);
                let alias = if case == "add a Latin alias" {
                    (*rng.pick(&LATIN_ALIASES)).to_owned()
                } else {
                    rng.pick(&look_alikes()).clone()
                };
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    if !doc.aliases.contains(&alias) {
                        doc.aliases.push(alias);
                    }
                });
            }
            "remove an alias" => {
                let with = self.paths_where(|_, doc| !doc.aliases.is_empty());
                if with.is_empty() {
                    return self.apply(rng, "add a Latin alias", change);
                }
                let path = rng.pick(&with).clone();
                let at = rng.below(self.files[&path].aliases.len());
                self.edit(&path, change, case, |doc| {
                    doc.aliases.remove(at);
                });
            }
            "mention feature-scoped inside its feature" => {
                let features = self.paths_where(|path, _| self.is_feature(path));
                if features.is_empty() {
                    return self.apply(rng, "mention feature-scoped outside its feature", change);
                }
                let path = rng.pick(&features).clone();
                self.add_line(rng, case, path, change);
            }
            "mention feature-scoped outside its feature" => {
                let others = self.paths_where(|path, _| !self.is_feature(path));
                let path = rng.pick(&others).clone();
                self.add_line(rng, case, path, change);
            }
            "link through link_base" => {
                let base = self.config.link_base().unwrap_or("docs");
                let away = self.paths_where(|path, _| dir_of(path) != base);
                let path = if away.is_empty() {
                    self.any_path(rng)
                } else {
                    rng.pick(&away).clone()
                };
                self.add_line(rng, case, path, change);
            }
            case if LINE_CASES.contains(&case) => {
                let path = self.any_path(rng);
                self.add_line(rng, case, path, change);
            }
            "canon with an anchor" | "canon without an anchor" => {
                let path = self.any_path(rng);
                let target = self.target(rng).replace("two words", "s01");
                let canon = if case == "canon with an anchor" {
                    format!("{target}#{}", rng.pick(&ANCHORS))
                } else if rng.chance(80) {
                    target
                } else {
                    any_r(rng)
                };
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.canon = Some(canon);
                });
            }
            "canon target canon" => {
                let named: Vec<String> = self
                    .files
                    .values()
                    .filter_map(|doc| doc.canon.as_deref())
                    .map(|canon| canon.split('#').next().unwrap_or(canon).to_owned())
                    .filter(|path| self.files.contains_key(path))
                    .collect();
                let path = if named.is_empty() {
                    self.any_path(rng)
                } else {
                    rng.pick(&named).clone()
                };
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.class = Some("canon");
                });
            }
            "canon target not canon" => {
                let canon = self.paths_where(|_, doc| doc.class == Some("canon"));
                let path = if canon.is_empty() {
                    self.any_path(rng)
                } else {
                    rng.pick(&canon).clone()
                };
                let class = if rng.chance(50) { None } else { Some("spec") };
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.class = class;
                });
            }
            "parent plain" | "parent scoped" | "parent sectioned" | "parent project:"
            | "parent escaped" => {
                let path = self.any_path(rng);
                let parent = match case {
                    "parent plain" => (*rng.pick(&DOC_IDS)).to_owned(),
                    "parent scoped" => format!("{}/{}", any_slug(rng), rng.pick(&AC_IDS)),
                    "parent sectioned" => {
                        if rng.chance(50) {
                            format!("{}#{}", rng.pick(&DOC_IDS), rng.pick(&SECTION_IDS))
                        } else {
                            format!(
                                "{}/{}#{}",
                                any_slug(rng),
                                rng.pick(&AC_IDS),
                                rng.pick(&AC_IDS)
                            )
                        }
                    }
                    "parent project:" => format!("other:{}", rng.pick(&DOC_IDS)),
                    _ => format!("\"{}\\/{}\"", any_slug(rng), rng.pick(&AC_IDS)),
                };
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.parent = Some(parent);
                });
            }
            "a scoped parent through its feature" => {
                // A scoped parent that resolves: a section ID of an existing
                // feature document, cited as `slug/ID` (escaped half the time).
                let mut found = Vec::new();
                for (path, doc) in &self.files {
                    if !self.is_feature(path) {
                        continue;
                    }
                    let slug = &path[dir_of(path).len() + 1..path.len() - 3];
                    for section in &doc.sections {
                        if let Some(id) = &section.id {
                            found.push((slug.to_owned(), id.clone()));
                        }
                    }
                }
                if found.is_empty() {
                    return self.apply(rng, "parent scoped", change);
                }
                let (slug, id) = rng.pick(&found).clone();
                let parent = if rng.chance(50) {
                    format!("{slug}/{id}")
                } else {
                    format!("\"{slug}\\/{id}\"")
                };
                let path = self.any_path(rng);
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.parent = Some(parent);
                });
            }
            "a look-alike slug/ID through its alias"
            | "a look-alike feature-scoped ID through its alias" => {
                let slug_form = case == "a look-alike slug/ID through its alias";
                let features = self.paths_where(|path, _| self.is_feature(path));
                if features.is_empty() {
                    let fallback = if slug_form {
                        "mention slug/ID"
                    } else {
                        "mention feature-scoped outside its feature"
                    };
                    return self.apply(rng, fallback, change);
                }
                let feature = rng.pick(&features).clone();
                let alias = look_alike_ac();
                let own = (!slug_form).then(|| format!("Own {alias} here."));
                self.edit(&feature, change, case, |doc| {
                    doc.front = true;
                    if !doc.aliases.contains(&alias) {
                        doc.aliases.push(alias.clone());
                    }
                    if let Some(line) = own {
                        doc.body.push(line);
                    }
                });
                if slug_form {
                    let slug = feature[dir_of(&feature).len() + 1..feature.len() - 3].to_owned();
                    let others = self.paths_where(|path, _| *path != feature);
                    let citer = rng.pick(&others).clone();
                    let line = format!("Per {slug}/{} there.", look_alike_ac());
                    self.edit(&citer, change, case, |doc| doc.body.push(line));
                }
            }
            "add the Latin twin alias" => {
                // Another file than the look-alike's feature document: a new
                // one, or one there.
                if self.files.len() < MAX_FILES && rng.chance(50) {
                    let free = self.free_paths(|path| TOP.contains(&path) || SUB.contains(&path));
                    if let Some(path) = free.first().cloned() {
                        let doc = Doc {
                            front: true,
                            aliases: vec![TWIN.to_owned()],
                            title: "Doc twin".to_owned(),
                            ..Doc::default()
                        };
                        self.files.insert(path.clone(), doc);
                        change.written.insert(path);
                        change.cases.push(case);
                        return;
                    }
                }
                let others = self.paths_where(|path, doc| {
                    !self.is_feature(path) && !doc.aliases.iter().any(|alias| alias == TWIN)
                });
                if others.is_empty() {
                    return self.apply(rng, "add a Latin alias", change);
                }
                let path = rng.pick(&others).clone();
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.aliases.push(TWIN.to_owned());
                });
            }
            "remove the Latin twin alias" => {
                let with = self.paths_where(|_, doc| doc.aliases.iter().any(|alias| alias == TWIN));
                if with.is_empty() {
                    return self.apply(rng, "add the Latin twin alias", change);
                }
                let path = rng.pick(&with).clone();
                if self.files.len() > MIN_FILES && rng.chance(50) {
                    self.files.remove(&path);
                    change.removed.insert(path);
                    change.cases.push(case);
                } else {
                    self.edit(&path, change, case, |doc| {
                        doc.aliases.retain(|alias| alias != TWIN);
                    });
                }
            }
            "a section of a name-shaped ID" => {
                // The name fallback's target: `TERM-glide` defined as a
                // section somewhere, `TERM-glide-fast` cited elsewhere.
                let holder = self.any_path(rng);
                let citer = self.any_path(rng);
                self.edit(&holder, change, case, |doc| {
                    doc.sections.push(Section {
                        level: 2,
                        title: "Notes",
                        id: Some("TERM-glide".to_owned()),
                        lines: vec!["Glide words.".to_owned()],
                    });
                });
                let line = "Term TERM-glide-fast used.".to_owned();
                self.edit(&citer, change, case, |doc| doc.body.push(line));
            }
            "status superseded-by" => {
                let path = self.any_path(rng);
                let target = front_reference(rng);
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.status = Some(format!("superseded-by {target}"));
                });
            }
            "a front-matter reference" => {
                let path = self.any_path(rng);
                let reference = front_reference(rng).replace("RQ", "R");
                let refs = rng.chance(30);
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    if refs {
                        doc.refs.push(reference);
                    } else {
                        doc.depends_on.push(reference);
                    }
                });
            }
            "a labelled link" => {
                let path = self.any_path(rng);
                let item = flow_item(rng);
                self.edit(&path, change, case, |doc| {
                    doc.front = true;
                    doc.follows.push(item);
                });
            }
            "remove a front-matter key" => {
                let with = self.paths_where(|_, doc| doc.has_key());
                if with.is_empty() {
                    return self.apply(rng, "a front-matter reference", change);
                }
                let path = rng.pick(&with).clone();
                let pick = rng.below(7);
                self.edit(&path, change, case, |doc| {
                    for offset in 0..7 {
                        let cleared = match (pick + offset) % 7 {
                            0 => doc.parent.take().is_some(),
                            1 => doc.status.take().is_some(),
                            2 => doc.canon.take().is_some(),
                            3 => doc.class.take().is_some(),
                            4 => !std::mem::take(&mut doc.refs).is_empty(),
                            5 => !std::mem::take(&mut doc.depends_on).is_empty(),
                            _ => !std::mem::take(&mut doc.follows).is_empty(),
                        };
                        if cleared {
                            break;
                        }
                    }
                });
            }
            "a failed front-matter" => {
                let path = self.any_path(rng);
                self.edit(&path, change, case, |doc| doc.broken_front = true);
            }
            "invalid UTF-8" => {
                let path = self.any_path(rng);
                self.edit(&path, change, case, |doc| doc.not_utf8 = true);
            }
            "chmod 000" => {
                let path = self.any_path(rng);
                self.edit(&path, change, case, |doc| doc.locked = true);
            }
            "repair a file" => {
                let damaged = self.paths_where(|_, doc| doc.damaged());
                if damaged.is_empty() {
                    return self.apply(rng, "edit prose", change);
                }
                let path = rng.pick(&damaged).clone();
                self.edit(&path, change, case, |doc| {
                    doc.broken_front = false;
                    doc.not_utf8 = false;
                    doc.locked = false;
                });
            }
            "edit prose" => {
                let path = self.any_path(rng);
                let word = rng.below(1000);
                self.edit(&path, change, case, |doc| {
                    if let Some(line) = doc.body.first_mut() {
                        line.push_str(&format!(" More {word}."));
                    } else {
                        doc.title.push_str(&format!(" {word}"));
                    }
                });
            }
            "remove a line" => {
                let with = self.paths_where(|_, doc| doc.line_count() > 0);
                if with.is_empty() {
                    return self.apply(rng, "edit prose", change);
                }
                let path = rng.pick(&with).clone();
                let at = rng.below(self.files[&path].line_count());
                self.edit(&path, change, case, |doc| {
                    let mut at = at;
                    if at < doc.body.len() {
                        doc.body.remove(at);
                        return;
                    }
                    at -= doc.body.len();
                    for section in &mut doc.sections {
                        if at < section.lines.len() {
                            section.lines.remove(at);
                            return;
                        }
                        at -= section.lines.len();
                    }
                });
            }
            "edit [paths] features" => {
                self.config.features = (self.config.features + 1) % FEATURE_DIRS.len();
                self.config_change(change, case);
            }
            "edit [paths] link_base" => {
                self.config.link_base = (self.config.link_base + 1) % LINK_BASES.len();
                self.config_change(change, case);
            }
            "edit [paths] exclude" => {
                self.config.exclude = !self.config.exclude;
                self.config_change(change, case);
            }
            "edit [ids]" => {
                self.config.extra_prefix = !self.config.extra_prefix;
                self.config_change(change, case);
            }
            other => panic!("no case {other}"),
        }
    }

    fn rename(&mut self, from: String, to: String, change: &mut Change, case: &'static str) {
        let doc = self.files.remove(&from).expect("renamed file");
        self.files.insert(to.clone(), doc);
        change.renamed = Some((from, to));
        change.cases.push(case);
    }

    fn edit(
        &mut self,
        path: &str,
        change: &mut Change,
        case: &'static str,
        edit: impl FnOnce(&mut Doc),
    ) {
        edit(self.files.get_mut(path).expect("edited file"));
        change.written.insert(path.to_owned());
        if change.cases.last() != Some(&case) {
            change.cases.push(case);
        }
    }

    fn add_line(&mut self, rng: &mut Rng, case: &'static str, path: String, change: &mut Change) {
        let line = self.line_for(rng, case, &path);
        let doc = self.files.get_mut(&path).expect("cited file");
        doc.push_line(rng, line);
        change.written.insert(path);
        change.cases.push(case);
    }

    fn config_change(&mut self, change: &mut Change, case: &'static str) {
        change.config = true;
        change.cases.push(case);
    }
}

/// A random document for `path`: a feature document gets `AC` sections.
fn random_doc(rng: &mut Rng, path: &str, feature: bool) -> Doc {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md");
    let mut doc = Doc {
        title: format!("Doc {stem}"),
        front: rng.chance(85),
        ..Doc::default()
    };
    if doc.front {
        if rng.chance(70) {
            doc.id = Some((*rng.pick(&DOC_IDS)).to_owned());
        }
        if rng.chance(20) {
            doc.aliases.push(if rng.chance(60) {
                (*rng.pick(&LATIN_ALIASES)).to_owned()
            } else {
                rng.pick(&look_alikes()).clone()
            });
        }
        if rng.chance(25) {
            doc.parent = Some(random_parent(rng));
        }
        if rng.chance(10) {
            doc.status = Some(format!("superseded-by {}", front_reference(rng)));
        }
        if rng.chance(10) {
            let target = rng.pick(&TOP);
            doc.canon = Some(if rng.chance(50) {
                format!("{target}#{}", rng.pick(&ANCHORS))
            } else {
                (*target).to_owned()
            });
        }
        if rng.chance(30) {
            for _ in 0..1 + rng.below(2) {
                doc.depends_on.push(front_reference(rng).replace("RQ", "R"));
            }
        }
        if rng.chance(15) {
            doc.follows.push(flow_item(rng));
        }
        if rng.chance(15) {
            doc.class = Some("canon");
        }
    }
    if feature && rng.chance(20) {
        doc.front = true;
        doc.aliases.push(look_alike_ac());
    }
    if feature {
        let first = rng.below(AC_IDS.len());
        for k in 0..1 + rng.below(3) {
            doc.sections.push(Section {
                level: 2 + rng.below(2),
                title: rng.pick(&TITLES),
                id: Some(AC_IDS[(first + k) % AC_IDS.len()].to_owned()),
                lines: Vec::new(),
            });
        }
    } else {
        for _ in 0..rng.below(3) {
            let id = rng.chance(70).then(|| (*rng.pick(&SECTION_IDS)).to_owned());
            doc.sections.push(Section {
                level: 2 + rng.below(2),
                title: rng.pick(&TITLES),
                id,
                lines: Vec::new(),
            });
        }
    }
    doc
}
