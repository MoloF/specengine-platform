//! docs/features/daemon-read.md "Description and interactions" (a
//! project's calls run one at a time), iteration 2 of the daemon: a
//! request waits for its project's turn in its own task, so a request
//! whose client is gone before its turn never runs its call.
//!
//! A generated corpus (4000 node files) makes one `tree` call cost a
//! measurable time `T` in a debug build. Phase "dropped": a first call
//! runs, five more queue behind it and their clients close the connection
//! while it runs, a last call is sent after them. Phase "control": the
//! same, the five clients kept. Dropped calls that never run cost no CPU
//! and no wait: the daemon's CPU over the dropped phase is about 2 calls'
//! worth against 7 in the control, and the last call ends about `2T` after
//! the first starts against `7T`. Asserted with wide margins (ratio below
//! 0.6 and 0.7 where 2/7 is expected, near 1 when the dropped calls run),
//! so it holds under a loaded machine. M: the turn locked with
//! `blocking_lock` inside `spawn_blocking` (the dropped calls run anyway).
//!
//! AC-06 of docs/features/ui-live.md: a `check`, which refreshes nothing,
//! takes the turn too. The same two phases with `/check` (one walk of the
//! 4000 files is `T`): queued checks whose clients left run no walk. And
//! with the generated corpus and A served by one daemon, a `/check` on the
//! corpus sent while the corpus's reads hold the turn is answered after
//! them, while a `/check` on A sent at the same time is answered before
//! the corpus's first read ends. M: `check` outside the turn.
//!
//! AC-06 of docs/features/ui-live-tasks.md, the turn's half: a task read
//! takes its project's turn. With the generated corpus and A in state S
//! (`task_state`) served by one daemon, the corpus's `/tasks/T-0003` and
//! `/tasks` sent while the corpus's reads hold its turn are answered after
//! them, while A's `/tasks/T-0003` and `/tasks` sent at the same time are
//! answered before the corpus's first read ends. M: the handler outside
//! `run` (a turn of its own).

mod common;
mod task_state;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use common::{HTTP_TIMEOUT, Scratch, Server, http_exchange, request_bytes, write};

/// Node files in the generated corpus (two nodes each).
const FILES: usize = 4000;

/// Requests queued behind the first call in each phase.
const QUEUED: usize = 5;

const TREE: &str = "/api/projects/big-corpus/tree";

const CHECK: &str = "/api/projects/big-corpus/check";

