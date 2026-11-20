import type { GuideIndex, GuidePage, GuideRule } from "../api/types";

// SpecEngine's own guide as the mock serves it (docs/features/specengine-guide.md "Data"; ADR-0085): the
// ten topics in the guide's order, each page shortened to its five parts and a few of its real `GD-`
// rules (IDs and headings as `docs/guide/` writes them, so a citation copied here resolves), every
// project alike, since the guide is the build's, not a project's. The daemon serves the pages its build
// embedded; these only stand in for them. Each page's text starts with a blank line, as the CLI's
// `text` does, and a rule's `line` counts the 8 lines of front-matter a page carries.

/** The front-matter's lines above a page's text: `---`, six keys, `---`. */
const FRONT_MATTER_LINES = 8;

interface MockTopic {
  topic: string;
  title: string;
  /** The opening paragraph's first sentence: the index's summary. */
  summary: string;
  /** The rest of the opening paragraph. */
  more: string;
  solves: string;
  when: string;
  how: string;
  lives: string;
  /** Each rule: its ID, its heading's words, its text. */
  rules: readonly (readonly [string, string, string])[];
  summarises: readonly string[];
}

const TOPICS: readonly MockTopic[] = [
  {
    topic: "start",
    title: "Adopting SpecEngine",
    summary: "SpecEngine keeps a project's specification in the project's own git repository, next to the code, and gives people and agents one fixed way to find it, change it and cite it.",
    more: "This page covers `spec init`, the paths and IDs a project declares, and the reading protocol.",
    solves: "Without one place for the spec, every agent session starts by guessing where things are.",
    when: "Once, when a project starts using SpecEngine, and whenever a newcomer asks where something goes.",
    how: "`spec init --pack gamedev` writes one `specengine.toml`; `spec check` then judges the tree. Records and canon are covered in guide:records.",
    lives: "`specengine.toml` at the root; the spec under `[paths]`.",
    rules: [
      ["GD-START-ONE-CONFIG", "One config holds everything project-specific", "Everything a project declares lives in `specengine.toml`; nothing else configures SpecEngine."],
      ["GD-START-READING-PROTOCOL", "Read by the protocol", "An agent reads the root index, then at most three documents."],
    ],
    summarises: ["docs/canon/presets.md#the-rule"],
  },
  {
    topic: "records",
    title: "Records, the spec tree and canon",
    summary: "Every document in a project belongs to one of four classes, and every statement worth citing has one Latin ID that is defined in exactly one place.",
    more: "Canon says how things work now; decisions say why.",
    solves: "Two copies of one rule drift apart; one ID in one place cannot.",
    when: "Whenever a statement will be cited by a card, a task or code.",
    how: "Give the statement a `{#ID}` heading; `spec show ID` prints it.",
    lives: "Under `[paths] spec`, each document with its front-matter.",
    rules: [
      ["GD-RECORDS-ONE-FILE", "One record, one file", "A second definition is the error `id-duplicate`."],
      ["GD-RECORDS-LATIN-IDS", "Never mix scripts in an ID", "A look-alike letter from another script is refused with its Latin fix."],
    ],
    summarises: ["docs/canon/spec-check.md#the-rule"],
  },
  {
    topic: "features",
    title: "Features, criteria and decisions",
    summary: "A feature spec says what one change will do and how to tell it is done, and a decision record says why a choice was made, while canon keeps saying how things work now.",
    more: "A shipped feature leaves its criteria and its canon diff.",
    solves: "Without criteria nobody can say when a change is done.",
    when: "Before any change larger than a typo.",
    how: "Write the spec under `[paths] features`, its criteria as `{#ID}` sections.",
    lives: "Feature specs under `docs/features/`, decisions under `docs/decisions/`.",
    rules: [["GD-FEATURES-PROMOTION", "An accepted decision changes canon in the same change", "Canon never lags a decision it follows."]],
    summarises: ["docs/canon/proposal-decision.md#the-rule"],
  },
  {
    topic: "proposals",
    title: "Proposals and the one writing door",
    summary: "Every change an agent or a tool makes to a spec file goes through one door: someone proposes it, a person confirms it, and SpecEngine applies it as one commit that records who proposed and who decided.",
    more: "Nothing else writes a spec file.",
    solves: "A spec edited by whoever runs last cannot be trusted.",
    when: "Each time an agent wants a spec file changed.",
    how: "`propose_change` queues it; the owner confirms on a terminal or, on a hub, with one click.",
    lives: "The queue, outside the repository; the commit, in git.",
    rules: [
      ["GD-PROPOSALS-ONE-DOOR", "Never write a spec file by hand from an agent", "An agent proposes; only `apply_proposal` writes, once a person confirms."],
      ["GD-PROPOSALS-NOTHING-BLOCKS", "Nothing waits on a decision", "An open proposal is information on a card, never a lock on its node."],
    ],
    summarises: ["docs/canon/proposal-apply.md#the-rule", "docs/canon/hub-confirm.md#the-rule"],
  },
  {
    topic: "cards",
    title: "Cards, departments and flows",
    summary: "A card is one piece of any department's work, tracked in the project's queue beside the spec it targets, and it moves through its department's own columns.",
    more: "A discipline picks its flow.",
    solves: "Work tracked away from the spec loses which rule it serves.",
    when: "For every piece of work a person or an agent takes.",
    how: "`spec card new` makes one; the Board moves it.",
    lives: "The queue; flows in `[[card.flow]]`.",
    rules: [
      ["GD-CARDS-HANDOFF", "Ask another department with a hand-off", "The asking side writes the hand-off and accepts what comes back."],
      ["GD-CARDS-NOTHING-CHAINS", "Never make one card wait on another", "Cards inform each other; none gates another."],
    ],
    summarises: ["docs/canon/cards.md#the-rule"],
  },
  {
    topic: "practices",
    title: "Department practices",
    summary: "A practice is a department's agreed rules, kept as one document that every card and agent task of that department carries.",
    more: "Its rules are `{#ID}` sections.",
    solves: "Rules kept in people's heads never reach an agent.",
    when: "When a department agrees how its work is done.",
    how: "Name the disciplines in the document's `disciplines:`; a card's context carries it.",
    lives: "Any document naming `disciplines:`.",
    rules: [["GD-PRACTICES-CITE-RULES", "Cite the rule, not its text", "A citation stays true when the rule's words change."]],
    summarises: ["docs/canon/card-practices.md#the-rule"],
  },
  {
    topic: "auto",
    title: "The `[auto]` mode",
    summary: "Auto mode is a committed rule in `specengine.toml` that lets SpecEngine's own code approve and land the tasks of a pack the owner started, so agents can work overnight.",
    more: "Without the table, the project is manual.",
    solves: "An owner asleep cannot click Confirm.",
    when: "For a solo owner's overnight runs.",
    how: "Commit an `[auto]` table, then start a pack on a terminal.",
    lives: "`specengine.toml`, `[auto]`.",
    rules: [["GD-AUTO-OWNER-STARTS", "Only the owner starts a pack", "No rule starts work by itself."]],
    summarises: ["docs/canon/auto-mode.md#the-rule"],
  },
  {
    topic: "markers",
    title: "Code markers and the lock",
    summary: "A marker is a one-line comment binding code to the spec node it implements, and `spec.lock` records the pair's hashes as the owner verified them.",
    more: "Drift is a change on either side since.",
    solves: "Code and spec part ways silently without a binding.",
    when: "On every function that implements a rule.",
    how: "Write `// @implements ID@rev`; `spec verify` judges the pair.",
    lives: "The code's comments; `spec.lock` at the root.",
    rules: [["GD-MARKERS-BIND-NEW-WORK", "Bind new work with a marker", "A marker names the rule and the `rev` it was written against."]],
    summarises: ["docs/canon/spec-lock.md#the-rule"],
  },
  {
    topic: "hub",
    title: "The hub",
    summary: "A hub is one shared SpecEngine server behind a login door, where the whole team works on the spec in a browser and each person's agent works over MCP under that person's login.",
    more: "Each confirm is one click by a login `[team]` entitles.",
    solves: "A team on laptops cannot share one queue.",
    when: "When more than one person works on the project.",
    how: "Deploy the image; `[team]` names who may confirm.",
    lives: "A server; the project's checkout on it.",
    rules: [["GD-HUB-UPGRADE-WITH-DUMP", "Dump before every upgrade", "An upgrade starts from a dump you can restore."]],
    summarises: ["docs/canon/hub-ops.md#upgrade"],
  },
  {
    topic: "documents",
    title: "The documents library",
    summary: "Documents lists every document of a project as one library, with facets to narrow it, the decisions behind each canon page, and SpecEngine's own guide on a shelf beside it.",
    more: "The guide is read only, and the build's.",
    solves: "Past a few dozen documents, nobody remembers paths.",
    when: "Looking for a document, or a review pass.",
    how: "The Documents section, or `spec library`; the guide with `spec guide`.",
    lives: "Computed per read, stored nowhere.",
    rules: [
      ["GD-DOCUMENTS-CITE-THE-GUIDE", "Cite the guide by topic and rule", "Write the citation as `guide:<topic>#<ID>`: a project's check gives it no finding, and the screen links it."],
      ["GD-DOCUMENTS-NO-GUIDE-COPY", "Never copy the guide into a project", "A copy freezes an old version and enters the project's check."],
    ],
    summarises: ["docs/canon/spec-library.md#the-rule", "docs/canon/spec-guide.md#the-rule"],
  },
];

