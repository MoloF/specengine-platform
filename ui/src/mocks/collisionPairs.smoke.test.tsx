import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { Evidence } from "../api/types";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import { GONE_TARGET, PAIR_AS_SERVED, PAIR_CARD, PAIR_EXAMPLE, PAIR_GONE, PAIR_SHARED, PAIR_THREE, SPRINT_FILE, STAMINA_FILE } from "./collisionPairs";

// docs/features/collision-pairs.md "Acceptance criteria", PP9, over the mock as `pnpm dev` shows it
// with `?scenario=collision-pairs` (src/mocks/collisionPairs.ts): the spec's example as two sections
// under "Cost" and "Regeneration" on the Inbox, the Questions screen and C-0001's Answer..., each heading
// opening the document modal; a three-target item's flat list as before; a served grouping no file
// match gives, drawn as served. The document modal a heading opens closes on Esc, focus back on the
// heading; in Answer... it stacks over the dialog. Agent text in a quote shows escaped on all three
// (docs/canon/round-ui.md "Screen"). The mock is imported here alone (policy AC-05).

const SLUG = "harbor-sim";

// The lazy chunks loaded once for the file, so a block never waits on its first transform.
beforeAll(async () => {
  await Promise.all([import("../board/CardItems"), import("../inbox/PairView"), import("../round/RoundScreen")]);
});