/// A project of `FILES` generated mechanic files, committed once.
fn corpus(scratch: &Scratch) -> std::path::PathBuf {
    let root = scratch.dir("big");
    write(
        &root,
        "specengine.toml",
        "[project]\nname = \"big corpus\"\nslug = \"big-corpus\"\n\n[ids]\n\
         MEC = { kind = \"mechanic\", shape = \"name\" }\n\
         RULE = { kind = \"rule\", shape = \"name\" }\n",
    );
    for i in 0..FILES {
        write(
            &root,
            &format!("docs/spec/gen/g{i:05}.md"),
            format!(
                "---\nid: MEC-GEN-{i:05}\nclass: canon\nstatus: accepted\nowner: owner\n\
                 reviewed: 2026-09-20\n---\n\n# Generated mechanic {i}\n\n\
                 The lantern burns oil at rate {i} while the keeper walks the walls.\n\n\
                 ## Rule {{#RULE-GEN-{i:05}}}\n\n\
                 Stamina regenerates at rest after {} seconds, see MEC-GEN-{:05}.\n",
                i % 7,
                (i + 1) % FILES
            ),
        );
    }
    let git = scratch.git();
    git.run(
        &root,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    git.quiet(&root);
    git.run(&root, &["add", "-A"]);
    git.run(&root, &["commit", "-q", "-m", "generated corpus"]);
    root
}

/// The daemon's CPU time so far, seconds (`ps -o time=`).
fn cpu(pid: u32) -> f64 {
    let output = Command::new("/bin/ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .expect("run ps");
    let text = String::from_utf8(output.stdout).expect("ps output");
    text.trim().split(':').fold(0.0, |total, part| {
        total * 60.0
            + part
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("ps time {text:?}"))
    })
}

/// The status of the answer on `stream`, read to its end.
fn status_of(mut stream: TcpStream) -> u16 {
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .expect("timeout");
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let head = String::from_utf8_lossy(&raw[..raw.len().min(64)]).into_owned();
    head.split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line: {head:?}"))
}

/// What one phase cost.
#[derive(Debug)]
struct Phase {
    /// The daemon's CPU seconds over the phase.
    cpu: f64,
    /// From the first request's send to the last call's answer.
    last: Duration,
}

/// A first call, `QUEUED` more behind it (closed by their clients while
/// the first runs when `drop_them`, else answered), a last call after
/// them, all `GET path`; `one` is a call's time alone.
fn phase(server: &Server, path: &str, one: Duration, drop_them: bool) -> Phase {
    let port = server.port;
    let host = format!("127.0.0.1:{port}");
    let request = request_bytes("GET", path, &[("Host", &host)]);
    let before = cpu(server.pid());
    let started = Instant::now();
    let first = {
        let bytes = request.clone();
        thread::spawn(move || http_exchange(port, &bytes, HTTP_TIMEOUT).status)
    };
    thread::sleep(one / 10);
    let mut waiting: Vec<TcpStream> = (0..QUEUED)
        .map(|_| {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
            stream.write_all(&request).expect("write the request");
            stream
        })
        .collect();
    // They wait for the turn while the first call runs.
    thread::sleep(one * 4 / 10);
    let readers: Vec<thread::JoinHandle<u16>> = if drop_them {
        for stream in waiting.drain(..) {
            let _ = stream.shutdown(Shutdown::Both);
        }
        Vec::new()
    } else {
        waiting
            .into_iter()
            .map(|stream| thread::spawn(move || status_of(stream)))
            .collect()
    };
    thread::sleep(one / 20);
    let last = http_exchange(port, &request, HTTP_TIMEOUT);
    let last_at = started.elapsed();
    assert_eq!(last.status, 200, "the last call: {}", last.text());
    assert_eq!(first.join().expect("the first call"), 200, "the first call");
    for reader in readers {
        assert_eq!(reader.join().expect("a kept call"), 200, "a kept call");
    }
    // A call still running would show in the CPU time: let it end.
    thread::sleep(Duration::from_millis(300));
    Phase {
        cpu: cpu(server.pid()) - before,
        last: last_at,
    }
}

#[test]
fn a_request_dropped_while_waiting_for_its_turn_never_runs() {
    let scratch = Scratch::new("turn-dropped");
    let root = corpus(&scratch);
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[Path::new(&root)]);
    // The first call builds the index; the next ones refresh it.
    let reply = server.get(TREE);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let started = Instant::now();
    server.get(TREE).status(200);
    let one = started.elapsed();
    assert!(
        one >= Duration::from_millis(150),
        "one call takes {one:?}: too fast for queued requests to be dropped while it runs; \
         the corpus must grow"
    );

    let dropped = phase(&server, TREE, one, true);
    let control = phase(&server, TREE, one, false);
    let cpu_ratio = dropped.cpu / control.cpu;
    let wall_ratio = dropped.last.as_secs_f64() / control.last.as_secs_f64();
    let report = format!(
        "one call {one:?}; dropped {dropped:?}; control {control:?}; CPU ratio {cpu_ratio:.2}, \
         wall ratio {wall_ratio:.2} ({} calls against {} expected: {:.2})",
        2,
        QUEUED + 2,
        2.0 / (QUEUED + 2) as f64
    );
    eprintln!("{report}");
    assert!(control.cpu > 0.0, "the control phase ran calls: {report}");
    assert!(
        cpu_ratio < 0.6,
        "the dropped requests' calls ran (CPU): {report}"
    );
    assert!(
        wall_ratio < 0.7,
        "the last call waited for the dropped requests' calls: {report}"
    );
}

#[test]
fn ac06_a_queued_check_dropped_by_its_client_never_walks() {
    let scratch = Scratch::new("turn-check-dropped");
    let root = corpus(&scratch);
    let home = scratch.home("h");
    let server = Server::serve(&home, scratch.path(), &[Path::new(&root)]);
    let reply = server.get(CHECK);
    assert_eq!(reply.status, 200, "{}", reply.text());
    let started = Instant::now();
    server.get(CHECK).status(200);
    let one = started.elapsed();
    assert!(
        one >= Duration::from_millis(150),
        "one check takes {one:?}: too fast for queued requests to be dropped while it runs; \
         the corpus must grow"
    );

    let dropped = phase(&server, CHECK, one, true);
    let control = phase(&server, CHECK, one, false);
    let cpu_ratio = dropped.cpu / control.cpu;
    let wall_ratio = dropped.last.as_secs_f64() / control.last.as_secs_f64();
    let report = format!(
        "one check {one:?}; dropped {dropped:?}; control {control:?}; CPU ratio \
         {cpu_ratio:.2}, wall ratio {wall_ratio:.2} (2 walks against {} expected: {:.2})",
        QUEUED + 2,
        2.0 / (QUEUED + 2) as f64
    );
    eprintln!("{report}");
    assert!(control.cpu > 0.0, "the control phase ran walks: {report}");
    assert!(
        cpu_ratio < 0.6,
        "the dropped requests' checks walked (CPU): {report}"
    );
    assert!(
        wall_ratio < 0.7,
        "the last check waited for the dropped requests' walks: {report}"
    );
}

