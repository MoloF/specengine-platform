//! What one call spent, by kind (`write-cost`, RM-0125, "Attribution: five
//! fields"): the check's introduced findings, the index's walks and
//! decodes, git's processes, the queue's and the record's synced writes.
//! A daemon's request line carries them as ` check_ms= index_ms= git_ms=
//! git= write_ms=`.
//!
//! - **Per call**: [`scoped`] counts on the calling thread for the extent of
//!   its closure, from zero, and gives the counts back; a nested scope
//!   replaces the outer one for its extent, restored on return or unwind.
//!   Outside a scope every charge is a no-op.
//! - **Innermost wins**: a site charges its kind for as long as its guard
//!   lives ([`charge`]); time inside a nested guard goes to the nested kind
//!   alone, so the kinds never overlap and their sum stays within the
//!   call's own time.
//! - **Git**: each spawn is one more `git` and its wall time, spawn to exit,
//!   is `git_ms` ([`spawn`]).
//! - **None ran**: a kind no guard entered is `None` (`-` on a line).

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A kind of cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Computing a write's introduced findings.
    Check,
    /// The walk, snapshot reads and decodes outside the check, the
    /// post-commit update, the outputs' render and blob parse.
    Index,
    /// A git process, spawn to exit.
    Git,
    /// A queue write transaction, the record file's write and syncs, the ID
    /// floor's sync.
    Write,
}

impl Kind {
    fn slot(self) -> usize {
        match self {
            Self::Check => 0,
            Self::Index => 1,
            Self::Git => 2,
            Self::Write => 3,
        }
    }
}

/// One call's costs; `None` where nothing of that kind ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallCost {
    pub check_ms: Option<u64>,
    pub index_ms: Option<u64>,
    pub git_ms: Option<u64>,
    pub write_ms: Option<u64>,
    /// Git processes spawned.
    pub git: Option<u64>,
}

impl CallCost {
    /// ` check_ms=<n|-> index_ms=<n|-> git_ms=<n|-> git=<n|-> write_ms=<n|->`,
    /// a request line's tail.
    pub fn fields(&self) -> String {
        let shown = |value: Option<u64>| value.map_or_else(|| "-".to_owned(), |n| n.to_string());
        format!(
            " check_ms={} index_ms={} git_ms={} git={} write_ms={}",
            shown(self.check_ms),
            shown(self.index_ms),
            shown(self.git_ms),
            shown(self.git),
            shown(self.write_ms)
        )
    }
}

/// A scope's books.
#[derive(Debug, Default)]
struct Books {
    /// Time charged so far per kind.
    spent: [Duration; 4],
    /// A guard of the kind was entered.
    ran: [bool; 4],
    spawns: u64,
    /// The guards in force, innermost last, each with its frame and the
    /// instant its time last resumed: only the innermost accrues.
    stack: Vec<(u64, Kind, Instant)>,
}

/// Frames of every scope, unique in the process.
static FRAMES: AtomicU64 = AtomicU64::new(0);

impl Books {
    fn enter(&mut self, kind: Kind) -> u64 {
        let now = Instant::now();
        if let Some((_, outer, resumed)) = self.stack.last_mut() {
            self.spent[outer.slot()] += now.saturating_duration_since(*resumed);
            *resumed = now;
        }
        self.ran[kind.slot()] = true;
        let frame = FRAMES.fetch_add(1, Ordering::Relaxed);
        self.stack.push((frame, kind, now));
        frame
    }

    /// The guard of `frame` ends: the innermost one's time is charged and
    /// the next resumes; one that outlived a guard entered after it (a
    /// `cat-file --batch` session) leaves the stack, its time already
    /// charged while it was innermost.
    fn leave(&mut self, frame: u64) {
        let now = Instant::now();
        let Some(at) = self.stack.iter().rposition(|(id, _, _)| *id == frame) else {
            return;
        };
        let innermost = at + 1 == self.stack.len();
        let (_, kind, resumed) = self.stack.remove(at);
        if innermost {
            self.spent[kind.slot()] += now.saturating_duration_since(resumed);
            if let Some((_, _, resumed)) = self.stack.last_mut() {
                *resumed = now;
            }
        }
    }

