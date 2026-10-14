import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError, type SpecEngineClient } from "../api/client";
import { HttpClient } from "../api/http";
import type { StageChoice } from "../api/types";
import { App } from "../app/App";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch } from "../test/daemonStub";
import { stamp } from "./build";
import { harborSim, SURGE_AMENDED, SURGE_FILE, SURGE_NEW_ID, SURGE_TEXT } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";
import { mockTasks, packageOf } from "./tasks";

// AC-U1 of docs/features/kind-amendment.md. harbor-sim holds one amendment, PR-0052, as the daemon
// will serve it ("Data", "Row"): a file-form create's row, its kind the one difference: no base,
// the whole new file its text, `target_id` the path, `target_ids` the path and the one new ID,
// never the amended ID, which only the text's `links: {amends: [...]}` holds. The Inbox list, the
// card and the stage dialog take it by its structure with no product change: what they show
// follows `diff`, `options`, `working_answer`, `interpretation` and the other keys, never the kind
// (a branch on it is the mutation `policy.test.ts` catches). Accept stages an approve with no
// option, answer or canon, Reject a reject with its reason; over the mock and over HttpClient.

/** The time the mock's fixtures are relative to: one clock for every render. */
const NOW = Date.UTC(2026, 9, 9, 12, 0, 0);

const SLUG = "harbor-sim";
const AMENDMENT = "PR-0052";

/** The rationale's first line: the inbox line and the card's title. */
const TITLE = "A storm surge raises high water above the tide table.";

function mock(): MockClient {
  return new MockClient("normal", { now: () => NOW, delayMs: 0 });
}

function renderOver(client: SpecEngineClient, hash: string) {
  window.history.replaceState(null, "", `/${hash}`);
  render(<App client={client} scenario={null} />);
}

/** A card's section by its heading. */
function sectionOf(scope: HTMLElement, title: string): HTMLElement {
  const found = within(scope).getByRole("heading", { level: 3, name: title }).closest("section");
  if (found === null) {
    throw new Error(`no section headed ${title}`);
  }
  return found;
}

/** The value a card's facts give a name. */
function fact(card: HTMLElement, name: string): string {
  const term = Array.from(card.querySelectorAll(".facts dt")).find((dt) => dt.textContent === name);
  return term?.nextElementSibling?.textContent ?? "";
}

/** The Inbox card of `id`, its review read and both targets answered. */
async function cardOf(id: string): Promise<HTMLElement> {
  await waitFor(() => {
    expect(screen.getByRole("article").querySelector(".card-id")?.textContent).toBe(id);
  });
  const card = screen.getByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  await waitFor(() => {
    expect(card.querySelectorAll(".target p.muted")).toHaveLength(2);
  });
  return card;
}

/** The section diff's lines as a screen reader gets them, in order. */
function diffLinesOf(card: HTMLElement): string[] {
  const figure = within(sectionOf(card, "Section diff")).getByRole("figure", { name: "Section diff" });
  return Array.from(figure.querySelectorAll("[data-line]"), (line) => `${line.getAttribute("data-line") ?? ""} ${line.textContent}`);
}

