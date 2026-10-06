import { frontMatter, link, proposal, specFile, stamp, type MockCorpus, type MockProject } from "../build";
import { nodeKinds, specStatuses } from "./kinds";

// ledger-api: an invented payments service, with a node vocabulary disjoint from harbor-sim's:
// a smaller tree three levels deep, nested sections, an archived endpoint, a few links.

const SLUG = "ledger-api";
const WINDOW_FILE = "docs/spec/refunds/window.md";
const ENDPOINT_FILE = "docs/spec/api/refunds.md";

const ENDPOINT_TEXT = [
  "## EP-REFUND-CREATE: POST /v1/refunds",
  "",
  "Creates a refund for a captured payment.",
  "",
  "- Body: payment_id, amount (minor units), reason.",
  "- Answers 201 with the refund, 422 when the amount exceeds the captured rest.",
].join("\n");

const ENDPOINT_DIFF = [
  `--- base ${ENDPOINT_FILE}`,
  `+++ proposed ${ENDPOINT_FILE}`,
  "@@ -3,4 +3,6 @@",
  " Creates a refund for a captured payment.",
  " ",
  " - Body: payment_id, amount (minor units), reason.",
  "-- Answers 201 with the refund, 422 when the amount exceeds the captured rest.",
  "+- Header: Idempotency-Key, required (POL-IDEMPOTENCY).",
  "+- Answers 201 with the refund, 200 with the same refund on a repeated key,",
  "+  422 when the amount exceeds the captured rest.",
].join("\n");

function ledgerCorpus(): MockCorpus {
  const documents = [
    specFile({
      path: "docs/spec/README.md",
      id: "SVC-LEDGER",
      kind: "service",
      title: "Ledger API",
      status: "accepted",
      summary: "The payments ledger and its public API.",
      lines: [
        ...frontMatter({ id: "SVC-LEDGER", kind: "service", status: "accepted", summary: "The payments ledger and its public API." }),
        "",
        "# Ledger API",
        "",
        "Every money movement is one balanced ledger entry; the API never edits an entry, it adds one.",
      ],
    }),
    specFile({
      path: "docs/spec/api/idempotency.md",
      id: "POL-WRITES",
      kind: "policy",
      title: "Write rules",
      status: "accepted",
      parent: "SVC-LEDGER",
      lines: [
        ...frontMatter({ id: "POL-WRITES", kind: "policy", status: "accepted", parent: "SVC-LEDGER" }),
        "",
        "# Write rules",
        "",
        "## POL-IDEMPOTENCY: Idempotency keys",
        "",
        "Every write endpoint takes an Idempotency-Key header and answers a repeated key",
        "with the first result.",
      ],
      sections: [{ id: "POL-IDEMPOTENCY", kind: "policy", title: "Idempotency keys" }],
    }),
    specFile({
      path: "docs/spec/refunds/README.md",
      id: "SVC-REFUNDS",
      kind: "service",
      title: "Refund service",
      status: "accepted",
      rev: 5,
      parent: "SVC-LEDGER",
      lines: [
        ...frontMatter({ id: "SVC-REFUNDS", kind: "service", status: "accepted", parent: "SVC-LEDGER", rev: 5 }),
        "",
        "# Refund service",
        "",
        "Owns refunds of captured card payments; writes one ledger entry per refund.",
      ],
    }),
    specFile({
      path: ENDPOINT_FILE,
      id: "EP-REFUNDS",
      kind: "endpoint",
      title: "Refund endpoints",
      status: "accepted",
      rev: 2,
      parent: "SVC-REFUNDS",
      lines: [
        ...frontMatter({ id: "EP-REFUNDS", kind: "endpoint", status: "accepted", parent: "SVC-REFUNDS", depends_on: "POL-WRITES" }),
        "",
        "# Refund endpoints",
        "",
        ...ENDPOINT_TEXT.split("\n"),
        "See POL-IDEMPOTENCY for repeated requests.",
      ],
      sections: [{ id: "EP-REFUND-CREATE", kind: "endpoint", title: "POST /v1/refunds" }],
    }),
    specFile({
      path: "docs/spec/api/legacy-v0.md",
      id: "EP-REFUND-V0",
      kind: "endpoint",
      title: "POST /v0/refund",
      status: "deprecated",
      parent: "EP-REFUNDS",
      archived: true,
      lines: [
        ...frontMatter({ id: "EP-REFUND-V0", kind: "endpoint", status: "deprecated", parent: "EP-REFUNDS" }),
        "",
        "# POST /v0/refund",
        "",
        "The first refund endpoint; replaced by EP-REFUND-CREATE.",
      ],
    }),
    specFile({
      path: WINDOW_FILE,
      id: "POL-REFUND-RULES",
      kind: "policy",
      title: "Refund rules",
      status: "draft",
      parent: "SVC-REFUNDS",
      lines: [
        ...frontMatter({ id: "POL-REFUND-RULES", kind: "policy", status: "draft", parent: "SVC-REFUNDS", constrains: "EP-REFUNDS" }),
        "",
        "# Refund rules",
        "",
        "## POL-REFUND-WINDOW: Refund window",
        "",
        "A payment is refundable within 30 days of capture; later requests go to support.",
      ],
      sections: [{ id: "POL-REFUND-WINDOW", kind: "policy", title: "Refund window" }],
    }),
  ];
  const lineOf = (path: string, needle: string): number =>
    (documents.find((file) => file.path === path)?.lines.findIndex((line) => line.includes(needle)) ?? -1) + 1;
  return {
    treeNotes: [],
    documents,
    links: [
      link({ type: "depends_on", origin: "frontmatter", written: "POL-WRITES", path: ENDPOINT_FILE, line: lineOf(ENDPOINT_FILE, "depends_on:"), to: "POL-WRITES" }),
      link({ type: "mentions", origin: "inline", written: "POL-IDEMPOTENCY", path: ENDPOINT_FILE, line: lineOf(ENDPOINT_FILE, "See POL-IDEMPOTENCY"), to: "POL-IDEMPOTENCY" }),
      link({ type: "constrains", origin: "frontmatter", written: "EP-REFUNDS", path: WINDOW_FILE, line: lineOf(WINDOW_FILE, "constrains:"), to: "EP-REFUNDS" }),
      link({
        type: "mentions",
        origin: "inline",
        written: "EP-REFUND-CREATE",
        path: "docs/spec/api/legacy-v0.md",
        line: lineOf("docs/spec/api/legacy-v0.md", "replaced by"),
        to: "EP-REFUND-CREATE",
      }),
    ],
  };
}