function renderMock(hash: string, client: MockClient = new MockClient("collision-pairs", { delayMs: 0 })) {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=collision-pairs${hash}`);
  render(<App client={client} scenario={null} />);
}

/** A bidi override and a bell in the code quote's path and in what it observed, as an agent may send them. */
function hostile(evidence: readonly Evidence[]): Evidence[] {
  return evidence.map((item, index) => (index === 2 ? { ...item, qpath: "stamina::\u202eregen", observed: "regen runs\u202e during\u0007 sprint" } : item));
}

/** The scenario's client, every item's code quote made hostile on its review and its round item alike. */
function hostileClient(): MockClient {
  const client = new MockClient("collision-pairs", { delayMs: 0 });
  const getProposal = client.getProposal.bind(client);
  vi.spyOn(client, "getProposal").mockImplementation(async (project, id) => {
    const review = await getProposal(project, id);
    return { ...review, evidence: hostile(review.evidence) };
  });
  const getRound = client.getRound.bind(client);
  vi.spyOn(client, "getRound").mockImplementation(async (project, options) => {
    const sheet = await getRound(project, options);
    return { ...sheet, items: sheet.items.map((item) => ({ ...item, evidence: hostile(item.evidence) })) };
  });
  return client;
}

/** The code quote in a scope, as shown: its code path and its two quotes, each written as the CLI prints it. */
function escapedQuote(scope: HTMLElement) {
  expect(scope.textContent).not.toMatch(/[\u0007\u202e]/u);
  const entry = within(scope).getByText("stamina::\\u{202e}regen").closest("li");
  expect(Array.from(entry?.querySelectorAll("dd") ?? [], (dd) => dd.textContent)).toEqual(["regen runs\\u{202e} during\\u{7} sprint", "a pause is specified"]);
}

/** Every dialog drawn, in the order their hosts stand in the page. */
function dialogs(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"]'));
}

/** Whether the element or one above it is inert, as a dialog stacked over it leaves it. */
function inert(element: HTMLElement): boolean {
  for (let node: HTMLElement | null = element; node !== null; node = node.parentElement) {
    if (node.inert) {
      return true;
    }
  }
  return false;
}

function hostOf(dialog: HTMLElement): HTMLElement {
  const host = dialog.closest<HTMLElement>("[data-dialog-host]");
  if (host === null) {
    throw new Error("no dialog host");
  }
  return host;
}

/**
 * Presses a side's heading: the document modal opens on `ref` (`?doc=` in the hash) over every dialog
 * drawn, each inert under it; Esc closes it alone, the top one live again, focus back on the heading.
 */
async function opensDoc(heading: HTMLElement, ref: string, under: readonly HTMLElement[] = []) {
  fireEvent.click(heading);
  await waitFor(() => {
    expect(window.location.hash).toContain(`doc=${ref}`);
  });
  const doc = await waitFor(() => {
    const opened = dialogs().filter((dialog) => !under.includes(dialog));
    expect(opened).toHaveLength(1);
    return opened[0] as HTMLElement;
  });
  await waitFor(() => {
    expect(doc.contains(document.activeElement)).toBe(true);
  });
  expect(inert(doc)).toBe(false);
  for (const dialog of under) {
    expect(inert(dialog)).toBe(true);
    expect(hostOf(dialog).compareDocumentPosition(hostOf(doc)) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  }
  fireEvent.keyDown(document.activeElement ?? doc, { key: "Escape" });
  await waitFor(() => {
    expect(document.body.contains(doc)).toBe(false);
  });
  expect(window.location.hash).not.toContain("doc=");
  expect(dialogs()).toEqual(under);
  // The dialog it stood over is live again; one under that stays inert beneath it.
  expect(under.map(inert)).toEqual(under.map((dialog) => dialog !== under.at(-1)));
  await waitFor(() => {
    expect(document.activeElement).toBe(heading);
  });
}

/** The region of a pair inside `scope`, once its chunk has landed. */
async function pairIn(scope: HTMLElement): Promise<HTMLElement> {
  return within(scope).findByRole("region", { name: "What conflicts with what" });
}

/** A side by its heading's name: the heading's section. */
function side(region: HTMLElement, name: string): HTMLElement {
  const heading = within(region).getByRole("heading", { level: 3, name });
  const section = heading.closest("section");
  if (section === null) {
    throw new Error(`no section under ${name}`);
  }
  return section;
}

/** The evidence entries' first lines (file and lines) in a scope, in order. */
function quotes(scope: HTMLElement): string[] {
  return Array.from(scope.querySelectorAll(".evidence > p:first-child"), (line) => line.textContent);
}

describe("the collision-pairs scenario's data (PP9)", () => {
  it("serves the example's pair after its evidence on the review and the round item alike; the three-target item none; `normal` holds none of the five", async () => {
    const client = new MockClient("collision-pairs", { delayMs: 0 });
    const review = await client.getProposal(SLUG, PAIR_EXAMPLE);
    const keys = Object.keys(review);
    expect(keys.slice(keys.indexOf("evidence"), keys.indexOf("evidence") + 2)).toEqual(["evidence", "pair"]);
    expect(review.pair).toEqual({
      left: { id: "RULE-SPRINT-COST", title: "Cost", path: SPRINT_FILE, evidence: [0] },
      right: { id: "RULE-STAM-REGEN", title: "Regeneration", path: STAMINA_FILE, evidence: [1] },
      other: [2],
    });
    const round = await client.getRound(SLUG);
    expect(round.items.find((item) => item.id === PAIR_EXAMPLE)?.pair).toEqual(review.pair);
    expect(round.items.find((item) => item.id === PAIR_THREE)?.pair).toBeNull();
    expect((await client.getProposal(SLUG, PAIR_GONE)).pair?.right).toEqual({ id: GONE_TARGET, title: null, path: null, evidence: [] });
    expect((await client.getProposal(SLUG, PAIR_SHARED)).pair).toMatchObject({ left: { evidence: [1] }, right: { evidence: [1] }, other: [0, 2] });
    const normal = new MockClient("normal", { delayMs: 0 });
    const ids = (await normal.getInbox(SLUG)).proposals.map((entry) => entry.id);
    expect([PAIR_EXAMPLE, PAIR_SHARED, PAIR_GONE, PAIR_THREE, PAIR_AS_SERVED].filter((id) => ids.includes(id))).toEqual([]);
  });
});

describe("the collision-pairs scenario on screen (PP9)", () => {
  it("the Inbox: two sections under Cost and Regeneration, each its own quote, the code's under Other evidence, no flat list; a heading opens the document modal", async () => {
    renderMock(`#/harbor-sim/inbox/${PAIR_EXAMPLE}`);
    const card = await screen.findByRole("article");
    const region = await pairIn(card);
    expect(quotes(side(region, "Cost"))).toEqual([`${SPRINT_FILE}:18-20`]);
    expect(quotes(side(region, "Regeneration"))).toEqual([`${STAMINA_FILE}:21-23`]);
    expect(quotes(side(region, "Other evidence"))).toEqual(["src/stamina.rs:40-52"]);
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(within(card).queryByRole("heading", { level: 3, name: "Evidence" })).toBeNull();
    await opensDoc(within(region).getByRole("button", { name: "Cost" }), "RULE-SPRINT-COST");
    await opensDoc(within(region).getByRole("button", { name: "Regeneration" }), "RULE-STAM-REGEN");
  });

  it("the Inbox: a bidi override and a control in a quote show escaped, in the pair and in a flat list alike", async () => {
    renderMock(`#/harbor-sim/inbox/${PAIR_EXAMPLE}`, hostileClient());
    escapedQuote(side(await pairIn(await screen.findByRole("article")), "Other evidence"));
    cleanup();
    renderMock(`#/harbor-sim/inbox/${PAIR_THREE}`, hostileClient());
    const card = await screen.findByRole("article");
    const evidence = await within(card).findByRole("heading", { level: 3, name: "Evidence" });
    escapedQuote(evidence.closest("section") ?? card);
  });

  it("the Inbox: a three-target item keeps the flat list and draws no pair", async () => {
    renderMock(`#/harbor-sim/inbox/${PAIR_THREE}`);
    const card = await screen.findByRole("article");
    const evidence = await within(card).findByRole("heading", { level: 3, name: "Evidence" });
    expect(quotes(evidence.closest("section") ?? card)).toEqual([`${SPRINT_FILE}:18-20`, `${STAMINA_FILE}:21-23`, "src/stamina.rs:40-52"]);
    expect(within(card).queryByRole("region", { name: "What conflicts with what" })).toBeNull();
  });

  it("the Inbox: a served grouping no file match gives is drawn as served (M: the page matching files itself)", async () => {
    renderMock(`#/harbor-sim/inbox/${PAIR_AS_SERVED}`);
    const region = await pairIn(await screen.findByRole("article"));
    const cost = side(region, "Cost");
    expect(quotes(cost)).toEqual([]);
    expect(within(cost).getByText("No evidence cites this file")).toBeTruthy();
    expect(quotes(side(region, "Other evidence"))).toEqual([`${SPRINT_FILE}:18-20`, "src/stamina.rs:40-52"]);
  });

  it("the Inbox: a gone side says so under its ID, its heading no button", async () => {
    renderMock(`#/harbor-sim/inbox/${PAIR_GONE}`);
    const region = await pairIn(await screen.findByRole("article"));
    const gone = side(region, GONE_TARGET);
    expect(within(gone).getByText("Not in the spec now")).toBeTruthy();
    expect(within(gone).queryByRole("button")).toBeNull();
    expect(within(region).getByRole("button", { name: "Cost" })).toBeTruthy();
  });

  it("Questions: the example's item lays the pair out after its text", async () => {
    renderMock("#/harbor-sim/questions");
    const article = await screen.findByRole("article", { name: PAIR_EXAMPLE });
    const region = await pairIn(article);
    expect(quotes(side(region, "Cost"))).toEqual([`${SPRINT_FILE}:18-20`]);
    expect(quotes(side(region, "Regeneration"))).toEqual([`${STAMINA_FILE}:21-23`]);
    expect(quotes(side(region, "Other evidence"))).toEqual(["src/stamina.rs:40-52"]);
    expect(within(screen.getByRole("article", { name: PAIR_THREE })).queryByRole("region", { name: "What conflicts with what" })).toBeNull();
  });

  it("Questions: each heading opens the document modal, Esc back to it", async () => {
    renderMock("#/harbor-sim/questions");
    const region = await pairIn(await screen.findByRole("article", { name: PAIR_EXAMPLE }));
    await opensDoc(within(region).getByRole("button", { name: "Cost" }), "RULE-SPRINT-COST");
    await opensDoc(within(region).getByRole("button", { name: "Regeneration" }), "RULE-STAM-REGEN");
  });

  it("Questions: a bidi override and a control in a quote show escaped", async () => {
    renderMock("#/harbor-sim/questions", hostileClient());
    escapedQuote(side(await pairIn(await screen.findByRole("article", { name: PAIR_EXAMPLE })), "Other evidence"));
  });

  /** C-0001's modal, then its Answer... on the example: both dialogs and the pair's region. */
  async function answerExample(client?: MockClient) {
    renderMock(`#/harbor-sim/board/${PAIR_CARD}`, client);
    const modal = await screen.findByRole("dialog", { name: "Write the tide table foreword" });
    const item = await within(modal).findByRole("article", { name: PAIR_EXAMPLE });
    fireEvent.click(within(item).getByRole("button", { name: "Answer\u2026" }));
    const dialog = await waitFor(() => {
      const all = dialogs();
      expect(all).toHaveLength(2);
      return all.at(-1) as HTMLElement;
    });
    return { modal, dialog, region: await pairIn(dialog) };
  }

  it("C-0001's Answer... on the example: the pair after its summary", async () => {
    const { region } = await answerExample();
    expect(quotes(side(region, "Cost"))).toEqual([`${SPRINT_FILE}:18-20`]);
    expect(quotes(side(region, "Regeneration"))).toEqual([`${STAMINA_FILE}:21-23`]);
    expect(quotes(side(region, "Other evidence"))).toEqual(["src/stamina.rs:40-52"]);
  });

  it("C-0001's Answer...: each heading opens the document modal over the dialog, Esc closing it alone, focus back on the heading", async () => {
    const { modal, dialog, region } = await answerExample();
    await opensDoc(within(region).getByRole("button", { name: "Cost" }), "RULE-SPRINT-COST", [modal, dialog]);
    await opensDoc(within(region).getByRole("button", { name: "Regeneration" }), "RULE-STAM-REGEN", [modal, dialog]);
  });

  it("C-0001's Answer...: a bidi override and a control in a quote show escaped", async () => {
    const { region } = await answerExample(hostileClient());
    escapedQuote(side(region, "Other evidence"));
  });
});
