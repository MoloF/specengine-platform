import { act, fireEvent, screen, within } from "@testing-library/react";
import { createElement, type ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { renderApp } from "../test/render";
import { treeClient } from "../test/treeStub";
import type * as NodePaneModule from "./NodePane";
import type * as SpecTreeModule from "./SpecTree";

// docs/features/ui-tree-node.md AC-11 (search "never per keystroke"): a keystroke in the search
// field renders the field alone. The tree and the node pane are wrapped to count how often the
// view renders them; the wrappers render the real components.

const renders = vi.hoisted(() => ({ tree: 0, node: 0 }));

vi.mock("./SpecTree", async (importOriginal) => {
  const actual = await importOriginal<typeof SpecTreeModule>();
  function CountedSpecTree(props: ComponentProps<typeof actual.SpecTree>) {
    renders.tree += 1;
    return createElement(actual.SpecTree, props);
  }
  return { ...actual, SpecTree: CountedSpecTree };
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
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(renders.tree).toBeGreaterThan(0);
    expect(renders.node).toBeGreaterThan(0);
    const before = { ...renders };
    const field = screen.getByLabelText<HTMLInputElement>("Search the spec");
    for (const value of ["w", "wi", "wid", "widg", "widge", "widget"]) {
      fireEvent.change(field, { target: { value } });
    }
    expect(field.value).toBe("widget");
    expect(renders).toEqual(before);
    fireEvent.submit(screen.getByRole("search"));
    expect(await screen.findByText(/No node matches/)).toBeTruthy();
    expect(client.search).toHaveBeenCalledWith("alpha", { query: "widget" });
  });
});
