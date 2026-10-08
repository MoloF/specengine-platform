import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Interpretation, NodeView, Proposal } from "../api/types";
import { aNode, aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import appCss from "../styles/app.css?raw";

// AC-U2 of docs/features/kind-interpretation.md on a stub client: the card shows an
// "Interpretation" section exactly when the review document's `interpretation` is not null,
// whatever the kind: the decision linked to its node when the read finds it, then how the decision
// was applied and why interpretation was needed, the agent's texts verbatim (controls written
// `\u{..}`, line feeds and tabs kept, never markdown, each line its own direction). The policy test
// turns red on a kind comparison (src/policy.test.ts, "compares no role, profile or kind").

const RLO = "\u202e";

const INTERPRETATION: Interpretation = {
  decision: "DEC-0023",
  how: `Rest is any speed under 40 % of walking.\n**not bold** <b>not markup</b>\tthen ${RLO}reversed?`,
  why: "DEC-0023 says rest; the rule names no speed.",
};

/** The labels of the section, in order. */
const LABELS = ["Decision", "How the decision was applied", "Why interpretation was needed"];

/** An update on R-1 carrying `interpretation` unless told otherwise; `summary` null, as an update's. */
function review(fields: Partial<Omit<Proposal, "id">> = {}): Proposal {
  return aProposal({
    id: "PR-7",
    kind: "update",
    target_id: "R-1",
    target_ids: ["R-1"],
    summary: null,
    severity: null,
    rationale: "Carry DEC-0023 into R-1.",
    diff: "--- base a.md\n+++ proposed a.md\n@@ -1,1 +1,1 @@\n-Rest regenerates.\n+Rest, under 40 % of walking, regenerates.",
    interpretation: INTERPRETATION,
    ...fields,
  });
}

/** What the stub's node read answers: the decision's document by its ID, R-1 as the builder writes it. */
function decisionView(ref: string, found: boolean): NodeView {
  return found
    ? { ref, reason: null, notes: [], nodes: [aNode({ id: ref, kind: null, title: "Rest is slow movement", path: "docs/decisions/DEC-0023-rest.md" })] }
    : { ref, reason: `\`${ref}\` resolves to no ID and no alias`, notes: [], nodes: [] };
}

/** The Inbox on `proposal`, the decision found (or not) by the node read; the card once its review is read. */
async function openCard(proposal: Proposal, found = true) {
  const client = stubClient([proposal]);
  client.getNode.mockImplementation((_project, ref) =>
    Promise.resolve(ref === "R-1" ? { ref, reason: null, notes: [], nodes: [aNode({ id: ref })] } : decisionView(ref, found)),
  );
  renderApp(client, `#/alpha/inbox/${proposal.id ?? ""}`);
  const card = await screen.findByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  return { client, card };
}

/** The card's section under the heading `title`, or null. */
function sectionOf(card: HTMLElement, title: string): HTMLElement | null {
  return within(card).queryByRole("heading", { level: 3, name: title })?.closest("section") ?? null;
}

/** The Interpretation section once its chunk has loaded (before it, the heading over a skeleton). */
async function interpretationOf(card: HTMLElement): Promise<HTMLElement> {
  return waitFor(() => {
    const section = sectionOf(card, "Interpretation");
    if (section?.querySelector("dl.interpretation") == null) {
      throw new Error("no Interpretation section loaded");
    }
    return section;
  });
}

/** The section's values by label: each `dd`'s text paragraph (or its whole text). */
function valueOf(section: HTMLElement, label: string): HTMLElement {
  const term = Array.from(section.querySelectorAll("dt")).find((dt) => dt.textContent === label);
  const value = term?.nextElementSibling;
  if (!(value instanceof HTMLElement) || value.tagName !== "DD") {
    throw new Error(`no value for ${label}`);
  }
  return value;
}

describe("the Interpretation section (AC-U2 of kind-interpretation)", () => {
  it("shows the decision linked to its node with its title, then how and why, after the rationale", async () => {
    const { card } = await openCard(review());
    const section = await interpretationOf(card);
    expect(Array.from(section.querySelectorAll("dt"), (dt) => dt.textContent)).toEqual(LABELS);
    const link = await within(section).findByRole("link", { name: "DEC-0023: Rest is slow movement" });
    expect(link.getAttribute("href")).toBe("#/alpha/tree/DEC-0023");
    expect(valueOf(section, "Why interpretation was needed").textContent).toBe(INTERPRETATION.why);
    // Placed where the commit puts the texts: after the rationale, before the provenance and the diff.
    const titles = within(card)
      .getAllByRole("heading", { level: 3 })
      .map((heading) => heading.textContent);
    expect(titles.slice(titles.indexOf("Rationale"), titles.indexOf("Provenance") + 1)).toEqual(["Rationale", "Interpretation", "Provenance"]);
  });

  it("writes the agent's texts verbatim: controls as \\u{..}, line feeds and tabs kept, never markdown, each line its own direction", async () => {
    // `why` with a C0 control, a CR before its line feed, an override and an isolate.
    const why = `DEC-0023 says rest;\u0007 the rule\r\nnames ${RLO}no speed\u2066 _at all_.`;
    const { card } = await openCard(review({ interpretation: { ...INTERPRETATION, why } }));
    const section = await interpretationOf(card);
    const how = valueOf(section, "How the decision was applied");
    const text = how.querySelector("p.agent-text");
    expect(text?.textContent).toBe("Rest is any speed under 40 % of walking.\n**not bold** <b>not markup</b>\tthen \\u{202e}reversed?");
    expect(text?.getAttribute("dir")).toBe("auto");
    const reason = valueOf(section, "Why interpretation was needed");
    const whyText = reason.querySelector("p.agent-text");
    expect(whyText?.textContent).toBe("DEC-0023 says rest;\\u{7} the rule\\u{d}\nnames \\u{202e}no speed\\u{2066} _at all_.");
    expect(whyText?.getAttribute("dir")).toBe("auto");
    for (const value of [how, reason]) {
      expect(Array.from(value.textContent).filter((char) => /[\u0000-\u0008\u000b-\u001f\u202a-\u202e\u2066-\u2069]/.test(char))).toEqual([]);
      expect(value.querySelector("strong, b, em")).toBeNull();
    }
  });

  it.each([
    ["an interpretation kind with the key null", review({ kind: "interpretation", interpretation: null }), false],
    ["another kind with the key", review({ kind: "widget-change" }), true],
    ["an update without the key", review({ interpretation: null }), false],
  ])("follows the key, never the kind: %s", async (_case, proposal, shown) => {
    const { card } = await openCard(proposal);
    expect(sectionOf(card, "Interpretation") !== null).toBe(shown);
    expect(within(card).queryByText("How the decision was applied") !== null).toBe(shown);
  });

  it("a decision no node answers now: its ID unlinked and the read's reason, a stored control written \\u{..} in both, the reason its own direction", async () => {
    const decision = `DEC-0023${RLO}`;
    const { card } = await openCard(review({ interpretation: { ...INTERPRETATION, decision } }), false);
    const value = valueOf(await interpretationOf(card), "Decision");
    const reason = await within(value).findByText("`DEC-0023\\u{202e}` resolves to no ID and no alias");
    expect([reason.tagName, reason.getAttribute("dir")]).toEqual(["P", "auto"]);
    expect(within(value).queryByRole("link")).toBeNull();
    expect(value.querySelector("bdi")?.textContent).toBe("DEC-0023\\u{202e}");
    expect(value.textContent).not.toContain(RLO);
  });

  it("while the decision is read: its known ID in a busy block, no skeleton, no link yet; then the link", async () => {
    const client = stubClient([review()]);
    let answer: (view: NodeView) => void = () => undefined;
    client.getNode.mockImplementation((_project, ref) =>
      ref === "R-1"
        ? Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref })] })
        : new Promise<NodeView>((resolve) => {
            answer = resolve;
          }),
    );
    renderApp(client, "#/alpha/inbox/PR-7");
    const value = valueOf(await interpretationOf(await screen.findByRole("article")), "Decision");
    const block = value.querySelector(".decision-ref");
    expect([block?.getAttribute("aria-busy"), block?.textContent]).toEqual(["true", "DEC-0023"]);
    expect(value.querySelector(".skeleton")).toBeNull();
    expect(within(value).queryByRole("link")).toBeNull();
    answer(decisionView("DEC-0023", true));
    await within(value).findByRole("link", { name: "DEC-0023: Rest is slow movement" });
    expect(value.querySelector('[aria-busy="true"]')).toBeNull();
  });

  it("styles the agent's texts with the Questions screen's rule: one rule for both classes", () => {
    const at = appCss.indexOf("\n.round-text,\n.agent-text {");
    expect(at).toBeGreaterThan(0);
    expect(appCss.slice(at, appCss.indexOf("}", at))).toMatch(/white-space: pre-wrap;[\s\S]*overflow-wrap: anywhere;[\s\S]*unicode-bidi: plaintext;/);
    expect(appCss.match(/\.agent-text\b/g)).toHaveLength(1);
    expect(appCss.match(/\.round-text\b/g)).toHaveLength(1);
  });

  it("a failed read of the decision: the daemon's words and Retry; once read, the link, focus on it rather than the body", async () => {
    const message = "spec index unavailable: the database is locked by another process";
    const client = stubClient([review()]);
    let failing = true;
    client.getNode.mockImplementation((_project, ref) => {
      if (ref === "R-1") {
        return Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref })] });
      }
      return failing ? Promise.reject(new ClientError({ status: 503, message })) : Promise.resolve(decisionView(ref, true));
    });
    renderApp(client, "#/alpha/inbox/PR-7");
    const card = await screen.findByRole("article");
    const section = await interpretationOf(card);
    const alert = await within(section).findByRole("alert");
    expect(alert.textContent).toBe(`The decision DEC-0023 could not be read${message}`);
    expect(within(section).queryByRole("link")).toBeNull();
    failing = false;
    const retry = within(section).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    const link = await within(section).findByRole("link", { name: "DEC-0023: Rest is slow movement" });
    await waitFor(() => {
      expect(document.activeElement).toBe(link);
    });
    expect(within(section).queryByRole("alert")).toBeNull();
  });

  it("the stage dialog names the decision the change carries; a plain update's says nothing of one", async () => {
    const { card } = await openCard(review());
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-7" });
    expect(within(dialog).getByText(/^It interprets decision/).textContent).toBe(
      "It interprets decision DEC-0023; the card's Interpretation section says how and why.",
    );
  });

  it("a plain update's stage dialog names no decision", async () => {
    const { card } = await openCard(review({ interpretation: null }));
    fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog", { name: "Reject PR-7" });
    expect(within(dialog).queryByText(/interprets decision/)).toBeNull();
  });
});
