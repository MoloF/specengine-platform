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

mod common;

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
/// them; `one` is a call's time alone.
fn phase(server: &Server, one: Duration, drop_them: bool) -> Phase {
    let port = server.port;
    let host = format!("127.0.0.1:{port}");
    let request = request_bytes("GET", TREE, &[("Host", &host)]);
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

    let dropped = phase(&server, one, true);
    let control = phase(&server, one, false);
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
