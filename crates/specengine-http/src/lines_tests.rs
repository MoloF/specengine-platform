//! docs/features/hub-turn-fairness.md (RM-0119) TF7, the lines' unit half: a
//! project's lines (`Project.lines`, [`Lines`]) keep an entry for a login only
//! while one of its calls holds or waits in its line, so once a burst like
//! TF2's settles the map is empty. The burst: `mira`'s first call heads her
//! line, seven more of hers wait behind it, `ann`'s one heads her own; each
//! leaves in turn, two of `mira`'s cancelled while waiting (their futures
//! dropped, as an HTTP disconnect drops them). The wire's half is
//! `tests/hub_fairness.rs`. M: entries never removed.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::time::timeout;

use crate::turn::{Lines, Place};

/// A call waiting for its place, polled once so it has joined its line.
type Waiting<'a> = Pin<Box<dyn Future<Output = Place> + 'a>>;

/// `login`'s call joins its line and, the line held, waits there.
async fn waiting<'a>(lines: &'a Lines, login: &'a str) -> Waiting<'a> {
    let mut call: Waiting<'a> = Box::pin(lines.line_up(login, None));
    assert!(
        timeout(Duration::from_millis(20), &mut call).await.is_err(),
        "{login}'s call waits behind the head of its line"
    );
    call
}

/// The place `call` reaches once the head before it left.
async fn moved_up(call: Waiting<'_>) -> Place {
    timeout(Duration::from_secs(10), call)
        .await
        .expect("the next in line moves up once the head leaves")
}

#[tokio::test]
async fn tf7_a_settled_burst_leaves_no_line() {
    let lines = Lines::default();
    assert_eq!(
        lines.logins(),
        Vec::<String>::new(),
        "no line before a call"
    );

    let head = lines.line_up("mira", None).await;
    let mut behind = Vec::new();
    for _ in 2..=8 {
        behind.push(waiting(&lines, "mira").await);
    }
    let ann = lines.line_up("ann", None).await;
    assert_eq!(lines.logins(), ["ann", "mira"], "one line per active login");

    drop(head);
    let mut held = moved_up(behind.remove(0)).await;
    drop(ann);
    assert_eq!(
        lines.logins(),
        ["mira"],
        "ann's line leaves with her one call"
    );
    // Two of mira's cancelled while they wait: the rest still move up.
    drop(behind.remove(0));
    drop(behind.remove(0));
    for call in behind {
        drop(held);
        held = moved_up(call).await;
        assert_eq!(lines.logins(), ["mira"], "mira still has a call");
    }
    drop(held);
    assert_eq!(
        lines.logins(),
        Vec::<String>::new(),
        "a settled burst leaves no entry"
    );
}

#[tokio::test]
async fn tf7_a_call_cancelled_alone_in_line_leaves_no_line() {
    let lines = Lines::default();
    let head = lines.line_up("mira", None).await;
    let cancelled = waiting(&lines, "mira").await;
    drop(head);
    drop(cancelled);
    assert_eq!(
        lines.logins(),
        Vec::<String>::new(),
        "the dropped waiter, the last of its login, removed its entry"
    );
    // The line is free for the next call at once.
    let next = timeout(Duration::from_secs(10), lines.line_up("mira", None))
        .await
        .expect("a free line");
    drop(next);
    assert_eq!(lines.logins(), Vec::<String>::new());
}
