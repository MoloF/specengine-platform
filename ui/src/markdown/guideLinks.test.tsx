import { render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { ApiProvider, createQueryClient } from "../api/provider";
import { queryKeys } from "../api/queries";
import type { Card } from "../api/types";
import { guideHash } from "../app/routes";
import { aCard, aCardList, aCardRow, aGuideIndex, aGuideTopic } from "../test/builders";
import { GuideShelf } from "../guide/GuideShelf";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { Markdown, type HeadingAnchors } from "./Markdown";

// docs/features/specengine-guide.md "The UI", GU14: a `guide:<topic>[#<ID>]` citation in served markdown
// and in a card's plain texts links to the topic's route only when the guide's served index holds the
// topic; one of an unknown topic, one in code and one with no index stay text; a text holding no
// `guide:` never reads the index. A rule heading the holder names is an anchor with its tool. M:
// linking an unknown topic turns "links a served topic alone" red.

/** A stub whose guide serves these topics. */
function guideClient(topics: string[] = ["records", "cards"]): StubClient {
  const stub = stubClient();
  stub.getGuide.mockResolvedValue(aGuideIndex(topics.map((topic) => aGuideTopic(topic))));
  return stub;
}

function show(stub: StubClient, source: string, anchors: HeadingAnchors | null = null) {
  const { container } = render(
    <ApiProvider client={stub}>
      <Markdown source={source} links={null} project="alpha" baseLevel={1} frontMatter={false} anchors={anchors} />
    </ApiProvider>,
  );
  const root = container.querySelector<HTMLElement>(".markdown");
  if (root === null) {
    throw new Error("no .markdown");
  }
  return root;
}

/** Every link in a scope: its words and where it goes. */
function links(scope: HTMLElement): [string, string][] {
  return within(scope)
    .queryAllByRole("link")
    .map((link) => [link.textContent, link.getAttribute("href") ?? ""]);
}

describe("guide citations in served markdown (GU14 of specengine-guide)", () => {
  it("links a served topic alone, at its rule; an unknown topic, one in code and an escaped one stay text (M: linking an unknown topic)", async () => {
    const stub = guideClient();
    const root = show(stub, "Hand off as guide:cards#GD-CARDS-HANDOFF says; see guide:records.\n\nNot guide:nope#GD-NOPE-X, not `guide:cards`, not guide\\:cards.\n");
    await waitFor(() => {
      expect(links(root)).toEqual([
        ["guide:cards#GD-CARDS-HANDOFF", guideHash("alpha", "cards", "GD-CARDS-HANDOFF")],
        ["guide:records", guideHash("alpha", "records")],
      ]);
    });
    expect(root.textContent).toContain("Not guide:nope#GD-NOPE-X, not guide:cards, not guide:cards.");
    expect(stub.getGuide).toHaveBeenCalledTimes(1);
  });

  it("links nothing while the index is out, nor when it failed (an older daemon's 404): the text stays whole", async () => {
    const stub = stubClient();
    stub.getGuide.mockRejectedValue(new ClientError({ status: 404, message: "no route /api/guide: the routes are /api/projects and the web UI" }));
    const root = show(stub, "See guide:records#GD-RECORDS-ONE-FILE.\n");
    await waitFor(() => {
      expect(stub.getGuide).toHaveBeenCalledTimes(1);
    });
    expect(links(root)).toEqual([]);
    expect(root.textContent).toBe("See guide:records#GD-RECORDS-ONE-FILE.");
  });

  it("keeps the index for the page's life, and a citation reader never asks a failed one again on mount: an older daemon is asked once however many texts cite; the shelf's mount asks again", async () => {
    const stub = stubClient();
    stub.getGuide.mockRejectedValue(new ClientError({ status: 404, message: "no route /api/guide: the routes are /api/projects and the web UI" }));
    const queryClient = createQueryClient();
    const tree = (...children: ReactNode[]) => (
      <ApiProvider client={stub} queryClient={queryClient}>
        {children}
      </ApiProvider>
    );
    const text = (key: string) => <Markdown key={key} source={`See guide:records (${key}).\n`} links={null} project="alpha" baseLevel={1} frontMatter={false} />;
    const { rerender } = render(tree(text("a")));
    await waitFor(() => {
      expect(stub.getGuide).toHaveBeenCalledTimes(1);
    });
    rerender(tree(text("a"), text("b"), text("c")));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(stub.getGuide).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryCache().find({ queryKey: queryKeys.guide })?.options.gcTime).toBe(Infinity);
    rerender(tree(text("a"), <GuideShelf key="shelf" project="alpha" />));
    await waitFor(() => {
      expect(stub.getGuide).toHaveBeenCalledTimes(2);
    });
  });

  it("never reads the index for a text holding no `guide:`", () => {
    const stub = guideClient();
    show(stub, "A plain text about guides and records.\n");
    expect(stub.getGuide).not.toHaveBeenCalled();
  });
});

