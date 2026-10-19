//! The binary's POSIX calls, through `libc`: the peak resident set size
//! (`getrusage`), whether this process leads its process group (`getpgrp`)
//! and SIGKILL to that whole group (`kill(0, ..)`).
//!
//! `ru_maxrss` is in bytes on macOS and in KiB on Linux; elsewhere its unit
//! is not known here and `peak_rss` is reported as unknown (`null`).

/// Unit of `ru_maxrss` in bytes, where known.
#[cfg(target_os = "macos")]
const MAXRSS_UNIT: Option<u64> = Some(1);
#[cfg(target_os = "linux")]
const MAXRSS_UNIT: Option<u64> = Some(1024);
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const MAXRSS_UNIT: Option<u64> = None;

#[cfg(unix)]
fn maxrss_bytes(who: libc::c_int) -> Option<u64> {
    use std::mem::MaybeUninit;

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

/// `peak_rss`: this process's peak RSS plus the largest peak among its
/// terminated and reaped children (cargo, rustc), in bytes.
#[cfg(unix)]
pub fn peak_rss() -> Option<u64> {
    maxrss_bytes(libc::RUSAGE_SELF)?.checked_add(maxrss_bytes(libc::RUSAGE_CHILDREN)?)
}

#[cfg(not(unix))]
pub fn peak_rss() -> Option<u64> {
    None
}

/// Whether this process leads its process group: its caller spawned it as
/// a group of its own, or an interactive shell with job control started it
/// as a job, alone or as the first command of a pipeline. In a pipeline,
/// unless it is the first command, or under a shell without job control,
/// another process leads the group, and the group is not this binary's to
/// kill.
#[cfg(unix)]
pub fn leads_its_group() -> bool {
    // SAFETY: plain system calls without arguments.
    let (group, own) = unsafe { (libc::getpgrp(), libc::getpid()) };
    group == own && group > 1
}

#[cfg(not(unix))]
pub fn leads_its_group() -> bool {
    false
}

/// SIGKILL to every process of this process's group, itself included —
/// only when it leads that group ([`leads_its_group`]); nothing is sent
/// otherwise.
#[cfg(unix)]
pub fn kill_own_group() {
    if !leads_its_group() {
        return;
    }
    // SAFETY: a plain system call without pointers; pid 0 addresses the
    // caller's own process group, which the caller leads (checked above).
    unsafe {
        libc::kill(0, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
pub fn kill_own_group() {}
