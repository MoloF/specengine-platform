import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button, buttonClasses, type ButtonAppearance } from "./Button";

// DF-4 of docs/features/design-foundation.md: each appearance and spacing gives exactly its classes,
// the global ones every `.button` of the UI already wears, and a button never submits unless told.

const APPEARANCES: [ButtonAppearance, string[]][] = [
  ["default", ["button"]],
  ["primary", ["button", "button-primary"]],
  ["subtle", ["button", "button-quiet"]],
  ["warning", ["button", "button-warning"]],
  ["danger", ["button", "button-danger"]],
];

function classesOf(element: HTMLElement): string[] {
  return Array.from(element.classList);
}

describe("Button (DF-4)", () => {
  it.each(APPEARANCES)("%s: exactly its classes, at 32 px and compact", (appearance, classes) => {
    render(
      <>
        <Button appearance={appearance}>Wide</Button>
        <Button appearance={appearance} spacing="compact">
          Narrow
        </Button>
      </>,
    );
    expect(classesOf(screen.getByRole("button", { name: "Wide" }))).toEqual(classes);
    expect(classesOf(screen.getByRole("button", { name: "Narrow" }))).toEqual([...classes, "button-compact"]);
    expect(buttonClasses(appearance)).toBe(classes.join(" "));
  });

  it("is a default button unless told: type button, the plain class", () => {
    render(<Button>Plain</Button>);
    const button = screen.getByRole("button", { name: "Plain" });
    expect(button.getAttribute("type")).toBe("button");
    expect(classesOf(button)).toEqual(["button"]);
  });

  it("takes a type when given, a layout class after its look, an icon before its label, and its other attributes", () => {
    render(
      <form>
        <Button type="submit" appearance="primary" icon="cardMove" className="board-move" aria-haspopup="menu" data-card-move="">
          Go
        </Button>
      </form>,
    );
    const button = screen.getByRole("button", { name: "Go" });
    expect(button.getAttribute("type")).toBe("submit");
    expect(classesOf(button)).toEqual(["button", "button-primary", "board-move"]);
    expect(button.firstElementChild?.tagName.toLowerCase()).toBe("svg");
    expect(button.firstElementChild?.getAttribute("aria-hidden")).toBe("true");
    expect([button.getAttribute("aria-haspopup"), button.hasAttribute("data-card-move")]).toEqual(["menu", true]);
  });
});