/** The daemon as a stubbed fetch over `served`: its reads and the decision POST; every request body kept. */
function daemonOver(served: MockClient) {
  const posted: { url: string; body: string }[] = [];
  const fetch = stubFetch(async (url, init) => {
    const body = typeof init?.body === "string" ? init.body : null;
    if (body !== null) {
      posted.push({ url, body });
    }
    if (url === "/api/projects") {
      return jsonAnswer(200, await served.getProjects());
    }
    const route = /^\/api\/projects\/([^/?]+)\/([a-z]+)(?:\/([^/?]+))?(?:\/([a-z]+))?(?:\?(.*))?$/.exec(url);
    const [, project, section, ref, tail] = route ?? [];
    if (project === undefined || section === undefined) {
      return errorAnswer(404, `no route ${url}`);
    }
    const slug = decodeURIComponent(project);
    const id = ref === undefined ? "" : decodeURIComponent(ref);
    if (section === "inbox") {
      return jsonAnswer(200, await served.getInbox(slug));
    }
    if (section === "proposals" && tail === "decision" && body !== null) {
      const { updated_at: updatedAt, ...stage } = JSON.parse(body) as StageChoice & { updated_at: string };
      try {
        return jsonAnswer(200, await served.stageDecision(slug, id, stage, updatedAt));
      } catch (error) {
        if (error instanceof ClientError) {
          return errorAnswer(error.status, error.message);
        }
        throw error;
      }
    }
    if (section === "proposals" && tail === undefined) {
      return jsonAnswer(200, await served.getProposal(slug, id));
    }
    if (section === "nodes" && tail === undefined) {
      const view = await served.getNode(slug, id);
      return jsonAnswer(view.nodes.length === 0 ? 404 : 200, view);
    }
    return errorAnswer(404, `no route ${url}`);
  });
  return { fetch, posted };
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("harbor-sim's amendment (AC-U1 of kind-amendment)", () => {
  it("holds one amendment, PR-0052: a new file's review document, no base, the amended ID in no key but the text", async () => {
    const client = mock();
    const { proposals } = await client.getInbox(SLUG);
    const reviews = await Promise.all(proposals.map((entry) => client.getProposal(SLUG, entry.id)));
    // Kinds are raw strings: spelled only in mocks and tests (ADR-0031).
    expect(reviews.filter((review) => review.kind === "amendment").map((review) => review.id)).toEqual([AMENDMENT]);
    const review = await client.getProposal(SLUG, AMENDMENT);
    // A new file's review document: the 47 keys of any review.
    expect(Object.keys(review)).toHaveLength(47);
    expect(review).toMatchObject({
      id: AMENDMENT,
      project: SLUG,
      kind: "amendment",
      status: "open",
      // A file-form create's columns: the path as `target_id` (no `id:`), then the new ID.
      target_id: SURGE_FILE,
      target_path: SURGE_FILE,
      target_ids: [SURGE_FILE, SURGE_NEW_ID],
      worktree: "/work/harbor-sim",
      branch: "main",
      base_hash: null,
      base_text: null,
      new_text: SURGE_TEXT,
      diagnostics: [],
      preview: "applies",
      conflict: null,
      // A new file writes no existing node: no revision to raise (docs/features/rev-discipline.md "Review document, stage, queue").
      revisions: null,
      decided_by: null,
      decided_at: null,
      decision_note: null,
      applied_commit: null,
      created_at: stamp(NOW, 15),
      updated_at: stamp(NOW, 15),
      // The intake and record keys null or empty: a change proposal.
      severity: null,
      gap_type: null,
      summary: null,
      working_answer: null,
      price_of_other: null,
      evidence: [],
      options: [],
      recommendation: null,
      distinct_from: [],
      linked: null,
      record_id: null,
      record_path: null,
      record_title: null,
      record_text: null,
      choice: null,
      task_id: null,
      staged: null,
      staged_at: null,
      interpretation: null,
      notes: [],
    });
    expect(review.patch_hash).toMatch(/^b3:[0-9a-f]{64}$/);
    expect(review.rationale?.split("\n")[0]).toBe(TITLE);
    // The text declares what it amends; the amended node is live in the corpus, and no other key names it.
    expect(SURGE_TEXT.split("\n").slice(0, 7)).toEqual(["---", "class: spec", "status: draft", "scope: [water]", "links:", `  amends: [${SURGE_AMENDED}]`, "---"]);
    const amended = (await client.getNode(SLUG, SURGE_AMENDED)).nodes;
    expect(amended.map((node) => [node.id, node.status])).toEqual([[SURGE_AMENDED, "accepted"]]);
    const { new_text: text, diff, ...rest } = review;
    expect([text?.includes(SURGE_AMENDED), diff?.includes(SURGE_AMENDED), JSON.stringify(rest).includes(SURGE_AMENDED)]).toEqual([true, true, false]);
  });

  it("serves its diff against an empty base, every line of the text added, and its inbox line as a create's", async () => {
    const client = mock();
    const review = await client.getProposal(SLUG, AMENDMENT);
    const lines = (review.diff ?? "").split("\n");
    const textLines = SURGE_TEXT.split("\n").slice(0, -1);
    expect(textLines).toHaveLength(13);
    expect(lines.slice(0, 3)).toEqual([`--- base ${SURGE_FILE}`, `+++ proposed ${SURGE_FILE}`, "@@ -0,0 +1,13 @@"]);
    // Git's hunks end in a newline: one empty piece after the last line.
    expect(lines.slice(3)).toEqual([...textLines.map((line) => `+${line}`), ""]);
    const entry = (await client.getInbox(SLUG)).proposals.find((candidate) => candidate.id === AMENDMENT);
    expect(entry).toEqual({
      id: AMENDMENT,
      kind: "amendment",
      status: "open",
      target_id: SURGE_FILE,
      target_ids: [SURGE_FILE, SURGE_NEW_ID],
      branch: "main",
      created_at: stamp(NOW, 15),
      rationale: TITLE,
      severity: null,
      summary: null,
      record_id: null,
      staged_at: null,
    });
  });

  it("is never on the round sheet; the sheet counts it among the open change proposals, and refuses an answer naming it a change", async () => {
    const client = mock();
    const sheet = await client.getRound(SLUG);
    expect(sheet.items.map((item) => item.id)).not.toContain(AMENDMENT);
    expect(sheet.notes).toContain("4 open change proposal(s) not on this sheet: PR-0042, PR-0049, PR-0051 and PR-0052; spec inbox lists them");
    const review = await client.getProposal(SLUG, AMENDMENT);
    const outcome = await client.stageRoundAnswers(SLUG, [
      { id: AMENDMENT, stage: { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, updated_at: review.updated_at ?? "" } },
    ]);
    expect(outcome.answers.map(({ id, outcome: made, reason }) => [id, made, reason])).toEqual([
      [AMENDMENT, "refused", "`PR-0052` is a change proposal (amendment): `spec approve PR-0052` decides it"],
    ]);
    expect((await client.getProposal(SLUG, AMENDMENT)).staged).toBeNull();
  });

  it("is listed by a task wanting the amended node, read from its text (W33), with no assumption; not by a task wanting neither", () => {
    const stored = mockTasks(SLUG).tasks.find((task) => task.id === "T-0113");
    if (stored === undefined) {
      throw new Error("no T-0113");
    }
    const queue = harborSim(NOW).proposals;
    const wanting = packageOf({ ...stored, spec_snapshot: null, targets: [], criteria: [], affected_nodes: [SURGE_AMENDED] }, queue);
    expect(wanting.open_proposals.map(({ id, kind, target_ids: targets, summary }) => [id, kind, targets, summary])).toEqual([
      [AMENDMENT, "amendment", [SURGE_FILE, SURGE_NEW_ID], TITLE],
    ]);
    expect(wanting.assumptions).toEqual([]);
    expect(packageOf(stored, queue).open_proposals.map(({ id }) => id)).not.toContain(AMENDMENT);
  });
});

describe("the Inbox takes the amendment by structure (AC-U1)", () => {
  it("lists it with its kind raw; the card shows the new file's diff, no options, no working answer, its targets not yet in the corpus", async () => {
    renderOver(mock(), `#/${SLUG}/inbox/${AMENDMENT}`);
    const card = await cardOf(AMENDMENT);
    const row = within(screen.getByRole("listbox")).getAllByRole("option").find((option) => option.dataset.proposal === AMENDMENT);
    expect(row?.querySelector(".queue-item-summary")?.textContent).toBe(TITLE);
    expect(Array.from(row?.querySelectorAll(".queue-item-meta .mono") ?? [], (cell) => cell.textContent)).toEqual([
      "amendment",
      `${SURGE_FILE}, ${SURGE_NEW_ID}`,
    ]);

    expect(within(card).getByRole("heading", { level: 2 }).textContent).toBe(TITLE);
    // The kind raw, as data; a change proposal's facts.
    expect(fact(card, "Kind")).toBe("amendment");
    expect(fact(card, "Gap type")).toBe("None");
    expect(fact(card, "Severity")).toBe("None");
    expect(fact(card, "Task")).toBe("No task");
    expect(fact(card, "Status")).toBe("Open");
    expect(Array.from(card.querySelectorAll(".card-body h3"), (heading) => heading.textContent)).toEqual([
      "Targets",
      "Options",
      "Rationale",
      "Provenance",
      "Section diff",
    ]);
    // Both targets are the file to come: the corpus answers neither yet.
    expect(Array.from(card.querySelectorAll(".target"), (target) => [target.querySelector(".target-head")?.textContent, target.querySelector("p.muted")?.textContent])).toEqual([
      [SURGE_FILE, `\`${SURGE_FILE}\` is no indexed document: not under the \`[paths]\` roots, excluded, or missing`],
      [SURGE_NEW_ID, `\`${SURGE_NEW_ID}\` resolves to no ID and no alias`],
    ]);
    expect(sectionOf(card, "Options").textContent).toBe("OptionsNo options attached.");
    const terms = Array.from(sectionOf(card, "Provenance").querySelectorAll("dt"), (dt) => dt.textContent);
    expect(terms).toEqual(["Author", "Model", "Run", "Raised", "Branch", "Worktree"]);
    const textLines = SURGE_TEXT.split("\n").slice(0, -1);
    expect(diffLinesOf(card)).toEqual([
      `file --- base ${SURGE_FILE}\n`,
      `file +++ proposed ${SURGE_FILE}\n`,
      "hunk @@ -0,0 +1,13 @@\n",
      ...textLines.map((line) => `added Added: +${line}\n`),
    ]);
    expect(within(sectionOf(card, "Section diff")).getByText("Apply preview: Applies to the current text as is.")).toBeTruthy();
    // The amended rule is named in the text only: no other section says it.
    expect(card.textContent.split(SURGE_AMENDED)).toHaveLength(2);
  });

  it("Accept offers nothing to choose: it stages an approve with option, answer and canon null; stored with no span hash", async () => {
    const client = mock();
    const stage = vi.spyOn(client, "stageDecision");
    const read = await client.getProposal(SLUG, AMENDMENT);
    renderOver(client, `#/${SLUG}/inbox/${AMENDMENT}`);
    const card = await cardOf(AMENDMENT);
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: `Accept ${AMENDMENT}` });
    // The dialog names the file by its path; no option, no working answer, no interpretation.
    expect(Array.from(dialog.querySelectorAll(".dialog-text"), (line) => line.textContent)).toEqual([
      SURGE_FILE,
      expect.stringMatching(/^Stages your approval; nothing is applied here\./),
      "This proposal has no options to choose from.",
    ]);
    expect(within(dialog).queryAllByRole("radio")).toEqual([]);
    expect(within(dialog).getAllByRole("textbox").map((field) => field.getAttribute("id"))).toEqual([
      within(dialog).getByLabelText("Note for the record (optional)").getAttribute("id"),
    ]);
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "Surge first, absorb later." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(stage.mock.calls).toEqual([
      [SLUG, AMENDMENT, { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: "Surge first, absorb later." }, read.updated_at],
    ]);
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByText(`spec approve ${AMENDMENT}`).tagName).toBe("CODE");
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Decision", "Accept"],
      ["Note", "Surge first, absorb later."],
    ]);
    // A new file has no base: the stage keeps no span hash (`docs/features/kind-amendment.md` "Rules and edge cases", Stage).
    expect((await client.getProposal(SLUG, AMENDMENT)).staged).toEqual({
      decision: "approve",
      option: null,
      answer: null,
      canon: null,
      editorial: null,
      note: "Surge first, absorb later.",
      span_hash: null,
    });
  });

  it("Reject stages a reject with its reason; the card shows it with its terminal command", async () => {
    const client = mock();
    const stage = vi.spyOn(client, "stageDecision");
    const read = await client.getProposal(SLUG, AMENDMENT);
    renderOver(client, `#/${SLUG}/inbox/${AMENDMENT}`);
    const card = await cardOf(AMENDMENT);
    fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog", { name: `Reject ${AMENDMENT}` });
    expect(within(dialog).queryAllByRole("radio")).toEqual([]);
    fireEvent.change(within(dialog).getByLabelText("Reason (required)"), { target: { value: "Surge belongs in the weather spec." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage reject" }));
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(stage.mock.calls).toEqual([[SLUG, AMENDMENT, { decision: "reject", reason: "Surge belongs in the weather spec." }, read.updated_at]]);
    expect(within(staged).getByText(`spec reject ${AMENDMENT}`).tagName).toBe("CODE");
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Decision", "Reject"],
      ["Reason", "Surge belongs in the weather spec."],
    ]);
  });

  it("over HttpClient: the same document, the same card and diff; the POSTs carry exactly an approve, then a reject", async () => {
    const shown = mock();
    renderOver(shown, `#/${SLUG}/inbox/${AMENDMENT}`);
    const overMock = await cardOf(AMENDMENT);
    const mockDiff = within(sectionOf(overMock, "Section diff")).getByRole("figure").cloneNode(true);
    const mockFacts = overMock.querySelector(".facts")?.cloneNode(true);
    cleanup();

    const served = mock();
    const { posted } = daemonOver(served);
    const http = new HttpClient();
    expect(await http.getProposal(SLUG, AMENDMENT)).toEqual(await served.getProposal(SLUG, AMENDMENT));
    const read = await served.getProposal(SLUG, AMENDMENT);
    renderOver(http, `#/${SLUG}/inbox/${AMENDMENT}`);
    const card = await cardOf(AMENDMENT);
    expect(screen.queryByText("Mock data")).toBeNull();
    expect(fact(card, "Kind")).toBe("amendment");
    // Elements, attributes and text alike, node for node (DOM equality, no serialised markup).
    expect(within(sectionOf(card, "Section diff")).getByRole("figure").isEqualNode(mockDiff)).toBe(true);
    expect(card.querySelector(".facts")?.isEqualNode(mockFacts ?? null)).toBe(true);

    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const accept = await screen.findByRole("dialog", { name: `Accept ${AMENDMENT}` });
    fireEvent.click(within(accept).getByRole("button", { name: "Stage accept" }));
    await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    const approved = await served.getProposal(SLUG, AMENDMENT);
    fireEvent.click(within(screen.getByRole("article")).getByRole("button", { name: "Reject" }));
    const reject = await screen.findByRole("dialog", { name: `Reject ${AMENDMENT}` });
    fireEvent.change(within(reject).getByLabelText("Reason (required)"), { target: { value: "Not this release." } });
    fireEvent.click(within(reject).getByRole("button", { name: "Stage reject" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    const path = `/api/projects/${SLUG}/proposals/${AMENDMENT}/decision`;
    expect(posted).toEqual([
      { url: path, body: `{"decision":"approve","option":null,"answer":null,"canon":null,"editorial":null,"note":null,"updated_at":"${read.updated_at ?? ""}"}` },
      { url: path, body: `{"decision":"reject","reason":"Not this release.","updated_at":"${approved.updated_at ?? ""}"}` },
    ]);
    expect((await served.getProposal(SLUG, AMENDMENT)).staged).toEqual({ decision: "reject", reason: "Not this release." });
  });
});
