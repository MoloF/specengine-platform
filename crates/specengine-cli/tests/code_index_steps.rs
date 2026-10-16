//! docs/features/symbol-index.md TE-2 ("Rules and edge cases": WHEN any
//! sequence of file, manifest, `[code]`, `[ids]` or stamp changes is
//! indexed, the dump SHALL equal a fresh rebuild and `refs` from the index
//! SHALL equal `refs` from the walk): a seeded random sequence of steps
//! over a scratch copy of `fixtures/code-index` and of `fixtures/verify-ecs`
//! (an ECS-style game crate: ADR-0008's second project). After every step
//! `spec index` in the running `HOME` gives the dump a fresh `spec index`
//! in a new `HOME` gives, byte for byte (every table), and `spec refs` of
//! two REFs prints the same from both (text). That the fresh index's
//! `refs` equal the walk's is RD-8's oracle (`refs.rs`' `index_oracle`
//! tests, which replay one step of every kind).
//!
//! The steps: a file added, removed, renamed, edited (lines shifted, an
//! item and a marker appended), a same-size edit with its mtime put back,
//! the package renamed in `Cargo.toml`, a `Cargo.toml` between a file and
//! its package written and removed, `[code]` roots and `exclude` changed,
//! an `[ids]` prefix added and removed, the stored stamps rewritten (the
//! index format, the analyzer's `code_fp`: through the system's `sqlite3`),
//! a file made CRLF and back, twins inserted, removed and `cfg`-edited
//! (rule 1 <-> rule 2), a `#[path]` written and dropped in the declaring
//! file, a cross-file twin renamed away and back. Every kind runs once in
//! a seeded order first, then kinds at random.
//!
//! Bounded: `SPECENGINE_TE2_STEPS` steps (default 80) and
//! `SPECENGINE_TE2_SECONDS` (default 150) past the first round;
//! `SPECENGINE_TE2_SEED` replays a failure (the message names the seed and
//! every step so far).

#![cfg(unix)]

mod code_index_common;
mod common;

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use code_index_common::{
    ECS, FIXTURE, SLUG, copy, db, dump, dump_diff, execute, same_size_same_mtime_edit, to_crlf,
    to_lf,
};
use common::{Scratch, read_text, spec, write};

/// xorshift64*: no dependency, the same sequence on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        assert!(n > 0);
        usize::try_from(self.next() % n as u64).expect("usize")
    }

    fn chance(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    AddFile,
    RemoveFile,
    RenameFile,
    EditFile,
    SameSizeEdit,
    Manifest,
    NestedManifest,
    CodeTable,
    Ids,
    Stamp,
    Crlf,
    TwinInsert,
    TwinRemove,
    TwinCfgEdit,
    PathAttribute,
    CrossTwin,
}

const KINDS: [Kind; 16] = [
    Kind::AddFile,
    Kind::RemoveFile,
    Kind::RenameFile,
    Kind::EditFile,
    Kind::SameSizeEdit,
    Kind::Manifest,
    Kind::NestedManifest,
    Kind::CodeTable,
    Kind::Ids,
    Kind::Stamp,
    Kind::Crlf,
    Kind::TwinInsert,
    Kind::TwinRemove,
    Kind::TwinCfgEdit,
    Kind::PathAttribute,
    Kind::CrossTwin,
];

