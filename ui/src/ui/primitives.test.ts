import { describe, expect, it } from "vitest";

// DS9 of docs/features/design-sweep.md "Acceptance criteria": Inbox, Questions, the Spec tree, the
// node's cards and the shared states build their buttons and empty states from the primitives, never
// from the classes by hand. The patterns are assembled so this file does not match itself.

const SOURCES = import.meta.glob<string>(["/src/{inbox,round,tree,trace}/**/*.tsx", "!/src/**/*.test.tsx", "/src/ui/states.tsx"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const RAW_BUTTON = new RegExp(["className=\"button", "\"button button-"].map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"));
const OLD_EMPTY = new RegExp(["className=\"empty-state\"", "\"empty-filter\""].join("|"));

function offending(pattern: RegExp): string[] {
  return Object.entries(SOURCES).flatMap(([path, text]) =>
    text
      .split("\n")
      .map((line, index) => ({ line, index }))
      .filter(({ line }) => pattern.test(line))
      .map(({ line, index }) => `${path}:${String(index + 1)}: ${line.trim()}`),
  );
}

describe("the four directories on the primitives (DS9 of design-sweep)", () => {
  it("reads the views of the four directories and the shared states", () => {
    expect(Object.keys(SOURCES)).toEqual(
      expect.arrayContaining(["/src/inbox/ProposalCard.tsx", "/src/inbox/StagedChoice.tsx", "/src/round/RoundScreen.tsx", "/src/tree/NodePane.tsx", "/src/trace/CardsBlock.tsx", "/src/ui/states.tsx"]),
    );
    expect(Object.keys(SOURCES).filter((path) => path.endsWith(".test.tsx"))).toEqual([]);
  });

  it("writes no button class by hand: a Button, or buttonClasses on a link", () => {
    expect(offending(RAW_BUTTON)).toEqual([]);
  });

  it("draws no old empty state: an EmptyState", () => {
    expect(offending(OLD_EMPTY)).toEqual([]);
  });

  it("catches each form it refuses (the detector)", () => {
    const button = ["<button type=\"button\" className=", "\"button\">"].join("");
    const look = ["className={a ? ", "\"button button-danger\" : x}"].join("");
    const empty = ["<div className=", "\"empty-state\">"].join("");
    const filter = ["<div className=", "\"empty-filter\">"].join("");
    expect([button, look, "<Button appearance=\"primary\">", "className=\"link-button\""].map((line) => RAW_BUTTON.test(line))).toEqual([true, true, false, false]);
    expect([empty, filter, "<EmptyState level={2} />", "className=\"empty-state-box\""].map((line) => OLD_EMPTY.test(line))).toEqual([true, true, false, false]);
  });
});
