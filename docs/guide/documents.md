---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/spec-library.md#the-rule, docs/canon/spec-library.md#decisions, docs/canon/spec-library.md#tags, docs/canon/spec-library.md#the-screen, docs/canon/spec-usage.md#the-rule, docs/canon/spec-guide.md#the-rule]
---

# The documents library

Documents lists every document of a project as one library, with facets to narrow it, the decisions behind each canon page, and SpecEngine's own guide on a shelf beside it. This page covers finding a document, tags, reading the decisions behind a page, orphans, and citing the guide.

## What it solves

Past a few dozen documents, nobody remembers paths, and a folder tree shows only one way of slicing them. A canon page can fall behind a decision accepted after it, and a document nobody links can sit unnoticed for months. The library answers "which documents are about X", "which canon lags its decisions" and "what is linked by nothing" from one read, computed fresh each time and stored nowhere.

## When to use it

- **Looking for a document** by class, kind, status, tag, discipline, module, owner, tier or scope, or by words in its title, name or path.
- **A review pass**: canon pages behind their decision, and orphans.
- **Asking how SpecEngine itself works**: the guide shelf, or `spec guide`.

## How

### CLI

```text
$ spec library
library | documents 42, archived 0 | decisions 12 | behind 1 | orphans 2
MEC-STAMINA | canon | mechanic | accepted | version 2 | tier 2 | owner design-lead | reviewed 2026-11-20 | tags movement | used by 4, mentioned by 6 | 2026-11-18T10:02:11Z | ann | decisions 1 accepted, 1 superseded, 0 rejected, 0 other | behind | Stamina
    decision DEC-0024 | accepted | 2026-11-20 | 2026-11-20T09:12:03Z | RULE-STAM-REGEN | newer
```

`--archive` adds Tier 3 documents; `--json` gives one document with every row. `spec show MEC-STAMINA --links` ends with who uses the node; `spec map` draws the whole corpus with its links; `spec catalog` tables one direction's documents.

The guide needs no project and writes nothing:

```text
$ spec guide
start | Adopting SpecEngine | SpecEngine keeps a project's specification ... | rules 6
...
$ spec guide practices
$ spec guide practices#GD-PRACTICES-REACH
```

### MCP

There is no library tool: `search`, `get_node` and `get_tree` find documents. `get_guide` with no arguments lists the guide's topics; `{topic}` returns a page and `{topic, rule}` one rule. The resource `guide://{topic}` reads a page. Both answer with or without a project.

### Screen

Documents (`#/<p>/documents`) has a search box, facets (Class, Kind, Status, Tags, Discipline, Module, Owner, Tier, Scope), the toggles Behind its decision, Orphans and Include archive, and columns for version, owner, last change, Used by, decisions and tags. A view switch, "Project documents | SpecEngine guide", opens the guide shelf: each topic with its summary and rule count, read-only, each rule with "Copy citation". The daemon serves them as `GET /api/projects/:p/library`, `GET /api/guide` and `GET /api/guide/:topic`.

## Where it lives

The library reads what the documents already say; it stores nothing. Tags are a front-matter list:

```markdown
---
id: MEC-STAMINA
class: canon
tags: [movement, balance, "night shift"]
---
```

A decision appears under the canon page its `canon:` points at. The guide is compiled into the `spec` you run; it is never a file in your project.

### Find by facet, not by path {#GD-DOCUMENTS-FIND-BY-FACET}

Narrow Documents by facets and search rather than remembering where a file lies. Facets combine (AND), values within one facet add up (OR), and each value shows how many documents it would keep. A department's module preselects its part of the corpus.

### Tags are front-matter {#GD-DOCUMENTS-TAGS}

`tags:` is a list of up to 32 tags, each 1 to 64 bytes, in any language. A tag changes like any front-matter, by a proposal or a commit; no screen writes one. A bad entry gets the warning `tag-invalid` and is left out. Use tags for cross-cutting topics no folder or kind captures.

### A page behind its decision needs its canon diff {#GD-DOCUMENTS-BEHIND-DECISION}

"Behind its decision" means an accepted decision pointing at the page was committed after the page last changed: canon may not say what was decided. Bring the page in line (`guide:features#GD-FEATURES-PROMOTION`), by an interpretation proposal or a commit that changes both together. It is a hint, never a check finding.

### An orphan is linked by nothing {#GD-DOCUMENTS-ORPHANS}

An orphan is a live document nothing links to, that links to nothing and has no child. Link it where it belongs, or retire it by a proposal that sets `status: abandoned`. An open card on it means "bring it in later".

### Cite the guide by topic and rule {#GD-DOCUMENTS-CITE-THE-GUIDE}

Write `guide:<topic>#<ID>`, such as `guide:proposals#GD-PROPOSALS-ONE-DOOR`, in a card, a document or a proposal. It links to the rule in the guide of the build that serves the project, and `spec check` gives it no finding.

## What not to do

### Never copy the guide into a project {#GD-DOCUMENTS-NO-GUIDE-COPY}

A copy freezes one version, enters your check and IDs, and would be a spec file written outside the queue. The guide that matches your build is always one `spec guide` away. Your own rules belong in your own canon and practices.

### Never archive by moving files {#GD-DOCUMENTS-ARCHIVE-BY-STATUS}

A document leaves default reads by its front-matter: `status: shipped`, `superseded-by`, `abandoned`. Moving it to another folder hides it from the next person who searches. Include archive shows retired documents when you need them.
