import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { Lozenge, LOZENGE_APPEARANCES } from "./Lozenge";

// DF-5 of docs/features/design-foundation.md: a status as a label and an icon, never colour alone,
// in one of six appearances; nothing is cut (W11).

/** The declarations of each rule whose selector is `selector` alone. */
function rulesOf(selector: string): string[] {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return Array.from(appCss.matchAll(new RegExp(`(^|\\n)${escaped}\\s*\\{([^}]*)\\}`, "g")), (match) => match[2] ?? "");
}

describe("Lozenge (DF-5)", () => {
  it.each(LOZENGE_APPEARANCES)("%s: its label, an aria-hidden icon, its appearance", (appearance) => {
    const { container } = render(<Lozenge appearance={appearance} icon="taskAccepted" label="struck" />);
    const lozenge = container.querySelector(".lozenge");
    expect(lozenge?.getAttribute("data-appearance")).toBe(appearance);
    expect(lozenge?.querySelector(".lozenge-label")?.textContent).toBe("struck");
    const icon = lozenge?.querySelector("svg");
    expect(icon?.getAttribute("aria-hidden")).toBe("true");
    expect(lozenge?.firstElementChild).toBe(icon);
    expect(lozenge?.textContent).toBe("struck");
    // Each appearance takes its surface and text roles.
    expect(appCss).toContain(`.lozenge[data-appearance="${appearance}"] { --lozenge-surface: var(--lozenge-${appearance}-surface); --lozenge-text: var(--lozenge-${appearance}-text); }`);
  });

  it("tags a served word English, and leaves the catalog's untagged", () => {
    const { container } = render(
      <>
        <Lozenge appearance="neutral" icon="unknown" label="triage" lang="en" />
        <Lozenge appearance="neutral" icon="open" label="To do" />
      </>,
    );
    expect(Array.from(container.querySelectorAll(".lozenge-label"), (label) => label.getAttribute("lang"))).toEqual(["en", null]);
  });

  it("wraps a long label inside 200 px by whole lines and never cuts it: no text-overflow, no nowrap, no hidden overflow", () => {
    const rule = rulesOf(".lozenge").join("\n");
    expect(rule).toMatch(/max-width:\s*var\(--lozenge-max-width\);/);
    expect(rule).toMatch(/min-height:\s*var\(--lozenge-height\);/);
    expect(rule).toMatch(/line-height:\s*var\(--line-height-small\);/);
    expect(rule).toMatch(/font-size:\s*var\(--font-size-small\);/);
    expect(rule).toMatch(/border-radius:\s*var\(--radius-s\);/);
    expect(rule).not.toMatch(/text-overflow|white-space:\s*nowrap|overflow:\s*hidden/);
    expect(rulesOf(".lozenge-label").join("\n")).not.toMatch(/text-overflow|white-space:\s*nowrap|overflow:\s*hidden/);
  });
});
