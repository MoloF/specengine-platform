import { frontMatter, link, specFile, type MockDocument, type MockLink } from "../build";

// The `aside` and `aside40` scenarios' corpus for harbor-sim (`docs/canon/spec-flow-screen.md` "Mock";
// RM-0094 flow-scenario): the invented pottery workshop of `fixtures/flow-aside`, with its IDs, titles,
// stages and links, in place of harbor-sim's own documents. What is in the flow is told by links,
// `[flow]` and `parent:` alone: the firing run's ten steps, the parts they use, the kiln's two cones
// and one hunch, and the gallery's ID-less "Open day"; the other parts, the desk papers, the eight
// ID-less notes and the 141 other records are not. `aside40` copies every record 40 times under the
// same prefixes (5 680 records), the rest as written. Only the mock computes membership (src/mocks/flow.ts).

/** The workshop's kinds: no game word, no pilot's word. */
export const ASIDE_KINDS: readonly string[] = ["atelier", "kilnrun", "potstep", "workbay", "conerule", "deskpaper", "hunch", "puzzle", "edict"];

/** The workshop's spec statuses: none is written. */
export const ASIDE_STATUSES: readonly string[] = [];

/** A flow link as the mock's flow reads it, `from` and `to` in reading direction. */
export interface AsideLink {
  from: string;
  to: string;
  type: string;
  kind?: string;
  when?: string;
}

/** The ten steps in walk order: ID word, title, stage. */
const STEPS = [
  ["WEDGE", "Wedging", "wheelwork"],
  ["THROW", "Throwing on the wheel", "wheelwork"],
  ["TRIM", "Trimming the foot", "wheelwork"],
  ["DRY", "Drying on the racks", "bisque"],
  ["BISQUE", "Bisque firing", "bisque"],
  ["GLAZE", "Dipping in glaze", "glazing"],
  ["FIRE", "Glost firing", "glazing"],
  ["SORT", "Sorting the kiln load", "crating"],
  ["MEND", "Mending a cracked pot", "crating"],
  ["PACK", "Packing the crates", "crating"],
] as const;

/** The fourteen parts of the pottery: ID word, title. */
const PARTS = [
  ["CLAY", "Clay cellar"],
  ["WHEEL", "Wheel room"],
  ["DRYING", "Drying loft"],
  ["KILN", "Kiln bay"],
  ["GLAZE", "Glaze kitchen"],
  ["SORTING", "Sorting bench"],
  ["PACKING", "Packing shed"],
  ["STORE", "Finished-ware store"],
  ["YARD", "Loading yard"],
  ["GALLERY", "Gallery bay"],
  ["OFFICE", "Front office"],
  ["WATER", "Water butts"],
  ["FUEL", "Fuel shed"],
  ["CANTEEN", "Staff canteen"],
] as const;

const NOTES = ["studio lighting", "the window sills", "visitors' boots", "the noticeboard", "the radio", "the swept floor", "the coat pegs", "the spare keys"] as const;

/** Each record kind: its prefix, its directory, how many the fixture holds, its title's start and subjects. */
const RECORDS = [
  { prefix: "EDICT", kind: "edict", directory: "edicts", count: 32, lead: "A rule on", subjects: ["sweeping the wheel room", "closing the clay bags", "labelling the glaze buckets", "logging each firing", "wearing aprons at the bench"] },
  { prefix: "HUNCH", kind: "hunch", directory: "hunches", count: 60, lead: "A hunch about", subjects: ["ash in the glaze", "wet weather and drying", "thick rims", "the morning firing", "warped plates"] },
  { prefix: "PUZZLE", kind: "puzzle", directory: "puzzles", count: 50, lead: "A puzzle of", subjects: ["pinholes on the rim", "a shelf that tilts", "a cone that bends late", "a crack at the foot", "a dull glaze"] },
] as const;

const ORDINALS = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "eleventh", "twelfth"] as const;

/** The one record a step uses: in the flow by its link alone, whatever its kind. */
const HUNCH_IN_FLOW = "HUNCH-0007";
const HUNCH_IN_FLOW_TITLE = "Glaze crawls on damp bisque";
/** The one record whose only link, `depends_on` a step, is no flow link. */
const PUZZLE_DEPENDING = "PUZZLE-0003";

const pad = (value: number) => String(value).padStart(4, "0");

const stepId = (word: string) => `POT-${word}`;
const partId = (word: string) => `WBAY-${word}`;

