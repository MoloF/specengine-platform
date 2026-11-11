import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { IssueView } from "./IssueView";

// The work item's layout (docs/canon/ui-design.md "Primitives"), its `meta` slot added by
// doc-modal (docs/features/doc-modal.md "Acceptance criteria", DM-19): under the title, before the acts.

describe("IssueView", () => {
  it("lays the head out: the trail and tools on one line, the title, the meta line, then the acts; main before side", () => {
    const { container } = render(
      <IssueView
        breadcrumbs={<nav aria-label="Trail">Trail</nav>}
        tools={<button type="button">Tool</button>}
        title={<h2>Title</h2>}
        meta={<span>State</span>}
        actions={<button type="button">Act</button>}
        main={<p>Main</p>}
        side={<p>Side</p>}
      />,
    );
    const head = container.querySelector(".issue-view-head");
    expect(Array.from(head?.children ?? [], (child) => child.className || child.tagName)).toEqual(["issue-view-bar", "H2", "issue-view-meta", "issue-view-actions"]);
    expect(head?.querySelector(".issue-view-meta")?.textContent).toBe("State");
    expect(Array.from(container.querySelector(".issue-view-body")?.children ?? [], (child) => child.className)).toEqual(["issue-view-main", "issue-view-side"]);
    expect(screen.getByRole("heading", { level: 2, name: "Title" })).toBeTruthy();
  });

  it("draws no meta line without one, nor for null", () => {
    const { container, rerender } = render(<IssueView breadcrumbs={null} title={<h1>T</h1>} main={null} side={null} />);
    expect(container.querySelector(".issue-view-meta")).toBeNull();
    rerender(<IssueView breadcrumbs={null} title={<h1>T</h1>} meta={null} main={null} side={null} />);
    expect(container.querySelector(".issue-view-meta")).toBeNull();
  });
});

/** The text of each block opened by `head` (`@media (<query>)`, `@container <name> (<query>)`), braces matched. */
function atBlocks(head: string): string[] {
  const found: string[] = [];
  let from = 0;
  for (;;) {
    const at = appCss.indexOf(head, from);
    if (at < 0) {
      return found;
    }
    const open = appCss.indexOf("{", at);
    let depth = 0;
    let end = open;
    for (; end < appCss.length; end += 1) {
      depth += appCss[end] === "{" ? 1 : appCss[end] === "}" ? -1 : 0;
      if (depth === 0) {
        break;
      }
    }
    found.push(appCss.slice(open + 1, end));
    from = end;
  }
}

/** The text of each `@media (<query>)` block. */
function mediaBlocks(query: string): string[] {
  return atBlocks(`@media (${query})`);
}

/** The text of each `@container issue-view (<query>)` block, read as `mediaBlocks` reads `@media`. */
function containerBlocks(query: string): string[] {
  return atBlocks(`@container issue-view (${query})`);
}

/** Every rule of `css` whose selector list holds `selector`, by its declarations. */
function rulesHolding(selector: string, css: string): string[] {
  return Array.from(css.matchAll(/([^{}]+)\{([^{}]*)\}/g)).flatMap((match) =>
    (match[1] ?? "")
      .split(",")
      .map((part) => part.trim().replace(/\s+/g, " "))
      .includes(selector)
      ? [match[2] ?? ""]
      : [],
  );
}

/** Where the next `@media` or `@container` block of `css` begins at or after `from`, or -1. */
function nextAtRule(css: string, from: number): number {
  const found = ["@media", "@container"].map((head) => css.indexOf(head, from)).filter((at) => at >= 0);
  return found.length === 0 ? -1 : Math.min(...found);
}

/** `css` with every `@media` and `@container` block cut out: the rules that hold at any width. */
function outsideAtRules(css: string): string {
  let out = "";
  let at = 0;
  for (let found = nextAtRule(css, at); found >= 0; found = nextAtRule(css, at)) {
    out += css.slice(at, found);
    let depth = 0;
    let end = css.indexOf("{", found);
    for (; end < css.length; end += 1) {
      depth += css[end] === "{" ? 1 : css[end] === "}" ? -1 : 0;
      if (depth === 0) {
        break;
      }
    }
    at = end + 1;
  }
  return out + css.slice(at);
}

