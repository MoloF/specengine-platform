//! The code index's analyzer and refresh (canon `symbol-index`, "Crates
//! and seams", "Tables"): the store's
//! [`CodeAnalyzer`] over the code walk's one listing
//! ([`code_walk::list`]) and package lookup ([`code_walk::disk_packages`]),
//! and `specengine-code` for every per-file row.
//!
//! - **One parse** per re-derived `.rs` file: the comment binding, the
//!   file's names (its package's [`TargetIndex`] prepared once), the levels
//!   and the display signatures all read that one tree; a `.ron` file's
//!   comments are anchored once.
//! - **Rows**: every item a `symbols` row (its levels or `cannot_verify`,
//!   never read from an item's binding state), its suffixes from the
//!   structured path ([`QPath::suffixes`]); every `#[path]` declaration;
//!   every marker and mention with what its comment binds, as `spec refs`
//!   lists them.
//! - **Lenient**: a file that cannot be read, or that the parser gives no
//!   tree, is a row with its `read_error`; bytes that are not UTF-8 a row
//!   with `utf8` 0 and a `note:`. Reads that need the whole tree ask
//!   [`CodeRefresh::require_complete`]: `spec: the code walk is incomplete:
//!   <gap>`, the walk's first gap by path.
//! - **Framework roles** (task spec `bevy-roles` "Role facts"): with
//!   `[code] frameworks` listed, the same tree gives each Rust file's role
//!   rows (`specengine_code::frameworks::role_facts`); the list is in the
//!   stamp, so a change re-derives every file. Its names are checked here,
//!   against the code crate's data, before any code row is touched
//!   ([`known_frameworks`]): the key's one reader.
//!
//! [`refresh_code`] is the one refresh `spec index` and `spec refs` call
//! (and later reads that walk no tree).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_code::frameworks::{self, KNOWN, role_facts};
use specengine_code::{
    Anchor, Binding, CODE_INDEX_FORMAT, Digest, FileRole, LevelHashes, LevelState, NamedFile,
    PackageTargets, RustParser, TargetIndex, UnitKind, bind_tree, comment_anchors, item_levels,
    item_signatures, level_header, markers_in, placed_name_file, qpath,
};
use specengine_core::{CodeConfig, ProjectError};
use specengine_model::{IdScheme, grammar};
use specengine_store::{
    CodeAnalyzer, CodeDeclRow, CodeFileRows, CodeListing, CodeMarkerRow, CodeMentionRow,
    CodePackageRow, CodeReport, CodeRoleRow, CodeSymbolRow, ListedCodeFile, ROLE_SHARED,
    ROLE_UNROOTED, SqliteIndex, StoreError, b3_hash,
};

use crate::code_walk::{self, ManifestSeen, RUST_EXTENSION};
use crate::project::config_error;
use crate::refs::{
    CANNOT_VERIFY, FILE, ITEM, Lines, MODULE, RON, RON_NO_VALUE, RON_TOO_DEEP, UNANCHORED,
};
use crate::{CliError, Message, store_error};

/// `code_files.lang` of a Rust file and of a RON file.
pub(crate) const LANG_RUST: &str = "rust";
pub(crate) const LANG_RON: &str = "ron";

/// The binding of a comment on a value path that names more than one
/// value (`spec refs`: `ron <path> ambiguous duplicate`).
pub(crate) const RON_AMBIGUOUS: &str = "ron_ambiguous";

/// `code_files.role` of a unit's crate root and of a module file (a shared
/// unit's file is [`ROLE_SHARED`], a file in no unit [`ROLE_UNROOTED`]).
const ROLE_CRATE_ROOT: &str = "crate_root";
const ROLE_MODULE: &str = "module";

/// `symbols.level_state` of an item with levels (the other is
/// [`CANNOT_VERIFY`]).
const HASHED: &str = "hashed";

/// The layer whose rules made every row (task spec `layer-c`): layer A.
const LAYER: &str = "a";

/// What a file the parser gives no tree is stored with.
const NO_TREE: &str = "cannot be parsed (the parser gave no tree)";

