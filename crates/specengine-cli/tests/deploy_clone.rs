//! docs/features/hub-deploy.md (RM-0064, Phase 7 step 11b-1) D16:
//! `scripts/deploy/clone.sh`, run by `sh` with the built `spec` first on
//! `PATH`, against a bare repository in the scratch folder by `file://` URL
//! with `GIT_ALLOW_PROTOCOL=file` (a path URL would ignore `--depth` and
//! `--filter`, and the mutations would not bite). The bare repository allows
//! filters (`uploadpack.allowFilter`), so a `--filter` clone would come out
//! partial instead of silently whole. Its project: two commits,
//! `specengine.toml` naming `roots = ["docs"]`, `docs/a.md` and
//! `src/main.rs`. Every git process runs in the CLI tests' `common::git`
//! sandbox, the clone from a template that turns automatic maintenance off.
//!
//! Beside the criterion, from "Data": the stdout line's count is the number
//! of paths `spec sparse-set` prints, the sparse patterns are `/<path>` of
//! each, non-cone; a name with a line break, a 65-byte name and five
//! arguments are refused too; a 64-byte name is a name.
//!
//! Iteration 2: a shallow source, a bare repository made with `git clone
//! --bare --depth 1` of the source by `file://`, is refused with exit 1 and
//! no folder (`--reject-shallow`).

#![cfg(unix)]

mod common;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::git::{Sandbox, quiet_config_text};
use common::{Run, SPEC, Scratch, program_with, repository_root, snapshot, spec_with, write};

const CONFIG: &str = "[project]\nname = \"clone-mini\"\nslug = \"clone-mini\"\n\n[paths]\nroots = [\"docs\"]\n\n[ids]\nR = { kind = \"requirement\", width = 4 }\n";

const COMMITTER: &str = "Hub Operator";
const EMAIL: &str = "hub@example.invalid";

/// A bare repository of the two-commit project, served by `file://` URL.
struct World {
    scratch: Scratch,
    git: Sandbox,
    templates: PathBuf,
    url: String,
}

