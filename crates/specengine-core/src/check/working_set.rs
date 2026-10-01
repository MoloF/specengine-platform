//! The worst-case working set W of a task (§3 of the documentation
//! convention: Tier 0 + one Tier 1 + the index + k Tier 2 documents),
//! reported on the summary line and as `counts.worst_w_bytes`. Not a check:
//! no cap and no finding come of it.
//!
//! Over the walked files as read (their `size`): every canon `tier: 0` file
//! summed, the largest canon `tier: 1` file, the `[paths] index` file (0 when
//! not configured or not walked), the largest walked live shard of the index
//! entry (0 when none; ADR-0030: the index step reads the root and at most
//! one live shard), and the three largest of the other files that are
//! neither Tier 3 ([`is_tier3`], the index render's predicate) nor
//! `class: generated`. The archive shard adds 0, whatever its front-matter.
//! The tier is read from `tier:` on class canon only; a file whose
//! front-matter failed has no class and is pooled. Which files are the index
//! comes from `[paths]` and the index entry alone: no path is written here.

use super::config::{DocClass, Generator};
use super::input::CheckInput;
use super::render::{is_tier3, readable_fields};
use crate::Paths;

/// How many of the largest pooled files W counts (`k` of §3); fixed by the
/// convention, not a configuration key.
const K: usize = 3;

/// W of `input` in bytes: a pure function of the walked files, `[paths]
/// index` and the shards of `index` (the `index = true` entry; `None`: no
/// shard), independent of the order of `input.files`. Saturates instead of
/// overflowing.
pub fn worst_w(input: &CheckInput, paths: &Paths, index: Option<&Generator>) -> u64 {
    let shards = index.map_or(&[][..], |generator| generator.shards.as_slice());
    let mut tier0: u64 = 0;
    let mut tier1: u64 = 0;
    let mut root: u64 = 0;
    let mut live_shard: u64 = 0;
    let mut pool: Vec<u64> = Vec::new();
    for file in &input.files {
        if paths.index.as_deref() == Some(file.path.as_str()) {
            root = root.max(file.size);
            continue;
        }
        if let Some(shard) = shards.iter().find(|shard| shard.path == file.path) {
            if !shard.is_archive() {
                live_shard = live_shard.max(file.size);
            }
            continue;
        }
        let fields = file.parsed.as_ref().and_then(readable_fields);
        let class = fields
            .and_then(|fields| fields.class.as_deref())
            .and_then(DocClass::parse);
        match (class, fields.and_then(|fields| fields.tier)) {
            (Some(DocClass::Canon), Some(0)) => tier0 = tier0.saturating_add(file.size),
            (Some(DocClass::Canon), Some(1)) => tier1 = tier1.max(file.size),
            (Some(DocClass::Generated), _) => {}
            _ if fields.is_some_and(is_tier3) => {}
            _ => pool.push(file.size),
        }
    }
    pool.sort_unstable_by(|a, b| b.cmp(a));
    let largest = pool
        .iter()
        .take(K)
        .fold(0_u64, |sum, size| sum.saturating_add(*size));
    tier0
        .saturating_add(tier1)
        .saturating_add(root)
        .saturating_add(live_shard)
        .saturating_add(largest)
}
