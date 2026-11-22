import { describe, expect, it } from "vitest";
import { aComment } from "../test/builders";
import { actorOf, canEdit, deleteStands, firstBytes, fitsLogin, placeOf, seenWord, takesReply } from "./labels";

// docs/features/comments.md "UI": who may act on a comment as the page shows it (the daemon judges
// either way), the prefill's byte cut at a character boundary, the login grammar, the word a row is
// kept seen by, and what a comment is under, told apart here alone.

const HUMAN = { type: "human", role: null, model: null, run: null };
const AGENT = { type: "agent", role: "spec-writer", model: null, run: null };

describe("who acts", () => {
  it("edits one's own live comment a person wrote: the login against author.login on a hub, the acting name against actor locally", () => {
    const mine = aComment({ id: "CM-1", actor: "mira", author: { ...HUMAN, login: "mira" } });
    expect(canEdit(mine, { login: "mira", name: "mira" })).toBe(true);
    expect(canEdit(mine, { login: "lev", name: "lev" })).toBe(false);
    expect(canEdit(mine, { login: null, name: " mira " })).toBe(true);
    expect(canEdit(mine, { login: null, name: "Mira" })).toBe(false);
    expect(canEdit(aComment({ id: "CM-2", actor: "local", author: HUMAN }), { login: null, name: "" })).toBe(true);
    // An agent's comment is never edited, its person's login or not; a deleted one neither.
    expect(canEdit(aComment({ id: "CM-3", actor: "mira", author: { ...AGENT, login: "mira" } }), { login: "mira", name: "mira" })).toBe(false);
    expect(canEdit({ ...mine, deleted: { by: "mira", at: "2026-11-22T10:00:00Z" } }, { login: "mira", name: "mira" })).toBe(false);
  });

  it("deletes one's own, another's where `allowed` lists delete_comment, and any locally", () => {
    const another = aComment({ id: "CM-1", actor: "mira", author: { ...HUMAN, login: "mira" } });
    expect(deleteStands(another, { login: "lev", name: "lev" }, false)).toBe(false);
    expect(deleteStands(another, { login: "lev", name: "lev" }, true)).toBe(true);
    expect(deleteStands(another, { login: "mira", name: "mira" }, false)).toBe(true);
    expect(deleteStands(aComment({ id: "CM-2", actor: "mira", author: { ...AGENT, login: "mira" } }), { login: "mira", name: "mira" }, false)).toBe(true);
    expect(deleteStands(another, { login: null, name: "lev" }, false)).toBe(true);
  });

  it("replies on a live top-level comment alone", () => {
    expect(takesReply(aComment({ id: "CM-1" }))).toBe(true);
    expect(takesReply(aComment({ id: "CM-2", reply_to: "CM-1" }))).toBe(false);
    expect(takesReply(aComment({ id: "CM-3", body: null, deleted: { by: "mira", at: "2026-11-22T10:00:00Z" } }))).toBe(false);
  });

  it("names the actor a write records: the login, the name trimmed, `local` with none", () => {
    expect([actorOf({ login: "mira", name: "mira" }), actorOf({ login: null, name: "  Ann " }), actorOf({ login: null, name: " " })]).toEqual(["mira", "Ann", "local"]);
  });
});

describe("words", () => {
  it("cuts a prefill at a character boundary, never inside a code point", () => {
    expect(firstBytes("abc", 1024)).toBe("abc");
    expect(firstBytes("\u0436\u0436\u0436", 5)).toBe("\u0436\u0436");
    expect(firstBytes("a\u{1F600}b", 4)).toBe("a");
    expect(firstBytes("a\u{1F600}b", 5)).toBe("a\u{1F600}");
    expect(new TextEncoder().encode(firstBytes("\u0436".repeat(600), 1024)).length).toBe(1024);
  });

  it("reads the login grammar", () => {
    for (const login of ["lev", "mira.k", "a", "a1-b_c.d"]) {
      expect([login, fitsLogin(login)]).toEqual([login, true]);
    }
    for (const name of ["Mira", "mira.", "1lev", "", "lev k", "-lev"]) {
      expect([name, fitsLogin(name)]).toEqual([name, false]);
    }
  });

  it("keeps a row seen by its ID and version", () => {
    expect(seenWord({ id: "CM-0012", version: 3 })).toBe("CM-0012@3");
  });

  it("tells the three places apart, another word none", () => {
    expect(["card", "proposal", "node", "board"].map((kind) => placeOf({ kind, id: "X" }))).toEqual(["card", "proposal", "node", null]);
  });
});
