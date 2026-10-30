---
class: canon
tier: 2
scope: [crates/specengine-import, crates/specengine-eval]
owner: owner
reviewed: 2026-10-30
---

# Import: list lead-ins and the kept walk

How the import reads a list item's strong lead-in (`specengine-import` `src/import/lead_in.rs`, `list_items` in `src/import/mod.rs`), what it reports when it takes no record from one, and the walk the emitted scheme keeps. The record model and the other positions: `docs/canon/import.md`. Every convention comes from the config (ADR-0008).

## Wrapped spans

A list item (after `[ ]` / `[x]`) opens a strong span `**…**` / `__…__`. A span closed on the item's line reads as it stands. A delimiter not closed there continues over soft line breaks to the first matching one (`lead_in::list_span`), each line it crosses, the closing line too, paragraph text (`crossed_text`). There is no lead-in when one of these comes first:

- a line blank as written or once its comments are cut, a table line, a line inside a fence;
- a line whose comment-free text opens, at any indentation, a list item, heading, fence, table row, block quote, thematic break or HTML block (`<!--` as written too).

A line opening inside a comment the span opened is paragraph text whatever follows `-->`. The content on the item's line is not blank and holds the ID: a candidate crossing a break is none. A soft line break and the blanks around it read as one space. `line`, `task_box` and the ID's token exclusion stay on the item's line. Records, the nested-record test ending a parent's text, the hyphenless lead-in position (`docs/canon/import.md` "IDs and tokens") and `not_taken` all read the span this way. A paragraph lead-in does not wrap.

## Titled lead-ins

One `[lists] separators` list splits ID from title and title from text.

- The span's trimmed content reads as ID [separator [title]]: the ID is the longest leading part that resolves, ending right before a listed separator or at the content end, each candidate trimmed; several separators at one position: the longest. Resolving: an ID, or an unmapped legacy prefix (→ `legacy.unmapped`, no record); a hyphenless match does not resolve, the next shorter part is tried. Nothing resolves: no record.
- A title exists iff the comment-free span holds content after the separator. Its text is the raw lines from right after the separator to the closing delimiter, each break one space, trimmed; inline markup, escapes and comments kept (`**ID. Title.**` → `Title.`): a heading and a YAML scalar are one line. It stays outside `text` and `hash`; ID-like tokens in it count as anywhere; the hyphenless lead-in test is unchanged.
- A span ending in a separator with no title strips no separator after it; any other span strips one (right after the span, else past blanks).
- A legacy titled ID → the Latin `id` and an alias; a feature-scoped one → a record.

## List item text

The raw closing line (the item's own for a span closed there) after the span and its separator, comments kept; then deeper and lazy lines, until a blank line before a shallower one, an item at the marker's indent or shallower, a nested record item (titled and wrapped ones included), or a heading. A lazy line continues the paragraph unless it opens a list item, heading, fence, table row, thematic break, HTML block, block quote, or starts `<!--` outside a comment. A fence opened on a deeper line holds code to its closing fence, no lazy line after it. Block syntax is read past the marker's columns (tab: next multiple of four), not the content column. Lines joined by LF (CR before LF dropped), trailing whitespace trimmed. The extent runs from the marker's line through the text's last line, the closing line at least.

## Not taken

With `[lists] lead_in` on, a list item read for records (no line opening inside a comment, no table line) lands in `Import.not_taken` when, after an optional task box, it opens a strong delimiter with content on its line, `ids.like` matches at that content's first non-blank character, emphasis delimiter runs read as blanks and unbounded after the match (`XQ-17a.` counts), and it gives no record of either role and no `legacy.unmapped` token. So does `XQ-12 (part)`, a separator not right after the ID, and a span never closed. `lead_in` off: none. No other count changes.

`NotTaken {path, line, lead}`, in (path, line) order; `lead`: the item's comment-free line from the opening delimiter through the closing one, else to the line end, trailing blanks trimmed. Reported as `import` `records.not_taken` (outside `total`), `layout` `before.not_taken`, and `not_taken.json` in both runs' detail. A clean hash result counts only what was recognised; this count shows what was not.

## Emitted `[paths]`

The after-tree's scheme keeps the before walk. `Scheme::parse(text, roots, exclude, hides_index_output)` takes it as core read it (eval `prepare`, `before.project.paths`). The import crate has no core dependency, so `hides_index_output` is a predicate: core's `WalkScope::is_excluded` on a copy of the paths holding that one glob, over each index output (`[paths] index`, the shards of the `index = true` generator).

- `roots`: the before roots (`Paths.roots`: as written, duplicates dropped; unwritten, the role defaults), each kept only if a tree path, document or index output, lies at or under it, since a written root naming nothing stops core's check. Then `[layout] records`, then `features`, each appended only if no kept root equals or holds it, it is no root yet, and a tree path lies under it.
- `exclude`: the before globs in written order, `[]` when none, less each matching an index output this scheme writes. Each dropped glob becomes, in written order, a comment right above `exclude`, the glob quoted as a TOML basic string so the comment stays well-formed: `# exclude dropped: "<pattern>" matches an index output this scheme writes`.

`index.files` = `tree.documents` holds when the kept walk covers every tree document. A census walk wider than the scheme's leaves residues written yet unwalked, as in the before walk: `index.files` lower, `hashes` unmoved, the verifier also reading the emitter's files the walk misses. The whole emitted file: `docs/canon/import-layout.md` "Emitted configs".

## Known limits

- A kept exclude can hide a directory the scheme itself adds as a root (`exclude = ["docs/**"]`, `records = "docs/records"`). `index.files` < `tree.documents` shows it, `hashes.missing` does not ("Emitted `[paths]`"); it is not dropped automatically.
- A setext `===` underline is crossed as paragraph text, though CommonMark reads it as a heading; `item_continuation` has the same gap. Very rare.
- An HTML tag line or an ordered marker other than `1.` ends a span, conservatively, though CommonMark continues the paragraph; an ID-led item there lands in `not_taken`.