/** The run's links, as the steps' front matter writes them: eleven `follows`, fifteen `uses`. */
export const ASIDE_LINKS: readonly AsideLink[] = [
  { from: "POT-WEDGE", to: "POT-THROW", type: "follows" },
  { from: "POT-THROW", to: "POT-TRIM", type: "follows" },
  { from: "POT-TRIM", to: "POT-DRY", type: "follows" },
  { from: "POT-DRY", to: "POT-BISQUE", type: "follows" },
  { from: "POT-BISQUE", to: "POT-GLAZE", type: "follows" },
  { from: "POT-FIRE", to: "POT-GLAZE", type: "follows", kind: "repeat" },
  { from: "POT-GLAZE", to: "POT-FIRE", type: "follows" },
  { from: "POT-FIRE", to: "POT-SORT", type: "follows" },
  { from: "POT-SORT", to: "POT-MEND", type: "follows", kind: "alternative", when: "cracked" },
  { from: "POT-SORT", to: "POT-PACK", type: "follows" },
  { from: "POT-MEND", to: "POT-PACK", type: "follows" },
  { from: "POT-WEDGE", to: "WBAY-CLAY", type: "uses" },
  { from: "POT-THROW", to: "WBAY-WHEEL", type: "uses" },
  { from: "POT-TRIM", to: "WBAY-WHEEL", type: "uses" },
  { from: "POT-TRIM", to: "WBAY-DRYING", type: "uses" },
  { from: "POT-BISQUE", to: "WBAY-KILN", type: "uses" },
  { from: "POT-BISQUE", to: "CONE-BISQUE", type: "uses" },
  { from: "POT-GLAZE", to: "WBAY-GLAZE", type: "uses" },
  { from: "POT-FIRE", to: "WBAY-KILN", type: "uses" },
  { from: "POT-FIRE", to: "CONE-GLOST", type: "uses" },
  { from: "POT-FIRE", to: HUNCH_IN_FLOW, type: "uses" },
  { from: "POT-SORT", to: "WBAY-SORTING", type: "uses" },
  { from: "POT-MEND", to: "WBAY-SORTING", type: "uses" },
  { from: "POT-PACK", to: "WBAY-PACKING", type: "uses" },
  { from: "POT-PACK", to: "WBAY-STORE", type: "uses" },
  { from: "POT-PACK", to: "WBAY-YARD", type: "uses" },
];

/** The ID-less start and end: the gallery's open day, named by its path. */
export const OPEN_DAY = "docs/workshop/gallery/open-day.md";

/** `[[flow.start]]` and `[[flow.end]]` by paths, as names; `[[flow.step]]` takes the kind `potstep`. */
export const ASIDE_STARTS: readonly string[] = [stepId("WEDGE"), OPEN_DAY];
export const ASIDE_ENDS: readonly string[] = [stepId("PACK"), OPEN_DAY];
export const ASIDE_STEPS: readonly string[] = STEPS.map(([word]) => stepId(word));

/** Each step's `stage:`. */
export const ASIDE_STAGES: ReadonlyMap<string, string> = new Map(STEPS.map(([word, , stage]) => [stepId(word), stage]));

/** A document's front matter and body: `links` its written block, `stage` its own. */
function file(fields: { path: string; id: string | null; kind: string | null; title: string; parent: string | null; stage?: string; links?: string[]; body?: string[]; sections?: { id: string; title: string }[] }): MockDocument {
  const lines = [
    ...frontMatter({ id: fields.id, class: "canon", title: fields.title, parent: fields.parent, stage: fields.stage ?? null }).slice(0, -1),
    ...(fields.links ?? []),
    "---",
    "",
    `# ${fields.title}`,
    "",
    ...(fields.body ?? ["Kept for whoever works here next."]),
  ];
  return specFile({
    path: fields.path,
    id: fields.id,
    kind: fields.kind,
    title: fields.title,
    status: null,
    parent: fields.parent,
    lines,
    sections: (fields.sections ?? []).map((section) => ({ id: section.id, kind: fields.kind, title: section.title })),
  });
}

/** The `follows` a step writes: the steps it follows, as its front matter names them. */
function followsBlock(id: string): string[] {
  const into = ASIDE_LINKS.filter((written) => written.type === "follows" && written.to === id);
  const uses = ASIDE_LINKS.filter((written) => written.type === "uses" && written.from === id);
  const lines: string[] = [];
  if (into.length > 0 || uses.length > 0) {
    lines.push("links:");
  }
  if (into.length > 0) {
    lines.push("  follows:");
    for (const written of into) {
      lines.push(written.kind === undefined ? `    - ${written.from}` : `    - {to: ${written.from}, kind: ${written.kind}${written.when === undefined ? "" : `, when: ${written.when}`}}`);
    }
  }
  if (uses.length > 0) {
    lines.push(`  uses: [${uses.map((written) => written.to).join(", ")}]`);
  }
  return lines;
}

/** A record's title: its kind's lead, a subject and an ordinal, never its ID. */
function recordTitle(record: (typeof RECORDS)[number], index: number): string {
  const subject = record.subjects[index % record.subjects.length] ?? "the kiln";
  const round = Math.floor(index / record.subjects.length);
  const ordinal = ORDINALS[round] ?? `copy ${String(round + 1)}`;
  return `${record.lead} ${subject}, the ${ordinal}`;
}