    fn cost(&self) -> CallCost {
        let ms = |kind: Kind| {
            let slot = kind.slot();
            self.ran[slot].then(|| u64::try_from(self.spent[slot].as_millis()).unwrap_or(u64::MAX))
        };
        CallCost {
            check_ms: ms(Kind::Check),
            index_ms: ms(Kind::Index),
            git_ms: ms(Kind::Git),
            write_ms: ms(Kind::Write),
            git: (self.spawns > 0).then_some(self.spawns),
        }
    }
}

thread_local! {
    /// The scope in force on this thread, if any.
    static BOOKS: RefCell<Option<Books>> = const { RefCell::new(None) };
}

/// Puts back the scope a nested one replaced, on return or unwind.
struct Restore(Option<Books>);

impl Drop for Restore {
    fn drop(&mut self) {
        let outer = self.0.take();
        let _ = BOOKS.try_with(|books| *books.borrow_mut() = outer);
    }
}

/// `run` counted from zero on this thread; its result and its costs.
pub fn scoped<R>(run: impl FnOnce() -> R) -> (R, CallCost) {
    let outer = BOOKS.with_borrow_mut(|books| books.replace(Books::default()));
    let restore = Restore(outer);
    let result = run();
    let cost = BOOKS
        .with_borrow(|books| books.as_ref().map(Books::cost))
        .unwrap_or_default();
    drop(restore);
    (result, cost)
}

/// A kind charged while it lives; a no-op outside a scope.
#[derive(Debug)]
pub struct Charge {
    frame: Option<u64>,
}

impl Drop for Charge {
    fn drop(&mut self) {
        if let Some(frame) = self.frame {
            let _ = BOOKS.try_with(|books| {
                if let Some(books) = books.borrow_mut().as_mut() {
                    books.leave(frame);
                }
            });
        }
    }
}

/// Charges `kind` from now until the guard drops (innermost wins).
pub fn charge(kind: Kind) -> Charge {
    let frame = BOOKS
        .try_with(|books| books.borrow_mut().as_mut().map(|books| books.enter(kind)))
        .ok()
        .flatten();
    Charge { frame }
}

/// One git process from now until the guard drops: one more `git`, its
/// time `git_ms`.
pub fn spawn() -> Charge {
    let _ = BOOKS.try_with(|books| {
        if let Some(books) = books.borrow_mut().as_mut() {
            books.spawns += 1;
        }
    });
    charge(Kind::Git)
}

#[cfg(test)]
mod tests {
    //! The books: innermost wins, nothing is charged twice, a scope starts
    //! from zero and a kind never entered is `None`.

    use std::thread::sleep;
    use std::time::Duration;

    use super::{Kind, charge, scoped, spawn};

    #[test]
    fn nested_time_goes_to_the_inner_kind_alone() {
        let ((), cost) = scoped(|| {
            let _index = charge(Kind::Index);
            sleep(Duration::from_millis(15));
            {
                let _git = spawn();
                sleep(Duration::from_millis(30));
            }
        });
        let index = cost.index_ms.expect("the walk ran");
        let git = cost.git_ms.expect("git ran");
        assert!(git >= 30, "{cost:?}");
        assert!((15..30).contains(&index), "{cost:?}");
        assert_eq!(cost.git, Some(1));
        assert_eq!(cost.check_ms, None);
        assert_eq!(cost.write_ms, None);
    }

    #[test]
    fn each_scope_starts_from_zero() {
        let run = || {
            let _git = spawn();
        };
        let ((), first) = scoped(run);
        let ((), second) = scoped(run);
        assert_eq!(first.git, Some(1));
        assert_eq!(second.git, Some(1), "not the sum of both");
        let ((), none) = scoped(|| {});
        assert_eq!(
            none.fields(),
            " check_ms=- index_ms=- git_ms=- git=- write_ms=-"
        );
    }

    #[test]
    fn a_session_outliving_an_inner_guard_charges_only_while_innermost() {
        let ((), cost) = scoped(|| {
            let session = spawn();
            sleep(Duration::from_millis(15));
            let index = charge(Kind::Index);
            sleep(Duration::from_millis(30));
            // The session ends before the guard entered after it.
            drop(session);
            sleep(Duration::from_millis(15));
            drop(index);
        });
        let git = cost.git_ms.expect("the session ran");
        let index = cost.index_ms.expect("the parse ran");
        assert!((15..30).contains(&git), "{cost:?}");
        assert!(index >= 45, "{cost:?}");
    }

    #[test]
    fn outside_a_scope_nothing_is_counted() {
        let _git = spawn();
        let ((), cost) = scoped(|| {});
        assert_eq!(cost.git, None);
    }
}
