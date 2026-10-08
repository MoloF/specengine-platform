import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SpecEngineClient } from "../api/client";
import { HttpClient } from "../api/http";
import { App } from "../app/App";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch } from "../test/daemonStub";
import { CLOCK_INTERPRETATION, WEATHER_DECISION } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";

// AC-U3 of docs/features/kind-interpretation.md: harbor-sim holds one interpretation, PR-0049, and
// it renders alike over the mock and over HttpClient, the daemon a stubbed `fetch` answering each
// read with the mock's own document (as `spec serve` would send it, JSON on the wire).

/** The time the mock's fixtures are relative to: one clock for both renders. */
const NOW = Date.UTC(2026, 9, 8, 12, 0, 0);

const HASH = "#/harbor-sim/inbox/PR-0049";

const LINK_NAME = `${WEATHER_DECISION}: Weather is simulated`;

/** The daemon as a stubbed fetch: each read the client asks for, answered with `mock`'s document. */
function daemonOf(mock: MockClient) {
  return stubFetch(async (url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, await mock.getProjects());
    }
    const read = /^\/api\/projects\/([^/?]+)\/(inbox|proposals|nodes)(?:\/([^/?]+))?(?:\?(.*))?$/.exec(url);
    const [, project, route, ref, query] = read ?? [];
    if (project === undefined || route === undefined) {
      return errorAnswer(404, `no route ${url}`);
    }
    const slug = decodeURIComponent(project);
    if (route === "inbox") {
      return jsonAnswer(200, await mock.getInbox(slug));
    }
    if (ref === undefined) {
      return errorAnswer(404, `no route ${url}`);
    }
    if (route === "proposals") {
      return jsonAnswer(200, await mock.getProposal(slug, decodeURIComponent(ref)));
    }
    const params = new URLSearchParams(query ?? "");
    const view = await mock.getNode(slug, decodeURIComponent(ref), {
      ...(params.getAll("with").includes("links") ? { with: ["links" as const] } : {}),
      ...(params.get("archive") === "true" ? { archive: true } : {}),
    });
    return jsonAnswer(view.nodes.length === 0 ? 404 : 200, view);
  });
}

/** PR-0049's card over `client`; the Interpretation section once its decision is read, and a detached copy of it. */
async function interpretationOver(client: SpecEngineClient): Promise<{ copy: Node; section: HTMLElement }> {
  window.history.replaceState(null, "", `/${HASH}`);
  render(<App client={client} scenario={null} />);
  const card = await screen.findByRole("article");
  // The section's chunk loaded and its decision read: the link stands in it.
  const link = await within(card).findByRole("link", { name: LINK_NAME });
  const section = link.closest("section");
  if (!(section instanceof HTMLElement) || section.querySelector("h3")?.textContent !== "Interpretation") {
    throw new Error("the decision's link stands in no Interpretation section");
  }
  return { copy: section.cloneNode(true), section };
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("harbor-sim's interpretation (AC-U3 of kind-interpretation)", () => {
  it("holds one interpretation, PR-0049, and its decision as a node of the corpus", async () => {
    const mock = new MockClient("normal", { now: () => NOW });
    const { proposals } = await mock.getInbox("harbor-sim");
    const reviews = await Promise.all(proposals.map((entry) => mock.getProposal("harbor-sim", entry.id)));
    expect(reviews.filter((review) => review.interpretation !== null).map((review) => [review.id, review.interpretation])).toEqual([
      ["PR-0049", CLOCK_INTERPRETATION],
    ]);
    const decision = await mock.getNode("harbor-sim", WEATHER_DECISION);
    expect(decision.nodes.map((node) => [node.id, node.title, node.status])).toEqual([[WEATHER_DECISION, "Weather is simulated", "accepted"]]);
  });

  it("renders the section alike over the mock and over HttpClient: the same document, the same DOM", async () => {
    const mock = new MockClient("normal", { now: () => NOW, delayMs: 0 });
    const shown = await interpretationOver(mock);
    expect(within(shown.section).getByRole("link", { name: LINK_NAME }).getAttribute("href")).toBe(`#/harbor-sim/tree/${WEATHER_DECISION}`);
    expect(shown.section.querySelector("p.agent-text")?.textContent).toBe(CLOCK_INTERPRETATION.how);
    expect(screen.getByText("Mock data")).toBeTruthy();
    cleanup();

    const served = new MockClient("normal", { now: () => NOW, delayMs: 0 });
    daemonOf(served);
    const http = new HttpClient();
    expect(await http.getProposal("harbor-sim", "PR-0049")).toEqual(await served.getProposal("harbor-sim", "PR-0049"));
    const over = await interpretationOver(http);
    expect(screen.queryByText("Mock data")).toBeNull();
    expect(over.section.textContent).toBe(shown.copy.textContent);
    // Elements, attributes and text alike, node for node (DOM equality, no serialised markup).
    expect(over.section.isEqualNode(shown.copy)).toBe(true);
  });
});
