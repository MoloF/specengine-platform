//! A data directory's proposal floor (task spec `apply-outputs`, "A hub's
//! `PR-` floor"): `<data dir>/proposal-floor`, a regular file holding the
//! decimal floor and LF (core [`parse_floor`], [`floor_text`]). A hub's
//! start writes it once, atomically, never replacing one ([`store_floor`]);
//! every mint in that
//! data directory reads it ([`read_floor`]) and mints `PR-` no lower. A data
//! directory where no hub ever started holds none: floor 1, the mint as
//! before. No dump carries it and no queue row holds it. Its presence is
//! also the data directory's spec-ID side ([`id_side`], the `spec-ids`
//! spec): every number-shape spec ID minted there is a hub's.

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use specengine_core::proposal::{FLOOR_PROBLEM, floor_text, parse_floor};
use specengine_core::record::IdSide;

use crate::error::StoreError;

/// The floor's file name in a data directory, beside `daemon.lock`.
pub const PROPOSAL_FLOOR_FILE: &str = "proposal-floor";

/// Names of the floor's siblings, unique within the process.
static SIBLING_SEQ: AtomicU64 = AtomicU64::new(0);

/// The floor stored in the data directory `dir`: 1 when it holds no
/// `proposal-floor`. An unreadable file, or one that is no floor, is an
/// error naming the file ([`FLOOR_PROBLEM`]): a mint then stores nothing.
pub fn read_floor(dir: &Path) -> Result<u64, StoreError> {
    stored_floor(dir).map(|floor| floor.unwrap_or(1))
}

/// The side the data directory `dir` mints number-shape spec IDs on (the
/// `spec-ids` spec, "The constant and the side"): [`IdSide::Hub`] when it
/// holds a `proposal-floor` that reads as a floor, whatever number it
/// holds; [`IdSide::Terminal`] when it holds none. The file alone decides,
/// never a flag. An invalid file is [`read_floor`]'s error: the mint then
/// stores nothing.
pub fn id_side(dir: &Path) -> Result<IdSide, StoreError> {
    Ok(match stored_floor(dir)? {
        Some(_) => IdSide::Hub,
        None => IdSide::Terminal,
    })
}

/// The floor `dir`'s `proposal-floor` holds, `None` when there is none;
/// the error of [`read_floor`].
fn stored_floor(dir: &Path) -> Result<Option<u64>, StoreError> {
    let path = dir.join(PROPOSAL_FLOOR_FILE);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StoreError::io(path, error)),
        Ok(meta) if !meta.file_type().is_file() => return Err(not_a_floor(&path)),
        Ok(_) => {}
    }
    let bytes = fs::read(&path).map_err(|error| StoreError::io(&path, error))?;
    parse_floor(&bytes)
        .map(Some)
        .ok_or_else(|| not_a_floor(&path))
}

/// A hub's start: `<dir>/proposal-floor` written atomically holding `floor`
/// when absent, else read and never rewritten (its bytes and its mtime
/// kept). The bytes go to a new sibling `.proposal-floor.specengine-<pid>-
/// <n>.tmp`, synced, hard-linked to `proposal-floor` (`link` never replaces
/// a name: an existing floor is read instead), the sibling removed, the
/// directory synced; so no reader and no crash ever sees the floor empty or
/// partial, and a sibling a crash leaves is never read as the floor. The
/// floor stored either way; an invalid file is the error [`read_floor`]
/// gives.
pub fn store_floor(dir: &Path, floor: u64) -> Result<u64, StoreError> {
    store_floor_with(dir, floor, &mut |_| ControlFlow::Continue(()))
        .map(|stored| stored.unwrap_or(floor))
}

/// A step of the floor's atomic write, in order: what [`store_floor_with`]
/// has just done when it calls its `step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloorStep {
    /// The sibling holds the floor's bytes.
    SiblingWritten,
    /// The sibling is synced and closed.
    SiblingSynced,
    /// `proposal-floor` is a hard link to the sibling.
    Linked,
    /// The sibling's name is removed.
    SiblingRemoved,
    /// The data directory is synced.
    DirectorySynced,
}