describe("a holder's anchors (GU14 of specengine-guide)", () => {
  it("makes a listed rule's heading an anchor, its ID its id, its tool after it; the wanted one handed back once drawn; an unlisted `{#ID}` stays a plain heading", () => {
    const reached: HTMLElement[] = [];
    const anchors: HeadingAnchors = {
      ids: ["GD-A"],
      tool: (id) => <button type="button">{`tool ${id}`}</button>,
      wanted: "GD-A",
      onReach: (heading) => {
        reached.push(heading);
      },
    };
    const root = show(guideClient(), "## Part\n\n### First rule {#GD-A}\n\nWhy.\n\n### Example {#PRA-X}\n\nText.\n", anchors);
    const rule = root.querySelector<HTMLElement>("#GD-A");
    expect(rule?.tagName).toBe("H3");
    expect(rule?.getAttribute("tabindex")).toBe("-1");
    expect(rule?.nextElementSibling?.textContent).toBe("tool GD-A");
    expect(reached).toEqual([rule]);
    expect(root.querySelector("#PRA-X")).toBeNull();
    expect(root.querySelectorAll("button")).toHaveLength(1);
    // No holder's anchors: no id, no tool.
    const plain = show(guideClient(), "### First rule {#GD-A}\n");
    expect(plain.querySelector("[id]")).toBeNull();
  });
});

describe("guide citations in a card's goal (GU14 of specengine-guide)", () => {
  const CARD: Card = aCard({
    id: "C-0001",
    title: "Caption the plans",
    goal: "Hand back as guide:cards#GD-CARDS-HANDOFF asks; guide:charts is no topic, `guide:cards` is code.",
    criteria: ["Per guide:records"],
    cited: [],
  });

  function client(): StubClient {
    const stub = guideClient();
    stub.getCards.mockResolvedValue(aCardList([aCardRow({ id: CARD.id, title: CARD.title })]));
    stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === CARD.id ? structuredClone(CARD) : { id, reason: `no card ${id}` }));
    return stub;
  }

  it("links the served topics only, at the rule, from the goal and a criterion (M: linking an unknown topic)", async () => {
    const stub = client();
    renderApp(stub, "#/alpha/board/C-0001");
    const dialog = await screen.findByRole("dialog", { name: CARD.title });
    const link = await within(dialog).findByRole("link", { name: "guide:cards#GD-CARDS-HANDOFF" });
    expect(link.getAttribute("href")).toBe(guideHash("alpha", "cards", "GD-CARDS-HANDOFF"));
    expect(within(dialog).getByRole("link", { name: "guide:records" }).getAttribute("href")).toBe(guideHash("alpha", "records"));
    expect(within(dialog).queryByRole("link", { name: /charts/ })).toBeNull();
    expect(within(dialog).queryAllByRole("link", { name: "guide:cards" })).toEqual([]);
    expect(dialog.textContent).toContain("guide:charts is no topic, `guide:cards` is code.");
  });

  it("a click on the citation opens the guide's topic at the rule", async () => {
    const stub = client();
    renderApp(stub, "#/alpha/board/C-0001");
    const dialog = await screen.findByRole("dialog", { name: CARD.title });
    const link = await within(dialog).findByRole("link", { name: "guide:cards#GD-CARDS-HANDOFF" });
    link.click();
    await waitFor(() => {
      expect(window.location.hash).toBe(guideHash("alpha", "cards", "GD-CARDS-HANDOFF"));
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Title of cards" })).toBeTruthy();
    expect(stub.getGuideTopic).toHaveBeenCalledWith("cards", expect.any(AbortSignal));
  });
});
