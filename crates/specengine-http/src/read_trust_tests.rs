//! The read-trust task spec, D9, the daemon's unit half: `answer::run`
//! runs its call inside the project's trust's scope, and a call that
//! panics or cannot run (exit 2) doubts that trust before the turn is
//! released; a call that answers doubts nothing. The hub's MCP turn is
//! `tests/hub_trust.rs`'.
//!
//! Each test serves a scratch project of its own under
//! `std::env::temp_dir()`, removed on drop, its slug unique to the test;
//! `answer::run` reads the process's `Env`, so the one read that opens an
//! index has its database removed from the data directory afterwards.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::StatusCode;
use specengine_cli::{CliError, Env, IndexTrust, Outcome, SearchRequest};
use tokio::sync::Mutex;

use crate::answer;
use crate::start::Project;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A scratch project: its root holds a config naming `slug`.
struct Served {
    root: PathBuf,
    slug: String,
}

impl Served {
    fn new(name: &str) -> Self {
        let serial = NEXT.fetch_add(1, Ordering::SeqCst);
        let slug = format!("trust-{name}-{}-{serial}", std::process::id());
        let dir = std::env::temp_dir().join(format!("specengine-http-{slug}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("docs")).expect("a scratch project");
        fs::write(
            dir.join("specengine.toml"),
            format!(
                "[project]\nslug = \"{slug}\"\n\n[ids]\nR = {{ kind = \"trust-entry\", width = 2 }}\n\n\
                 [paths]\nroots = [\"docs\"]\n"
            ),
        )
        .expect("the config");
        Self {
            root: fs::canonicalize(&dir).expect("canonical"),
            slug,
        }
    }

    /// The project as a hub serves it, with a trust of its own.
    fn project(&self) -> (Arc<Project>, Arc<IndexTrust>) {
        let trust = Arc::new(IndexTrust::new(self.root.clone()));
        let project = Arc::new(Project {
            slug: self.slug.clone(),
            root: self.root.clone(),
            turn: Arc::new(Mutex::new(())),
            trust: Some(Arc::clone(&trust)),
        });
        (project, trust)
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        if let Ok(dir) = Env::from_process().and_then(|env| specengine_cli::data_dir(&env)) {
            for suffix in ["db", "db-wal", "db-shm"] {
                let _ = fs::remove_file(dir.join(format!("{}.{suffix}", self.slug)));
            }
        }
    }
}

#[tokio::test]
async fn d9_a_call_that_panics_doubts_the_project_before_the_turn_is_released() {
    let served = Served::new("panic");
    let (project, trust) = served.project();
    let response = answer::run(
        Arc::clone(&project),
        |_env, _globals| -> Result<Outcome, CliError> {
            panic!("a panic inside the turn, on purpose")
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(trust.is_dirty(), "a call that panicked doubts its project");
    assert!(
        project.turn.try_lock().is_ok(),
        "the turn is released after the panic"
    );
}

#[tokio::test]
async fn d9_a_call_that_cannot_run_doubts_the_project() {
    let served = Served::new("cannot");
    let (project, trust) = served.project();
    let response = answer::run(
        Arc::clone(&project),
        |_env, _globals| -> Result<Outcome, CliError> {
            Err(CliError::cannot("refused on purpose"))
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        trust.is_dirty(),
        "a call that cannot run doubts its project"
    );
}

#[tokio::test]
async fn d9_a_call_that_answered_ran_in_scope_and_doubts_nothing() {
    let served = Served::new("answered");
    let (project, trust) = served.project();
    let response = answer::run(Arc::clone(&project), |env, globals| {
        specengine_cli::search(
            env,
            globals,
            &SearchRequest {
                terms: vec!["anything".to_owned()],
                kinds: Vec::new(),
                limit: None,
                archive: false,
            },
        )
        .map(Outcome::Search)
    })
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!trust.is_dirty(), "a call that answered doubts nothing");
    assert_eq!(
        trust.walks(),
        1,
        "the call ran in the trust's scope and walked"
    );
}