export function ledgerApi(now: number): MockProject {
  const at = (minutesAgo: number) => stamp(now, minutesAgo);
  const agent = (role: string, run: string) => ({ type: "agent", role, model: "claude-opus-5-5", run });
  return {
    project: { slug: SLUG, name: "Ledger API", root: "/work/ledger-api", branch: "main" },
    nodeKinds,
    specStatuses,
    notes: ["2 proposals of an archived task are left out (spec inbox --all lists them)"],
    corpus: ledgerCorpus(),
    proposals: [
      proposal({
        id: "PR-0007",
        project: SLUG,
        kind: "discrepancy",
        severity: "high",
        gap_type: "contradicts",
        task_id: "T-0031",
        target_id: "POL-REFUND-WINDOW",
        target_path: WINDOW_FILE,
        target_ids: ["POL-REFUND-WINDOW"],
        branch: "task/T-0031",
        summary: "Refunds are accepted for 60 days; POL-REFUND-WINDOW allows 30",
        evidence: [
          {
            file: "src/refunds/window.rs",
            qpath: "refunds::window::is_refundable",
            lines: "12-20",
            observed: "REFUND_WINDOW_DAYS = 60",
            documented: "A payment is refundable within 30 days of capture.",
          },
        ],
        options: [
          {
            label: "Code to spec",
            effect: "Set REFUND_WINDOW_DAYS to 30.",
            price: "Support tickets for days 31 to 60 rise; finance agrees.",
          },
          {
            label: "Spec to code",
            effect: "Write 60 days into POL-REFUND-WINDOW.",
            price: "Chargeback exposure grows; needs the risk team's sign-off.",
          },
        ],
        recommendation: 0,
        working_answer: "The service keeps 60 days until you decide.",
        author: agent("rust-developer", "R-0418"),
        created_at: at(25),
      }),
      proposal({
        id: "PR-0008",
        project: SLUG,
        kind: "update",
        severity: "normal",
        task_id: "T-0033",
        target_id: "EP-REFUND-CREATE",
        target_path: ENDPOINT_FILE,
        target_ids: ["EP-REFUND-CREATE", "POL-IDEMPOTENCY"],
        summary: "Require an Idempotency-Key on POST /v1/refunds",
        rationale: "POL-IDEMPOTENCY covers every write endpoint; the refund endpoint never said so.",
        base_text: ENDPOINT_TEXT,
        diff: ENDPOINT_DIFF,
        preview: "applies",
        author: agent("spec-writer", "R-0422"),
        diagnostics: [
          {
            code: "ref-dangling",
            severity: "warning",
            path: ENDPOINT_FILE,
            line: 8,
            subject: "POL-IDEMPOTENCY",
            message: "POL-IDEMPOTENCY is cited inline but not declared in links",
          },
        ],
        created_at: at(5 * 60),
      }),
      proposal({
        id: "PR-0009",
        project: SLUG,
        kind: "question",
        severity: "low",
        target_id: "POL-IDEMPOTENCY",
        target_ids: ["POL-IDEMPOTENCY"],
        summary: "How long are idempotency keys kept?",
        options: [
          { label: "24 hours", effect: "Keys expire after a day.", price: "A client retrying after a day creates a second refund." },
          { label: "7 days", effect: "Keys live for a week.", price: "About 40 MB more in the key store." },
        ],
        recommendation: 1,
        working_answer: "Keys are kept for 7 days.",
        author: agent("requirement-analyst", "R-0399"),
        created_at: at(3 * 24 * 60),
      }),
    ],
  };
}
