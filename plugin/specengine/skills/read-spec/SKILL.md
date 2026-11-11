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
   - one node's current text and `ancestors`, its path from the top: `get_node` with its `id`; add `with: ["links"]` for its links both ways, `with: ["bindings"]` for the code bound to it and whether it still matches the spec (each binding's `sync`);
   - a term or a behaviour without a known ID: `search`, then open only the hits you need;
   - the shape of the spec, or where a new piece belongs: `get_tree`, from a `root` and to a `depth` when the whole tree is too large;
   - the ordered flow, its typed edges and gap rows, which stop nothing: `get_flow`; if too large, from a `root`, to a `depth`, or `in_flow: true` to keep only what is in the flow;
   - the state of a node against the cards answering it, which stops nothing: `get_trace`, with a `ref` to read one node and what it holds;
   - an item at a line or a name, and the spec markers bound to it, before you change code (the project's config needs a `[code]` table): `find_symbols` with exactly one of `at` (a root-relative path, a colon and the line), `query` (a name, the end of a qualified path or a whole one), `node` (an ID: the items bound to it) or `file` (a code file's items);
   - what in the code binds a node: `refs` with its `id`, the markers and the mentions naming it;
   - where code has no spec: `unmapped`, with a `path` (a code root, a directory or a code file), or without one for every root.
3. **Files last.** Open spec files directly only for what the tools did not give you; never list or read the whole spec instead of a `search`.

## Using what you read

- Cite the IDs you relied on: in your answer, your plan and your report.
- A node's `tested`, in `get_node` with its bindings, in `get_task`, in `get_impact` and as lines under a bundle's headers, says whether the owner's own passing test run still vouches for that node, judged now: evidence to read, never yours to claim, and nothing waits on it.
- The answers are deterministic: reading an unchanged node again only spends context.
- An answer cut for size says how to narrow the request: follow it instead of asking for everything again.
- A reference that names nothing is an error with the reason: check the ID, or `search` for it. A look-alike or mixed-script ID is refused with its Latin fix: use the fix.
- If a tool answers that there is no project (no `specengine.toml` here or above), stop using these tools, do not set one up, and say so rather than guess what the spec holds.
- When the spec is silent or ambiguous, or the code disagrees with it, do not fill the gap silently: raise it with the ask-owner skill.
- Work of any trade stands on cards, queue rows, not in the spec: `list_cards` and `get_card` read them, `create_card`, `split_card`, `edit_card` and `move_card` file and change one, and no tool accepts a card. A hand-off is a child card (`parent`) in the other trade's flow, its `reviewer` the side that asked; a move into a flow's `return` column needs a `note`, why it goes back. People use the web UI's words: a card is a task, a child card a subtask, a task of `get_task` an agent task, a workspace a department.