// docs/features/doc-modal.md "Acceptance criteria", DM-21's 320 px look (the review's L2): the work item
// in a dialog, a card's or a document's, scrolls its body inside at one column, the head staying; under
// 36rem, where the dialog is the screen, the dialog is the one scroll container and the head scrolls
// away. At two columns Main and the side scroll apart (docs/features/room-and-scroll.md "Acceptance criteria", RS-8).
// jsdom lays nothing out, so these pin the rules; the rects are the look's.
describe("the work item in a dialog scrolls", () => {
  const uncommented = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, "");
  const ANY = uncommented(outsideAtRules(appCss));
  const NARROW = uncommented(mediaBlocks("max-width: 36rem").join("\n"));
  const TWO_COLUMNS = uncommented(containerBlocks("min-width: 44.5rem").join("\n"));

  it("above 36rem at one column: the dialog clips, the work item's body is the one scroll container", () => {
    expect(rulesHolding(".dialog:has(> .issue-view)", ANY).join("")).toMatch(/display:\s*flex;[\s\S]*overflow:\s*hidden;/);
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).toMatch(/overflow:\s*auto;/);
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).toMatch(/min-height:\s*0;/);
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).toMatch(/padding:\s*var\(--space-050\);/);
  });

  it("at two columns (RS-8): the body clips, its one row the dialog's at most, and Main and the side each scroll on their own, padded for a focus ring", () => {
    const body = rulesHolding(".dialog > .issue-view > .issue-view-body", TWO_COLUMNS).join("");
    for (const declaration of [/overflow:\s*hidden;/, /grid-template-rows:\s*minmax\(0, 1fr\);/, /align-items:\s*stretch;/, /padding:\s*0;/]) {
      expect([declaration.source, declaration.test(body)]).toEqual([declaration.source, true]);
    }
    for (const column of [".issue-view-main", ".issue-view-side"]) {
      // M: Main's `overflow-y: auto` dropped; the split written outside the container query.
      const rule = rulesHolding(`.dialog > .issue-view > .issue-view-body > ${column}`, TWO_COLUMNS).join("");
      for (const declaration of [/min-height:\s*0;/, /overflow-y:\s*auto;/, /overscroll-behavior:\s*contain;/, /padding:\s*var\(--space-050\);/]) {
        expect([column, declaration.source, declaration.test(rule)]).toEqual([column, declaration.source, true]);
      }
    }
    // The body's one-column rules stand outside it, and the head is no scroller anywhere.
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).not.toMatch(/overflow:\s*hidden;/);
    const heads = Array.from(uncommented(appCss).matchAll(/([^{}]+)\{([^{}]*)\}/g)).flatMap((match) =>
      (match[1] ?? "")
        .split(",")
        .map((part) => part.trim().replace(/\s+/g, " "))
        .filter((part) => /\.issue-view-head(:[a-z-]+(\([^)]*\))?)*$/.test(part))
        .map((part) => ({ part, body: match[2] ?? "" })),
    );
    expect(heads.length).toBeGreaterThanOrEqual(1);
    // M: `.dialog > .issue-view > .issue-view-head { overflow: auto }` inside the container block.
    expect(heads.filter(({ body }) => /(^|[\s;])overflow(-x|-y)?:[^;]*\b(auto|scroll)\b/.test(body)).map(({ part }) => part)).toEqual([]);
  });

  it("splits no page (RS-8): no rule without `.dialog >` gives Main, the side or the head an overflow", () => {
    const everyRule = Array.from(uncommented(appCss).matchAll(/([^{}]+)\{([^{}]*)\}/g));
    const parts = everyRule.flatMap((match) =>
      (match[1] ?? "")
        .split(",")
        .map((part) => part.trim().replace(/\s+/g, " "))
        .filter((part) => /\.issue-view-(main|side|head)(:[a-z-]+(\([^)]*\))?)*$/.test(part))
        .map((part) => ({ part, body: match[2] ?? "" })),
    );
    expect(parts.length).toBeGreaterThanOrEqual(4);
    // M: the split written as `.issue-view-body > .issue-view-main`, without `.dialog >`.
    expect(parts.filter(({ part, body }) => !part.startsWith(".dialog >") && /(^|[\s;])overflow(-x|-y)?:/.test(body)).map(({ part }) => part)).toEqual([]);
  });

  it("under 36rem: the dialog scrolls as one; the work item and its body neither scroll nor shrink", () => {
    expect(rulesHolding(".dialog:has(> .issue-view)", NARROW).join("")).toMatch(/overflow-y:\s*auto;/);
    for (const part of [".dialog > .issue-view", ".dialog > .issue-view > .issue-view-body"]) {
      const rule = rulesHolding(part, NARROW).join("");
      expect([part, /overflow:\s*visible;/.test(rule), /flex:\s*none;/.test(rule)]).toEqual([part, true, true]);
    }
  });

  it("leaves no second scroll container to a card's or a document's modal, and clips nothing sideways", () => {
    for (const modal of [".card-modal", ".doc-modal"]) {
      const own = [...rulesHolding(modal, ANY), ...rulesHolding(modal, NARROW)].join("");
      expect([modal, /(^|[\s;])overflow(-x|-y)?:/.test(own), /overflow-wrap:\s*anywhere;/.test(rulesHolding(modal, ANY).join(""))]).toEqual([modal, false, true]);
      expect([modal, /width:\s*100vw;/.test(rulesHolding(modal, NARROW).join(""))]).toEqual([modal, true]);
    }
    expect(rulesHolding(".dialog:has(> .issue-view)", NARROW).join("")).not.toMatch(/overflow-x:\s*hidden/);
  });
});
