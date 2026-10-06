import { frontMatter, link, specFile, type MockDocument, type MockLink } from "../build";

// The `large` scenario's addition to harbor-sim: 12 domains, 6 mechanics each, 6 sub-mechanics
// each, every sub-mechanic with 2 rules nested three deep. 12 + 72 + 432 + 2 592 = 3 108 nodes,
// depth 0 to 5, in harbor-sim's kinds; deterministic, and uncut as the browser reads it.
// Front-matter `depends_on` (docs/features/ui-graph.md "Mock"): each mechanic on its domain, each
// sub-mechanic on its mechanic, DOM-GEN-02...12 on DOM-GEN-01: Impact from DOM-GEN-01 reaches 17,
// 102 and 396 nodes at distance 1 to 3 (516 with the REF).

const pad = (value: number) => String(value).padStart(2, "0");

/** The front-matter `depends_on` of each generated document, by its ID. */
function dependsOn(id: string): string | null {
  const domain = /^DOM-GEN-(\d\d)$/.exec(id);
  if (domain !== null) {
    return domain[1] === "01" ? null : "DOM-GEN-01";
  }
  const sub = /^(MEC-GEN-\d\d-\d\d)-\d\d$/.exec(id);
  if (sub !== null) {
    return sub[1] ?? null;
  }
  const mechanic = /^MEC-GEN-(\d\d)-\d\d$/.exec(id);
  return mechanic === null ? null : `DOM-GEN-${mechanic[1] ?? ""}`;
}

const RULE_WORDS = ["queue", "draft", "crew", "tide", "berth", "pilot", "cargo", "lock"] as const;

function word(seed: number): string {
  return RULE_WORDS[seed % RULE_WORDS.length] ?? "harbour";
}

export function largeDocuments(): MockDocument[] {
  const files: MockDocument[] = [];
  for (let d = 1; d <= 12; d += 1) {
    const domain = `DOM-GEN-${pad(d)}`;
    files.push(
      specFile({
        path: `docs/spec/gen/d${pad(d)}/README.md`,
        id: domain,
        kind: "domain",
        title: `Generated domain ${String(d)}`,
        status: "accepted",
        lines: [
          ...frontMatter({ id: domain, kind: "domain", status: "accepted", depends_on: dependsOn(domain) }),
          "",
          `# Generated domain ${String(d)}`,
          "",
          `Domain ${String(d)} of the large scenario groups six mechanics.`,
        ],
      }),
    );
    for (let m = 1; m <= 6; m += 1) {
      const mechanic = `MEC-GEN-${pad(d)}-${pad(m)}`;
      files.push(
        specFile({
          path: `docs/spec/gen/d${pad(d)}/m${pad(m)}.md`,
          id: mechanic,
          kind: "mechanic",
          title: `Generated mechanic ${String(d)}.${String(m)}`,
          status: "proposed",
          parent: domain,
          lines: [
            ...frontMatter({ id: mechanic, kind: "mechanic", status: "proposed", parent: domain, depends_on: domain }),
            "",
            `# Generated mechanic ${String(d)}.${String(m)}`,
            "",
            `Mechanic ${String(m)} of domain ${String(d)}: the ${word(d + m)} part.`,
          ],
        }),
      );
      for (let k = 1; k <= 6; k += 1) {
        const sub = `${mechanic}-${pad(k)}`;
        const lines = [
          ...frontMatter({ id: sub, kind: "mechanic", status: "draft", parent: mechanic, depends_on: mechanic }),
          "",
          `# Generated step ${String(d)}.${String(m)}.${String(k)}`,
          "",
          `Step ${String(k)} handles the ${word(d * m + k)} of mechanic ${String(m)}.`,
        ];
        const sections = [];
        for (let r = 1; r <= 2; r += 1) {
          const rule = `RULE-GEN-${pad(d)}-${pad(m)}-${pad(k)}-${String(r)}`;
          lines.push("", `## ${rule}: Rule ${String(r)} of step ${String(k)}`, "", `The ${word(r + k)} waits for the ${word(d + r)}.`);
          lines.push("", `### ${rule}-A: Case A`, "", `Case A applies when the ${word(m + r)} is free.`);
          lines.push("", `#### ${rule}-A-X: Exception`, "", `Except at night, when the ${word(k + r + m)} is closed.`);
          sections.push(
            { id: rule, kind: "rule", title: `Rule ${String(r)} of step ${String(k)}` },
            { id: `${rule}-A`, kind: "rule", title: "Case A" },
            { id: `${rule}-A-X`, kind: "rule", title: "Exception" },
          );
        }
        files.push(
          specFile({
            path: `docs/spec/gen/d${pad(d)}/m${pad(m)}/s${pad(k)}.md`,
            id: sub,
            kind: "mechanic",
            title: `Generated step ${String(d)}.${String(m)}.${String(k)}`,
            status: "draft",
            parent: mechanic,
            lines,
            sections,
          }),
        );
      }
    }
  }
  return files;
}

/** The `depends_on` links the generated documents' front matter writes, each resolved. */
export function largeLinks(documents: readonly MockDocument[]): MockLink[] {
  return documents.flatMap((file) => {
    const target = file.node.id === null ? null : dependsOn(file.node.id);
    const line = file.lines.findIndex((text) => text.startsWith("depends_on:")) + 1;
    if (target === null || line === 0) {
      return [];
    }
    return [link({ type: "depends_on", origin: "frontmatter", written: target, path: file.path, line, to: target })];
  });
}