fn world(label: &str) -> World {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let templates = scratch.dir("templates");
    fs::write(templates.join("config"), quiet_config_text()).expect("the template config");

    let work = scratch.dir("work");
    git.init(&work);
    write(&work, "specengine.toml", CONFIG);
    write(&work, "docs/a.md", "# A\n\nThe first text.\n");
    write(&work, "src/main.rs", "fn main() {}\n");
    git.add_all(&work);
    git.commit(&work, "one");
    write(&work, "docs/a.md", "# A\n\nThe second text.\n");
    write(
        &work,
        "src/main.rs",
        "fn main() {\n    println!(\"two\");\n}\n",
    );
    git.add_all(&work);
    git.commit(&work, "two");

    let bare = scratch.join("forge.git");
    git.init_with(&bare, &["--bare"]);
    // A server refusing filters turns a `--filter` clone into a whole one.
    git.git(&bare, &["config", "uploadpack.allowFilter", "true"]);
    git.git(&bare, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    git.git(
        &work,
        &["push", "-q", bare.to_str().expect("a UTF-8 path"), "main"],
    );
    let url = format!("file://{}", bare.display());
    World {
        scratch,
        git,
        templates,
        url,
    }
}

impl World {
    /// The sandbox's variables, `PATH` with the built `spec` first, and
    /// `extra`.
    fn vars(&self, extra: &[(&str, &OsStr)]) -> Vec<(String, OsString)> {
        let bin = Path::new(SPEC).parent().expect("the binary's folder");
        let mut path = OsString::from(bin);
        path.push(":");
        path.push(self.git.var("PATH").expect("the sandbox's PATH"));
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .filter_map(|(name, value)| name.into_string().ok().map(|name| (name, value)))
            .filter(|(name, _)| name != "PATH")
            .collect();
        vars.push(("PATH".to_owned(), path));
        for (name, value) in extra {
            vars.push(((*name).to_owned(), (*value).to_owned()));
        }
        vars
    }

    /// `sh clone.sh args` under `SPECENGINE_PROJECTS=projects`.
    fn clone_sh(&self, projects: &Path, args: &[&str]) -> Run {
        let script = repository_root().join("scripts/deploy/clone.sh");
        let vars = self.vars(&[
            ("SPECENGINE_PROJECTS", projects.as_os_str()),
            ("GIT_ALLOW_PROTOCOL", OsStr::new("file")),
            ("GIT_TEMPLATE_DIR", self.templates.as_os_str()),
        ]);
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
            .collect();
        let mut all = vec![script.to_str().expect("a UTF-8 path")];
        all.extend_from_slice(args);
        program_with(OsStr::new("/bin/sh"), self.scratch.path(), &all, &env)
    }

    /// `spec sparse-set --root <checkout> args` in the checkout.
    fn sparse_set(&self, checkout: &Path, args: &[&str]) -> Run {
        let vars = self.vars(&[]);
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
            .collect();
        let mut all = vec!["sparse-set", "--root", checkout.to_str().expect("UTF-8")];
        all.extend_from_slice(args);
        spec_with(checkout, &all, &env)
    }

    /// `git args` in `cwd`: its exit code and its stdout, trimmed.
    fn git_run(&self, cwd: &Path, args: &[&str], extra: &[(&str, &OsStr)]) -> (i32, String) {
        let output = self.git.git_output(cwd, args, extra);
        (
            output.status.code().expect("git exited"),
            String::from_utf8(output.stdout)
                .expect("UTF-8 git output")
                .trim()
                .to_owned(),
        )
    }
}

/// D16: the clone exits 0 with its line; the checkout is not shallow, has no
/// promisor remote or `extensions.partialClone`, and misses no object;
/// `src/main.rs` is absent from the worktree while `HEAD:src/main.rs` is
/// there with lazy fetching off; both commits; `@{u}` is `origin/main`;
/// `user.name` and `user.email` are the given ones; `spec sparse-set --json`
/// gives `"missing": []`. M: `--filter=blob:none`; `--depth 1`; skip the
/// sparse step.
#[test]
fn d16_a_clone_holds_every_object_and_commit_sparse_to_the_set() {
    let world = world("deploy-clone");
    let projects = world.scratch.dir("projects");
    let run = world.clone_sh(&projects, &[world.url.as_str(), "forge", COMMITTER, EMAIL]);
    run.code(0);
    let checkout = projects.join("forge");

    let set = world.sparse_set(&checkout, &[]);
    set.code(0);
    let paths: Vec<&str> = set.stdout.lines().collect();
    assert!(paths.contains(&"docs/"), "the set: {paths:?}");
    assert_eq!(
        run.stdout,
        format!(
            "cloned forge: main tracking origin/main, {} paths\n",
            paths.len()
        ),
        "{}",
        run.show()
    );
    let json = world.sparse_set(&checkout, &["--json"]);
    json.code(0);
    assert_eq!(
        json.json()["missing"],
        serde_json::json!([]),
        "{}",
        json.show()
    );
    assert!(!json.stderr.contains("warning:"), "no gap: {}", json.show());

    let none: &[(&str, &OsStr)] = &[];
    assert_eq!(
        world.git_run(&checkout, &["rev-parse", "--is-shallow-repository"], none),
        (0, "false".to_owned()),
        "not shallow"
    );
    for key in [
        "extensions.partialClone",
        "remote.origin.partialclonefilter",
    ] {
        assert_eq!(
            world.git_run(&checkout, &["config", "--get", key], none),
            (1, String::new()),
            "no {key}"
        );
    }
    assert_eq!(
        world.git_run(
            &checkout,
            &["config", "--get-regexp", r"^remote\..*\.promisor$"],
            none
        ),
        (1, String::new()),
        "no promisor remote"
    );
    let (code, objects) = world.git_run(
        &checkout,
        &["rev-list", "--objects", "--all", "--missing=print"],
        none,
    );
    assert_eq!(code, 0);
    assert!(
        !objects.lines().any(|line| line.starts_with('?')),
        "a missing object: {objects}"
    );
    assert!(
        !checkout.join("src").exists(),
        "src/ lies outside the sparse set"
    );
    assert!(checkout.join("docs/a.md").is_file());
    assert!(checkout.join("specengine.toml").is_file());
    assert_eq!(
        fs::read_to_string(checkout.join("docs/a.md")).expect("docs/a.md"),
        "# A\n\nThe second text.\n"
    );
    let no_lazy = [("GIT_NO_LAZY_FETCH", OsStr::new("1"))];
    assert_eq!(
        world
            .git_run(&checkout, &["cat-file", "-e", "HEAD:src/main.rs"], &no_lazy)
            .0,
        0,
        "the blob is local"
    );
    assert_eq!(
        world.git_run(&checkout, &["rev-list", "--count", "HEAD"], none),
        (0, "2".to_owned()),
        "every commit"
    );
    assert_eq!(
        world.git_run(
            &checkout,
            &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
            none
        ),
        (0, "origin/main".to_owned())
    );
    for (key, want) in [("user.name", COMMITTER), ("user.email", EMAIL)] {
        assert_eq!(
            world.git_run(&checkout, &["config", "--local", "--get", key], none),
            (0, want.to_owned()),
            "{key}"
        );
    }
    // "Data" 3-4: the set as anchored, non-cone patterns.
    let (code, patterns) = world.git_run(&checkout, &["sparse-checkout", "list"], none);
    assert_eq!(code, 0);
    let want: Vec<String> = paths.iter().map(|path| format!("/{path}")).collect();
    assert_eq!(
        patterns.lines().collect::<Vec<_>>(),
        want,
        "the sparse patterns"
    );
    assert_ne!(
        world
            .git_run(
                &checkout,
                &["config", "--get", "core.sparseCheckoutCone"],
                none
            )
            .1,
        "true",
        "non-cone"
    );
}

/// D16: a bad name, an existing folder, three arguments (and five) exit 2
/// and clone nothing; a 64-byte name is a name.
#[test]
fn d16_a_bad_name_a_taken_folder_or_a_wrong_count_clones_nothing() {
    let world = world("deploy-clone-bad");
    let projects = world.scratch.dir("projects");
    write(&projects, "taken/marker.txt", "kept\n");
    let before = snapshot(&projects);
    let long = "a".repeat(65);
    let names = [
        "Forge",
        "-forge",
        "for_ge",
        "a/b",
        "..",
        "",
        "forge ",
        "forge\nx",
        long.as_str(),
        "taken",
    ];
    for name in names {
        let run = world.clone_sh(&projects, &[world.url.as_str(), name, COMMITTER, EMAIL]);
        assert_eq!(run.code, 2, "name {name:?}: {}", run.show());
        assert_eq!(snapshot(&projects), before, "name {name:?}: nothing cloned");
    }
    for args in [
        vec![world.url.as_str(), "three", COMMITTER],
        vec![world.url.as_str(), "five", COMMITTER, EMAIL, "extra"],
    ] {
        let run = world.clone_sh(&projects, &args);
        assert_eq!(run.code, 2, "{} arguments: {}", args.len(), run.show());
        assert_eq!(snapshot(&projects), before, "{} arguments", args.len());
    }
    let edge = "a".repeat(64);
    world
        .clone_sh(
            &projects,
            &[world.url.as_str(), edge.as_str(), COMMITTER, EMAIL],
        )
        .code(0);
    assert!(projects.join(edge).join(".git").is_dir());
}

/// D16 (iteration 2): a shallow source (a bare repository made with `git
/// clone --bare --depth 1` of the source by `file://`) exits 1 and leaves no
/// folder. M: drop `--reject-shallow`.
#[test]
fn d16_a_shallow_source_is_refused_and_leaves_no_folder() {
    let world = world("deploy-clone-shallow");
    let shallow = world.scratch.join("shallow.git");
    world.git.git_env(
        world.scratch.path(),
        &[
            "clone",
            "-q",
            "--bare",
            "--template=",
            "--depth",
            "1",
            world.url.as_str(),
            shallow.to_str().expect("a UTF-8 path"),
        ],
        &[("GIT_ALLOW_PROTOCOL", OsStr::new("file"))],
    );
    world.git.quiet(&shallow);
    let none: &[(&str, &OsStr)] = &[];
    assert_eq!(
        world.git_run(&shallow, &["rev-parse", "--is-shallow-repository"], none),
        (0, "true".to_owned()),
        "the source is shallow"
    );
    let projects = world.scratch.dir("projects");
    let url = format!("file://{}", shallow.display());
    let run = world.clone_sh(&projects, &[url.as_str(), "shallow", COMMITTER, EMAIL]);
    assert_eq!(run.code, 1, "{}", run.show());
    assert!(
        snapshot(&projects).is_empty(),
        "no folder is left: {:?}\n{}",
        snapshot(&projects).keys().collect::<Vec<_>>(),
        run.show()
    );
}
