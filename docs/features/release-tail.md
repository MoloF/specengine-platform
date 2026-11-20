---
class: spec
status: shipped
shipped: 2026-11-20
scope: [scripts/deploy, crates/specengine-model, crates/specengine-store, crates/specengine-http, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: RM-0131 and RM-0133; the analyst's breakdown of 2026-11-20; the owner's word of 2026-11-20 ("fix what remains, to the end"), on which the main session decided his open questions; RM-0129's review findings; patch release 0.49.1
adrs: []
---

# release-tail: the door's "no TOTP" read right, the install on the current tag, one racy test, Documents and guide-citation nits (0.49.1)

## Why

This is a maintenance slice inside accepted decisions (ADR-0068, ADR-0015, ADR-0085, ADR-0012, ADR-0072; ADR-0008 holds), so it has no ADR. It shipped as one commit: plugin 0.49.1, a version-only PATCH, tag `v0.49.1`.

1. `hub-admin reset` logged `totp-left` for a TOTP that never existed, because the pinned door answers a delete on nothing with exit 1 (RM-0131).
2. A fresh install cloned `v0.29.1`, so it ran that release's image and scripts.
3. `rn4_until` raced the runner's `past_until` decision against the shim's clock.
4. Documents had three nits:
   - a failed Retry left the focus armed;
   - the last-answer line had no live region;
   - `behind: null` looked like `false`.
5. RM-0129's review found three citation issues:
   - `guide:x#R-1#S-2` left `S-2` a bare reference;
   - the UI's left boundary before `guide` differed from the grammar's;
   - a 0.01rem gap in a scroll-margin query.

Decided 2026-11-20 by the main session on the owner's word:
- skip the `#ID` after a guide citation, with `INDEX_FORMAT` going to 21;
- the UI moves to the grammar's boundary;
- the install tag is pinned by a test alone;
- "no TOTP" means nothing to delete, and other failures keep the warning;
- `behind: null` gets its own neutral look.

## Data

Sections the code cites; the canon holds the truth.

### The door's answer

On `authelia/authelia` 4.39.28 (`dump-door`'s digest), recorded 2026-11-20:
- `storage user totp delete <login>` with no TOTP exits 1 with `Error: failed to delete TOTP configuration for user '<login>': no TOTP configuration for user`;
- `webauthn delete --all` on nothing exits 0.

Only that exit and that line, naming the reset's own login, mean nothing to delete (`hub-deploy.md` "Gate"; `hub-accounts.md` AC19).

### The README's install line

Exactly one `git clone --branch v<X> https://github.com/MoloF/specengine-platform`, where X is `plugin.json`'s version (`hub-deploy.md` "Image").

### The grammar (RT9, RT10)

A guide citation's tail, a `#ID` and then an `@rev`, is read by shape and is no reference. `INDEX_FORMAT` is 21 (the model README's rule (7); `spec-guide.md` "Citations").

## Acceptance criteria

- [x] **RT1 Nothing to delete.** The stub answers the recorded line with exit 1. `reset mira` exits 0 with no `warning:`, logs `password passkeys totp`, writes the digest and shows the password once. M: today's `if ! delete_totp`.
- [x] **RT2 Other failures still warn.** Each of these gives the warning and `totp-left`:
  - another message with exit 1;
  - the recorded line with exit 2 or 124;
  - the same words naming another login.

  A failed passkey delete beside the recorded TOTP answer gives `passkeys-left` alone, exit 1, "do not enable". The tests cover six failure cases. M: any exit 1 read as nothing to delete; a substring match.
- [ ] **RT3 The pinned image agrees** (`deploy_admin.rs` `rt03_*`, `#[ignore]`, HB5's harness): `totp delete` of a login with no TOTP answers exactly the recorded line, exit 1. **An owner's gate:** Docker is off on the laptop by his choice. The record of 2026-11-20 by RM-0131's developer stands until his run is recorded here.
- [x] **RT4 The install clones the current release** (`deploy.rs` `rt04_*`): exactly one `git clone --branch v<X>` line, X equal to `plugin.json`'s version. M: the version bumped, the README left behind.
- [x] **RT5 `rn4_until` deterministic.** It takes `rn4_stop`'s shape:
  - one clock reading gives `until` and its epoch, `UNTIL_AHEAD` (8 s) ahead;
  - the first leg holds;
  - the test waits for `held`, then for the wall clock past the epoch, then releases.

  It asserts exactly one call, exit 0 with `autopilot: until <TIME>`, nothing landed, and the held leg finished. It passed 20 of 20 unloaded, and 20 of 20 at 10 `yes` processes (9.6-9.8 s each); the full suite is in "Release". M: the `past_until` check removed from the leg branch.
- [x] **RT6 The "not known" look.** `behind: null` shows a neutral "Not known" mark, its title the whole phrase, in en and ru. It is distinct from `true`'s lozenge and from `false`. The toggle and the count line count only `true`. M: null shown as false; null counted as behind.
- [x] **RT7 The last-answer line is announced.** "Showing the last answer…" sits in a polite live region, mounted before its words appear. M: the region removed.
- [x] **RT8 Focus after a failed Retry.** Retry fails, then the lone Include archive is pressed: when the answer lands, focus is on the bar's checkbox. A successful Retry still takes focus to the table. M: the failed Retry stays armed.
- [x] **RT9 (a) The citation's tail is no reference.**
  - `guide:x#R-1#S-2` and `…#S-2@3` give none, and so do the alias rows (`guide:x#R-1#OLD-5`).
  - `R-1#S-2` still gives R-1 with section S-2.
  - `guide:x#R-1 S-2` gives `S-2`.

  The scan stays one pass. M: the skip removed.
- [x] **RT10 (b) Both sides agree.** The table's 17 rows (`#`, `@`, `.`, `:`, `/`, `-` or `(` before `guide`, and the RT9 forms) agree on both sides. Its copies `fixtures/guide-citations.json` and `ui/src/markdown/guide-citations.json` are byte-identical. M: the old left boundary in `guide.ts`; a row edited on one side.
- [x] **RT11 (c) No gap.** The query is `not all and (min-width: 68.75rem)`, pinned in `header.test.tsx`. M: `68.74rem` restored.
- [x] **RT12 0.49.1.**
  - `plugin.json`, `.env.example` and the README's tag are 0.49.1, and `tag-check.sh v0.49.1` prints it.
  - `PINS` gains 0.49.1, version-only, hashing as 0.49.0's; `SCHEMA_PINS` is unchanged; `ac8d` is 0.49.1.
  - `INDEX_FORMAT` is 21, with its `format_history.txt` line.

  M: `plugin.json` bumped alone.

## Out of scope

The default Documents filter (it matches the canon), the probe-phase wording, the flow studio's `64rem` pair, a CLI form for `behind`, `tag-check.sh` reading the README, RM-0132, RM-0126 and concurrent trusted reads.

## Implementation

| Module | What it does |
|---|---|
| `scripts/deploy/admin/hub-admin` | `delete_totp` reads exit 1 with `no_totp_said` for its login as nothing to delete |
| model `grammar.rs` | after `cites_the_guide` the scan resumes at `guide_citation_end` (the ID, a `#ID`, an `@rev`, by shape) |
| store `lib.rs` | `INDEX_FORMAT` 21 |
| ui `markdown/guide.ts` | `citesAlone`: `GUIDE_STOP`, rule (5), on the left; `standsAlone` on the right |
| ui `DocumentsView.tsx`, `app.css`, `en.ts`, `ru.ts` | `BehindMark`; `useRetryFocusUnlessFailed`; the `documents-shown` live region; the query; `documents.behindUnknown.*` |
| version | `plugin.json`, `.env.example`, `scripts/deploy/README.md` |
| tests | http `deploy_admin.rs`, `deploy.rs`, `spec_ids_hub.rs`; cli `autopilot.rs`; mcp `plugin_*.rs`; model `grammar.rs`; store `format*`; four UI files; `fixtures/guide-citations.json` |

**Canon:** the model and store READMEs; `spec-guide.md`, `spec-library.md`, `hub-deploy.md` and `plugin-files.md`; the deploy README; `hub-accounts.md` AC19 and `hub-backups.md`.

**Deviations:**
1. The shared table lives under `ui/`, and the model's copy is compared byte for byte.
2. RT6 takes three keys: the mark, a description read before it, and the title.
3. The tail is read by shape, so an alias's `#ID` goes too.
4. RT8 wraps the shared hook in Documents alone.
5. The UI's right side stays `standsAlone`'s (`spec-guide.md` "Known limits").

### Release

Full run: 6 554 of 6 557 passed; the 3 failures lie outside the slice's code:
- `import_cli` and `layout_cli` see the untracked `fixtures/guide-citations.json`, green once committed;
- `card_split` `rh_r4` was a wall-clock flake under load, and now masks its stamps.

Clippy `-D warnings` and `spec check` are clean. UI: 249 files, 6 519 tests; first load 838 738 B of 845 kB (+105 B). Under 10 `yes` processes: `rn4_until` 20 of 20 (9.6-9.8 s each), `rt09_the_scan_stays_one_pass` 10 of 10.
