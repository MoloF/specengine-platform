//! docs/features/write-cost.md (RM-0125), the hub half `write_cost_golden.rs`
//! (WC9) and `write_cost_git.rs` (WC7) share: a corpus opted in to a hub by a
//! committed `[team]` (`ann`, its `owner`), the real daemon over it, an
//! update and a question raised and staged through the library on an injected
//! clock (so the read's `updated_at` is fixed), and the confirm's GET and POST
//! through the test proxy as `ann`. The corpora, the probe node and the texts
//! are the CLI's `write_cost_common/mod.rs`.

#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use specengine_cli::{
    Globals, Outcome, ProposeRequest, ProposedText, QuestionRequest, StageBody, StageRequest,
    propose, propose_question, stage,
};

use crate::common::hub::{ORIGIN, Proxied, hub_args, through};
use crate::common::{Reply, Server, product_env};
use crate::write_cost_common::{NOW, Probe, Repo, appended, printed, real_git};

/// The login that confirms: the project's `owner`.
pub const LOGIN: &str = "ann";

/// The `[team]` every corpus is opted in with.
pub const TEAM: &str = "\n[team.ann]\nrole = \"owner\"\nemail = \"ann@example.org\"\n";

const ROLE: &str = "spec-writer";
const MODEL: &str = "claude-opus-5-5";

/// A scratch repository of `corpus` with [`TEAM`] appended to its config, in
/// a commit of its own.
pub fn opted_in(label: &str, corpus: &str) -> Repo {
    let repo = Repo::new(label, corpus);
    let config = repo.root.join("specengine.toml");
    let mut text = fs::read_to_string(&config).expect("a config");
    text.push_str(TEAM);
    fs::write(&config, text).expect("write the config");
    repo.commit_all("the team");
    repo
}

/// The project's slug, as its config's `[project]` names it.
pub fn slug(repo: &Repo) -> Option<String> {
    let text = fs::read_to_string(repo.root.join("specengine.toml")).ok()?;
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[project]";
        } else if inside && let Some(rest) = line.strip_prefix("slug") {
            let value = rest.trim_start().strip_prefix('=')?.trim();
            return Some(value.trim_matches('"').to_owned());
        }
    }
    None
}

/// The hub over `repo` under `env` (its `HOME` the repository's own), cwd an
/// empty scratch directory.
pub fn start(repo: &Repo, env: &[(OsString, OsString)]) -> Server {
    start_with(repo, env, &[])
}

/// [`start`] with `flags` after the hub's own.
pub fn start_with(repo: &Repo, env: &[(OsString, OsString)], flags: &[&str]) -> Server {
    let cwd = repo.scratch.dir("cwd");
    let mut args: Vec<&OsStr> = hub_args(&[repo.root.as_path()], ORIGIN);
    args.extend(flags.iter().map(OsStr::new));
    Server::try_start_env(env, &cwd, &args)
        .unwrap_or_else(|refused| panic!("the hub did not start: {refused:?}"))
}

/// The daemon's environment: the tests' product environment.
pub fn hub_env(repo: &Repo) -> Vec<(OsString, OsString)> {
    product_env(&repo.home)
}

/// An update of the probe node, its text with the dangling lines `tag`:
/// the answer as printed and the stored ID.
pub fn propose_update(repo: &Repo, probe: &Probe, tag: &str) -> (Value, Option<String>) {
    let result = propose(
        &repo.env(),
        &Globals::default(),
        &ProposeRequest {
            target: probe.target.clone(),
            base: probe.base.clone(),
            text: ProposedText::Given(appended(&probe.text, tag).into_bytes()),
            rationale: format!("Probe {}.", probe.target),
            author_role: Some(ROLE.to_owned()),
            author_model: Some(MODEL.to_owned()),
            run: None,
            login: None,
            now: NOW.to_owned(),
            git: repo.git_env(),
        },
    );
    let id = result
        .as_ref()
        .ok()
        .filter(|outcome| outcome.refusal.is_none())
        .and_then(|outcome| outcome.document.id.clone());
    (
        printed(result.map(|outcome| Outcome::Proposal(Box::new(outcome)))),
        id,
    )
}

