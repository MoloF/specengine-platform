import { describe, expect, it } from "vitest";
import { NODE_ID, nodeCommands } from "./commands";

// UI-5 of docs/features/ui-node-bindings.md: a node's commands are fixed words and the tab's
// checked REF, copied for a terminal; a REF of any other form gets none, never a quoted or escaped
// one; a REF naming several holders, or a document with no ID, gets verify alone.

describe("nodeCommands (UI-5 of ui-node-bindings)", () => {
  it.each(["LAW-BOUND", "slug/AC-07", "R-14", "DOC_A.v2", "docs/spec/b.md"])("gives %s its three commands, in order", (id) => {
    expect(nodeCommands(id)?.map((command) => [command.action, command.text])).toEqual([
      ["verify", `spec verify --node ${id}`],
      ["drift", `spec propose drift ${id}`],
      ["accept", `spec lock accept ${id}`],
    ]);
  });

  it.each([
    ["empty", ""],
    ["a space", "LAW BOUND"],
    ["a trailing space", "LAW-BOUND "],
    ["a semicolon", "LAW-BOUND;ls"],
    ["a command substitution", "$(touch x)"],
    ["a dollar inside", "LAW-$(id)"],
    ["a backtick", "LAW-`id`"],
    ["a double quote", 'LAW-"BOUND"'],
    ["a single quote", "LAW-'BOUND'"],
    ["a newline", "LAW-BOUND\nrm -rf ~"],
    ["a trailing newline", "LAW-BOUND\n"],
    ["a leading dash (an option)", "-rf"],
    ["a leading dot", "../x"],
    ["a section mark", "MEC-TIDES#RULE-TIDE-WINDOW"],
    ["a pipe", "LAW|sh"],
    ["an ampersand", "LAW&&x"],
    ["a redirect", "LAW>x"],
    ["a tab", "LAW\tBOUND"],
    // LAW-BOUND with a Cyrillic capital A (U+0410), a look-alike.
    ["a Cyrillic look-alike", `L${String.fromCodePoint(0x410)}W-BOUND`],
    ["a non-Latin letter", `R-${String.fromCodePoint(0x434)}`],
    ["a fullwidth letter", `R-${String.fromCodePoint(0xff21)}`],
  ])("refuses %s: no command at all", (_what, id) => {
    expect(nodeCommands(id)).toBeNull();
  });

  it.each(["slug/AC-07", "docs/spec/b.md"])("gives %s verify alone when asked (several holders, or a document with no ID): propose drift and lock accept each need one node with an ID", (ref) => {
    expect(nodeCommands(ref, true)?.map((command) => command.text)).toEqual([`spec verify --node ${ref}`]);
    expect(nodeCommands(`${ref} x`, true)).toBeNull();
    expect(nodeCommands(ref, false)).toHaveLength(3);
    expect(nodeCommands(ref)).toHaveLength(3);
  });

  it("says drift judges HEAD and the node's own rows; only accept asks [y/N]", () => {
    const meanings = Object.fromEntries((nodeCommands("R-14") ?? []).map((command) => [command.action, command.meaning]));
    expect(meanings.drift).toBe("Queue its drift: judges the node's own rows at HEAD (commit edits first) and queues an Inbox item for your decision");
    expect(Object.values(meanings).filter((meaning) => meaning.includes("[y/N]"))).toEqual([meanings.accept]);
  });

  it("puts nothing in a command but the fixed words and the REF", () => {
    for (const command of nodeCommands("slug/AC-07") ?? []) {
      expect(command.text.split(" ").at(-1)).toBe("slug/AC-07");
      expect(command.text).toMatch(/^spec (verify --node|propose drift|lock accept) slug\/AC-07$/);
      expect(command.meaning).not.toMatch(/bl[o]ck/i);
    }
  });

  it("anchors the pattern at both ends, Latin only", () => {
    expect(NODE_ID.source).toBe("^[A-Za-z0-9][A-Za-z0-9._/-]*$");
    expect(NODE_ID.flags).toBe("");
  });
});