/// The code tree read for one refresh: every listed file's bytes, each
/// Rust file's package, the packages' target tables; the listing's notes.
pub(crate) struct CodeFiles {
    listing: CodeListing,
    /// By package directory (`""` the root).
    tables: BTreeMap<String, PackageTargets>,
    packages: Vec<CodePackageRow>,
    /// The roots that name nothing, in the order written.
    notes: Vec<Message>,
}

impl CodeFiles {
    /// Lists the `[code]` roots under `root` as the walk lists them, reads
    /// every file, finds each readable UTF-8 Rust file's package.
    pub(crate) fn read(root: &Path, config: &CodeConfig) -> Self {
        let mut notes = Vec::new();
        let listing = code_walk::list(root, config, &mut notes);
        let mut files: Vec<ListedCodeFile> = listing
            .files
            .iter()
            .map(|path| match fs::read(root.join(path)) {
                Ok(bytes) => ListedCodeFile {
                    path: path.clone(),
                    bytes: Some(bytes),
                    read_error: None,
                    package: None,
                },
                Err(error) => ListedCodeFile {
                    path: path.clone(),
                    bytes: None,
                    read_error: Some(format!("cannot be read ({error})")),
                    package: None,
                },
            })
            .collect();
        let rust: Vec<usize> = files
            .iter()
            .enumerate()
            .filter(|(_, file)| {
                file.path.ends_with(RUST_EXTENSION)
                    && file
                        .bytes
                        .as_deref()
                        .is_some_and(|bytes| std::str::from_utf8(bytes).is_ok())
            })
            .map(|(at, _)| at)
            .collect();
        let paths: Vec<&str> = rust.iter().map(|&at| files[at].path.as_str()).collect();
        let code_walk::DiskPackages { dirs, tables, seen } =
            code_walk::disk_packages(root, &listing, &paths);
        for (&at, dir) in rust.iter().zip(dirs) {
            files[at].package = dir.as_deref().map(dir_text);
        }
        let tables: BTreeMap<String, PackageTargets> = tables
            .into_iter()
            .map(|(dir, table)| (dir_text(&dir), table))
            .collect();
        let packages = tables
            .iter()
            .map(|(dir, table)| CodePackageRow {
                dir: dir.clone(),
                manifest_b3: match seen.get(dir) {
                    Some(ManifestSeen::Bytes(bytes)) => Some(b3_hash(bytes)),
                    _ => None,
                },
                source: table.source.as_str().to_owned(),
                targets: targets_json(table),
            })
            .collect();
        Self {
            listing: CodeListing {
                files,
                gaps: listing.unlisted.iter().cloned().collect(),
                skipped: listing.skipped_names,
            },
            tables,
            packages,
            notes,
        }
    }
}

