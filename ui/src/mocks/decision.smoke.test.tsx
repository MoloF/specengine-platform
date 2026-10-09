import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError, type SpecEngineClient } from "../api/client";
import { HttpClient } from "../api/http";
import type { RoundAnswerBody, RoundItem, StageChoice } from "../api/types";
import { App } from "../app/App";
import { draftOf, offers } from "../round/drafts";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch } from "../test/daemonStub";
import { CRANES_DECISION, CRANES_FILE, harborSim } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";
import { mockTasks, packageOf } from "./tasks";

// AC-U1 and AC-U2 of docs/features/kind-decision.md. harbor-sim holds one decision, PR-0050, as the
// daemon will serve it ("Data": a question's columns, no options, the decision its working answer,
// `linked` its canon diff), and that update, PR-0051, linked back. The Inbox card, the decision
// dialog and the Questions screen take it by its structure with no product change (W23): their
// controls follow `options.length` and `working_answer`, so a branch on the kind (a word that is
// `decision` here, never `question`) would drop a control these tests look for. Staging it sends a
// question's stage: no option, the working answer (`answer` null) or the owner's own `answer`.

/** The time the mock's fixtures are relative to: one clock for every render. */
const NOW = Date.UTC(2026, 9, 9, 12, 0, 0);

const SLUG = "harbor-sim";
const DECISION = "PR-0050";
const UPDATE = "PR-0051";
const CRANES = "MEC-CARGO-CRANES";

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

/** The Inbox card of `id`, its review read. */
async function cardOf(id: string): Promise<HTMLElement> {
  await waitFor(() => {
    expect(screen.getByRole("article").querySelector(".card-id")?.textContent).toBe(id);
  });
  const card = screen.getByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  return card;
}

/** The Questions screen's card of `id`. */
function roundCard(id: string): HTMLElement {
  return screen.getByRole("article", { name: id });
}

function radiosOf(card: HTMLElement): { label: string; checked: boolean }[] {
  return within(card)
    .getAllByRole("radio")
    .map((radio) => ({ label: radio.closest("label")?.textContent ?? "", checked: (radio as HTMLInputElement).checked }));
}

