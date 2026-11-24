---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, scripts/deploy, ui]
owner: owner
reviewed: 2026-11-24
---

# Work references: promotion into git

ADR-0089, built by `reference-promotion` (RM-0138, plugin 0.52.0); formats and code: `work-references-promotion-surfaces.md`. Promotion copies a reference's file (`work-references.md`) into the repository beside a document.

<a id="the-rule"></a>
## The rule

A reference's file enters git only through a proposal of kind `reference` (`spec propose reference RF --to REF`, MCP `propose_change`, the page's "Promote to canon"), applied by `apply_proposal` once a person confirms: on a terminal, one `[y/N]`; on a hub, one click by a login `[team]` entitles whose zones cover both paths. Nothing else writes the file; `[auto]` never applies the kind (`auto-mode.md` "Judging an item": `proposal-kind`). The apply is one commit with provenance that writes:

- the file, its bytes BLAKE3-equal to the reference's, streamed from the blob store, at the home `[references] homes` names for the document's kind, beside the document;
- one entry `<path> | <what matters>` appended to the document's `references:` key, never counted ("The key").

A link is never promoted. The reference stays queue data after its apply; deleting it leaves git's copy. Nothing blocks on a promotion (ADR-0012).

## Homes

```toml
[references]
allow = ["image", "audio", "model"]   # RM-0137's

[[references.homes]]
kinds = ["location", "character"]     # [ids] kinds whose documents take promoted files
dir   = "media"                       # under the document's directory; omitted: that one
```

