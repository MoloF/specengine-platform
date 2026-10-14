import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError, type SpecEngineClient } from "../api/client";
import { HttpClient } from "../api/http";
import type { Proposal, StageChoice } from "../api/types";
import { App } from "../app/App";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch } from "../test/daemonStub";
import { CLOSED_NODE, CLOSED_NOTE, CLOSED_REFUSAL, DECISION_FIX, HARBOR_LOCK, REWORDED, UNREADABLE_LOCK_NOTE, WORDING_FIX, WORDING_NOTE } from "./editorial";
import { MockClient } from "./MockClient";
import { scenarioFromSearch } from "./scenario";

// UI-1, UI-2 and UI-4 of docs/features/rev-discipline.md over the mock ("UI"): every change that
// alters a node's meaning carries its `revisions`, a new file none; `?scenario=editorial` ("Mock
// data") counts the lock entries an editorial approval refreshes (harbor-sim's lock read,
// ledger-api's unreadable), holds PR-0053 with an editorial approve staged and PR-0054, whose raise
// a closed class refuses (Q15). The contract test: Accept with "Editorial: keep the revision" stages
// `editorial: true` alike over the mock and over HttpClient (the daemon a stubbed `fetch` over the
// mock), the POST body exactly the stage; the card shows its Editorial row and offers the bare
// `spec approve PR`, which confirms the stage (Q14).

/** The time the mock's fixtures are relative to: one clock for every render. */
const NOW = Date.UTC(2026, 9, 13, 12, 0, 0);

const SLUG = "harbor-sim";
const DRAFT_CHANGE = "PR-0042";
const AMENDMENT = "PR-0052";
const EDITORIAL = "Editorial: keep the revision";

function mock(scenario: "normal" | "editorial" = "editorial"): MockClient {
  return new MockClient(scenario, { now: () => NOW, delayMs: 0 });
}