/// A question on the probe node, asked again naming its hits when the first
/// ask met any: each answer as printed, and the stored ID.
pub fn raise_question(repo: &Repo, probe: &Probe) -> (Vec<Value>, Option<String>) {
    let mut request = QuestionRequest {
        node_ids: vec![probe.target.clone()],
        text: "Does the write cost probe hold as written?".to_owned(),
        working_answer: "Keep it as written.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        gist: None,
        matters: None,
        options: None,
        recommendation: None,
        severity: None,
        distinct_from: Vec::new(),
        card: None,
        author_role: Some("developer".to_owned()),
        author_model: Some(MODEL.to_owned()),
        run: None,
        login: None,
        now: NOW.to_owned(),
        git: repo.git_env(),
    };
    let ask =
        |request: &QuestionRequest| propose_question(&repo.env(), &Globals::default(), request);
    let mut answers = Vec::new();
    let mut result = ask(&request);
    let hits = result.as_ref().ok().and_then(|outcome| {
        (outcome.refusal.is_none() && !outcome.document.created).then(|| {
            outcome
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect::<Vec<_>>()
        })
    });
    answers.push(printed(
        result
            .clone()
            .map(|outcome| Outcome::Intake(Box::new(outcome))),
    ));
    if let Some(names) = hits {
        request.distinct_from = names;
        result = ask(&request);
        answers.push(printed(
            result
                .clone()
                .map(|outcome| Outcome::Intake(Box::new(outcome))),
        ));
    }
    let id = result
        .ok()
        .filter(|outcome| outcome.document.created)
        .and_then(|outcome| outcome.document.id);
    (answers, id)
}

/// `id`'s approve staged by [`LOGIN`] through the library at [`NOW`], against
/// the row as it was stored (at [`NOW`]): the answer as printed.
pub fn stage_approve(repo: &Repo, id: &str) -> Value {
    let result = stage(
        &repo.env(),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                editorial: None,
                note: None,
                updated_at: NOW.to_owned(),
                actor: Some(LOGIN.to_owned()),
            },
            now: NOW.to_owned(),
            git: repo.git_env(),
            actor: Some(LOGIN.to_owned()),
        },
    );
    printed(result.map(|outcome| Outcome::Stage(Box::new(outcome))))
}

/// `…/proposals/<id>/confirm` of the project `slug`.
pub fn confirm_route(slug: &str, id: &str) -> String {
    format!("/api/projects/{slug}/proposals/{id}/confirm")
}

/// The confirm's GET by [`LOGIN`].
pub fn confirm_get(server: &Server, slug: &str, id: &str) -> Reply {
    through(
        server,
        "GET",
        &confirm_route(slug, id),
        &Proxied::fetch(LOGIN),
        b"",
    )
}

/// The confirm's POST by [`LOGIN`] of what `read`, a 200, showed.
pub fn confirm_post(server: &Server, slug: &str, id: &str, read: &Reply) -> Reply {
    let document = read.json();
    let body = json!({"question": document["question"], "updated_at": document["updated_at"]});
    through(
        server,
        "POST",
        &confirm_route(slug, id),
        &Proxied::write(LOGIN),
        body.to_string().as_bytes(),
    )
}

/// A reply as a golden holds it: its status and its body's text.
pub fn answered(reply: &Reply) -> Value {
    json!({"status": reply.status, "body": reply.text()})
}

// ------------------------------------------------------------ the git shim

/// docs/features/write-cost.md "Goldens and the git shim": an executable
/// `git` in `dir` that appends its arguments to `log`, one line per call,
/// each argument followed by a tab (a path may hold a space), in one write,
/// then execs the real git by its absolute path.
pub fn git_shim(dir: &Path, log: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("the shim's directory");
    let shim = dir.join("git");
    let quote = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nline=''\nfor arg in \"$@\"; do line=\"$line$arg\t\"; done\n\
             printf '%s\\n' \"$line\" >> {}\nexec {} \"$@\"\n",
            quote(log),
            quote(&real_git())
        ),
    )
    .expect("write the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("an executable shim");
    shim
}

/// `env` with `dir` first on its `PATH`.
pub fn shimmed(env: &[(OsString, OsString)], dir: &Path) -> Vec<(OsString, OsString)> {
    env.iter()
        .map(|(name, value)| {
            if name == "PATH" {
                let mut dirs = vec![dir.to_path_buf()];
                dirs.extend(std::env::split_paths(value));
                (name.clone(), std::env::join_paths(dirs).expect("PATH"))
            } else {
                (name.clone(), value.clone())
            }
        })
        .collect()
}

/// The shim's lines so far.
pub fn logged(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}
