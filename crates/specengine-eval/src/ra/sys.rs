//! Three POSIX calls std does not expose, through `libc`: the peak resident
//! set size (`getrusage`), signalling a process group (`killpg`) and the
//! caller's process group (`getpgrp`); and the current resident size of a
//! whole process group ([`GroupRss`]: `proc_listpgrppids` +
//! `proc_pidinfo(PROC_PIDTASKINFO)` on macOS, `/proc/<pid>/stat` on Linux).
//!
//! `ru_maxrss` is in bytes on macOS and in KiB on Linux; on other systems its
//! unit is not known here, so peak RSS is reported as unknown there, and
//! there is no group reading.

use std::mem::MaybeUninit;

/// Unit of `ru_maxrss` in bytes, where known.
#[cfg(target_os = "macos")]
const MAXRSS_UNIT: Option<u64> = Some(1);
#[cfg(target_os = "linux")]
const MAXRSS_UNIT: Option<u64> = Some(1024);
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const MAXRSS_UNIT: Option<u64> = None;

fn maxrss_bytes(who: libc::c_int) -> Option<u64> {
    let unit = MAXRSS_UNIT?;
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: `usage` points to writable storage for one `struct rusage` of
    // this target's layout (`libc`'s definition); `getrusage` writes only
    // into it.
    let status = unsafe { libc::getrusage(who, usage.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    // SAFETY: `getrusage` returned 0, so it filled the whole struct.
    let usage = unsafe { usage.assume_init() };
    u64::try_from(usage.ru_maxrss)
        .ok()
        .and_then(|value| value.checked_mul(unit))
}

/// Peak RSS of this process, bytes.
pub fn peak_rss_bytes() -> Option<u64> {
    maxrss_bytes(libc::RUSAGE_SELF)
}

/// Largest peak RSS among this process's terminated and reaped children
/// (and the descendants they reaped), bytes.
pub fn children_peak_rss_bytes() -> Option<u64> {
    maxrss_bytes(libc::RUSAGE_CHILDREN)
}

/// SIGKILL to every process of group `leader` (a child spawned with
/// `process_group(0)`, so its pid is the group id). Never group 0 (the
/// caller's own) or 1.
pub fn kill_group(leader: u32) {
    let Ok(group) = libc::pid_t::try_from(leader) else {
        return;
    };
    if group <= 1 {
        return;
    }
    // SAFETY: a plain system call without pointers.
    unsafe {
        libc::killpg(group, libc::SIGKILL);
    }
}

/// SIGKILL to the caller's own process group, which ends the caller too —
/// only when the caller leads that group; `false` otherwise.
pub fn kill_own_group() -> bool {
    // SAFETY: a plain system call without arguments.
    let group = unsafe { libc::getpgrp() };
    if u32::try_from(group).ok() != Some(std::process::id()) || group <= 1 {
        return false;
    }
    // SAFETY: a plain system call without pointers; the group is the
    // caller's own, which the caller leads (checked above).
    unsafe {
        libc::killpg(group, libc::SIGKILL);
    }
    true
}

/// Upper bound of the pid buffer of one macOS group listing.
#[cfg(target_os = "macos")]
const MAX_GROUP_PIDS: usize = 65_536;

/// Reads the summed resident size of every live process of one process
/// group, again at each [`GroupRss::resident_bytes`] call. Pages shared
/// between processes count once per process, as the system reports each
/// process's resident size.
pub struct GroupRss {
    #[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
    group: libc::pid_t,
    /// Reused listing buffer.
    #[cfg(target_os = "macos")]
    pids: Vec<libc::pid_t>,
    /// Bytes per page (`/proc/<pid>/stat` counts resident pages).
    #[cfg(target_os = "linux")]
    page: u64,
}

impl GroupRss {
    /// The reading of group `leader` (a child spawned with `process_group(0)`,
    /// so its pid is the group id); `None` on a system without one (neither
    /// macOS nor Linux) and for group 0 or 1.
    pub fn new(leader: u32) -> Option<Self> {
        let group = libc::pid_t::try_from(leader)
            .ok()
            .filter(|group| *group > 1)?;
        Self::for_group(group)
    }

    #[cfg(target_os = "macos")]
    fn for_group(group: libc::pid_t) -> Option<Self> {
        Some(Self {
            group,
            pids: vec![0; 256],
        })
    }

    #[cfg(target_os = "linux")]
    fn for_group(group: libc::pid_t) -> Option<Self> {
        // SAFETY: a plain system call without pointers.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let page = u64::try_from(page).ok().filter(|page| *page > 0)?;
        Some(Self { group, page })
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn for_group(_group: libc::pid_t) -> Option<Self> {
        None
    }

    /// Sum of the resident sizes of the group's live processes now, bytes;
    /// `None` when not one process of the group could be read (the group
    /// is gone, or the listing failed). A process that exits between the
    /// listing and its read is skipped.
    #[cfg(target_os = "macos")]
    pub fn resident_bytes(&mut self) -> Option<u64> {
        let count = loop {
            let capacity = self.pids.len();
            let size = capacity.checked_mul(std::mem::size_of::<libc::pid_t>())?;
            let size = libc::c_int::try_from(size).ok()?;
            // SAFETY: `self.pids` is writable storage of `size` bytes; the
            // call writes at most `size` bytes of pids into it and returns
            // how many it wrote (0 on error).
            let listed =
                unsafe { libc::proc_listpgrppids(self.group, self.pids.as_mut_ptr().cast(), size) };
            let listed = usize::try_from(listed).ok()?;
            // A full buffer may have cut the listing short: list again with
            // a larger one.
            if listed < capacity || capacity >= MAX_GROUP_PIDS {
                break listed.min(capacity);
            }
            self.pids
                .resize(capacity.saturating_mul(2).min(MAX_GROUP_PIDS), 0);
        };
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_taskinfo>()).ok()?;
        let mut total: Option<u64> = None;
        for &pid in &self.pids[..count] {
            let mut info = MaybeUninit::<libc::proc_taskinfo>::uninit();
            // SAFETY: `info` is writable storage for one `proc_taskinfo` of
            // `size` bytes (`libc`'s definition); the call writes at most
            // `size` bytes into it.
            let written = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTASKINFO,
                    0,
                    info.as_mut_ptr().cast(),
                    size,
                )
            };
            // Exited since the listing, or a zombie (no task left).
            if written != size {
                continue;
            }
            // SAFETY: the call reported the whole struct written.
            let info = unsafe { info.assume_init() };
            total = Some(total.unwrap_or(0).saturating_add(info.pti_resident_size));
        }
        total
    }

    /// Sum of the resident sizes of the group's live processes now, bytes;
    /// `None` when not one process of the group could be read (the group
    /// is gone, or `/proc` could not be listed). A process that exits
    /// between the listing and its read is skipped.
    #[cfg(target_os = "linux")]
    pub fn resident_bytes(&mut self) -> Option<u64> {
        let entries = std::fs::read_dir("/proc").ok()?;
        let mut total: Option<u64> = None;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let is_pid = name
                .to_str()
                .is_some_and(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()));
            if !is_pid {
                continue;
            }
            let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            let Some((group, pages)) = group_and_resident_pages(&stat) else {
                continue;
            };
            if group == self.group {
                let bytes = pages.saturating_mul(self.page);
                total = Some(total.unwrap_or(0).saturating_add(bytes));
            }
        }
        total
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn resident_bytes(&mut self) -> Option<u64> {
        None
    }
}