/// A root-relative directory as stored: `/`-separated, `""` the root.
/// Only where `\` separates components is it rewritten: on Unix it is a
/// character of a name, kept as the walk keeps it.
fn dir_text(dir: &Path) -> String {
    let text = dir.to_string_lossy();
    if std::path::MAIN_SEPARATOR == '\\' {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// One target of a package's table, as `code_packages.targets` holds it.
#[derive(Serialize)]
struct TargetRow<'a> {
    kind: &'a str,
    name: &'a str,
    root: String,
}

/// The target table as JSON, in the table's order.
fn targets_json(table: &PackageTargets) -> String {
    let rows: Vec<TargetRow<'_>> = table
        .targets
        .iter()
        .map(|target| TargetRow {
            kind: &target.kind,
            name: &target.name,
            root: dir_text(&target.root),
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_default()
}

/// The analyzer the store calls over one [`CodeFiles`].
pub(crate) struct Analyzer<'f> {
    files: &'f CodeFiles,
    config: &'f CodeConfig,
    scheme: &'f IdScheme,
    /// Each package's table prepared once.
    indexes: BTreeMap<&'f str, TargetIndex<'f>>,
    parser: RefCell<RustParser>,
}

impl<'f> Analyzer<'f> {
    pub(crate) fn new(
        files: &'f CodeFiles,
        config: &'f CodeConfig,
        scheme: &'f IdScheme,
    ) -> Result<Self, CliError> {
        let parser = RustParser::new().map_err(|error| {
            CliError::spec(format!("the Rust grammar cannot be loaded: {error}"))
        })?;
        Ok(Self {
            files,
            config,
            scheme,
            indexes: files
                .tables
                .iter()
                .map(|(dir, table)| (dir.as_str(), TargetIndex::new(table)))
                .collect(),
            parser: RefCell::new(parser),
        })
    }

    fn rust(&self, file: &ListedCodeFile, text: &str) -> CodeFileRows {
        let Some(tree) = self.parser.borrow_mut().parse(text) else {
            return CodeFileRows {
                lang: LANG_RUST.to_owned(),
                utf8: true,
                read_error: Some(NO_TREE.to_owned()),
                ..CodeFileRows::default()
            };
        };
        let binding = bind_tree(&tree, text);
        let analysis = &binding.analysis;
        let path = Path::new(&file.path);
        let package = file.package.as_deref().map(Path::new);
        let index = file
            .package
            .as_deref()
            .and_then(|dir| self.indexes.get(dir));
        let named = NamedFile {
            path,
            package,
            analysis,
        };
        let placed = placed_name_file(&named, index);
        let (names, role) = (&placed.names, &placed.role);
        let every: Vec<usize> = (0..analysis.items.len()).collect();
        let levels = item_levels(&tree, text, &every);
        let signatures = item_signatures(&tree, text, &every);
        let lines = Lines::new(text);

        // An item's parent: the nearest item before it whose bytes hold it
        // (only an inline `mod`, an `impl` or a trait holds items).
        let mut parents: Vec<Option<usize>> = Vec::with_capacity(analysis.items.len());
        let mut open: Vec<usize> = Vec::new();
        for (at, item) in analysis.items.iter().enumerate() {
            while let Some(&last) = open.last() {
                let outer = &analysis.items[last].byte_range;
                if outer.start <= item.byte_range.start && item.byte_range.end <= outer.end {
                    break;
                }
                open.pop();
            }
            parents.push(open.last().copied());
            open.push(at);
        }

        let mut symbols = Vec::with_capacity(analysis.items.len());
        for (at, item) in analysis.items.iter().enumerate() {
            let (Some(name), Some(structured)) = (names.items.get(at), placed.paths.get(at)) else {
                continue;
            };
            let state = levels.get(at).cloned().flatten();
            let (level_state, categories, hashes): (&str, Vec<&str>, Option<LevelHashes>) =
                match &state {
                    Some(LevelState::Hashed(hashes)) => (HASHED, Vec::new(), Some(*hashes)),
                    Some(LevelState::CannotVerify { categories }) => (
                        CANNOT_VERIFY,
                        categories
                            .iter()
                            .map(|category| category.as_str())
                            .collect(),
                        None,
                    ),
                    None => (CANNOT_VERIFY, Vec::new(), None),
                };
            let digest = |pick: fn(&LevelHashes) -> Digest| {
                hashes
                    .as_ref()
                    .map(|hashes| format!("b3:{}", pick(hashes).to_hex()))
            };
            symbols.push(CodeSymbolRow {
                ord: number(at),
                item_kind: item.kind.clone(),
                name: structured.name.clone(),
                owner: item.owner.clone(),
                qpath: name.qpath.clone(),
                discriminator: name.discriminator.clone(),
                parent_ord: parents.get(at).copied().flatten().map(number),
                start_line: number(item.line),
                end_line: number(lines.at(item.byte_range.end).0),
                start_byte: number(item.byte_range.start),
                end_byte: number(item.byte_range.end),
                signature: signatures.get(at).cloned().flatten(),
                level_state: level_state.to_owned(),
                level_categories: serde_json::to_string(&categories).unwrap_or_default(),
                sig_hash: digest(|hashes| hashes.sig),
                body_hash: digest(|hashes| hashes.body),
                deps_hash: digest(|hashes| hashes.deps),
                skeleton_hash: digest(|hashes| hashes.skeleton),
                quote_hash: digest(|hashes| hashes.quote),
                body_bytes: hashes.map(|hashes| hashes.body_bytes),
                layer: LAYER.to_owned(),
                suffixes: structured.suffixes(name.discriminator.as_deref()),
            });
        }

        let decls = analysis
            .mod_declarations
            .iter()
            .filter_map(|declaration| {
                let value = declaration.path_attribute.as_deref()?;
                Some((qpath::resolve_path_attribute(path, value), declaration.line))
            })
            .enumerate()
            .map(
                |(ord, (target, line)): (usize, (PathBuf, usize))| CodeDeclRow {
                    ord: number(ord),
                    target: dir_text(&target),
                    line: number(line),
                },
            )
            .collect();

        let module = names.module.qpath.clone();
        let encode = |binding: Binding| -> Bound {
            match binding {
                Binding::Item(item) => Bound::new(ITEM, Some(number(item)), None),
                Binding::Module(None) => Bound::new(MODULE, None, Some(module.clone())),
                Binding::Module(Some(item)) => Bound::new(MODULE, Some(number(item)), None),
                Binding::Unanchored(category) => {
                    Bound::new(UNANCHORED, None, Some(category.as_str().to_owned()))
                }
                Binding::CannotVerify(category) => {
                    Bound::new(CANNOT_VERIFY, None, Some(category.as_str().to_owned()))
                }
                Binding::File => Bound::new(FILE, None, None),
            }
        };
        let mut found = Found::default();
        for comment in &binding.comments {
            let Some(body) = text.get(comment.range.clone()) else {
                continue;
            };
            self.scan(
                body,
                comment.range.start,
                &lines,
                &encode(comment.binding),
                &encode(comment.mention),
                &mut found,
            );
        }

        // The listed frameworks' role facts, on the same tree; none listed:
        // no detector runs.
        let framework_roles = if self.config.frameworks.is_empty() {
            Vec::new()
        } else {
            role_facts(&self.config.frameworks, &tree, text, &analysis.items)
                .into_iter()
                .enumerate()
                .map(|(ord, fact)| CodeRoleRow {
                    ord: number(ord),
                    word: fact.word.to_owned(),
                    source: fact.source.to_owned(),
                    form: fact.form.map(str::to_owned),
                    origin: fact.origin.to_owned(),
                    group_text: fact.group,
                    symbol_ord: fact.symbol_ord.map(number),
                    subject: fact.subject,
                    subject_name: fact.subject_name,
                    certainty: fact.certainty.to_owned(),
                    category: fact.category.map(str::to_owned),
                    line: number(fact.line),
                })
                .collect()
        };

        CodeFileRows {
            lang: LANG_RUST.to_owned(),
            utf8: true,
            read_error: None,
            role: Some(role_word(role).to_owned()),
            module: Some(module),
            orphan_errors: number(analysis.orphan_errors),
            symbols,
            decls,
            markers: found.markers,
            mentions: found.mentions,
            framework_roles,
        }
    }

    fn ron(&self, text: &str) -> CodeFileRows {
        let lines = Lines::new(text);
        let mut found = Found::default();
        for anchor in comment_anchors(text) {
            let Some(body) = text.get(anchor.comment.clone()) else {
                continue;
            };
            let (marker, mention) = match &anchor.anchor {
                Anchor::Path { path, .. } => {
                    let bound = Bound::new(RON, None, Some(path.clone()));
                    (bound.clone(), bound)
                }
                Anchor::Ambiguous { path, .. } => {
                    let bound = Bound::new(RON_AMBIGUOUS, None, Some(path.clone()));
                    (bound.clone(), bound)
                }
                Anchor::Unanchored => (
                    Bound::new(UNANCHORED, None, Some(RON_NO_VALUE.to_owned())),
                    Bound::new(FILE, None, None),
                ),
                Anchor::CannotVerify => {
                    let bound = Bound::new(CANNOT_VERIFY, None, Some(RON_TOO_DEEP.to_owned()));
                    (bound.clone(), bound)
                }
            };
            self.scan(
                body,
                anchor.comment.start,
                &lines,
                &marker,
                &mention,
                &mut found,
            );
        }
        CodeFileRows {
            lang: LANG_RON.to_owned(),
            utf8: true,
            markers: found.markers,
            mentions: found.mentions,
            ..CodeFileRows::default()
        }
    }

    /// The markers and mentions of one comment (`body`, starting at byte
    /// `start` of its file): every marker; every reference the `[ids]`
    /// grammar finds outside a marker's own token.
    fn scan(
        &self,
        body: &str,
        start: usize,
        lines: &Lines,
        marker_bound: &Bound,
        mention_bound: &Bound,
        found: &mut Found,
    ) {
        let markers = markers_in(body);
        let tokens: Vec<std::ops::Range<usize>> = markers
            .iter()
            .map(|marker| marker.token_range(body))
            .collect();
        for marker in &markers {
            let (line, column) = lines.at(start + marker.offset);
            let levels = marker.levels.effective().map(|levels| {
                let words: Vec<&str> = levels.iter().map(|level| level.as_str()).collect();
                serde_json::to_string(&words).unwrap_or_default()
            });
            found.markers.push(CodeMarkerRow {
                ord: number(found.markers.len()),
                relation: marker.relation.as_str().to_owned(),
                token: marker.token(body).to_owned(),
                id: marker.id.clone(),
                rev: marker.rev,
                levels,
                levels_state: marker.levels.state().to_owned(),
                note: marker.note.clone(),
                line: number(line),
                column: number(column),
                binding: marker_bound.binding.to_owned(),
                symbol_ord: marker_bound.symbol_ord,
                anchor: marker_bound.anchor.clone(),
            });
        }
        for hit in grammar::scan(body, 0, self.scheme) {
            let Some(span) = hit.reference.span else {
                continue;
            };
            if tokens.iter().any(|token| token.contains(&span.start)) {
                continue;
            }
            let Some(written) = body.get(span.start..span.end) else {
                continue;
            };
            let (line, column) = lines.at(start + span.start);
            found.mentions.push(CodeMentionRow {
                ord: number(found.mentions.len()),
                written: written.to_owned(),
                line: number(line),
                column: number(column),
                binding: mention_bound.binding.to_owned(),
                symbol_ord: mention_bound.symbol_ord,
                anchor: mention_bound.anchor.clone(),
            });
        }
    }
}

/// What a comment binds, as a marker or mention row stores it.
#[derive(Clone)]
struct Bound {
    binding: &'static str,
    symbol_ord: Option<u32>,
    anchor: Option<String>,
}

impl Bound {
    fn new(binding: &'static str, symbol_ord: Option<u32>, anchor: Option<String>) -> Self {
        Self {
            binding,
            symbol_ord,
            anchor,
        }
    }
}

/// The markers and mentions of one file, in source order.
#[derive(Default)]
struct Found {
    markers: Vec<CodeMarkerRow>,
    mentions: Vec<CodeMentionRow>,
}

/// A count, line or offset as a row stores it.
fn number(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// `code_files.role` of a file's role.
fn role_word(role: &FileRole) -> &'static str {
    let shared = role
        .unit()
        .is_some_and(|unit| unit.kind == UnitKind::Shared);
    match role {
        FileRole::Unrooted => ROLE_UNROOTED,
        _ if shared => ROLE_SHARED,
        FileRole::CrateRoot(_) => ROLE_CRATE_ROOT,
        FileRole::Module(..) => ROLE_MODULE,
    }
}

/// What the code fingerprint covers besides the bytes, path, package
/// table and `[ids]`: the analyzer's stamp, the level recipe's header and
/// the parsed `[code]` table, `frameworks` always (sorted, `[]` when
/// absent).
#[derive(Serialize)]
struct Stamp<'a> {
    format: u32,
    levels: String,
    roots: &'a [String],
    exclude: &'a [String],
    frameworks: &'a [String],
}

impl CodeAnalyzer for Analyzer<'_> {
    fn list(&self) -> Result<CodeListing, StoreError> {
        Ok(self.files.listing.clone())
    }

    fn packages(&self, _listing: &CodeListing) -> Result<Vec<CodePackageRow>, StoreError> {
        Ok(self.files.packages.clone())
    }

    fn fingerprint(&self) -> Vec<u8> {
        let header: String = level_header()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        serde_json::to_vec(&Stamp {
            format: CODE_INDEX_FORMAT,
            levels: header,
            roots: &self.config.roots,
            exclude: &self.config.exclude,
            frameworks: &self.config.frameworks,
        })
        .unwrap_or_default()
    }

    fn analyze(&self, file: &ListedCodeFile, _package: Option<&CodePackageRow>) -> CodeFileRows {
        let rust = file.path.ends_with(RUST_EXTENSION);
        let lang = if rust { LANG_RUST } else { LANG_RON };
        let Some(bytes) = file.bytes.as_deref() else {
            return CodeFileRows {
                lang: lang.to_owned(),
                utf8: true,
                read_error: file.read_error.clone(),
                ..CodeFileRows::default()
            };
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            return CodeFileRows {
                lang: lang.to_owned(),
                utf8: false,
                ..CodeFileRows::default()
            };
        };
        if rust {
            self.rust(file, text)
        } else {
            self.ron(text)
        }
    }
}

