import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createElement, StrictMode, type ComponentProps } from "react";
import type * as ReactMarkdownModule from "react-markdown";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceDocument } from "../api/types";
import { App } from "../app/App";
import { aNode } from "../test/builders";
import { renderApp } from "../test/render";
import { treeClient } from "../test/treeStub";
import type * as MarkdownModule from "./Markdown";

// docs/features/ui-markdown.md AC-13 ("one parse per text change"): react-markdown parses on every
// render of its component, so the renderer renders it again only when the text changes, and a render
// React starts again before committing it (cut by a later read's answer, or Strict Mode's second
// call) takes the parse the first one made. The package's component and the renderer's own are
// wrapped to count how often each runs. The renderer keeps its last parses for the module's life, so
// each test parses a text of its own.

const counts = vi.hoisted(() => ({ parses: 0, renders: 0 }));

vi.mock("react-markdown", async (importOriginal) => {
  const actual = await importOriginal<typeof ReactMarkdownModule>();
  function CountedMarkdown(options: ReactMarkdownModule.Options) {
    counts.parses += 1;
    return actual.default(options);
  }
  return { ...actual, default: CountedMarkdown };
});

vi.mock("./Markdown", async (importOriginal) => {
  const actual = await importOriginal<typeof MarkdownModule>();
  function CountedRenderer(props: ComponentProps<typeof actual.Markdown>) {
    counts.renders += 1;
    return createElement(actual.Markdown, props);
  }
  return { ...actual, Markdown: CountedRenderer };
});

describe("one parse per text change (AC-13)", () => {
  beforeEach(() => {
    counts.parses = 0;
    counts.renders = 0;
  });

  it("parses a node's text once: the links read landing renders the text again, not the parse; tree keys neither", async () => {
    const client = treeClient();
    const text = "## DOC-A\n\nSee [a](docs/x.md) and **more**.\n";
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    client.getNode.mockImplementation(async (_project, ref, options) => {
      if (options?.with !== undefined) {
        await held;
      }
      return { ref, reason: null, notes: [], nodes: [aNode({ id: ref, text })] };
    });
    renderApp(client, "#/alpha/tree/DOC-A");
    await screen.findByText("more", { selector: "strong" });
    expect(counts.parses).toBe(1);
    const renders = counts.renders;
    // The links read lands when released; its render is awaited as observed (the query's
    // notification and act's exit are timers of their own: a fixed tick races them under load).
    await act(async () => {
      release();
      await held;
    });
    await waitFor(() => {
      expect(counts.renders).toBeGreaterThan(renders);
    });
    expect(counts.parses).toBe(1);
    const tree = screen.getByRole("tree", { name: "Spec tree" });
    const first = within(tree).getAllByRole("treeitem")[0];
    if (first === undefined) {
      throw new Error("no row");
    }
    first.focus();
    // Every move, ending on the second row: the keys are proven handled before parses is read.
    for (const key of ["ArrowDown", "ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp", "End", "Home", "j", "k", "j"]) {
      fireEvent.keyDown(document.activeElement ?? first, { key });
    }
    await waitFor(() => {
      expect(document.activeElement).toBe(within(tree).getAllByRole("treeitem")[1]);
    });
    expect(document.activeElement).not.toBe(first);
    expect(counts.parses).toBe(1);
  });

  // docs/features/workspaces.md "The UI": the Spec tree waits for its module's root, the node pane
  // never does, so the tree's answer lands after the text, where it may cut into the render mounting
  // the renderer; React then makes that render again. Strict Mode makes the same second call on every
  // render, so it stands in for the cut deterministically. Neither may parse again.
  it("parses a node's text once when the workspaces read lands after it, the tree after that, and a render is made twice", async () => {
    const client = treeClient();
    const text = "## DOC-A\n\nThe order of the reads, and **twice**.\n";
    client.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, text })] }));
    let release: () => void = () => undefined;
    const read = client.getWorkspaces.getMockImplementation();
    client.getWorkspaces.mockImplementation(
      (project) =>
        new Promise<WorkspaceDocument>((resolve, reject) => {
          release = () => {
            read?.(project).then(resolve, reject);
          };
        }),
    );
    window.history.replaceState(null, "", "/#/alpha/tree/DOC-A");
    render(
      <StrictMode>
        <App client={client} scenario={null} />
      </StrictMode>,
    );
    await screen.findByText("twice", { selector: "strong" });
    // The text is up and parsed; the tree has not been asked: it waits for its root.
    expect(client.getTree).not.toHaveBeenCalled();
    expect(counts.parses).toBe(1);
    await act(async () => {
      release();
      await Promise.resolve();
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(screen.getByText("twice", { selector: "strong" })).toBeTruthy();
    expect(counts.parses).toBe(1);
  });
});