function renderOver(client: SpecEngineClient, hash: string, scenario: string | null) {
  window.history.replaceState(null, "", `/${hash}`);
  render(<App client={client} scenario={scenario} />);
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

/** A card's section by its heading. */
function sectionOf(scope: HTMLElement, title: string): HTMLElement {
  const found = within(scope).getByRole("heading", { level: 3, name: title }).closest("section");
  if (found === null) {
    throw new Error(`no section headed ${title}`);
  }
  return found;
}

/** Every review of a project's inbox, by ID. */
async function reviewsOf(client: MockClient, project: string): Promise<Map<string, Proposal>> {
  const { proposals } = await client.getInbox(project);
  const reviews = await Promise.all(proposals.map((entry) => client.getProposal(project, entry.id)));
  return new Map(reviews.map((review) => [review.id ?? "", review]));
}

/** The daemon as a stubbed fetch over `served`: its reads and the decision POST; every request body kept. */
function daemonOver(served: MockClient) {
  const posted: { url: string; body: string }[] = [];
  stubFetch(async (url, init) => {
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
  return posted;
}

/** Accept on PR-0042's card with the editorial toggle checked and a note; the staged region once shown. */
async function stageEditorial(): Promise<HTMLElement> {
  const card = await cardOf(DRAFT_CHANGE);
  fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
  const dialog = await screen.findByRole("dialog", { name: `Accept ${DRAFT_CHANGE}` });
  fireEvent.click(within(dialog).getByRole("checkbox", { name: EDITORIAL }));
  fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "typo" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
  await waitFor(() => {
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  return within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the mock's revisions (UI-4 of rev-discipline)", () => {
  it("reads ?scenario=editorial as the editorial scenario", () => {
    expect(scenarioFromSearch("?scenario=editorial")).toBe("editorial");
  });

  it("normal: each change altering a node lists it at the node's revision now, no lock entry, none refused; every other review null", async () => {
    const client = mock("normal");
    for (const project of ["harbor-sim", "ledger-api"]) {
      const reviews = await reviewsOf(client, project);
      for (const review of reviews.values()) {
        for (const entry of review.revisions ?? []) {
          const node = (await client.getNode(project, entry.id)).nodes[0];
          expect([review.id, entry.id, entry.rev, entry.lock_entries, entry.refused, typeof entry.to]).toEqual([review.id, entry.id, node?.rev, 0, null, "number"]);
        }
      }
      const listed = [...reviews.values()].filter((review) => review.revisions !== null).map((review) => [review.id, review.kind, review.preview]);
      expect([project, listed]).toEqual([
        project,
        project === SLUG
          ? [
              [DRAFT_CHANGE, "update", "rebases"],
              ["PR-0049", "interpretation", "applies"],
              ["PR-0051", "update", "applies"],
            ]
          : [["PR-0008", "update", "applies"]],
      ]);
    }
    // A file-form create writes no existing node: null, and no editorial toggle (UI-1).
    expect((await client.getProposal(SLUG, AMENDMENT)).revisions).toBeNull();
  });

  it("editorial: harbor-sim's counts read from its lock, PR-0053 staged editorial on its node's span; ledger-api's lock unreadable, said in a note", async () => {
    const client = mock();
    const harbor = await reviewsOf(client, SLUG);
    expect(
      [...harbor.values()].filter((review) => review.revisions !== null).flatMap((review) => (review.revisions ?? []).map((entry) => [review.id, entry.id, entry.lock_entries])),
    ).toEqual([
      [DRAFT_CHANGE, "RULE-BERTH-DRAFT", HARBOR_LOCK.get("RULE-BERTH-DRAFT")],
      ["PR-0049", "RULE-HARBOR-CLOCK", HARBOR_LOCK.get("RULE-HARBOR-CLOCK")],
      ["PR-0051", "MEC-CARGO-CRANES", 0],
      [WORDING_FIX, REWORDED.id, HARBOR_LOCK.get(REWORDED.id)],
      [DECISION_FIX, CLOSED_NODE.id, 0],
    ]);
    const fix = harbor.get(WORDING_FIX);
    const node = (await client.getNode(SLUG, REWORDED.id)).nodes[0];
    expect(fix?.revisions).toEqual([{ id: REWORDED.id, rev: node?.rev, to: REWORDED.to, lock_entries: 1, refused: null }]);
    expect([fix?.base_text, fix?.base_hash]).toEqual([node?.text, node?.span_hash]);
    expect(fix?.staged).toEqual({ decision: "approve", option: null, answer: null, canon: null, editorial: true, note: WORDING_NOTE, span_hash: node?.span_hash });
    expect(fix?.rationale?.split("\n")[0]).toMatch(/^Editorial: /);
    const ledger = await client.getProposal("ledger-api", "PR-0008");
    expect([ledger.revisions?.map((entry) => entry.lock_entries), ledger.notes.at(-1)]).toEqual([[null], UNREADABLE_LOCK_NOTE]);
  });

  it("editorial: PR-0054's raise refused by the decision's closed class, to null, the reason naming --editorial, its note; the preview still applies, nothing staged", async () => {
    const client = mock();
    const fix = await client.getProposal(SLUG, DECISION_FIX);
    const node = (await client.getNode(SLUG, CLOSED_NODE.id)).nodes[0];
    expect(fix.revisions).toEqual([{ id: CLOSED_NODE.id, rev: node?.rev, to: null, lock_entries: 0, refused: CLOSED_REFUSAL }]);
    expect(CLOSED_REFUSAL).toMatch(/--editorial$/);
    expect([fix.preview, fix.staged, fix.notes]).toEqual(["applies", null, [CLOSED_NOTE]]);
    expect([fix.base_text, fix.base_hash]).toEqual([node?.text, node?.span_hash]);
  });

  it("stages a meaning approve over PR-0054's refused raise as any: the terminal's step 6 refuses it, not the stage", async () => {
    const client = mock();
    const read = await client.getProposal(SLUG, DECISION_FIX);
    const staged = await client.stageDecision(SLUG, DECISION_FIX, { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, read.updated_at ?? "");
    expect(staged.staged).toEqual({ decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: read.base_hash });
  });

  it("refuses an editorial approve on a new file as the daemon does (400, a usage refusal), nothing staged", async () => {
    const client = mock();
    const read = await client.getProposal(SLUG, AMENDMENT);
    const refused = await client
      .stageDecision(SLUG, AMENDMENT, { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: null }, read.updated_at ?? "")
      .then(
        () => null,
        (error: unknown) => error,
      );
    expect(refused).toBeInstanceOf(ClientError);
    expect([(refused as ClientError).status, (refused as ClientError).message]).toEqual([
      400,
      "spec: --editorial: `PR-0052` writes no existing node, so it raises no revision; approve it without --editorial",
    ]);
    expect((await client.getProposal(SLUG, AMENDMENT)).staged).toBeNull();
  });

  it("refuses an editorial answer on the round alone: a question writes no existing node", async () => {
    const client = mock();
    const item = (await client.getRound(SLUG)).items.find((entry) => entry.id === "PR-0048");
    const outcome = await client.stageRoundAnswers(SLUG, [
      { id: "PR-0048", stage: { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: null, updated_at: item?.updated_at ?? "" } },
    ]);
    expect(outcome.answers.map(({ id, outcome: made, reason }) => [id, made, reason])).toEqual([
      ["PR-0048", "refused", "--editorial: `PR-0048` writes no existing node, so it raises no revision"],
    ]);
  });
});

describe("the editorial scenario on screen (UI-1, UI-4 of rev-discipline)", () => {
  it("shows Mock data with the scenario, PR-0053's staged editorial approve, its Editorial row and bare command, its revisions as sent", async () => {
    renderOver(mock(), `#/${SLUG}/inbox/${WORDING_FIX}`, "editorial");
    const card = await cardOf(WORDING_FIX);
    expect(screen.getByText("Mock data")).toBeTruthy();
    expect(screen.getByText("scenario: editorial")).toBeTruthy();
    const staged = await within(card).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByText(`spec approve ${WORDING_FIX}`).tagName).toBe("CODE");
    expect(staged.textContent).not.toContain("--editorial");
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Decision", "Accept"],
      ["Editorial", "Keep the revision"],
      ["Note", WORDING_NOTE],
    ]);
    expect(Array.from(sectionOf(card, "Revisions").querySelectorAll("li"), (item) => item.textContent)).toEqual([
      `${REWORDED.id} rev ${String(REWORDED.rev)} \u2192 ${String(REWORDED.to)} (1 lock entry)`,
    ]);
  });

  it("editorial_toggle_hidden_without_revisions: the file-form create PR-0052 offers no editorial toggle; the change PR-0042 does", async () => {
    renderOver(mock(), `#/${SLUG}/inbox/${AMENDMENT}`, "editorial");
    const amendment = await cardOf(AMENDMENT);
    expect(within(amendment).queryByRole("heading", { level: 3, name: "Revisions" })).toBeNull();
    fireEvent.click(within(amendment).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: `Accept ${AMENDMENT}` });
    expect(within(dialog).queryByRole("checkbox")).toBeNull();
    expect(within(dialog).queryByText(EDITORIAL)).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    window.location.hash = `#/${SLUG}/inbox/${DRAFT_CHANGE}`;
    const change = await cardOf(DRAFT_CHANGE);
    fireEvent.click(within(change).getByRole("button", { name: "Accept" }));
    const offered = await screen.findByRole("dialog", { name: `Accept ${DRAFT_CHANGE}` });
    expect(within(offered).getByRole<HTMLInputElement>("checkbox", { name: EDITORIAL }).checked).toBe(false);
  });

  it("editorial_toggle_shown_for_a_refused_raise: PR-0054 lists its raise as refused with the reason, and Accept offers the toggle, unchecked", async () => {
    renderOver(mock(), `#/${SLUG}/inbox/${DECISION_FIX}`, "editorial");
    const card = await cardOf(DECISION_FIX);
    const section = sectionOf(card, "Revisions");
    expect(
      Array.from(section.querySelectorAll(".revision-list > li"), (item) => [
        item.querySelector(".revision-line")?.textContent,
        item.querySelector(".revision-refused")?.textContent,
      ]),
    ).toEqual([[`${CLOSED_NODE.id} rev ${String(CLOSED_NODE.rev)} \u2192 refused (0 lock entries)`, CLOSED_REFUSAL]]);
    expect(section.textContent).not.toContain("null");
    expect(within(sectionOf(card, "Notes")).getByText(CLOSED_NOTE)).toBeTruthy();
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: `Accept ${DECISION_FIX}` });
    expect(within(dialog).queryByRole<HTMLInputElement>("checkbox", { name: EDITORIAL })?.checked).toBe(false);
  });

  it("shows ledger-api's unreadable lock as unknown counts and the daemon's note", async () => {
    renderOver(mock(), "#/ledger-api/inbox/PR-0008", "editorial");
    const card = await cardOf("PR-0008");
    expect(Array.from(sectionOf(card, "Revisions").querySelectorAll("li"), (item) => item.textContent)).toEqual([
      "EP-REFUND-CREATE rev 2 \u2192 3 (lock entries unknown)",
    ]);
    expect(within(sectionOf(card, "Notes")).getByText(UNREADABLE_LOCK_NOTE)).toBeTruthy();
  });
});

