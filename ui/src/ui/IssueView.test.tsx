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

/** The text of each `@media (<query>)` block, braces matched. */
function mediaBlocks(query: string): string[] {
  const found: string[] = [];
  let from = 0;
  for (;;) {
    const at = appCss.indexOf(`@media (${query})`, from);
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

/** `css` with every `@media` block cut out: the rules that hold at any width. */
function outsideMedia(css: string): string {
  let out = "";
  let at = 0;
  for (let found = css.indexOf("@media", at); found >= 0; found = css.indexOf("@media", at)) {
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
// in a dialog, a card's or a document's, scrolls its body inside, the head staying; under 36rem, where
// the dialog is the screen, the dialog is the one scroll container and the head scrolls away. jsdom lays
// nothing out, so these pin the rules; the rects are the look's.
describe("the work item in a dialog scrolls", () => {
  const uncommented = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, "");
  const ANY = uncommented(outsideMedia(appCss));
  const NARROW = uncommented(mediaBlocks("max-width: 36rem").join("\n"));

  it("above 36rem: the dialog clips, the work item's body is the one scroll container", () => {
    expect(rulesHolding(".dialog:has(> .issue-view)", ANY).join("")).toMatch(/display:\s*flex;[\s\S]*overflow:\s*hidden;/);
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).toMatch(/overflow:\s*auto;/);
    expect(rulesHolding(".dialog > .issue-view > .issue-view-body", ANY).join("")).toMatch(/min-height:\s*0;/);
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
