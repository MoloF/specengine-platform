//! docs/features/bevy-roles.md TE-2 ("Rules and edge cases": WHEN any
//! sequence of `.rs`, manifest, `[code]` (`frameworks` too), `[ids]` or
//! stamp changes is indexed, the index dump SHALL equal a fresh rebuild,
//! `code_roles` in): a seeded random sequence of steps over a scratch copy
//! of `fixtures/code-bevy` and of `fixtures/verify-ecs` opted in (an
//! ECS-style crate of its own `App`: ADR-0008's second project, the same
//! steps knowing neither). After every step `spec index` in the running
//! `HOME` gives the dump a fresh `spec index` in a new `HOME` gives, byte
//! for byte (every table), and the same `roles` line and JSON
//! `code.roles` (links are computed per read: a stored link or a
//! cross-file pass shows here).
//!
//! The steps: a file with derives and registrations added, removed,
//! renamed; a file edited (lines shifted, a message type and its
//! registration appended); a registered system renamed away and back (its
//! registration unlinked and re-linked, its own file not re-derived);
//! `frameworks` on, off, `[]`; the stored stamps rewritten (the index
//! format, the analyzer's `code_fp`); a file made CRLF and back; a derive
//! word swapped for another of its size with the mtime put back; the
//! package renamed in `Cargo.toml`; a `cfg` twin's predicate changed or
//! dropped beside a bare registration of the twins; an `[ids]` prefix
//! added and removed (every code file re-derived). Every kind runs once in
//! a seeded order first, then kinds at random.
//!
//! Bounded: `SPECENGINE_TE2_STEPS` steps (default 60) and
//! `SPECENGINE_TE2_SECONDS` (default 120) past the first round;
//! `SPECENGINE_TE2_SEED` replays a failure (the message names the seed and
//! every step so far).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_roles_common::roles_line;
use code_index_common::{
    db, dump, dump_diff, execute, same_size_same_mtime_edit, to_crlf, to_lf, with_code_table,
    with_ids_line,
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
    RenameSystem,
    Frameworks,
    Stamp,
    Crlf,
    SameSizeEdit,
    Manifest,
    CfgTwin,
    Ids,
}

const KINDS: [Kind; 12] = [
    Kind::AddFile,
    Kind::RemoveFile,
    Kind::RenameFile,
    Kind::EditFile,
    Kind::RenameSystem,
    Kind::Frameworks,
    Kind::Stamp,
    Kind::Crlf,
    Kind::SameSizeEdit,
    Kind::Manifest,
    Kind::CfgTwin,
    Kind::Ids,
];

/// The `[ids]` line the Ids step toggles (a prefix no fixture uses).
const IDS_LINE: &str = "NIB  = { kind = \"nib\", width = 2 }\n";