Read by core `ReferencesConfig` beside `allow`: a bad table refuses reference writes, as before; no `spec check` code reads it. Each home: `kinds` 1 to 32 kinds some `[ids]` prefix declares, none in two homes; `dir` a clean relative path (no leading `/`, no `.`, `..`, `.git` or empty part, no `\` or control), 1 to 255 bytes. Unknown keys are errors at their line. The core names no kind and no directory (ADR-0008): a project's config is the only source, and the gamedev preset carries a commented example (`presets.md`). The document's kind is the one it declares, else its prefix's; with none, or no home taking it, the propose cannot run (exit 2).

The file's path is `<document's directory>/<dir>/<name>`: `name` the reference's stored name or `--name`, its extension one of its media type's, never `.md`, so never a walked document. The document lies under a `[paths] roots` directory, keeping the file in a hub's sparse set.

<a id="the-key"></a>
## The key

`references:` is a typed front-matter key (`TYPED_KEYS` 35, a list of text) no recipe counts: neither the norm hash nor a revision sees it, so a promotion raises no revision, adds no drift and needs no lock accept (`node-revision.md` "The recipe"). The document's bytes still change, so its span hash and a bundle holding it move. Each entry is `<path>` or `<path> | <what matters>`, split at the first ` | `: the path relative to the document's directory and clean, no `.git` part, not `.md`; the text one line of at most 512 characters, no control character. Entries are judged in order, kept while fewer than 32 are. `reference-entry`, a warning at the key's line, names each one left out; a key that is no list is `frontmatter-type`, as for `tags`. A hand-written entry equals a promoted one.

An apply appends its entry as `  - "<entry>"` (`\` and `"` escaped) after the key's last item, or adds the key before the closing fence; `references: []` becomes a block. Any other form (a flow list with items, a scalar, an alias) is refused at propose and apply with its fix. Every other byte of the document stays.

## Propose

`spec propose reference RF --to REF [--name N] [--matters TEXT | --matters-file F|-] --rationale T [--task T] [the author flags] [--brief]`, its outputs `propose update`'s. In order, the first refusal answering, exit 1 unless named, nothing stored:

1. **Form** (exit 2): clap, the author grammar, `[ids]` claiming `RF` or `PR`, an unbound place, a bad `[references]`.
2. **The reference**: a look-alike exit 2; `no reference RF-0099`; `RF-0012 is deleted: nothing to promote`; `RF-0010 is a link: only a file is promoted into git`; `RF-0012's file is not on this host: nothing to promote`.
3. **The document**: `--to` resolves as an update's target (`proposal-queue.md` "Creation" 1); a bare ID several files hold, the non-feature document; a section resolves to its document, noted `` `RULE-X` is a section of `LOC-Y`: the file goes beside the document ``; outside every `[paths] roots` directory refused.
4. **The home** (exit 2): `` `<path>` has no kind: a [references] home is chosen by kind ``; `` no [references] home takes kind `<k>` (`<ID>`): add one to [[references.homes]] in specengine.toml; nothing stored ``.
5. **The name**: RM-0137's name rules, then no `|`, no leading `.`, no trailing space or `.`, and an extension of its media type's: `` name `harbour.png` does not end in an extension of image/jpeg (.jpg, .jpeg): give one with --name ``. The path at most 4 096 bytes, outside `[paths] generated`.
6. **The path**: no symlink on the way. Free: neither the disk nor `HEAD` holds it. In `HEAD`, `HEAD`'s blob alone is judged, never the disk: a plain home's blob ID the bytes' object ID, an LFS home's blob their pointer, then `` `<path>` is in place: only the entry is added ``. On disk or staged alone, it must hold the bytes (BLAKE3) or their pointer, and is added as it stands. Else `` `<path>` holds other bytes: a promotion never replaces a file; name it with --name ``. A path step 7 refuses is never hashed; the refusals keep their order.
7. **Attributes**: "LFS"'s, judged now; git-lfs missing is a note.
8. **The entry**: `--matters`, else (absent or blank) the reference's text, each run of line breaks one space; over 512 characters or a control: `` RF-0012's what matters is <n> characters: an entry holds at most 512; give one with --matters ``. Inserted ("The key") into the document now.
9. **A live twin**, read in the insert's transaction: `` RF-0012 is already proposed for `<ID>` by PR-0042 (open) ``. Then an update's validation, findings stored, never refusing, and the insert.

The row (queue schema 17): `target_id` the document's canonical ID, `base_hash` its span hash, `new_text` the entry, `ref_id` the RF; the rest an update's.

## Apply

An update's steps (`proposal-apply.md` "Apply steps"), with these:
- **1** `apply PR-0042 to <document> on <branch> in <worktree> (applies; adds <file>, RF-0012, image/png 2.1 MiB, LFS)? [y/N]` (`rebases`, `plain`; `(applies; <file> is in place)`).
- **3-6** The path as at propose, read now; the reference live and stored; the entry inserted into the bytes just read, `rebases` when they moved, never `git merge-file`; the norm hash unchanged; "LFS" again, git-lfs missing an exit 2 before the question.
- **8** Stray temps swept, each over an hour old or its pid another, ended process's (a live run's stays): `.*.specengine-<pid>-<n>.tmp` in the tree and `<common dir>/specengine-tmp/`, `specengine-<pid>-<n>` in `<lfs storage>/tmp`. A free path: the bytes streamed from the store through BLAKE3 and the object hash (plain), or through BLAKE3 and SHA-256 into the LFS store (an object there hashed: kept, nothing written, when it matches), into a temp in `<common dir>/specengine-tmp/` holding the bytes or the pointer, hard-linked into place, never replacing an entry (across devices, copied beside it). A file added as it stands: only a missing LFS object written. A BLAKE3 unlike the reference's, or its blob gone (`its file is not on this host`), writes nothing.
- **9** The file intent-to-add, one `git commit --only` of the document, the file and the index's outputs, trailers ending `Reference: RF-0012`; failed: the file this run wrote and its new directories removed, the index entry dropped only when none existed before the run, the document restored.
- **10** Exactly `M <document>` and `A <file>` (in `HEAD` already: `M` alone) beside the outputs; whenever it adds the file, the committed blob's ID is the one expected: the bytes' object ID, or the pointer's.

Completion by the trailer, a killed run and a reject follow a create's for the file and an update's for the document (`proposal-create-apply.md`); `--open` walks the kind.

## LFS

The home follows the project's own `.gitattributes`, read by `git check-attr` in the place at propose and again at apply:
- `filter` `lfs`: the commit holds a v1 pointer (`oid sha256:`, `size`), never the raw blob. The apply streams the bytes through SHA-256 into the LFS store itself, `lfs.storage` else `<common dir>/lfs/objects/<aa>/<bb>/<oid>`, and writes the pointer; `git lfs ls-files`, `fsck --objects` and an upload accept them. It never runs `git lfs clean`, which writes hooks and config into the repository (git-lfs 3.8). git-lfs 3.0+ must still be installed where it applies, asked only `git lfs version`, which writes nothing (exit 2 otherwise).
- `filter` unset: a plain blob of at most 50 MiB; any other filter, or `text` set, is refused.

The worktree holds what was committed: the pointer for an LFS home (`git lfs checkout -- <path>` brings the bytes), the file for a plain one. A terminal's push is its owner's git, whose LFS hook uploads the object.

## Reads and context

The index (`INDEX_FORMAT` 22) keeps each document's kept entries, the path root-relative. Read per call after the freshness step, as practices are (`card-practices.md`):
- a card's context lists the entries of every target's document (a target section reaches its document), a task's package those of its nodes' documents, after their own references, counted in `U` and outside the bundle (`card-context.md`);
- `spec ref list`, `list_references` and `GET .../references` add them on a card or a node as "In the spec";
- the page's References part shows them with their documents and offers "Promote to canon" on a stored file.

## On a hub

- **Author.** A page's promotion keeps its person: the login, else the page's `as` when a valid login, the author and the `as` of `proposal.created`.
- **Cover.** A confirm names two paths, the document and the file; a lead's zones must hold both, else 403 (`hub-confirm.md` "Rules"). The owner passes.
- **Turn.** The copy runs in the apply, in the project's turn, in 64 KiB pieces, never the file whole (ADR-0086).
- **Image.** git-lfs is installed, never as a filter; the image and every sync child set `GIT_LFS_SKIP_SMUDGE=1`, so a clone, a fast-forward or a merge writes pointers and fetches nothing (`hub-deploy.md` "Image").
- **Push.** Before pushing, a `--push` sync uploads the LFS objects its outgoing commits add (`git lfs push`, listed by its `--dry-run`), under the deadline; a failed upload is `retrying` and the push waits (`hub-git.md` "Sync"). Before and after every upload it removes a hook (`pre-push`, `post-checkout`, `post-commit`, `post-merge`) only when its whole text is git-lfs's three-line template for that name, byte for byte (`#!/bin/sh`, the guard naming it, `git lfs <name> "$@"`, one final LF allowed); a hook with git-lfs's lines merged in stays. `git lfs version` probes git-lfs. The deploy key carries both.

## Known limits

- A confirm holds the project's turn while the bytes copy. Soak RP, one every 10 s: the turn held p95 about 785 ms (POST confirm), 430 (promote), 190 (GET confirm), 185 (decision); the page p95 203 → 699 and 217 → 610 ms, its 1-2 ms reads waiting for the turn. Real use: a few a day.
- A hub keeps a promoted file twice: in the blob store while a reference names it, and in its checkout's LFS store or objects for good.
- `.gitattributes` is read as the worktree holds it; `text=auto` turning a file git takes for text is caught only after the commit, which stays and is named.
- A promoted file whose entry is removed stays in git; an entry whose file is gone is no finding.
- A sync's upload leaves git-lfs's `lfs.repositoryformatversion` in the checkout's git config: untracked, harmless.
- A renamed document's apply is refused (no re-targeting); its entries move with it, their paths relative.
