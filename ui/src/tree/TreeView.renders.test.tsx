import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { createElement, type ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import type { QueueEvent } from "../api/types";
import { aDriftRow, aHealthReport, measuredDrift } from "../test/builders";
import { renderApp } from "../test/render";
import { treeClient } from "../test/treeStub";
import type * as NodePaneModule from "./NodePane";
import type * as RowContentModule from "./RowContent";
import type * as SpecTreeModule from "./SpecTree";
import { argsOf } from "../test/stubClient";

// docs/features/ui-tree-node.md AC-11 (search "never per keystroke"): a keystroke in the search
// field renders the field alone. The tree and the node pane are wrapped to count how often the
// view renders them; the wrappers render the real components. docs/features/ui-tree-drift.md UI-5:
// a health answer renders again only the tree lines whose drift counts changed, and with none
// changed not the tree at all: every prop the view gives it the same (the same map: drift.ts
// `treeDrift`), so its memo skips it; each line's content
// (`RowContent.tsx`) is wrapped to record whose it rendered, and the tree memoises the wrapper as it
// does the real one (SpecTree.tsx).

const renders = vi.hoisted(() => ({ tree: 0, node: 0, lines: [] as string[] }));

/** The props the view last gave the tree: the memoised tree renders again only when one of them changes. */
const given: { tree: Readonly<Record<string, unknown>> } = vi.hoisted(() => ({ tree: {} }));

vi.mock("./SpecTree", async (importOriginal) => {
  const actual = await importOriginal<typeof SpecTreeModule>();
  function CountedSpecTree(props: ComponentProps<typeof actual.SpecTree>) {
    renders.tree += 1;
    given.tree = { ...props };
    return createElement(actual.SpecTree, props);
  }
  return { ...actual, SpecTree: CountedSpecTree };
});

vi.mock("./RowContent", async (importOriginal) => {
  const actual = await importOriginal<typeof RowContentModule>();
  function CountedRowContent(props: ComponentProps<typeof actual.RowContent>) {
    renders.lines.push(props.node.id ?? props.node.path);
    return createElement(actual.RowContent, props);
  }
  return { ...actual, RowContent: CountedRowContent };
});

vi.mock("./NodePane", async (importOriginal) => {
  const actual = await importOriginal<typeof NodePaneModule>();
  function CountedNodePane(props: ComponentProps<typeof actual.NodePane>) {
    renders.node += 1;
    return createElement(actual.NodePane, props);
  }
  return { ...actual, NodePane: CountedNodePane };
});

describe("the search field's keystrokes (AC-11)", () => {
  it("render neither the tree nor the node pane; a submit still searches what was typed", async () => {
    const client = treeClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    const tree = await screen.findByRole("tree", { name: "Spec tree" });
    await within(tree).findAllByRole("treeitem");
    await screen.findByRole("tab", { name: "Text" });
    // Every read on entering has answered and been rendered: the counts hold still between two
    // looks (a fixed tick could take the snapshot before a late answer's render).
    let seen = "";
    await waitFor(() => {
      const now = JSON.stringify([renders.tree, renders.node]);
      const still = now === seen;
      seen = now;
      expect(still).toBe(true);
    });
    expect(renders.tree).toBeGreaterThan(0);
    expect(renders.node).toBeGreaterThan(0);
    const before = { tree: renders.tree, node: renders.node };
    const field = screen.getByLabelText<HTMLInputElement>("Search the spec");
    for (const value of ["w", "wi", "wid", "widg", "widge", "widget"]) {
      fireEvent.change(field, { target: { value } });
    }
    expect(field.value).toBe("widget");
    expect({ tree: renders.tree, node: renders.node }).toEqual(before);
    fireEvent.submit(screen.getByRole("search"));
    expect(await screen.findByText(/No node matches/)).toBeTruthy();
    expect(argsOf(client.search)).toContainEqual(["alpha", { query: "widget" }]);
  });
});

/** Waits until the counts hold still between two looks: every answer so far rendered. */
async function stillRenders() {
  let seen = "";
  await waitFor(() => {
    const now = JSON.stringify(renders);
    const still = now === seen;
    seen = now;
    expect(still).toBe(true);
  });
}

describe("a health answer under Show drift (UI-5 of ui-tree-drift)", () => {
  it("renders again only the lines whose counts changed", async () => {
    const client = treeClient();
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = (_project, onEvent) => {
      emit = onEvent;
      return () => undefined;
    };
    const first = [aDriftRow({ node: "DOC-A", class: "fix" }), aDriftRow({ node: "SEC-A1", class: "check" }), aDriftRow({ node: "DOC-C", class: "cannot" })];
    const second = [...first, aDriftRow({ node: "SEC-A1", class: "check", relation: "verifies" })];
    client.getHealth
      .mockResolvedValueOnce(aHealthReport([], { drift: measuredDrift(first) }))
      .mockResolvedValue(aHealthReport([], { drift: measuredDrift(second) }));
    renderApp(client, "#/alpha/tree");
    const tree = await screen.findByRole("tree", { name: "Spec tree" });
    fireEvent.click(screen.getByRole("checkbox", { name: "Show drift" }));
    await within(tree).findByText("Check 1");
    await stillRenders();
    renders.lines = [];
    // An approval reads Health again while the tree shows its drift (useLiveQueue); SEC-A1 gains a row.
    act(() => {
      emit({ seq: 1, type: "proposal.approved", payload: { id: "PR-1" } });
    });
    await within(tree).findByText("Check 2");
    await stillRenders();
    expect(client.getHealth).toHaveBeenCalledTimes(2);
    expect(within(tree).getByText("Fix 1")).toBeTruthy();
    expect(within(tree).getByText("Cannot verify 1")).toBeTruthy();
    expect(renders.lines).toEqual(["SEC-A1"]);
  });

  it("with every line's counts unchanged renders neither the tree nor a line again", async () => {
    const client = treeClient();
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = (_project, onEvent) => {
      emit = onEvent;
      return () => undefined;
    };
    const first = [aDriftRow({ node: "DOC-A", class: "fix" }), aDriftRow({ node: "SEC-A1", class: "check" })];
    // Other rows, the same counts per line: a new answer that changes no badge.
    const second = [aDriftRow({ node: "DOC-A", class: "fix", detail: "body" }), aDriftRow({ node: "SEC-A1", class: "check", relation: "verifies" })];
    client.getHealth
      .mockResolvedValueOnce(aHealthReport([], { drift: measuredDrift(first) }))
      .mockResolvedValue(aHealthReport([], { drift: measuredDrift(second) }));
    renderApp(client, "#/alpha/tree");
    const tree = await screen.findByRole("tree", { name: "Spec tree" });
    fireEvent.click(screen.getByRole("checkbox", { name: "Show drift" }));
    await within(tree).findByText("Check 1");
    await stillRenders();
    const before = given.tree;
    renders.lines = [];
    act(() => {
      emit({ seq: 1, type: "proposal.approved", payload: { id: "PR-1" } });
    });
    await waitFor(() => {
      expect(client.getHealth).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(document.querySelector(".tree-drift")?.getAttribute("aria-busy")).toBe("false");
    });
    await stillRenders();
    expect(within(tree).getByText("Fix 1")).toBeTruthy();
    const after = given.tree;
    expect(after).not.toBe(before);
    expect(Object.keys(after).filter((name) => after[name] !== before[name])).toEqual([]);
    expect(renders.lines).toEqual([]);
  });
});
