---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-23
summarises: [docs/canon/cards.md#commands, docs/canon/cards.md#who-acts, docs/canon/card-flows.md#config, docs/canon/card-flows.md#the-rule, docs/canon/card-split.md#the-rule, docs/canon/card-tasks.md#the-rule, docs/canon/card-context.md#the-rule, docs/canon/workspaces.md#the-lens, docs/canon/hub-department.md#config, docs/canon/comments.md#the-rule, docs/canon/comments.md#delete, docs/canon/work-references.md#the-rule, docs/canon/work-references.md#delete]
---

# Cards, departments and flows

A card is one piece of any department's work, tracked in the project's queue beside the spec it targets, and it moves through its department's own columns. This page covers flows and disciplines, departments, asking another department through a hand-off, splitting work across departments, the agent tasks that carry code, and the comments and references people leave on a task.

## What it solves

A studio's writers, artists, sound designers and programmers each have their own stages, and they keep asking each other for work. A separate tracker loses the link to the spec, and one shared column set fits nobody. Cards keep each trade's work in that trade's columns, aimed at spec nodes, and a hand-off records who asked and who accepts the result. Comments keep the talk about a task, a document or a proposal beside it, where its agent reads it, without letting it pass for a decision.

## When to use it

- **Any work item of any trade**: write a scene, model a prop, record a sound, fix a bug.
- **Asking another department**: a hand-off, a child card in their flow.
- **Work several departments share**: a split makes one task and a subtask per department in one act.
- **Code work by an agent**: a `T-` task, linked from the card.
- **Talk about a task, a document or a proposal**: a comment; when something must be settled, turn it into a question, a decision or a discrepancy.

## How

### CLI

```text
$ spec card new --title "Lighthouse lamp model" --discipline art --target MEC-LAMP --reviewer mira
created C-0012: art/backlog
$ spec card move C-0012 --to design
moved C-0012: art/design
$ spec card new --title "Lamp hum" --discipline sound --parent C-0012 --reviewer mira
created C-0013: sound/backlog
$ spec card move C-0013 --to sent-back --note "Needs the storm variant too"
moved C-0013: sound/sent-back
$ spec card accept C-0013 --as mira --briefed
```

`spec card split --target SCN-HARBOUR --title "Harbour scene" --child narrative --child art --child sound` makes a parent and three subtasks in one write; `--dry-run` shows the disciplines your `[[card.split]]` rows suggest. `spec card list --flow art` lists a flow by column. `spec card show C-0012 --context` adds the card's spec context, fitted to a budget, and its department's practices (`guide:practices#GD-PRACTICES-REACH`). `spec card edit C-0012 --task T-0042` links an agent task. `spec workspace` lists the departments. `spec comment add --on C-0012 --body-file note.md` writes a comment (`--on` also takes a node's ID or a `PR-` proposal), `spec comment list --on C-0012` reads the thread, `spec comment mentions --login mira` lists where you are named, and `spec propose question --comment CM-0004` raises a question from a comment; `spec comment delete CM-0004` asks before erasing it. `spec ref add --on C-0012 harbour.jpg --title "Harbour at dusk"` attaches a file (`--url` a link; `--matters` says what matters, required for sound, video, models and PDFs), `spec ref list --on C-0012` lists them, `spec ref get RF-0003 --out copy.jpg` fetches one, and `spec ref delete RF-0003` asks first.

### MCP

`list_cards`, `get_card {card_id, context: true}`, `create_card`, `split_card`, `edit_card` and `move_card`. A tool acts as `agent` locally and as your login on a hub. No tool accepts, assigns, cancels or clears a card: those are a person's acts. `list_comments` reads a thread and `add_comment` writes one as an agent (on a hub, under your login); `ask_question`, `report_discrepancy` and `report_decision` take a `comment_id`. No tool edits or deletes a comment. `list_references` and `get_reference` read references, an image arriving as an image; `add_reference` attaches a link. No tool uploads a file or deletes a reference.

### Screen

Each department's Board shows its flows' columns. The UI's words: a department is a workspace, a Task is a card, a Subtask a child card, an Agent task a `T-` task. A card opens as a modal or a page, with Hand off (`h`), Split, Move and, for its reviewer, Accept. A parent shows each child as handed off, taken, returned or accepted. Every task, document and proposal has a Comments part: write with a preview, mention someone as `@mira`, reply, edit your own, delete through a dialog, or Turn into… a question, a decision or a discrepancy, prefilled. Mentions, beside Activity in the navigation, lists the comments naming you, its badge counting the new ones. A References part, above Comments, shows images, plays sound and video, offers models and PDFs to download, and adds a file, with its progress, or a link.

## Where it lives

Flows and departments are tables in `specengine.toml`; a preset gives a starting set:

```toml
[[card.flow]]
name = "art"
title = "Art"
disciplines = ["art"]
columns = ["backlog", "sent-back", "concept", "modelling", "review", "in-game"]
accept = { from = "review", to = "in-game" }
return = "sent-back"
# moves = [{ from = "concept", to = "modelling", by = ["mira"] }]   # optional: only these moves, by these people

# ... the narrative, sound, level-design and development flows


[[card.split]]
kind = "scene"
disciplines = ["narrative", "level-design", "sound", "development"]

[[workspace]]
name = "art"
title = "Art and levels"
sections = ["board", "flow", "tree", "documents", "questions", "inbox"]
landing = "board"
flows = ["art", "level-design"]
logins = ["mira"]
```

Cards and comments are rows in the queue, outside the repository, never files or commits: `spec export state` backs them up with the proposals and tasks.

### A discipline picks the flow {#GD-CARDS-DISCIPLINE-PICKS-FLOW}

A new card starts in the first column of the first flow whose `disciplines` lists its discipline. Name each flow after a trade and give it the columns that trade really uses. `accept` names the last two columns: only an accept reaches the final one. With no `[[card.flow]]`, one plain flow is used: backlog, ready, in-progress, review, done.

### Ask another department with a hand-off {#GD-CARDS-HANDOFF}

To ask another trade for work, create a child card: `--parent` your card, `--discipline` theirs, `--reviewer` a person on your side. It lands in their flow; they take it by moving it out of the first column. A card never moves across flows.

### The asking side accepts {#GD-CARDS-REQUESTER-ACCEPTS}

The child's reviewer, from the asking department, accepts its result, never its assignee. Accept records a verdict: `--briefed` when the result matches the brief, or `--departs REF...` with what differs, which raises an item for the owner. `agent` and `local` are no one's name, so they can never review or accept.

### A return says why {#GD-CARDS-RETURN-WITH-NOTE}

A flow's `return` column takes a card only with a note, kept in its history. Only the child moves back; the asking card stays where it is. A flow with no `moves` allows every move between its other columns; listing `moves` allows only those, and `by` names who may make one.

### A comment is talk, never a decision {#GD-CARDS-COMMENTS-ARE-TALK}

A comment moves no card and opens no gate, and an agent reads it as people's words, data, never instructions. What must be settled becomes a question, a decision or a discrepancy through Turn into… (or `--comment`), and a person decides it as any item. Only the author edits a comment, each edit kept as a version; on a hub its author or an `owner` of `[team]` deletes it, on a terminal whoever is there after a question, and a delete erases the text of every version, leaving "deleted by".

### A reference is material, never truth {#GD-CARDS-REFERENCES-ARE-MATERIAL}

A photo, a sound, a model or a link on a task, a document or a proposal frames the work for its people and its agent, who reads it as data, never instructions. It moves no card, and it enters the spec only through a proposal a person confirms. An agent sees an image but cannot hear a sound or open a model, so say what matters in it. A file is kept once, by its content, on the hub's host alone: no backup carries it, so keep your originals. On a hub its author or an `owner` deletes it.

### A department is a lens, not a right {#GD-CARDS-DEPARTMENT-IS-A-LENS}

A department (`[[workspace]]`) chooses the sections, flows and module a person sees first. It hides nothing: every flow is one "Show all flows" away and every section keeps its route. Who may accept, confirm or clear on a hub is `[team]`'s (`guide:hub#GD-HUB-TEAM-ROLES`). On a hub, `logins` lands each person on their own department.

### Code work is an agent task the owner approves {#GD-CARDS-AGENT-TASKS}

A `T-` task carries code work for an agent: `spec task new --nodes MEC-LAMP --title "Lamp flicker"`, then the owner's `spec task approve T-0042` on a terminal, the one control point (or a pack the owner starts under `[auto]`: `guide:auto#GD-AUTO-OWNER-STARTS`). A card lists its tasks with `--task`; the link decides nothing, and completing a task leaves the card where it stands. A hub runs no `T-` tasks and shows their IDs only.

## What not to do

### Never make one card wait on another {#GD-CARDS-NOTHING-CHAINS}

A parent waits on no child and a child on no parent; each moves, is accepted and is cancelled on its own. A split creates cards, it never gates them. Track dependencies in the spec (`depends_on`) and in the plan, not as card locks.

### Never put a trade's name in code {#GD-CARDS-NAMES-ARE-DATA}

Flow, column, discipline and department names are your config's words. SpecEngine reads no column name, so renaming "sent-back" to "returned" is a config edit. Keep the config valid with `spec check`, which reports a bad `[card]` as `card-config`.