describe("the editorial approve's contract (UI-2 of rev-discipline)", () => {
  it("over the mock: Accept with Editorial stages editorial true; stored so; the card shows its Editorial row, the command bare", async () => {
    const client = mock();
    const stage = vi.spyOn(client, "stageDecision");
    const read = await client.getProposal(SLUG, DRAFT_CHANGE);
    renderOver(client, `#/${SLUG}/inbox/${DRAFT_CHANGE}`, "editorial");
    const staged = await stageEditorial();
    expect(stage.mock.calls).toEqual([[SLUG, DRAFT_CHANGE, { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: "typo" }, read.updated_at]]);
    expect((await client.getProposal(SLUG, DRAFT_CHANGE)).staged).toEqual({
      decision: "approve",
      option: null,
      answer: null,
      canon: null,
      editorial: true,
      note: "typo",
      span_hash: read.base_hash,
    });
    expect(staged.querySelector(".staged-line")?.textContent).toMatch(new RegExp(`Confirm on a terminal: spec approve ${DRAFT_CHANGE}$`));
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Decision", "Accept"],
      ["Editorial", "Keep the revision"],
      ["Note", "typo"],
    ]);
  });

  it("over HttpClient: one POST, its body exactly the 7-key stage less span_hash plus updated_at; the daemon's stage carries editorial, the card its row, the command bare", async () => {
    const served = mock();
    const posted = daemonOver(served);
    const read = await served.getProposal(SLUG, DRAFT_CHANGE);
    renderOver(new HttpClient(), `#/${SLUG}/inbox/${DRAFT_CHANGE}`, null);
    const staged = await stageEditorial();
    expect(screen.queryByText("Mock data")).toBeNull();
    expect(posted).toEqual([
      {
        url: `/api/projects/${SLUG}/proposals/${DRAFT_CHANGE}/decision`,
        body: `{"decision":"approve","option":null,"answer":null,"canon":null,"editorial":true,"note":"typo","updated_at":"${read.updated_at ?? ""}"}`,
      },
    ]);
    expect((await served.getProposal(SLUG, DRAFT_CHANGE)).staged).toMatchObject({ decision: "approve", editorial: true, note: "typo" });
    expect(staged.querySelector(".staged-line")?.textContent).toMatch(new RegExp(`Confirm on a terminal: spec approve ${DRAFT_CHANGE}$`));
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => dt.textContent)).toEqual(["Decision", "Editorial", "Note"]);
  });
});