/// `GET path` on `port` in a thread: its status and when its answer
/// ended, measured from `start`.
fn timed(port: u16, path: &str, start: Instant) -> thread::JoinHandle<(u16, Duration)> {
    let host = format!("127.0.0.1:{port}");
    let bytes = request_bytes("GET", path, &[("Host", &host)]);
    thread::spawn(move || {
        let reply = http_exchange(port, &bytes, HTTP_TIMEOUT);
        (reply.status, start.elapsed())
    })
}

#[test]
fn ac06_a_check_waits_for_its_own_projects_reads_not_anothers() {
    let scratch = Scratch::new("turn-check-projects");
    let root = corpus(&scratch);
    let a = scratch.repo("spec-a", "a", "main");
    // A fresh HOME: the corpus's first read builds its whole index.
    let home = scratch.home("fresh");
    let server = Server::serve(&home, scratch.path(), &[Path::new(&root), &a]);
    let port = server.port;
    let start = Instant::now();
    // The corpus's turn: its first read, then a second one queued.
    let first = timed(port, TREE, start);
    thread::sleep(Duration::from_millis(100));
    let second = timed(port, TREE, start);
    thread::sleep(Duration::from_millis(100));
    // Sent while the corpus's reads hold its turn: a check of the corpus
    // and one of A.
    let own = timed(port, CHECK, start);
    thread::sleep(Duration::from_millis(50));
    let other = timed(port, "/api/projects/lantern-keep/check", start);

    let (status, other_at) = other.join().expect("A's check");
    assert_eq!(status, 200, "A's check");
    let (status, first_at) = first.join().expect("the corpus's first read");
    assert_eq!(status, 200, "the first read");
    let (status, second_at) = second.join().expect("the corpus's second read");
    assert_eq!(status, 200, "the second read");
    let (status, own_at) = own.join().expect("the corpus's check");
    assert_eq!(status, 200, "the corpus's check");
    let report = format!(
        "answers after the start: A's check {other_at:?}, the corpus's reads {first_at:?} and \
         {second_at:?}, its check {own_at:?}"
    );
    eprintln!("{report}");
    assert!(
        first_at > Duration::from_millis(300),
        "the first read is too quick to hold the turn while the checks arrive: {report}"
    );
    assert!(
        other_at < first_at,
        "A's check waited for another project's read: {report}"
    );
    assert!(
        own_at > second_at,
        "the corpus's check ran beside its project's reads, outside the turn: {report}"
    );
}

#[test]
fn ac06_a_task_read_waits_for_its_own_projects_reads_not_anothers() {
    let scratch = Scratch::new("turn-tasks");
    let root = corpus(&scratch);
    // A fresh HOME for the corpus (its first read builds its whole
    // index); A in state S in it.
    let home = scratch.home("fresh");
    let a = task_state::repo_in_s(&scratch, &task_state::A, "a", &home);
    let server = Server::serve(&home, scratch.path(), &[Path::new(&root), &a]);
    let port = server.port;
    let start = Instant::now();
    // The corpus's turn: its first read, then a second one queued.
    let first = timed(port, TREE, start);
    thread::sleep(Duration::from_millis(100));
    let second = timed(port, TREE, start);
    thread::sleep(Duration::from_millis(100));
    // Sent while the corpus's reads hold its turn: the corpus's task reads
    // and A's.
    let own_show = timed(port, "/api/projects/big-corpus/tasks/T-0003", start);
    let own_list = timed(port, "/api/projects/big-corpus/tasks", start);
    thread::sleep(Duration::from_millis(50));
    let other_show = timed(port, "/api/projects/lantern-keep/tasks/T-0003", start);
    let other_list = timed(port, "/api/projects/lantern-keep/tasks", start);

    let (status, other_show_at) = other_show.join().expect("A's task");
    assert_eq!(status, 200, "A's T-0003");
    let (status, other_list_at) = other_list.join().expect("A's list");
    assert_eq!(status, 200, "A's list");
    let (status, first_at) = first.join().expect("the corpus's first read");
    assert_eq!(status, 200, "the first read");
    let (status, second_at) = second.join().expect("the corpus's second read");
    assert_eq!(status, 200, "the second read");
    let (status, own_show_at) = own_show.join().expect("the corpus's task");
    assert_eq!(status, 404, "the corpus has no T-0003");
    let (status, own_list_at) = own_list.join().expect("the corpus's list");
    assert_eq!(status, 200, "the corpus's list");
    let report = format!(
        "answers after the start: A's task {other_show_at:?} and list {other_list_at:?}, the \
         corpus's reads {first_at:?} and {second_at:?}, its task {own_show_at:?} and list \
         {own_list_at:?}"
    );
    eprintln!("{report}");
    assert!(
        first_at > Duration::from_millis(300),
        "the first read is too quick to hold the turn while the task reads arrive: {report}"
    );
    assert!(
        other_show_at < first_at && other_list_at < first_at,
        "A's task reads waited for another project's read: {report}"
    );
    assert!(
        own_show_at > second_at && own_list_at > second_at,
        "the corpus's task reads ran beside its project's reads, outside the turn: {report}"
    );
}