/// One project the steps run over.
struct Project {
    fixture: &'static str,
    slug: &'static str,
    /// The package directory and its `src`.
    package: &'static str,
    names: [&'static str; 2],
    refs: [&'static str; 2],
    /// A marker line naming a node of the project.
    marker: &'static str,
    /// A comment naming another node (a mention).
    mention: &'static str,
    /// The `[code]` tables the CodeTable step cycles through.
    code_tables: [&'static str; 3],
    /// The `[ids]` line the Ids step toggles, and the comment it turns
    /// into a mention (written at setup).
    ids_line: &'static str,
    ids_comment: &'static str,
    /// The fixture's own split twin (`CrossTwin`), if it has one.
    cross_twin: Option<(&'static str, &'static str, &'static str)>,
}

const INKWELL: Project = Project {
    fixture: FIXTURE,
    slug: SLUG,
    package: "crates/inkwell",
    names: ["inkwell-core", "inkwell-press"],
    refs: ["TENET-WIDTH", "TENET-ORDER"],
    marker: "// @implements TENET-WIDTH@2",
    mention: "// see QUILL-02 and SLAB-INK",
    code_tables: [
        "[code]\nroots = [\"crates\", \"data\"]\n",
        "[code]\nroots = [\"crates\", \"data\"]\nexclude = [\"crates/inkwell/src/gen/**\"]\n",
        "[code]\nroots = [\"crates\"]\n",
    ],
    ids_line: "NIB   = { kind = \"nib\", width = 2 }\n",
    ids_comment: "// a nib: NIB-07\n",
    cross_twin: Some((
        "crates/inkwell/src/split/mod.rs",
        "pub fn joined()",
        "pub fn apart()",
    )),
};

const GAME: Project = Project {
    fixture: ECS,
    slug: "verify-ecs",
    package: "crates/game",
    names: ["game", "game-two"],
    refs: ["PERK-DRAIN", "TUNE-01"],
    marker: "// @implements PERK-DRAIN",
    mention: "// see PERK-WALK and TUNE-02",
    code_tables: [
        "[code]\nroots = [\"crates\", \"assets\"]\n",
        "[code]\nroots = [\"crates\", \"assets\"]\nexclude = [\"crates/game/src/gen/**\"]\n",
        "[code]\nroots = [\"crates\"]\n",
    ],
    ids_line: "NIB  = { kind = \"nib\", width = 2 }\n",
    ids_comment: "// a nib: NIB-07\n",
    cross_twin: None,
};

/// A member of the generated twin file.
#[derive(Debug, Clone)]
enum Twin {
    Fn(Option<&'static str>),
    Impl,
}

const PREDICATES: [&str; 4] = ["unix", "not(unix)", "test", "feature = \"fast\""];

/// The state the steps change and the files they write.
struct World<'a> {
    project: &'a Project,
    scratch: &'a Scratch,
    root: PathBuf,
    home: PathBuf,
    rng: Rng,
    counter: usize,
    /// Generated `.rs`/`.ron` files that exist, root-relative.
    generated: Vec<String>,
    twins: Vec<Twin>,
    seed_digit: u8,
    renamed: bool,
    nested: bool,
    code_table: usize,
    ids: bool,
    stamp_flip: bool,
    path_attribute: bool,
    cross_twin_away: bool,
    log: Vec<String>,
}

impl World<'_> {
    fn src(&self) -> String {
        format!("{}/src", self.project.package)
    }

    fn twin_file(&self) -> String {
        format!("{}/twin_gen.rs", self.src())
    }

    fn write_twins(&self) {
        let mut text = format!(
            "//! Twins written by TE-2.\n{}\npub struct Pad;\n\npub const SEED: u32 = {};\n",
            self.project.ids_comment, self.seed_digit
        );
        for (at, twin) in self.twins.iter().enumerate() {
            match twin {
                Twin::Fn(Some(predicate)) => text.push_str(&format!(
                    "\n#[cfg({predicate})]\npub fn pair() -> u32 {{\n    {at}\n}}\n"
                )),
                Twin::Fn(None) => {
                    text.push_str(&format!("\npub fn pair() -> u32 {{\n    {at}\n}}\n"));
                }
                Twin::Impl => text.push_str(&format!(
                    "\n{}\nimpl Pad {{\n    pub fn at_{at}(&self) {{}}\n}}\n",
                    self.project.marker
                )),
            }
        }
        write(&self.root, &self.twin_file(), text);
    }

    /// Every `.rs` and `.ron` file under the project's code (not `.git`).
    fn code_files(&self) -> Vec<String> {
        common::snapshot(&self.root)
            .into_iter()
            .filter(|(path, bytes)| {
                bytes.is_some()
                    && !path.starts_with(".git")
                    && (path.ends_with(".rs") || path.ends_with(".ron"))
            })
            .map(|(path, _)| path)
            .collect()
    }

    fn step(&mut self, kind: Kind) -> String {
        self.counter += 1;
        let n = self.counter;
        match kind {
            Kind::AddFile => {
                let path = match self.rng.below(3) {
                    0 => format!("{}/gen_{n}.rs", self.src()),
                    1 => format!("{}/gen/deep_{n}.rs", self.src()),
                    _ => format!("{}/bin/tool_{n}.rs", self.src()),
                };
                let mut text = format!(
                    "{}\npub fn made_{n}() -> u32 {{\n    {n}\n}}\n",
                    self.project.marker
                );
                if self.rng.chance() {
                    text.push_str(&format!(
                        "\n{}\npub struct Made{n};\n\nimpl Made{n} {{}}\n\nimpl Made{n} {{}}\n",
                        self.project.mention
                    ));
                }
                write(&self.root, &path, text);
                self.generated.push(path.clone());
                format!("add {path}")
            }
            Kind::RemoveFile => {
                if self.generated.is_empty() {
                    return self.step(Kind::AddFile);
                }
                let at = self.rng.below(self.generated.len());
                let path = self.generated.remove(at);
                fs::remove_file(self.root.join(&path)).expect("remove");
                format!("remove {path}")
            }
            Kind::RenameFile => {
                if self.generated.is_empty() {
                    return self.step(Kind::AddFile);
                }
                let at = self.rng.below(self.generated.len());
                let from = self.generated.remove(at);
                let to = if self.rng.chance() {
                    format!("{}/moved_{n}.rs", self.src())
                } else {
                    format!("{}/sub_{n}/mod.rs", self.src())
                };
                fs::create_dir_all(self.root.join(&to).parent().unwrap()).unwrap();
                fs::rename(self.root.join(&from), self.root.join(&to)).expect("rename");
                self.generated.push(to.clone());
                format!("rename {from} -> {to}")
            }
            Kind::EditFile => {
                let files: Vec<String> = self
                    .code_files()
                    .into_iter()
                    .filter(|path| path.ends_with(".rs") && *path != self.twin_file())
                    .collect();
                let path = files[self.rng.below(files.len())].clone();
                let text = fs::read_to_string(self.root.join(&path)).unwrap();
                let edited = if self.rng.chance() {
                    format!("\n\n{text}")
                } else {
                    format!(
                        "{text}\n{}\npub fn edited_{n}() {{}}\n{}\n",
                        self.project.marker, self.project.mention
                    )
                };
                write(&self.root, &path, edited);
                format!("edit {path}")
            }
            Kind::SameSizeEdit => {
                self.write_twins();
                let digit = (self.seed_digit + 1 + u8::try_from(self.rng.below(8)).unwrap()) % 10;
                same_size_same_mtime_edit(
                    &self.root,
                    &self.twin_file(),
                    &format!("SEED: u32 = {};", self.seed_digit),
                    &format!("SEED: u32 = {digit};"),
                );
                self.seed_digit = digit;
                format!("same-size edit to {digit}")
            }
            Kind::Manifest => {
                let manifest = format!("{}/Cargo.toml", self.project.package);
                let [one, two] = self.project.names;
                let (from, to) = if self.renamed { (two, one) } else { (one, two) };
                let text = read_text(&self.root, &manifest);
                write(
                    &self.root,
                    &manifest,
                    text.replacen(
                        &format!("name = \"{from}\""),
                        &format!("name = \"{to}\""),
                        1,
                    ),
                );
                self.renamed = !self.renamed;
                format!("package {from} -> {to}")
            }
            Kind::NestedManifest => {
                let manifest = format!("{}/vendor/side/Cargo.toml", self.project.package);
                if self.nested {
                    fs::remove_file(self.root.join(&manifest)).unwrap();
                } else {
                    write(&self.root, &manifest, "[package]\nname = \"side\"\n");
                }
                self.nested = !self.nested;
                format!("nested manifest {}", if self.nested { "on" } else { "off" })
            }
            Kind::CodeTable => {
                let next = (self.code_table + 1 + self.rng.below(2)) % 3;
                code_index_common::with_code_table(&self.root, self.project.code_tables[next]);
                self.code_table = next;
                format!("[code] #{next}")
            }
            Kind::Ids => {
                let text = read_text(&self.root, "specengine.toml");
                if self.ids {
                    write(
                        &self.root,
                        "specengine.toml",
                        text.replacen(self.project.ids_line, "", 1),
                    );
                } else {
                    code_index_common::with_ids_line(&self.root, self.project.ids_line);
                }
                self.ids = !self.ids;
                format!("[ids] NIB {}", if self.ids { "on" } else { "off" })
            }
            Kind::Stamp => {
                let database = db(&self.home, self.project.slug);
                self.stamp_flip = !self.stamp_flip;
                if self.stamp_flip {
                    execute(
                        &database,
                        "UPDATE code_worktrees SET code_fp = 'b3:another-analyzer'",
                    );
                    "code_fp rewritten".to_owned()
                } else {
                    execute(
                        &database,
                        "UPDATE index_meta SET value = '0' WHERE key = 'format'",
                    );
                    "index format rewritten".to_owned()
                }
            }
            Kind::Crlf => {
                let files = self.code_files();
                let path = files[self.rng.below(files.len())].clone();
                let bytes = fs::read(self.root.join(&path)).unwrap();
                if bytes.windows(2).any(|pair| pair == b"\r\n") {
                    to_lf(&self.root, &path);
                    format!("LF {path}")
                } else {
                    to_crlf(&self.root, &path);
                    format!("CRLF {path}")
                }
            }
            Kind::TwinInsert => {
                let twin = match self.rng.below(4) {
                    0 => Twin::Impl,
                    1 => Twin::Fn(None),
                    _ => Twin::Fn(Some(PREDICATES[self.rng.below(PREDICATES.len())])),
                };
                let at = self.rng.below(self.twins.len() + 1);
                self.twins.insert(at, twin.clone());
                self.write_twins();
                format!("twin {twin:?} at {at}")
            }
            Kind::TwinRemove => {
                if self.twins.is_empty() {
                    return self.step(Kind::TwinInsert);
                }
                let at = self.rng.below(self.twins.len());
                let twin = self.twins.remove(at);
                self.write_twins();
                format!("twin {twin:?} removed at {at}")
            }
            Kind::TwinCfgEdit => {
                let fns: Vec<usize> = (0..self.twins.len())
                    .filter(|&at| matches!(self.twins[at], Twin::Fn(_)))
                    .collect();
                if fns.is_empty() {
                    self.twins.push(Twin::Fn(Some("unix")));
                    self.twins.push(Twin::Fn(Some("not(unix)")));
                    self.write_twins();
                    return "cfg twins seeded".to_owned();
                }
                let at = fns[self.rng.below(fns.len())];
                let before = self.twins[at].clone();
                self.twins[at] = match before {
                    Twin::Fn(Some(_)) if self.rng.chance() => Twin::Fn(None),
                    _ => Twin::Fn(Some(PREDICATES[self.rng.below(PREDICATES.len())])),
                };
                self.write_twins();
                format!("twin {at}: {before:?} -> {:?}", self.twins[at])
            }
            Kind::PathAttribute => {
                let lib = format!("{}/lib.rs", self.src());
                to_lf(&self.root, &lib);
                let text = read_text(&self.root, &lib);
                let attribute = "#[path = \"extra/moved.rs\"]\nmod moved;\n";
                let plain = "mod moved;\n";
                let edited = if self.path_attribute {
                    text.replacen(attribute, plain, 1)
                } else {
                    text.replacen(plain, attribute, 1)
                };
                assert_ne!(edited, text, "{lib}: the declaration is there");
                write(&self.root, &lib, edited);
                self.path_attribute = !self.path_attribute;
                format!("#[path] {}", if self.path_attribute { "on" } else { "off" })
            }
            Kind::CrossTwin => {
                let Some((path, joined, apart)) = self.project.cross_twin else {
                    return self.step(Kind::TwinInsert);
                };
                to_lf(&self.root, path);
                let text = read_text(&self.root, path);
                let (from, to) = if self.cross_twin_away {
                    (apart, joined)
                } else {
                    (joined, apart)
                };
                write(&self.root, path, text.replacen(from, to, 1));
                self.cross_twin_away = !self.cross_twin_away;
                format!("cross twin {from} -> {to}")
            }
        }
    }
}

fn env_number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// TE-2 over `project`: the setup (the files a step toggles), a first
/// index, then the steps, each checked against a fresh index.
fn run_steps(project: &Project, seed: u64) {
    let scratch = Scratch::new("ci-steps");
    let root = copy(&scratch, project.fixture, "copy");
    let home = scratch.home("h");
    // What some steps toggle: a file a nested manifest re-parents, a
    // `#[path]` target and its plain declaration.
    write(
        &root,
        &format!("{}/vendor/side/src/lib.rs", project.package),
        format!("{}\npub fn side() {{}}\n", project.marker),
    );
    write(
        &root,
        &format!("{}/src/extra/moved.rs", project.package),
        format!("{}\npub fn shift() {{}}\n", project.marker),
    );
    let lib = format!("{}/src/lib.rs", project.package);
    let text = read_text(&root, &lib);
    write(&root, &lib, format!("{text}\nmod moved;\n"));
    let mut world = World {
        project,
        scratch: &scratch,
        root: root.clone(),
        home: home.clone(),
        rng: Rng(seed | 1),
        counter: 0,
        generated: Vec::new(),
        twins: vec![
            Twin::Fn(Some("unix")),
            Twin::Fn(Some("not(unix)")),
            Twin::Impl,
            Twin::Impl,
        ],
        seed_digit: 5,
        renamed: false,
        nested: false,
        code_table: 0,
        ids: false,
        stamp_flip: false,
        path_attribute: false,
        cross_twin_away: false,
        log: Vec::new(),
    };
    world.write_twins();
    spec(&home, &root, &["index"]).code(0);

    let steps = env_number("SPECENGINE_TE2_STEPS", 80).max(KINDS.len() as u64);
    let budget = Duration::from_secs(env_number("SPECENGINE_TE2_SECONDS", 150));
    let started = Instant::now();
    let mut round: Vec<Kind> = KINDS.to_vec();
    for at in (1..round.len()).rev() {
        let other = world.rng.below(at + 1);
        round.swap(at, other);
    }
    let mut step = 0;
    while (step as u64) < steps {
        if step >= KINDS.len() && started.elapsed() > budget {
            break;
        }
        let kind = if step < round.len() {
            round[step]
        } else {
            KINDS[world.rng.below(KINDS.len())]
        };
        let what = world.step(kind);
        world.log.push(format!("{step}: {kind:?}: {what}"));
        check(&world, seed, step);
        step += 1;
    }
    assert!(step >= KINDS.len(), "every kind ran: {:?}", world.log);
    eprintln!(
        "TE-2 {}: seed {seed}, {step} steps in {:.1} s",
        project.fixture,
        started.elapsed().as_secs_f64()
    );
}

/// The running index refreshed equals a fresh one; `refs` alike.
fn check(world: &World<'_>, seed: u64, step: usize) {
    let project = world.project;
    let context = || {
        format!(
            "{}: seed {seed} (SPECENGINE_TE2_SEED), step {step}:\n{}",
            project.fixture,
            world.log.join("\n")
        )
    };
    let run = spec(&world.home, &world.root, &["index"]);
    assert_eq!(run.code, 0, "{}\n{}", context(), run.show());
    let have = dump(&world.home, &world.root, project.slug);
    let fresh = world.scratch.home(&format!("fresh-{step}"));
    let run = spec(&fresh, &world.root, &["index"]);
    assert_eq!(run.code, 0, "{}\n{}", context(), run.show());
    let want = dump(&fresh, &world.root, project.slug);
    assert!(
        have == want,
        "{}\nthe index differs from a fresh rebuild:\n{}",
        context(),
        dump_diff(&have, &want)
    );
    for reference in project.refs {
        let a = spec(&world.home, &world.root, &["refs", reference]);
        let b = spec(&fresh, &world.root, &["refs", reference]);
        assert_eq!(
            (a.code, &a.stdout, &a.stderr),
            (b.code, &b.stdout, &b.stderr),
            "{}\nrefs {reference}",
            context()
        );
    }
    let _ = fs::remove_dir_all(&fresh);
}

fn seed() -> u64 {
    env_number("SPECENGINE_TE2_SEED", 0x6a2_5eed_1d0c)
}

/// TE-2 over code-index. M: each RD-2 and RD-3 mutation.
#[test]
fn seeded_steps_keep_the_index_equal_to_a_rebuild() {
    run_steps(&INKWELL, seed());
}

/// TE-2 over verify-ecs, the ECS-style crate (ADR-0008): the same steps,
/// none of them knowing the project.
#[test]
fn seeded_steps_keep_the_ecs_index_equal_to_a_rebuild() {
    run_steps(&GAME, seed().rotate_left(17));
}

/// The step generator itself is a pure function of the seed.
#[test]
fn the_step_order_is_a_function_of_the_seed() {
    let order = |seed: u64| {
        let mut rng = Rng(seed | 1);
        (0..64).map(|_| rng.below(16)).collect::<Vec<_>>()
    };
    assert_eq!(order(7), order(7));
    assert_ne!(order(7), order(9));
}
