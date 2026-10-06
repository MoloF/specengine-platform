import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { createElement, type ComponentProps } from "react";
import type * as ReactMarkdownModule from "react-markdown";
import { describe, expect, it, vi } from "vitest";
import { aNode } from "../test/builders";
import { renderApp } from "../test/render";
import { treeClient } from "../test/treeStub";
import type * as MarkdownModule from "./Markdown";

// docs/features/ui-markdown.md AC-13 ("one parse per text change"): react-markdown parses on every
// render of its component, so the renderer renders it again only when the text changes. The
// package's component and the renderer's own are wrapped to count how often each runs.

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
});