/// One code-index refresh: what the store did, the notes of the walk (a
/// root that names nothing, then each file that is not UTF-8, by path),
/// and the walk's first gap, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRefresh {
    pub report: CodeReport,
    pub messages: Vec<Message>,
    gap: Option<String>,
}

impl CodeRefresh {
    /// The first gap by path, as the walk names it: a directory that
    /// cannot be listed (when it comes before every file that cannot be
    /// read or parsed), else that file.
    pub fn gap(&self) -> Option<&str> {
        self.gap.as_deref()
    }

    /// Exit 2 on a gap: `spec: the code walk is incomplete: <gap>`, never
    /// a partial answer.
    pub fn require_complete(&self) -> Result<(), CliError> {
        match &self.gap {
            Some(gap) => Err(CliError::spec(format!(
                "the code walk is incomplete: {gap}"
            ))),
            None => Ok(()),
        }
    }
}

/// Every name of `[code] frameworks` is one the code crate knows
/// (`specengine_code::frameworks::KNOWN`): else the first other, by name,
/// at its line of `config_label` (``<config>:<line>: `[code] frameworks`:
/// `<name>` is not known: known `…` ``), exit 2.
pub fn known_frameworks(code: &CodeConfig, config_label: &str) -> Result<(), CliError> {
    let Some(name) = code
        .frameworks
        .iter()
        .find(|name| frameworks::framework(name).is_none())
    else {
        return Ok(());
    };
    let problem = ProjectError {
        line: code.framework_line(name),
        message: format!(
            "`[code] frameworks`: `{name}` is not known: known `{}`",
            KNOWN.join("`, `")
        ),
    };
    Err(config_error(config_label, &problem))
}

