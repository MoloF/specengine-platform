import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Breadcrumbs } from "./Breadcrumbs";

// DF-8 of docs/features/design-foundation.md: a `nav` named "Breadcrumbs" around an `ol`; every item
// but the last a link with its `href`; the last the current item, `aria-current="page"`, no `href`;
// the separators hidden.

const ITEMS = [
  { key: "project", label: "Alpha", href: "#/alpha" },
  { key: "board", label: "Board", href: "#/alpha/board" },
  { key: "card", label: "C-0001", mono: true },
];

describe("Breadcrumbs (DF-8)", () => {
  it("is a nav named Breadcrumbs around an ol; each item but the last a link, the last current with no href; separators hidden", () => {
    render(<Breadcrumbs items={ITEMS} />);
    const nav = screen.getByRole("navigation", { name: "Breadcrumbs" });
    const list = nav.querySelector("ol");
    expect(list).not.toBeNull();
    const items = within(nav).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual(["Alpha", "/Board", "/C-0001"]);
    expect(within(nav).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["Alpha", "#/alpha"],
      ["Board", "#/alpha/board"],
    ]);
    const current = items.at(-1)?.querySelector('[aria-current="page"]');
    expect(current?.textContent).toBe("C-0001");
    expect(current?.closest("a")).toBeNull();
    expect(current?.hasAttribute("href")).toBe(false);
    expect(nav.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
    const separators = Array.from(nav.querySelectorAll(".breadcrumbs-separator"));
    expect(separators.map((separator) => [separator.textContent, separator.getAttribute("aria-hidden")])).toEqual([
      ["/", "true"],
      ["/", "true"],
    ]);
    expect(current?.querySelector("bdi")?.className).toBe("mono");
  });

  it("follows a crumb in place on a plain click, when it says how; a modified click is the browser's", () => {
    const follow = vi.fn();
    render(<Breadcrumbs items={[{ key: "p", label: "C-0007", href: "#/alpha/board/C-0007", onFollow: follow }, { key: "c", label: "C-0008" }]} />);
    const link = screen.getByRole("link", { name: "C-0007" });
    fireEvent.click(link, { button: 0, metaKey: true });
    expect(follow).not.toHaveBeenCalled();
    fireEvent.click(link, { button: 0 });
    expect(follow).toHaveBeenCalledTimes(1);
  });
});