/// [`store_floor`], `step` called after each [`FloorStep`] of the write:
/// `Break` stops right there, as a crash would, leaving whatever is on
/// disk (`Ok(None)`). `Ok(Some(floor))`: the floor stored, written now or
/// read.
fn store_floor_with(
    dir: &Path,
    floor: u64,
    step: &mut dyn FnMut(FloorStep) -> ControlFlow<()>,
) -> Result<Option<u64>, StoreError> {
    let path = dir.join(PROPOSAL_FLOOR_FILE);
    if fs::symlink_metadata(&path).is_ok() {
        return read_floor(dir).map(Some);
    }
    let (sibling, mut file) = new_sibling(dir)?;
    // Nothing but the floor's own name is ever read: a sibling this run
    // cannot finish is removed, one a crash leaves is skipped.
    let abandon = |error: io::Error| {
        let _ = fs::remove_file(&sibling);
        StoreError::io(&sibling, error)
    };
    if let Err(error) = file.write_all(floor_text(floor).as_bytes()) {
        drop(file);
        return Err(abandon(error));
    }
    if step(FloorStep::SiblingWritten).is_break() {
        return Ok(None);
    }
    let synced = file.sync_all();
    drop(file);
    if let Err(error) = synced {
        return Err(abandon(error));
    }
    if step(FloorStep::SiblingSynced).is_break() {
        return Ok(None);
    }
    match fs::hard_link(&sibling, &path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&sibling);
            return read_floor(dir).map(Some);
        }
        Err(error) => {
            let _ = fs::remove_file(&sibling);
            return Err(StoreError::io(path, error));
        }
    }
    if step(FloorStep::Linked).is_break() {
        return Ok(None);
    }
    // The floor is in place: a sibling left by a failed removal is never
    // read as the floor.
    let _ = fs::remove_file(&sibling);
    if step(FloorStep::SiblingRemoved).is_break() {
        return Ok(None);
    }
    fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|error| StoreError::io(dir, error))?;
    if step(FloorStep::DirectorySynced).is_break() {
        return Ok(None);
    }
    Ok(Some(floor))
}