/// Refreshes the code index of `index`'s worktree (`root`, canonical) from
/// the `[code]` walk `code` (read from `config_label`), `[ids]` from
/// `scheme`: the files whose cache key moved are re-derived (`full`: every
/// file), the rest kept, the cross-file pass run. A framework name the code
/// crate does not know is refused first ([`known_frameworks`]), no code
/// row touched. Lenient: a gap is stored and reported
/// ([`CodeRefresh::gap`]), never an error here. The spec index must be
/// refreshed first (the store's `NotIndexed` otherwise).
pub fn refresh_code(
    index: &mut SqliteIndex,
    root: &Path,
    code: &CodeConfig,
    config_label: &str,
    scheme: &IdScheme,
    full: bool,
) -> Result<CodeRefresh, CliError> {
    known_frameworks(code, config_label)?;
    let files = CodeFiles::read(root, code);
    let analyzer = Analyzer::new(&files, code, scheme)?;
    let report = index
        .update_code(&analyzer, scheme, full)
        .map_err(store_error)?;
    let mut messages = files.notes.clone();
    for path in &report.not_utf8 {
        messages.push(Message::Note(format!("{path} is not UTF-8: skipped")));
    }
    let dir = report.gaps.iter().min();
    let file = report.read_errors.first();
    let gap = match (dir, file) {
        (Some(dir), Some((path, _))) if dir.as_str() < path.as_str() => Some(listing_gap(dir)),
        (Some(dir), None) => Some(listing_gap(dir)),
        (_, Some((path, error))) => Some(format!("`{path}` {error}")),
        (None, None) => None,
    };
    Ok(CodeRefresh {
        report,
        messages,
        gap,
    })
}

/// The files the `[code]` walk lists under `root`, byte-sorted: what a copy
/// of the code tree holds (the measurement harness's scratch copy).
pub fn listed_code_files(root: &Path, code: &CodeConfig) -> Vec<String> {
    let mut notes = Vec::new();
    code_walk::list(root, code, &mut notes)
        .files
        .into_iter()
        .collect()
}

/// The gap of a directory that cannot be listed (`""`: the root).
fn listing_gap(dir: &str) -> String {
    let shown = if dir.is_empty() { "." } else { dir };
    format!("the directory `{shown}` cannot be listed")
}
