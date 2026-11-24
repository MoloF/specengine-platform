---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: front-matter of the repository's documents
---

# Documentation index: `docs/decisions/*.md`

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

A shard of [docs/index.md](index.md), the index's one entry point.

## Decisions

- [ADR-0040](decisions/ADR-0040.md) A node's revision is its own; none written is 1 · storage · accepted
- [ADR-0041](decisions/ADR-0041.md) spec.lock is written only on the owner's terminal or by a passing test run · architecture · accepted
- [ADR-0042](decisions/ADR-0042.md) spec verify blocks only by a project's choice and never passes what it could not verify · architecture · accepted
- [ADR-0043](decisions/ADR-0043.md) Approval raises a changed node's revision; an editorial one refreshes the lock in its commit · architecture · accepted
- [ADR-0044](decisions/ADR-0044.md) Bound tests run by the project's own command, only on the owner's terminal · architecture · accepted
- [ADR-0045](decisions/ADR-0045.md) Drift reaches the owner as a queue item; approving it runs the lock accept · architecture · accepted
- [ADR-0046](decisions/ADR-0046.md) Reads show each binding's sync and never fail on the code layer · architecture · accepted
- [ADR-0047](decisions/ADR-0047.md) Health lists code assuming a decision, unbound nodes and drift, read-only · architecture · accepted
- [ADR-0048](decisions/ADR-0048.md) Layer B facts are files the project writes; they annotate symbols, never bind · code · accepted
- [ADR-0049](decisions/ADR-0049.md) Layer C runs as its own executable, on request, and only advises · code · accepted
- [ADR-0050](decisions/ADR-0050.md) spec lock rebase re-hashes the lock from the bytes the owner accepted · architecture · accepted
- [ADR-0051](decisions/ADR-0051.md) Health's code rows read the code index the call refreshes, as spec refs does · architecture · accepted
- [ADR-0052](decisions/ADR-0052.md) Plan items are files in git; tasks execute them; position is computed · architecture · accepted
- [ADR-0053](decisions/ADR-0053.md) Reads show what changed since a lock entry · architecture · accepted
- [ADR-0054](decisions/ADR-0054.md) The MCP form stages only the owner's answer · architecture, crates/specengine-mcp · accepted
- [ADR-0055](decisions/ADR-0055.md) Flow is three shared link types; cycles live in follows, never in depends_on · architecture · accepted
- [ADR-0056](decisions/ADR-0056.md) A card is a separate lighter queue record; T- stays the code-work task · architecture · accepted
- [ADR-0057](decisions/ADR-0057.md) The local page writes cards under a typed name, a convention until the hub · architecture · accepted
- [ADR-0058](decisions/ADR-0058.md) A hub mode of specengine-http serves named people; the local daemon stays as it is · architecture · accepted
- [ADR-0059](decisions/ADR-0059.md) A hub trusts one proxy-set login header and holds no account · architecture · accepted
- [ADR-0060](decisions/ADR-0060.md) A hub's [team] entitles a login to accept, clear and stage · architecture · accepted
- [ADR-0061](decisions/ADR-0061.md) On a hub a login [team] entitles confirms one staged choice · architecture · accepted
- [ADR-0062](decisions/ADR-0062.md) An apply commit carries the index's outputs, rendered from the committed tree · architecture · accepted
- [ADR-0063](decisions/ADR-0063.md) A hub's queue mints proposal IDs from a stored floor, PR-100001 upward · architecture · accepted
- [ADR-0064](decisions/ADR-0064.md) A hub with --push fetches, merges and pushes its branch, never rebasing; credentials are git's · architecture · accepted
- [ADR-0065](decisions/ADR-0065.md) A hub serves MCP at /mcp/<slug> under each request's login; its protocol sessions identify no one · architecture · accepted
- [ADR-0066](decisions/ADR-0066.md) A hub ships as a container image built on a version tag; local use stays cargo install and the plugin · architecture · accepted
- [ADR-0067](decisions/ADR-0067.md) A hub's compose file is its whole host, and only the proxy sets Remote-User: the door's session, or on /mcp a per-person secret · architecture · accepted
- [ADR-0068](decisions/ADR-0068.md) A hub's accounts are the deployment's script, run from the host's shell in a one-shot container with no network · architecture · accepted
- [ADR-0069](decisions/ADR-0069.md) A read trusts a file's stat key outside a 2 s racy window, and a hub's reads alone trust the index its writers keep · architecture · accepted
- [ADR-0070](decisions/ADR-0070.md) A hub mints spec IDs from 100001 and a terminal below it; propose refuses the other side, apply never · architecture · accepted
- [ADR-0071](decisions/ADR-0071.md) A hub's state leaves the host every 15 minutes, restic-encrypted, by the deployment's sidecars; no key and no SpecEngine code · scripts/deploy · accepted
- [ADR-0072](decisions/ADR-0072.md) The UI's chrome is English or Russian, each viewer's choice, from one catalog, the only home of Russian text · ui, docs · accepted
- [ADR-0073](decisions/ADR-0073.md) A workspace is a trade's lens from project config, never an entitlement, a control point or a hidden route · architecture · accepted
- [ADR-0074](decisions/ADR-0074.md) A hand-off is a child card the asking side reviews; a return is a move with a reason; no card's write touches another · architecture · accepted
- [ADR-0075](decisions/ADR-0075.md) A card links T- tasks on its own row; the link and a task's state decide nothing; local only · architecture · accepted
- [ADR-0076](decisions/ADR-0076.md) A department is a workspace in the owner's words; a module may be a kinds direction; a hand-off names a department · architecture · accepted
- [ADR-0077](decisions/ADR-0077.md) A question, discrepancy or decision may name a card, raised from it on the page; the link decides nothing · architecture · accepted
- [ADR-0078](decisions/ADR-0078.md) A split makes a task and a subtask per discipline in one act; `[[card.split]]` only pre-selects · architecture · accepted
- [ADR-0079](decisions/ADR-0079.md) A subtask is binding or its department decides, named by the asking side; nothing reads the mandate · architecture · accepted
- [ADR-0080](decisions/ADR-0080.md) The corpus map lays out with d3-force and draws on a 2D canvas, in a lazy chunk · ui · accepted
- [ADR-0081](decisions/ADR-0081.md) A committed [auto] rule, started per pack on a terminal, approves the pack's tasks and lands each finished one · architecture · accepted
- [ADR-0082](decisions/ADR-0082.md) A hub's code applies a covered update or create its sole owner login raised over /mcp · architecture · accepted
- [ADR-0083](decisions/ADR-0083.md) A write's findings take the files as the call's walk indexed them · architecture · accepted
- [ADR-0084](decisions/ADR-0084.md) A document naming disciplines is a practice, standing context for their cards and tasks · architecture · accepted
- [ADR-0085](decisions/ADR-0085.md) SpecEngine's guide ships in the build and is served read-only beside a project, never inside it · architecture · accepted
- [ADR-0086](decisions/ADR-0086.md) A crash, a hang or a leak is a critical defect, judged by soaks against written budgets · architecture · accepted
- [ADR-0087](decisions/ADR-0087.md) A comment is queue data under a card, a node or a proposal, never truth · architecture · accepted
- [ADR-0088](decisions/ADR-0088.md) A work reference is queue data; its file stays in the data directory, never in git, a dump or off the host · architecture · accepted
- [ADR-0089](decisions/ADR-0089.md) A reference's file enters git only through a confirmed `reference` proposal, beside its document, LFS as the project says · architecture · accepted