/** The daemon as a stubbed fetch over `served`: its reads, the decision POST and the round's two routes; every request body kept. */
function daemonOver(served: MockClient) {
  const posted: { url: string; body: string }[] = [];
  const answer = async (run: () => Promise<unknown>): Promise<Response> => {
    try {
      return jsonAnswer(200, await run());
    } catch (error) {
      if (error instanceof ClientError) {
        return errorAnswer(error.status, error.message);
      }
      throw error;
    }
  };
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
      return answer(() => served.stageDecision(slug, id, stage, updatedAt));
    }
    if (section === "proposals" && tail === undefined) {
      return jsonAnswer(200, await served.getProposal(slug, id));
    }
    if (section === "nodes" && tail === undefined) {
      const view = await served.getNode(slug, id);
      return jsonAnswer(view.nodes.length === 0 ? 404 : 200, view);
    }
    if (section === "round" && ref === "answers" && body !== null) {
      const sent = JSON.parse(body) as { answers: RoundAnswerBody[] };
      return answer(() => served.stageRoundAnswers(slug, sent.answers));
    }
    if (section === "round" && ref === undefined) {
      return answer(() => served.getRound(slug));
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

describe("harbor-sim's decision (AC-U1 of kind-decision)", () => {
  it("holds one decision, PR-0050: a question's columns, no options, its decision the working answer, linked to PR-0051", async () => {
    const client = mock();
    const { proposals } = await client.getInbox(SLUG);
    const reviews = await Promise.all(proposals.map((entry) => client.getProposal(SLUG, entry.id)));
    // Kinds are raw strings: spelled only in mocks and tests (ADR-0031).
    expect(reviews.filter((review) => review.kind === "decision").map((review) => review.id)).toEqual([DECISION]);
    const decision = await client.getProposal(SLUG, DECISION);
    expect(decision).toMatchObject({
      id: DECISION,
      project: SLUG,
      kind: "decision",
      status: "open",
      target_id: CRANES,
      target_path: CRANES_FILE,
      target_ids: [CRANES],
      // An intake row: none of an update's texts, no diff, no preview, no findings.
      base_hash: null,
      base_text: null,
      new_text: null,
      patch_hash: null,
      rationale: null,
      diagnostics: [],
      diff: null,
      preview: null,
      conflict: null,
      // A question's columns: what had to be decided, the decision, another answer's cost; none of a discrepancy's.
      severity: "normal",
      gap_type: null,
      summary: CRANES_DECISION.text,
      working_answer: CRANES_DECISION.working_answer,
      price_of_other: CRANES_DECISION.price_of_other,
      evidence: [],
      options: [],
      recommendation: null,
      distinct_from: [],
      linked: UPDATE,
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
    expect(proposals.find((entry) => entry.id === DECISION)).toMatchObject({
      kind: "decision",
      target_ids: [CRANES],
      rationale: null,
      severity: "normal",
      summary: CRANES_DECISION.text,
    });
  });

  it("holds its canon diff as the update PR-0051: linked back, raised in the same intake, based on the page as it is", async () => {
    const client = mock();
    const decision = await client.getProposal(SLUG, DECISION);
    const update = await client.getProposal(SLUG, UPDATE);
    expect(update).toMatchObject({
      id: UPDATE,
      kind: "update",
      status: "open",
      target_id: CRANES,
      target_path: CRANES_FILE,
      target_ids: [CRANES],
      preview: "applies",
      conflict: null,
      // An update: the eleven intake keys null or empty but `linked`.
      severity: null,
      gap_type: null,
      summary: null,
      working_answer: null,
      price_of_other: null,
      evidence: [],
      options: [],
      recommendation: null,
      distinct_from: [],
      linked: DECISION,
      interpretation: null,
    });
    // One intake: the same place, author, time and task.
    for (const key of ["worktree", "branch", "base_commit", "author", "created_at", "task_id"] as const) {
      expect([key, update[key]]).toEqual([key, decision[key]]);
    }
    const page = (await client.getNode(SLUG, CRANES)).nodes[0];
    expect([update.base_text, update.base_hash]).toEqual([page?.text, page?.span_hash]);
    const base = update.base_text ?? "";
    const added = (update.new_text ?? "").slice(base.length);
    expect((update.new_text ?? "").startsWith(base) && added.length > 0).toBe(true);
    expect(update.diff?.split("\n").filter((line) => /^[-+](?![-+]{2} )/.test(line))).toEqual([`+${added.trimEnd()}`]);
    expect((await client.getInbox(SLUG)).proposals.find((entry) => entry.id === UPDATE)).toMatchObject({
      kind: "update",
      severity: null,
      summary: null,
      rationale: update.rationale?.split("\n")[0],
    });
  });

  it("puts the decision on the round sheet as a question, options empty; its update is noted as a change", async () => {
    const sheet = await mock().getRound(SLUG);
    expect(sheet.items.find((item) => item.id === DECISION)).toMatchObject({
      kind: "decision",
      severity: "normal",
      gap_type: null,
      summary: CRANES_DECISION.text,
      working_answer: CRANES_DECISION.working_answer,
      price_of_other: CRANES_DECISION.price_of_other,
      options: [],
      recommendation: null,
      evidence: [],
      targets: [{ id: CRANES, path: CRANES_FILE, title: "Cargo cranes" }],
      tasks: [],
      staged: null,
    });
    expect(sheet.items.map((item) => item.id)).not.toContain(UPDATE);
    expect(sheet.notes.join("\n")).toMatch(/open change proposal\(s\) not on this sheet: [^;]*PR-0051; spec inbox lists them/);
  });

  it("lists both in a task package whose nodes hold the page: the decision by its text, assumed as its working answer (W21)", () => {
    const stored = mockTasks(SLUG).tasks.find((task) => task.id === "T-0113");
    if (stored === undefined) {
      throw new Error("no T-0113");
    }
    const raised = harborSim(NOW).proposals.filter(({ review }) => review.id === DECISION || review.id === UPDATE);
    const bound = packageOf({ ...stored, spec_snapshot: null, affected_nodes: [CRANES] }, raised);
    expect(bound.open_proposals.map(({ id, kind, summary }) => [id, kind, summary])).toEqual([
      [DECISION, "decision", CRANES_DECISION.text],
      [UPDATE, "update", raised[1]?.review.rationale?.split("\n")[0]],
    ]);
    expect(bound.assumptions).toEqual([{ proposal: DECISION, text: CRANES_DECISION.working_answer }]);
  });
});

describe("the Inbox card and the decision dialog take the decision by structure (AC-U2)", () => {
  it("the card shows its text, the working answer and its price, no options, and the linked update as a link; the update's card links back", async () => {
    renderOver(mock(), `#/${SLUG}/inbox/${DECISION}`);
    const card = await cardOf(DECISION);
    expect(within(card).getByRole("heading", { level: 2 }).textContent).toBe(CRANES_DECISION.text);
    // The kind raw, as data.
    expect(fact(card, "Kind")).toBe("decision");
    expect(sectionOf(card, "Options").textContent).toBe("OptionsNo options attached.");
    const working = sectionOf(card, "Working answer");
    expect(within(working).getByText(CRANES_DECISION.working_answer)).toBeTruthy();
    expect(within(working).getByText("Price of another answer:")).toBeTruthy();
    expect(within(working).getByText(CRANES_DECISION.price_of_other)).toBeTruthy();
    expect(sectionOf(card, "Section diff").textContent).toContain("No section diff attached.");
    const provenance = sectionOf(card, "Provenance");
    const terms = Array.from(provenance.querySelectorAll("dt")).map((dt) => dt.textContent);
    expect(terms).toContain("Linked proposal");
    const link = within(provenance).getByRole("link", { name: UPDATE });
    expect(link.getAttribute("href")).toBe(`#/${SLUG}/inbox/${UPDATE}`);
    // Information only: the question on the same page and the decision's own update.
    const others = sectionOf(card, "Also open on these nodes");
    expect(within(others).getAllByRole("button").map((button) => button.textContent)).toEqual(["PR-0048", UPDATE]);

    fireEvent.click(link);
    // jsdom follows a clicked hash link only a task later; the browser at once.
    window.location.hash = `#/${SLUG}/inbox/${UPDATE}`;
    const update = await cardOf(UPDATE);
    expect(fact(update, "Kind")).toBe("update");
    expect(within(sectionOf(update, "Provenance")).getByRole("link", { name: DECISION }).getAttribute("href")).toBe(`#/${SLUG}/inbox/${DECISION}`);
    const diff = within(sectionOf(update, "Section diff")).getByRole("figure", { name: "Section diff" });
    expect(Array.from(diff.querySelectorAll('[data-line="added"]')).map((line) => line.textContent)).toEqual([
      "Added: +Both cranes work the moored ship together; neither waits for the other to finish.\n",
    ]);
  });

  it("Accept offers no option and records the working answer: it stages option and answer null, the card shows the working answer staged", async () => {
    const client = mock();
    const stage = vi.spyOn(client, "stageDecision");
    const read = await client.getProposal(SLUG, DECISION);
    renderOver(client, `#/${SLUG}/inbox/${DECISION}`);
    const card = await cardOf(DECISION);
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: `Accept ${DECISION}` });
    expect(within(dialog).queryAllByRole("radio")).toEqual([]);
    expect(within(dialog).queryByRole("group", { name: "Option" })).toBeNull();
    expect(within(dialog).getByText("The working answer is recorded:")).toBeTruthy();
    expect(within(dialog).getByText(CRANES_DECISION.working_answer)).toBeTruthy();
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "Two cranes, as built." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(stage.mock.calls).toEqual([
      [SLUG, DECISION, { decision: "approve", option: null, answer: null, canon: null, note: "Two cranes, as built." }, read.updated_at],
    ]);
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByText("spec approve PR-0050").tagName).toBe("CODE");
    const pairs = Array.from(staged.querySelectorAll("dt")).map((dt) => [dt.textContent, dt.nextElementSibling?.textContent]);
    expect(pairs).toEqual([
      ["Decision", "Accept"],
      ["Choice", "The working answer"],
      ["Note", "Two cranes, as built."],
    ]);
    // Stored as a deciding kind's stage: no span hash.
    expect((await client.getProposal(SLUG, DECISION)).staged).toEqual({
      decision: "approve",
      option: null,
      answer: null,
      canon: null,
      note: "Two cranes, as built.",
      span_hash: null,
    });
  });

  it("over HttpClient the decision POST carries exactly the question-shaped stage and the updated_at read", async () => {
    const served = mock();
    const { posted } = daemonOver(served);
    const read = await served.getProposal(SLUG, DECISION);
    renderOver(new HttpClient(), `#/${SLUG}/inbox/${DECISION}`);
    const card = await cardOf(DECISION);
    expect(screen.queryByText("Mock data")).toBeNull();
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: `Accept ${DECISION}` });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(posted).toEqual([
      {
        url: `/api/projects/${SLUG}/proposals/${DECISION}/decision`,
        body: `{"decision":"approve","option":null,"answer":null,"canon":null,"note":null,"updated_at":"${read.updated_at ?? ""}"}`,
      },
    ]);
  });

  it("the daemon's refusal of an option on it is shown in its words: the mock refuses `option` as the daemon does (400)", async () => {
    const client = mock();
    const read = await client.getProposal(SLUG, DECISION);
    const refused = await client.stageDecision(SLUG, DECISION, { decision: "approve", option: 0, answer: null, canon: null, note: null }, read.updated_at ?? "").then(
      () => null,
      (error: unknown) => error,
    );
    expect(refused).toBeInstanceOf(ClientError);
    expect([(refused as ClientError).status, (refused as ClientError).message]).toEqual([400, "spec: --option: `PR-0050` has no options"]);
    expect((await client.getProposal(SLUG, DECISION)).staged).toBeNull();
  });
});

