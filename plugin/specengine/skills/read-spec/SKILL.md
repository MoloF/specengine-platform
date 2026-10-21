---
name: read-spec
description: Read the project's spec through the SpecEngine tools before reading or changing behaviour the spec describes, or when asked what the spec says. A context bundle first (a search first when no ID is known), then a node, its links or the tree; spec files only for what the bundle lacks.
---

# Reading the spec

The project's CLAUDE.md and its roles take precedence over this skill.

Use this before you read or change behaviour the spec describes, and whenever you need to know what the spec says. The spec is a tree of documents and sections named by IDs; the SpecEngine tools read it as it is on disk now.

## Order

1. **The bundle first.** Call `get_context_bundle` with the IDs (or root-relative `.md` paths) your task names, in `node_ids`. One answer holds the targets, their open questions, ancestors, criteria, the code bound to them with each binding's `sync`, decisions, neighbours, terms and the tests bound to them or their criteria; what did not fit the budget is named by ID (a binding by its symbol, else its file) with its cost.
   With no ID known, as for "what does the spec say about this?", call `search` first with a `query` of plain words, then bundle the hits you need by their IDs (else their paths).
2. **Then narrow, only where the bundle is not enough:**
   - one node's current text: `get_node` with its `id`; add `with: ["links"]` for its links when you need what it points at and what points at it, `with: ["bindings"]` for the code bound to it and whether that code still matches the spec (each binding's `sync`);
   - a term or a behaviour without a known ID: `search`, then open only the hits you need;
   - the shape of the spec, or where a new piece belongs: `get_tree`, from a `root` and to a `depth` when the whole tree is too large;
   - an item at a line or a name, and the spec markers bound to it, before you change code (the project's config needs a `[code]` table): `find_symbols` with exactly one of `at` (a root-relative path, a colon and the line), `query` (a name, the end of a qualified path or a whole one), `node` (an ID: the items bound to it) or `file` (a code file's items);
   - what in the code binds a node: `refs` with its `id`, the markers and the mentions naming it;
   - where code has no spec: `unmapped`, with a `path` (a code root, a directory or a code file), or without one for every root.
3. **Files last.** Open spec files directly only for what the tools did not give you; never list or read the whole spec instead of a `search`.

## Using what you read

- Cite the IDs you relied on: in your answer, your plan and your report.
- The answers are deterministic: the same files and the same request give the same answer, so reading an unchanged node again only spends context.
- An answer cut for size says how to narrow the request: follow it instead of asking for everything again.
- A reference that names nothing is an error with the reason: check the ID, or `search` for it. A look-alike or mixed-script ID is refused with its Latin fix: use the fix.
- If a tool answers that there is no project (no `specengine.toml` here or above), the working directory is not a SpecEngine project: stop using these tools for this session, do not set one up yourself, and say so instead of guessing what the spec holds.
- When the spec is silent or ambiguous, or the code disagrees with it, do not fill the gap silently: raise it with the ask-owner skill.