/// A new sibling of the floor in `dir`, created exclusively (a name a crash
/// left is skipped), open for writing.
fn new_sibling(dir: &Path) -> Result<(PathBuf, fs::File), StoreError> {
    loop {
        let sibling = dir.join(format!(
            ".{PROPOSAL_FLOOR_FILE}.specengine-{}-{}.tmp",
            std::process::id(),
            SIBLING_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&sibling)
        {
            Ok(file) => return Ok((sibling, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(StoreError::io(sibling, error)),
        }
    }
}

/// The error of a file at the floor's path that holds no floor.
fn not_a_floor(path: &Path) -> StoreError {
    StoreError::io(
        path,
        io::Error::new(io::ErrorKind::InvalidData, FLOOR_PROBLEM),
    )
}
#[cfg(test)]
mod tests {
    //! docs/features/apply-outputs.md O15 (iteration 2): the floor written
    //! atomically ("Data": a dot-named sibling written and synced, hard-linked
    //! to `proposal-floor`, the sibling removed, the directory synced), judged
    //! after each step through the seam [`store_floor_with`]: absent or whole at
    //! every step; stopped at any step as a crash would, a second
    //! [`store_floor`] ends whole with no sibling left; leftover siblings are
    //! never read as the floor. M: `create_new` then write. The daemon's half
    //! is `specengine-http`'s `tests/hub_ids.rs`.

    use std::sync::atomic::AtomicUsize;

    use super::*;

    /// The floor a hub's start writes, and its bytes.
    const FLOOR: u64 = specengine_core::proposal::HUB_PROPOSAL_FLOOR;
    const BYTES: &[u8] = b"100001\n";

    /// The siblings' name prefix ("Data").
    const SIBLING: &str = ".proposal-floor.specengine-";

    static SCRATCH: AtomicUsize = AtomicUsize::new(0);

    /// A data directory of its own under the system temp dir, removed on drop.
    struct DataDir(PathBuf);

    impl DataDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-floor-{label}-{}-{}",
                std::process::id(),
                SCRATCH.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch data directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for DataDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The floor's bytes (`None`: absent) and every sibling's, by name.
    type State = (Option<Vec<u8>>, Vec<(String, Vec<u8>)>);

    /// The [`State`] of the data directory `dir`.
    fn state(dir: &Path) -> State {
        let floor = match fs::read(dir.join(PROPOSAL_FLOOR_FILE)) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => panic!("the floor: {error}"),
        };
        let mut siblings: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
            .expect("the data directory")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(SIBLING))
            .map(|name| {
                let bytes = fs::read(dir.join(&name)).expect("a sibling");
                (name, bytes)
            })
            .collect();
        siblings.sort();
        (floor, siblings)
    }

    /// O15: the state seen after each of the five steps. Before the link the
    /// floor is absent and its one sibling already whole; from the link on the
    /// floor is whole; from the sibling's removal on no sibling is left. At no
    /// step is the floor empty or partial. M: `create_new` then write.
    #[test]
    fn o15_the_floor_is_absent_or_whole_after_every_step_of_its_write() {
        let dir = DataDir::new("steps");
        let mut seen = Vec::new();
        let stored = store_floor_with(dir.path(), FLOOR, &mut |step| {
            seen.push((step, state(dir.path())));
            ControlFlow::Continue(())
        })
        .expect("the floor stored");
        assert_eq!(stored, Some(FLOOR), "stored, not stopped");
        assert_eq!(seen.len(), 5, "five steps: {seen:?}");
        for (at, (step, (floor, siblings))) in seen.iter().enumerate() {
            let context = format!("after step {} ({step:?})", at + 1);
            if let Some(bytes) = floor {
                assert_eq!(bytes.as_slice(), BYTES, "{context}: whole or absent");
            }
            match at + 1 {
                1 | 2 => {
                    assert!(floor.is_none(), "{context}: no floor before the link");
                    assert_eq!(siblings.len(), 1, "{context}: one sibling: {siblings:?}");
                    assert_eq!(siblings[0].1.as_slice(), BYTES, "{context}: whole");
                }
                3 => {
                    assert!(floor.is_some(), "{context}: linked");
                    assert_eq!(siblings.len(), 1, "{context}: {siblings:?}");
                }
                _ => {
                    assert!(floor.is_some(), "{context}");
                    assert!(siblings.is_empty(), "{context}: {siblings:?}");
                }
            }
        }
        assert_eq!(state(dir.path()), (Some(BYTES.to_vec()), Vec::new()));
        assert_eq!(read_floor(dir.path()).expect("the floor read"), FLOOR);
    }

    /// O15: the write stopped at each step in turn (`Break`, as a crash would
    /// stop it), the floor then absent or whole; a second, unstopped
    /// [`store_floor`] ends with the floor whole and read as 100001, the
    /// stopped run's sibling (if any) never read as it, and leaves no sibling
    /// of its own. M: `create_new` then write.
    #[test]
    fn o15_a_write_stopped_at_any_step_then_stored_again_ends_whole() {
        for stop in 1..=5 {
            let dir = DataDir::new(&format!("stop-{stop}"));
            let mut steps = 0;
            let _ = store_floor_with(dir.path(), FLOOR, &mut |_| {
                steps += 1;
                if steps == stop {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            });
            assert_eq!(steps, stop, "stopped at step {stop}");
            let (floor, left) = state(dir.path());
            if let Some(bytes) = floor {
                assert_eq!(
                    bytes.as_slice(),
                    BYTES,
                    "stopped at step {stop}: whole or absent"
                );
            }
            assert_eq!(
                store_floor(dir.path(), FLOOR).expect("stored again"),
                FLOOR,
                "stopped at step {stop}"
            );
            let (floor, siblings) = state(dir.path());
            assert_eq!(
                floor.as_deref(),
                Some(BYTES),
                "stopped at step {stop}, stored again: whole"
            );
            assert_eq!(read_floor(dir.path()).expect("read"), FLOOR);
            assert!(
                siblings.iter().all(|sibling| left.contains(sibling)),
                "stopped at step {stop}: the second run left a sibling of its own: \
                 {siblings:?} beside {left:?}"
            );
        }
    }

    /// O15: leftover siblings a crash would leave, one partial (`42` and LF)
    /// and one empty, are never read as the floor: [`store_floor`] writes and
    /// returns 100001, and [`read_floor`] reads it. M: a sibling taken for the
    /// floor.
    #[test]
    fn o15_leftover_siblings_are_never_read_as_the_floor() {
        let dir = DataDir::new("leftover");
        fs::write(dir.path().join(format!("{SIBLING}1-0.tmp")), b"42\n").expect("a sibling");
        fs::write(dir.path().join(format!("{SIBLING}2-0.tmp")), b"").expect("an empty one");
        assert_eq!(read_floor(dir.path()).expect("no floor yet"), 1);
        assert_eq!(store_floor(dir.path(), FLOOR).expect("stored"), FLOOR);
        assert_eq!(state(dir.path()).0.as_deref(), Some(BYTES));
        assert_eq!(read_floor(dir.path()).expect("read"), FLOOR);
    }
}
