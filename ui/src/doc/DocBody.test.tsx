import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { TreeView } from "../api/types";
import { ASIDE, asideClient } from "../test/asideStub";
import { anImplicitWorkspaces, aTreeNode, aTreeView } from "../test/builders";
import { docClient, FERRY, FERRY_FRONT_MATTER, FERRY_LINES, ferryWorkspaces } from "../test/docStub";
import { renderApp } from "../test/render";
import { argsOf } from "../test/stubClient";

// DM-12 and DM-13 of docs/features/doc-modal.md on the river ferry: the document as a work item, its
// sections in order; Inside read on its first expansion alone.

/** The ferry's modal over the Inbox, its body landed. */
async function ferry(client = docClient()) {
  renderApp(client, `#/alpha/inbox?doc=${FERRY}`);
  const dialog = await screen.findByRole("dialog", { name: "The ferry" });
  await within(dialog).findByText("A flat boat crosses the river on a cable.");
  return { client, dialog, shown: within(dialog) };
}

function insideButton(dialog: HTMLElement): HTMLElement {
  return within(dialog).getByRole("button", { name: "Inside" });
}

describe("the sections (DM-12 of doc-modal)", () => {
  it("heads with the path from the top: Project, the river, the ferry current with no href", async () => {
    const { shown } = await ferry();
    const crumbs = within(shown.getByRole("navigation", { name: "Breadcrumbs" }));
    expect(crumbs.getAllByRole("listitem").map((item) => item.textContent.replace("/", ""))).toEqual(["Alpha", "The river", "The ferry"]);
    expect(crumbs.getByRole("link", { name: "Alpha" }).getAttribute("href")).toBe("#/alpha");
    expect(crumbs.getByRole("link", { name: "The river" }).getAttribute("href")).toBe("#/alpha/tree/DOM-RIVER/page");
    const current = crumbs.getByText("The ferry").closest("[aria-current]");
    expect([current?.getAttribute("aria-current"), current?.closest("a")]).toEqual(["page", null]);
  });

  it("shows the work state as a lozenge with its label and icon, and the raw status neutral under lang=en", async () => {
    const { dialog } = await ferry();
    const meta = dialog.querySelector(".issue-view-meta");
    await waitFor(() => {
      expect(meta?.querySelectorAll(".lozenge")).toHaveLength(2);
    });
    const [state, status] = Array.from(meta?.querySelectorAll(".lozenge") ?? []);
    expect([state?.textContent, state?.querySelector("svg") !== null, state?.getAttribute("data-appearance")]).toEqual(["In work", true, "information"]);
    expect([status?.textContent, status?.getAttribute("data-appearance"), status?.querySelector(".lozenge-label")?.getAttribute("lang")]).toEqual(["accepted", "neutral", "en"]);
    expect(meta?.textContent).toBe("Work stateIn workStatusaccepted");
  });

  it("acts in order: New card, the one primary; Open in Flow under the last ancestor; Open in Spec tree; the tools Open as a page and Close", async () => {
    const { dialog, shown } = await ferry();
    const acts = Array.from(dialog.querySelectorAll(".doc-acts > *"));
    expect(acts.map((act) => act.textContent)).toEqual(["New task", "Open in Flow", "Open in Spec tree"]);
    expect(dialog.querySelectorAll(".button-primary")).toHaveLength(1);
    expect(acts[0]?.className).toContain("button-primary");
    expect(shown.getByRole("link", { name: "Open in Flow" }).getAttribute("href")).toBe("#/alpha/flow/DOM-FERRY?scope=DOM-RIVER");
    expect(shown.getByRole("link", { name: "Open in Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/DOM-FERRY");
    const tools = dialog.querySelector(".issue-view-tools");
    expect(within(tools as HTMLElement).getByRole("link", { name: "Open as a page" }).getAttribute("href")).toBe("#/alpha/tree/DOM-FERRY/page");
    expect(within(tools as HTMLElement).getByRole("button", { name: /^Close/ })).toBeTruthy();
    fireEvent.click(shown.getByRole("button", { name: "New task" }));
    const card = await screen.findByRole("dialog", { name: "New task" });
    expect(within(card).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe(FERRY);
  });

  it("gives the text at reading width without its front-matter or a second title; both paragraphs and the section; the AI line after (M: front-matter in the body)", async () => {
    const { dialog } = await ferry();
    const text = dialog.querySelector<HTMLElement>(".doc-text");
    expect(text).not.toBeNull();
    await waitFor(() => {
      expect(text?.querySelectorAll("p")).toHaveLength(3);
    });
    expect(text?.textContent).not.toContain("id: DOM-FERRY");
    expect(text?.querySelector(".md-front-matter")).toBeNull();
    // The text's own headings nest under Text's (h3 in the modal): one level below, h4.
    expect(Array.from(text?.querySelectorAll(".md-heading") ?? [], (heading) => [heading.tagName, heading.textContent])).toEqual([["H4", "Tides RULE-FERRY-TIDES"]]);
    expect(Array.from(text?.querySelectorAll("p") ?? [], (paragraph) => paragraph.textContent)).toEqual(["A flat boat crosses the river on a cable.", "It runs from dawn to dusk.", "The ferry waits for slack water."]);
    expect(within(dialog).getAllByRole("heading", { name: "The ferry" })).toHaveLength(1);
    expect(text?.closest("section")?.querySelector(":scope > p.muted")?.textContent).toBe("Changes go through your AI, as proposals.");
  });

  it("lists the links by their ends' titles: Out, In, the type raw under lang=en; Mentions collapsed; the dangling one under Technical details alone (M: links by name)", async () => {
    const { shown } = await ferry();
    const links = within(shown.getByRole("region", { name: "Links" }));
    const out = within(links.getByRole("group", { name: "Links out" }));
    const boarding = out.getByRole("link", { name: "Boarding" });
    expect(boarding.getAttribute("href")).toBe("#/alpha/tree/MEC-BOARDING/page");
    const type = boarding.closest("li")?.querySelector(".lozenge");
    expect([type?.textContent, type?.querySelector(".lozenge-label")?.getAttribute("lang"), type?.getAttribute("data-appearance")]).toEqual(["uses", "en", "neutral"]);
    expect(out.queryByText("MEC-GANGWAY")).toBeNull();
    const into = within(links.getByRole("group", { name: "Links in" }));
    expect(into.getByRole("link", { name: "Landing" }).closest("li")?.textContent).toBe("followsLanding");
    const mentions = links.getByRole("button", { name: "Mentions (1)" });
    expect(mentions.getAttribute("aria-expanded")).toBe("false");
    expect(document.getElementById(mentions.getAttribute("aria-controls") ?? "")?.hidden).toBe(true);
    expect(links.getByRole("link", { name: "The river", hidden: true })).toBeTruthy();
    expect(shown.getByRole("region", { name: "Links" }).textContent).not.toContain("MEC-GANGWAY");
  });

  it("lists the cards with no New card of their own", async () => {
    const { shown } = await ferry();
    const cards = within(shown.getByRole("region", { name: "Tasks" }));
    expect(await cards.findByRole("link", { name: /C-0007/ })).toBeTruthy();
    expect(cards.queryByRole("button", { name: "New task" })).toBeNull();
  });

  it("folds Technical details, collapsed; open: the ID with Copy, kind, place, rev, tokens, both hashes, the front-matter, the unresolved, Open in Graph (M: open by default)", async () => {
    const { dialog, shown } = await ferry();
    const button = shown.getByRole("button", { name: "Technical details" });
    expect(button.getAttribute("aria-expanded")).toBe("false");
    const region = document.getElementById(button.getAttribute("aria-controls") ?? "") as HTMLElement;
    expect(region.hidden).toBe(true);
    fireEvent.click(button);
    expect(region.hidden).toBe(false);
    const facts = within(region);
    expect(facts.getByRole("button", { name: /^Copy/ })).toBeTruthy();
    expect(region.querySelector(".doc-technical-id .mono")?.textContent).toBe(FERRY);
    expect(facts.getByText("domain").getAttribute("lang")).toBe("en");
    expect(facts.getByText(`docs/ferry/ferry.md:1-${String(FERRY_LINES.length)}`)).toBeTruthy();
    expect(facts.getByText("3")).toBeTruthy();
    expect(facts.getByText("42 tokens")).toBeTruthy();
    expect(region.textContent).toContain("b3:0000000000000000000000000000000000000000000000000000000000000000");
    expect(region.textContent).toContain("b3:1111111111111111111111111111111111111111111111111111111111111111");
    expect(region.querySelector(".doc-front-matter")?.textContent).toBe(FERRY_FRONT_MATTER);
    expect(facts.getByText("1 link does not resolve")).toBeTruthy();
    expect(facts.getByText("uses MEC-GANGWAY").getAttribute("lang")).toBe("en");
    expect(facts.getByRole("link", { name: "Open in Graph" }).getAttribute("href")).toBe("#/alpha/graph/DOM-FERRY");
    expect(dialog.querySelectorAll(".doc-technical")).toHaveLength(1);
  });
});

describe("Inside (DM-13 of doc-modal)", () => {
  it("reads nothing until expanded, then one rooted read two deep; a second expansion reads nothing (M: the read made on open)", async () => {
    const { client, dialog } = await ferry();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(client.getTree).not.toHaveBeenCalled();
    fireEvent.click(insideButton(dialog));
    await within(dialog).findByRole("heading", { name: "Directly under this document" });
    expect(argsOf(client.getTree)).toEqual([["alpha", { root: FERRY, depth: 2 }]]);
    fireEvent.click(insideButton(dialog));
    fireEvent.click(insideButton(dialog));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(client.getTree).toHaveBeenCalledTimes(1);
  });

  it("groups by holder, the document first, then by raw kind; the ID-less counted for the tree; the grandchild absent", async () => {
    // No direction served: the raw kinds alone, as before departments.
    const client = docClient();
    client.getWorkspaces.mockResolvedValue(anImplicitWorkspaces());
    const { dialog } = await ferry(client);
    fireEvent.click(insideButton(dialog));
    const direct = (await within(dialog).findByRole("heading", { name: "Directly under this document" })).closest("section") as HTMLElement;
    const under = within(dialog).getByRole("heading", { name: "Under Tides" }).closest("section") as HTMLElement;
    expect(direct.compareDocumentPosition(under) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(within(direct).getByRole("group", { name: "mechanic" })).getAllByRole("link").map((link) => link.textContent)).toEqual(["Boarding"]);
    expect(within(within(under).getByRole("group", { name: "assumption" })).getAllByRole("link").map((link) => link.textContent)).toEqual(["The tide table is right", "Fog stops the ferry"]);
    expect(within(within(under).getByRole("group", { name: "question" })).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([["Who rings the bell?", "#/alpha/tree/QST-0001/page"]]);
    expect(within(under).getByText("assumption").closest("[lang]")?.getAttribute("lang")).toBe("en");
    const noId = within(dialog).getByRole("link", { name: "1 document without an ID is in the Spec tree" });
    expect(noId.getAttribute("href")).toBe("#/alpha/tree/DOM-FERRY");
    expect(within(dialog).queryByText("A plank is enough")).toBeNull();
  });

  it("keeps the choice for the project's next document: opened there, it reads that one", async () => {
    const { client, dialog } = await ferry();
    fireEvent.click(insideButton(dialog));
    await within(dialog).findByRole("heading", { name: "Directly under this document" });
    fireEvent.click(within(within(dialog).getByRole("region", { name: "Links" })).getByRole("link", { name: "Boarding" }));
    const other = await screen.findByRole("dialog", { name: "Boarding" });
    await within(other).findByText("The boarding as written.");
    expect(insideButton(other).getAttribute("aria-expanded")).toBe("true");
    await waitFor(() => {
      expect(argsOf(client.getTree)).toEqual([
        ["alpha", { root: FERRY, depth: 2 }],
        ["alpha", { root: "MEC-BOARDING", depth: 2 }],
      ]);
    });
    expect(await within(other).findByText("Nothing is listed under this document.")).toBeTruthy();
  });

  it("shows a skeleton while read, the daemon's words and Retry on a failure, the empty line for nothing", async () => {
    const client = docClient();
    let land: (tree: TreeView) => void = () => undefined;
    client.getTree.mockImplementationOnce(() => new Promise((resolve) => {
      land = resolve;
    }));
    const { dialog } = await ferry(client);
    fireEvent.click(insideButton(dialog));
    expect(await within(dialog).findByRole("status", { name: "Reading what is inside…" })).toBeTruthy();
    act(() => {
      land(aTreeView([aTreeNode({ id: FERRY, depth: 0, path: "docs/ferry/ferry.md" })], { ref: FERRY }));
    });
    expect(await within(dialog).findByText("Nothing is listed under this document.")).toBeTruthy();
  });

  it("says a failed read in the daemon's words under lang=en, and Retry reads again", async () => {
    const client = docClient();
    client.getTree.mockRejectedValueOnce(new ClientError({ status: 503, message: "the index is being rebuilt" }));
    const { dialog } = await ferry(client);
    fireEvent.click(insideButton(dialog));
    expect(await within(dialog).findByText("What is inside could not be read.")).toBeTruthy();
    expect(within(dialog).getByText("the index is being rebuilt").getAttribute("lang")).toBe("en");
    fireEvent.click(within(dialog).getByRole("button", { name: "Retry" }));
    expect(await within(dialog).findByRole("heading", { name: "Directly under this document" })).toBeTruthy();
    expect(client.getTree).toHaveBeenCalledTimes(2);
  });

  it("shows the kiln bay's hunches 50 at a time: Show all 60 takes focus to the 51st", async () => {
    renderApp(asideClient(), `#/${ASIDE}?doc=WBAY-KILN`);
    const dialog = await screen.findByRole("dialog", { name: "Kiln bay" });
    fireEvent.click(await within(dialog).findByRole("button", { name: "Inside" }));
    const hunch = within(await within(dialog).findByRole("group", { name: "hunch" }));
    expect(hunch.getAllByRole("link")).toHaveLength(50);
    expect(within(within(dialog).getByRole("group", { name: "edict" })).getAllByRole("link")).toHaveLength(32);
    expect(within(within(dialog).getByRole("group", { name: "conerule" })).getAllByRole("link").map((link) => link.textContent)).toEqual(["Bisque cone", "Glost cone"]);
    expect(within(dialog).queryByText("The glost shelf")).toBeNull();
    fireEvent.click(hunch.getByRole("button", { name: "Show all 60" }));
    await waitFor(() => {
      expect(hunch.getAllByRole("link")).toHaveLength(60);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(hunch.getAllByRole("link")[50]);
    });
  });
});

describe("Inside by direction (DP-13 of departments)", () => {
  /** A holder's sets as their names and their links' titles, in order. */
  function sets(section: HTMLElement): [string, string[]][] {
    return within(section)
      .getAllByRole("group")
      .map((group) => [document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent ?? "", within(group).getAllByRole("link").map((link) => link.textContent)]);
  }

  it("groups as the stub says: raw mechanic under the document; Records, then Asked, under Tides, each labelled by its title in a bdi with no lang; a raw kind keeps lang=en (M: the last matching direction; directions after raw kinds)", async () => {
    const { dialog } = await ferry();
    fireEvent.click(insideButton(dialog));
    const direct = (await within(dialog).findByRole("heading", { name: "Directly under this document" })).closest("section") as HTMLElement;
    const under = within(dialog).getByRole("heading", { name: "Under Tides" }).closest("section") as HTMLElement;
    await waitFor(() => {
      expect(sets(under)).toEqual([
        ["Records", ["The tide table is right", "Fog stops the ferry"]],
        ["Asked", ["Who rings the bell?"]],
      ]);
    });
    expect(sets(direct)).toEqual([["mechanic", ["Boarding"]]]);
    const records = within(under).getByRole("group", { name: "Records" });
    const label = document.getElementById(records.getAttribute("aria-labelledby") ?? "") as HTMLElement;
    // The project's words carry no lang: neither the label nor anything between it and the dialog.
    expect([label.querySelector("bdi")?.textContent, label.querySelector("[lang]"), label.closest("[lang]") === document.documentElement]).toEqual(["Records", null, true]);
    const mechanic = document.getElementById(within(direct).getByRole("group", { name: "mechanic" }).getAttribute("aria-labelledby") ?? "") as HTMLElement;
    expect(mechanic.getAttribute("lang")).toBe("en");
    expect(within(under).queryByRole("group", { name: "assumption" })).toBeNull();
  });

  it("expanding makes one getTree call and no getWorkspaces call beyond the Shell's", async () => {
    const { client, dialog } = await ferry();
    await waitFor(() => {
      expect(client.getWorkspaces).toHaveBeenCalled();
    });
    const shells = client.getWorkspaces.mock.calls.length;
    fireEvent.click(insideButton(dialog));
    await within(dialog).findByRole("group", { name: "Records" });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(argsOf(client.getTree)).toEqual([["alpha", { root: FERRY, depth: 2 }]]);
    expect(client.getWorkspaces).toHaveBeenCalledTimes(shells);
  });

  it("with the workspaces read failed, the groups are the raw kinds at once", async () => {
    const client = docClient();
    client.getWorkspaces.mockRejectedValue(new ClientError({ status: 503, message: "the config could not be read" }));
    const { dialog } = await ferry(client);
    fireEvent.click(insideButton(dialog));
    const under = (await within(dialog).findByRole("heading", { name: "Under Tides" })).closest("section") as HTMLElement;
    expect(sets(under)).toEqual([
      ["assumption", ["The tide table is right", "Fog stops the ferry"]],
      ["question", ["Who rings the bell?"]],
    ]);
  });

  it("with the workspaces read pending, the groups are the raw kinds at once, never waiting; directions apply once it lands", async () => {
    const client = docClient();
    let land: () => void = () => undefined;
    client.getWorkspaces.mockImplementation(
      () =>
        new Promise((resolve) => {
          land = () => {
            resolve(ferryWorkspaces());
          };
        }),
    );
    const { dialog } = await ferry(client);
    fireEvent.click(insideButton(dialog));
    const under = (await within(dialog).findByRole("heading", { name: "Under Tides" })).closest("section") as HTMLElement;
    expect(sets(under).map(([name]) => name)).toEqual(["assumption", "question"]);
    act(() => {
      land();
    });
    await waitFor(() => {
      expect(sets(under).map(([name]) => name)).toEqual(["Records", "Asked"]);
    });
  });
});