/** A topic's page text: after its front-matter, a blank line first. */
function textOf(entry: MockTopic): string {
  const rules = entry.rules.map(([id, title, body]) => `### ${title} {#${id}}\n\n${body}\n`).join("\n");
  return [
    "",
    `# ${entry.title}`,
    "",
    `${entry.summary} ${entry.more}`,
    "",
    "## What it solves",
    "",
    entry.solves,
    "",
    "## When to use it",
    "",
    entry.when,
    "",
    "## How",
    "",
    "### CLI",
    "",
    entry.how,
    "",
    "## Where it lives",
    "",
    entry.lives,
    "",
    "## What not to do",
    "",
    rules,
  ].join("\n");
}

/** A page's rules: each heading's line in the page's file, its front-matter counted. */
function rulesOf(entry: MockTopic, text: string): GuideRule[] {
  const lines = text.split("\n");
  return entry.rules.map(([id, title]) => ({ id, title, line: FRONT_MATTER_LINES + lines.findIndex((line) => line.endsWith(`{#${id}}`)) + 1 }));
}

/** The guide's index, `spec guide --json`. */
export function guideIndexOf(): GuideIndex {
  return { topics: TOPICS.map((entry) => ({ topic: entry.topic, title: entry.title, summary: entry.summary, rules: entry.rules.length })) };
}

/** A topic's page, `spec guide TOPIC --json`; null for a topic the guide lacks. */
export function guidePageOf(topic: string): GuidePage | null {
  const entry = TOPICS.find((candidate) => candidate.topic === topic);
  if (entry === undefined) {
    return null;
  }
  const text = textOf(entry);
  return { topic, title: entry.title, summary: entry.summary, summarises: [...entry.summarises], reviewed: "2026-11-20", rules: rulesOf(entry, text), text };
}

/** The CLI's line for a topic the guide lacks, the 404's message. */
export function noGuideTopic(topic: string): string {
  return `spec: no guide topic \`${topic}\`; topics: ${TOPICS.map((entry) => entry.topic).join(", ")}`;
}