/** The records under the kiln bay, `times` copies of each under the same prefixes. */
function records(times: number): MockDocument[] {
  const files: MockDocument[] = [];
  for (const record of RECORDS) {
    for (let index = 0; index < record.count * times; index += 1) {
      const id = `${record.prefix}-${pad(index + 1)}`;
      const title = id === HUNCH_IN_FLOW ? HUNCH_IN_FLOW_TITLE : recordTitle(record, index);
      files.push(file({ path: `docs/${record.directory}/${id}.md`, id, kind: record.kind, title, parent: partId("KILN"), links: id === PUZZLE_DEPENDING ? ["links:", `  depends_on: [${stepId("BISQUE")}]`] : [] }));
    }
  }
  return files;
}

/** The pottery's 181 documents; `times` 40 gives `aside40`'s 5 719. */
export function asideDocuments(times = 1): MockDocument[] {
  return [
    ...records(times),
    ...NOTES.map((subject, index) => file({ path: `docs/notes/note-${String(index + 1)}.md`, id: null, kind: null, title: `A note on ${subject}`, parent: "ATL-POTTERY", body: ["Written down so it is not forgotten."] })),
    file({ path: "docs/workshop/atelier.md", id: "ATL-POTTERY", kind: "atelier", title: "The pottery", parent: null, body: ["A small pottery: a firing run, its bays and what is kept about them."] }),
    ...PARTS.map(([word, title]) => file({ path: `docs/workshop/bays/${word.toLowerCase()}.md`, id: partId(word), kind: "workbay", title, parent: "ATL-POTTERY" })),
    file({ path: "docs/workshop/cones/bisque.md", id: "CONE-BISQUE", kind: "conerule", title: "Bisque cone", parent: partId("KILN"), body: ["The bisque firing stops when the low cone bends."] }),
    file({
      path: "docs/workshop/cones/glost.md",
      id: "CONE-GLOST",
      kind: "conerule",
      title: "Glost cone",
      parent: partId("KILN"),
      body: ["The glost firing stops when the high cone bends to the shelf.", "", "## Shelf spacing {#CONE-GLOST-SHELF}", "", "Glazed pots stand a finger apart so no two fuse."],
      sections: [{ id: "CONE-GLOST-SHELF", title: "Shelf spacing" }],
    }),
    file({ path: OPEN_DAY, id: null, kind: null, title: "Open day", parent: partId("GALLERY"), body: ["The gallery opens its doors: the day begins and ends here."] }),
    file({ path: "docs/workshop/office/ledger.md", id: "DESK-LEDGER", kind: "deskpaper", title: "Order ledger", parent: partId("OFFICE") }),
    file({ path: "docs/workshop/office/rota.md", id: "DESK-ROTA", kind: "deskpaper", title: "Weekly rota", parent: partId("OFFICE") }),
    file({ path: "docs/workshop/run/firing.md", id: "KRUN-FIRING", kind: "kilnrun", title: "Firing run", parent: "ATL-POTTERY", body: ["From the clay cellar to the loading yard, one batch at a time."] }),
    ...STEPS.map(([word, title, stage]) =>
      file({ path: `docs/workshop/run/${word.toLowerCase()}.md`, id: stepId(word), kind: "potstep", title, parent: "KRUN-FIRING", stage, links: followsBlock(stepId(word)) }),
    ),
  ].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}

/** The links the documents write, each resolved, where it is written: the flow's and the one `depends_on`. */
export function asideCorpusLinks(documents: readonly MockDocument[]): MockLink[] {
  const pathOf = new Map(documents.map((written) => [written.node.id ?? written.path, written.path]));
  const lineOf = (path: string, key: string) => {
    const lines = documents.find((written) => written.path === path)?.lines ?? [];
    return Math.max(1, lines.findIndex((line) => line.includes(key)) + 1);
  };
  const flow = ASIDE_LINKS.map((written) => {
    // `follows` is written on the later step, naming the earlier; `uses` on the user.
    const holder = written.type === "follows" ? written.to : written.from;
    const named = written.type === "follows" ? written.from : written.to;
    const path = pathOf.get(holder) ?? "";
    return link({ type: written.type, origin: "frontmatter", written: named, path, line: lineOf(path, named), to: named });
  });
  const puzzle = pathOf.get(PUZZLE_DEPENDING);
  const depends = puzzle === undefined ? [] : [link({ type: "depends_on", origin: "frontmatter", written: stepId("BISQUE"), path: puzzle, line: lineOf(puzzle, "depends_on"), to: stepId("BISQUE") })];
  return [...flow, ...depends];
}