describe("the Questions screen takes the decision by structure (AC-U2)", () => {
  it("lists it with its text, working answer and price; offers the working answer and an own answer, never an option", async () => {
    renderOver(mock(), `#/${SLUG}/questions`);
    await screen.findByRole("article", { name: DECISION });
    const card = roundCard(DECISION);
    expect(card.querySelector(".kind-tag")?.textContent).toBe("decision");
    expect(within(card).getByText(CRANES_DECISION.text)).toBeTruthy();
    expect(within(card).getByText(CRANES_DECISION.working_answer)).toBeTruthy();
    expect(within(card).getByText(CRANES_DECISION.price_of_other)).toBeTruthy();
    expect(within(card).getByRole("link", { name: `Cargo cranes (${CRANES})` }).getAttribute("href")).toBe(`#/${SLUG}/tree/${CRANES}`);
    expect(within(card).getByRole("link", { name: `Open in Inbox: ${DECISION}` }).getAttribute("href")).toBe(`#/${SLUG}/inbox/${DECISION}`);
    // `ok` and a text are answers; a number is not: no option to pick, Leave open the default.
    expect(radiosOf(card)).toEqual([
      { label: "Take the working answer", checked: false },
      { label: "Your answer", checked: false },
      { label: "Reject with a reason", checked: false },
      { label: "Leave open", checked: true },
    ]);
    expect(within(card).queryByText(/^\[\d+\]$/)).toBeNull();
  });

  it("its own answer is staged as a question's: option null, the text as `answer`; the card shows it staged", async () => {
    const client = mock();
    const post = vi.spyOn(client, "stageRoundAnswers");
    const read = (await client.getRound(SLUG)).items.find((item) => item.id === DECISION);
    renderOver(client, `#/${SLUG}/questions`);
    await screen.findByRole("article", { name: DECISION });
    fireEvent.click(within(roundCard(DECISION)).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(roundCard(DECISION)).getByLabelText(`Your answer to ${DECISION}`), {
      target: { value: "  No: one crane per ship until the second quay opens.  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(roundCard(DECISION)).findByRole("region", { name: "Staged decision" });
    expect(post.mock.calls).toEqual([
      [
        SLUG,
        [
          {
            id: DECISION,
            stage: {
              decision: "approve",
              option: null,
              answer: "No: one crane per ship until the second quay opens.",
              canon: null,
              note: null,
              updated_at: read?.updated_at,
            },
          },
        ],
      ],
    ]);
    expect(within(staged).getByText("No: one crane per ship until the second quay opens.")).toBeTruthy();
    expect(within(staged).getByText(`spec approve ${DECISION}`)).toBeTruthy();
  });

  it("over HttpClient, Take the working answer POSTs exactly a question's answer: option and answer null", async () => {
    const served = mock();
    const { posted } = daemonOver(served);
    const read = (await served.getRound(SLUG)).items.find((item) => item.id === DECISION);
    renderOver(new HttpClient(), `#/${SLUG}/questions`);
    await screen.findByRole("article", { name: DECISION });
    fireEvent.click(within(roundCard(DECISION)).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(screen.getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(roundCard(DECISION)).findByRole("region", { name: "Staged decision" });
    expect(posted).toEqual([
      {
        url: `/api/projects/${SLUG}/round/answers`,
        body: `{"project":"${SLUG}","answers":[{"id":"${DECISION}","stage":{"decision":"approve","option":null,"answer":null,"canon":null,"note":null,"updated_at":"${read?.updated_at ?? ""}"}}]}`,
      },
    ]);
    expect(within(staged).getByText("The working answer")).toBeTruthy();
  });

  it("a number is not an answer to it: the screen's picks follow its structure, and an option sent anyway is refused alone", async () => {
    const client = mock();
    const sheet = await client.getRound(SLUG);
    const item: RoundItem | undefined = sheet.items.find((entry) => entry.id === DECISION);
    if (item === undefined) {
      throw new Error("PR-0050 is not on the sheet");
    }
    const fresh = draftOf(item);
    expect([
      offers(item, { ...fresh, pick: "working" }),
      offers(item, { ...fresh, pick: "answer", answer: "mine" }),
      offers(item, { ...fresh, pick: "option", option: 0 }),
    ]).toEqual([true, true, false]);
    const outcome = await client.stageRoundAnswers(SLUG, [
      { id: DECISION, stage: { decision: "approve", option: 0, answer: null, canon: null, note: null, updated_at: item.updated_at } },
      { id: "PR-0048", stage: { decision: "approve", option: null, answer: null, canon: null, note: null, updated_at: sheet.items.find((entry) => entry.id === "PR-0048")?.updated_at ?? "" } },
    ]);
    expect(outcome.answers.map(({ id, outcome: made, reason }) => [id, made, reason])).toEqual([
      [DECISION, "refused", "--option: `PR-0050` has no options"],
      ["PR-0048", "staged", null],
    ]);
  });
});