/// Process group (field 5) and resident pages (field 24) of a
/// `/proc/<pid>/stat` line. The command name (field 2) is parenthesised and
/// may hold spaces and parentheses, so fields are counted after its last `)`.
#[cfg(target_os = "linux")]
fn group_and_resident_pages(stat: &str) -> Option<(libc::pid_t, u64)> {
    let (_, rest) = stat.rsplit_once(')')?;
    // From field 3 (state): 3 state, 4 ppid, 5 pgrp, then 6 to 24 rss.
    let mut fields = rest.split_ascii_whitespace();
    let group = fields.nth(2)?.parse().ok()?;
    let pages = fields.nth(18)?.parse().ok()?;
    Some((group, pages))
}

/// Bytes → MiB, one decimal.
pub fn mib(bytes: u64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0
}

/// `/proc/<pid>/stat` parsing (Linux only; the reading itself is exercised
/// end to end by `tests/ra_cli.rs` on the host system).
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::group_and_resident_pages;

    /// A `/proc/<pid>/stat` line of pid 4242 with command name `comm`, in the
    /// kernel's layout (`proc(5)`): field 4 ppid 17, field 5 pgrp `group`,
    /// field 6 session 16, field 8 tpgid 15, field 23 vsize 5926912, field 24
    /// rss `pages`, field 25 rsslim, then fields 26 to 52. Every neighbour of
    /// fields 5 and 24 differs from them, so an off-by-one reads a wrong value.
    fn stat_line(comm: &str, group: i32, pages: u64) -> String {
        format!(
            "4242 ({comm}) S 17 {group} 16 34816 15 4194560 1180 0 3 0 12 7 0 0 20 0 9 0 \
             123456 5926912 {pages} 18446744073709551615 94000000000000 94000000020000 \
             140730000000000 0 0 0 0 4096 17663 0 0 0 17 3 0 0 0 0 0 94000000030000 \
             94000000031000 94000001000000 140730000001000 140730000001020 \
             140730000001020 140730000002000 0\n"
        )
    }

    #[test]
    fn reads_group_and_resident_pages_of_a_plain_command_name() {
        assert_eq!(
            group_and_resident_pages(&stat_line("cargo", 4100, 187)),
            Some((4100, 187))
        );
    }

    #[test]
    fn counts_fields_after_the_last_parenthesis_of_the_command_name() {
        // A command name may hold spaces, parentheses and digits (`comm` is
        // whatever the process set, up to 15 bytes): fields are counted after
        // its last `)`, never after the first one or by whitespace from the
        // start of the line.
        for comm in [
            "tokio runtime",
            "a) 1 2 3 (b",
            ")",
            ") S 9 8 7 6 5",
            "((x))",
            "rust (srv)",
        ] {
            assert_eq!(
                group_and_resident_pages(&stat_line(comm, 4100, 187)),
                Some((4100, 187)),
                "comm {comm:?}"
            );
        }
    }

    #[test]
    fn a_truncated_or_malformed_line_reads_as_nothing() {
        let full = stat_line("worker", 4100, 187);
        // Cut right after field 23 (vsize): field 24 is missing.
        let cut = full
            .find(" 5926912 ")
            .map(|at| &full[..at + " 5926912".len()])
            .expect("the line holds vsize");
        assert_eq!(group_and_resident_pages(cut), None, "{cut:?}");
        // No `)` at all: no command name to skip.
        assert_eq!(group_and_resident_pages("4242 worker S 17 4100 16"), None);
        assert_eq!(group_and_resident_pages(""), None);
        // A non-numeric group or rss.
        let bad_group = full.replacen(" 4100 ", " x ", 1);
        assert_eq!(group_and_resident_pages(&bad_group), None, "{bad_group:?}");
        let bad_pages = full.replacen(" 187 ", " -1 ", 1);
        assert_eq!(group_and_resident_pages(&bad_pages), None, "{bad_pages:?}");
    }
}