/// One project the steps run over.
struct Project {
    fixture: &'static str,
    slug: &'static str,
    /// The package directory the generated files go to.
    package: &'static str,
    /// Its `[package] name` and the one the Manifest step toggles to.
    names: [&'static str; 2],
    /// `[code]` with `frameworks` on, off and `[]`.
    code_tables: [&'static str; 3],
    /// A registered system's definition: (file, as written, renamed).
    rename: (&'static str, &'static str, &'static str),
    /// Files written at setup (the project's own registrations), if any.
    setup: &'static [(&'static str, &'static str)],
}

const FARM: Project = Project {
    fixture: "code-bevy",
    slug: "code-bevy",
    package: "crates/farm",
    names: ["farm", "farm-two"],
    code_tables: [
        "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = [\"bevy\"]\n",
        "[code]\nroots = [\"crates\", \"assets\"]\n",
        "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = []\n",
    ],
    rename: (
        "crates/farm/src/herd.rs",
        "pub fn spawn_every(",
        "pub fn spawn_each(",
    ),
    setup: &[],
};

const GAME: Project = Project {
    fixture: "verify-ecs",
    slug: "verify-ecs",
    package: "crates/game",
    names: ["game", "game-two"],
    code_tables: [
        "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = [\"bevy\"]\n",
        "[code]\nroots = [\"crates\", \"assets\"]\n",
        "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = []\n",
    ],
    rename: (
        "crates/game/src/stamina.rs",
        "pub fn drain_system(",
        "pub fn drain_systems(",
    ),
    setup: &[(
        "crates/game/src/wiring.rs",
        "use bevy::prelude::*;\n\npub fn wire(app: &mut App) {\n    app.add_systems(Update, (crate::stamina::regen_system, drain_system));\n}\n",
    )],
};

/// The predicates the CfgTwin step gives the first twin (`None`: no
/// `cfg`).
const PREDICATES: [Option<&str>; 4] =
    [Some("unix"), Some("test"), Some("feature = \"fast\""), None];

/// The state the steps change and the files they write.
struct World<'a> {
    project: &'a Project,
    root: PathBuf,
    home: PathBuf,
    rng: Rng,
    counter: usize,
    /// Generated `.rs` files that exist, root-relative.
    generated: Vec<String>,
    renamed_system: bool,
    code_table: usize,
    stamp_flip: bool,
    manifest_renamed: bool,
    event_seed: bool,
    twin: usize,
    ids: bool,
    log: Vec<String>,
}

impl World<'_> {
    fn src(&self) -> String {
        format!("{}/src", self.project.package)
    }

    fn twin_file(&self) -> String {
        format!("{}/twin_gen.rs", self.src())
    }

    fn seed_file(&self) -> String {
        format!("{}/role_gen.rs", self.src())
    }

    /// Two `pair` twins (the first's predicate the step's), the second
    /// `not(unix)`, and a bare registration of `pair`.
    fn write_twins(&self) {
        let first = match PREDICATES[self.twin] {
            Some(predicate) => format!("#[cfg({predicate})]\npub fn pair() {{}}\n"),
            None => "pub fn pair() {}\n".to_owned(),
        };
        write(
            &self.root,
            &self.twin_file(),
            format!(
                "use bevy::prelude::*;\n\n{first}\n#[cfg(not(unix))]\npub fn pair() {{}}\n\npub fn wire_pairs(app: &mut App) {{\n    app.add_systems(Update, pair);\n}}\n"
            ),
        );
    }

    /// Every `.rs` file under the project's code (not `.git`).
    fn rust_files(&self) -> Vec<String> {
        common::snapshot(&self.root)
            .into_iter()
            .filter(|(path, bytes)| {
                bytes.is_some() && !path.starts_with(".git") && path.ends_with(".rs")
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
                write(
                    &self.root,
                    &path,
                    format!(
                        "use bevy::prelude::*;\n\n#[derive(Component, Reflect)]\npub struct Made{n};\n\npub fn made_{n}() {{}}\n\npub fn wire_{n}(app: &mut App) {{\n    app.add_systems(Update, (made_{n}, pair));\n    app.add_message::<Made{n}>();\n}}\n"
                    ),
                );
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
                fs::create_dir_all(self.root.join(&to).parent().expect("a parent"))
                    .expect("a directory");
                fs::rename(self.root.join(&from), self.root.join(&to)).expect("rename");
                self.generated.push(to.clone());
                format!("rename {from} -> {to}")
            }
            Kind::EditFile => {
                let files: Vec<String> = self
                    .rust_files()
                    .into_iter()
                    .filter(|path| *path != self.twin_file() && *path != self.seed_file())
                    .collect();
                let path = files[self.rng.below(files.len())].clone();
                let text = fs::read_to_string(self.root.join(&path)).expect("UTF-8");
                let edited = if self.rng.chance() {
                    format!("\n\n{text}")
                } else {
                    format!(
                        "{text}\n#[derive(Message)]\npub struct Edited{n};\n\npub fn edited_{n}(app: &mut App) {{\n    app.add_message::<Edited{n}>();\n}}\n"
                    )
                };
                write(&self.root, &path, edited);
                format!("edit {path}")
            }
            Kind::RenameSystem => {
                let (file, written, renamed) = self.project.rename;
                let (from, to) = if self.renamed_system {
                    (renamed, written)
                } else {
                    (written, renamed)
                };
                common::replace(&self.root, file, from, to);
                self.renamed_system = !self.renamed_system;
                format!("system {from} -> {to}")
            }
            Kind::Frameworks => {
                let next = (self.code_table + 1 + self.rng.below(2)) % 3;
                with_code_table(&self.root, self.project.code_tables[next]);
                self.code_table = next;
                format!("frameworks #{next} (0 on, 1 off, 2 [])")
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
                let files = self.rust_files();
                let path = files[self.rng.below(files.len())].clone();
                let bytes = fs::read(self.root.join(&path)).expect("read");
                if bytes.windows(2).any(|pair| pair == b"\r\n") {
                    to_lf(&self.root, &path);
                    format!("LF {path}")
                } else {
                    to_crlf(&self.root, &path);
                    format!("CRLF {path}")
                }
            }
            Kind::SameSizeEdit => {
                let (from, to) = if self.event_seed {
                    ("#[derive(Event)]", "#[derive(Debug)]")
                } else {
                    ("#[derive(Debug)]", "#[derive(Event)]")
                };
                same_size_same_mtime_edit(&self.root, &self.seed_file(), from, to);
                self.event_seed = !self.event_seed;
                format!("same-size {from} -> {to}")
            }
            Kind::Manifest => {
                let manifest = format!("{}/Cargo.toml", self.project.package);
                let [one, two] = self.project.names;
                let (from, to) = if self.manifest_renamed {
                    (two, one)
                } else {
                    (one, two)
                };
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
                self.manifest_renamed = !self.manifest_renamed;
                format!("package {from} -> {to}")
            }
            Kind::CfgTwin => {
                let next =
                    (self.twin + 1 + self.rng.below(PREDICATES.len() - 1)) % PREDICATES.len();
                self.twin = next;
                self.write_twins();
                format!("twin predicate {:?}", PREDICATES[next])
            }
            Kind::Ids => {
                let text = read_text(&self.root, "specengine.toml");
                if self.ids {
                    write(
                        &self.root,
                        "specengine.toml",
                        text.replacen(IDS_LINE, "", 1),
                    );
                } else {
                    with_ids_line(&self.root, IDS_LINE);
                }
                self.ids = !self.ids;
                format!("[ids] NIB {}", if self.ids { "on" } else { "off" })
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

/// TE-2 over `project`: the setup, a first index, then the steps, each
/// checked against a fresh index.
fn run_steps(project: &Project, seed: u64) {
    let scratch = Scratch::new("br-steps");
    let root = code_index_common::copy(&scratch, project.fixture, "copy");
    let home = scratch.home("h");
    with_code_table(&root, project.code_tables[0]);
    for (path, text) in project.setup {
        write(&root, path, text);
    }
    let mut world = World {
        project,
        root: root.clone(),
        home: home.clone(),
        rng: Rng(seed | 1),
        counter: 0,
        generated: Vec::new(),
        renamed_system: false,
        code_table: 0,
        stamp_flip: false,
        manifest_renamed: false,
        event_seed: true,
        twin: 0,
        ids: false,
        log: Vec::new(),
    };
    write(
        &root,
        &world.seed_file(),
        "use bevy::prelude::*;\n\n#[derive(Event)]\npub struct Seed;\n",
    );
    world.write_twins();
    spec(&home, &root, &["index"]).code(0);
    check(&world, &scratch, seed, 0);

    let steps = env_number("SPECENGINE_TE2_STEPS", 60).max(KINDS.len() as u64);
    let budget = Duration::from_secs(env_number("SPECENGINE_TE2_SECONDS", 120));
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
        check(&world, &scratch, seed, step + 1);
        step += 1;
    }
    assert!(step >= KINDS.len(), "every kind ran: {:?}", world.log);
    eprintln!(
        "TE-2 {}: seed {seed}, {step} steps in {:.1} s",
        project.fixture,
        started.elapsed().as_secs_f64()
    );
}

/// The running index refreshed equals a fresh one, every table; the
/// `roles` line and JSON `code.roles` alike.
fn check(world: &World<'_>, scratch: &Scratch, seed: u64, step: usize) {
    let project = world.project;
    let context = || {
        format!(
            "{}: seed {seed} (SPECENGINE_TE2_SEED), step {step}:\n{}",
            project.fixture,
            world.log.join("\n")
        )
    };
    let running = spec(&world.home, &world.root, &["index"]);
    assert_eq!(running.code, 0, "{}\n{}", context(), running.show());
    let have = dump(&world.home, &world.root, project.slug);
    let fresh_home = scratch.home(&format!("fresh-{step}"));
    let fresh = spec(&fresh_home, &world.root, &["index"]);
    assert_eq!(fresh.code, 0, "{}\n{}", context(), fresh.show());
    let want = dump(&fresh_home, &world.root, project.slug);
    assert!(
        have == want,
        "{}\nthe index differs from a fresh rebuild:\n{}",
        context(),
        dump_diff(&have, &want)
    );
    assert_eq!(
        roles_line(&running),
        roles_line(&fresh),
        "{}\nthe roles line",
        context()
    );
    let a = spec(&world.home, &world.root, &["--json", "index"]).json();
    let b = spec(&fresh_home, &world.root, &["--json", "index"]).json();
    assert_eq!(
        a["code"]["roles"],
        b["code"]["roles"],
        "{}\nJSON code.roles",
        context()
    );
    let _ = fs::remove_dir_all(&fresh_home);
}

fn seed() -> u64 {
    env_number("SPECENGINE_TE2_SEED", 0x10a1_5eed_b0e5)
}

/// TE-2 over code-bevy. M: each RD-3 mutation (a link stored, a
/// cross-file pass, `frameworks` outside the stamp).
#[test]
fn seeded_steps_keep_the_role_index_equal_to_a_rebuild() {
    run_steps(&FARM, seed());
}

/// TE-2 over verify-ecs opted in, the ECS-style crate (ADR-0008): the
/// same steps, none of them knowing the project.
#[test]
fn seeded_steps_keep_the_ecs_role_index_equal_to_a_rebuild() {
    run_steps(&GAME, seed().rotate_left(17));
}

/// The step generator itself is a pure function of the seed.
#[test]
fn the_role_step_order_is_a_function_of_the_seed() {
    let order = |seed: u64| {
        let mut rng = Rng(seed | 1);
        (0..64).map(|_| rng.below(KINDS.len())).collect::<Vec<_>>()
    };
    assert_eq!(order(7), order(7));
    assert_ne!(order(7), order(9));
}
