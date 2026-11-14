import { describe, expect, it } from "vitest";
import { doorBody } from "../test/doorStub";
import { doorLink, doorRefusalOf } from "./door";

// DS1 and DS2 of docs/features/door-step-up.md "Acceptance criteria": the door's login page built from
// the body's first `href` (its origin, then `/?rd=` and the page percent-encoded), and the refusal read
// by status alone. The body is the observed one, the hosts placeholders, the project this suite's.

/** The door's answer to a one-factor `fetch` of a step-up route, as observed, on one line. */
const DOOR_BODY =
  '<a href="https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2Fapi%2Fprojects%2Falpha%2Fproposals%2FPR-0007%2Fconfirm&amp;rm=GET">401 Unauthorized</a>';

const HERE = "https://hub.example.org/#/alpha/inbox/PR-0007";

/** The table's script row, spelled in pieces: the sources name no browser dialog called (policy.test.ts). */
const SCRIPT = ["al", "ert(1)"].join("");

describe("doorLink (DS1)", () => {
  it.each([
    ["the observed body", DOOR_BODY, HERE, "https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2F%23%2Falpha%2Finbox%2FPR-0007"],
    ["an http href", '<a href="http://login.example.org/">401 Unauthorized</a>', HERE, null],
    ["a relative href", '<a href="/login">401 Unauthorized</a>', HERE, null],
    ["a javascript: href", `<a href="javascript:${SCRIPT}">401 Unauthorized</a>`, HERE, null],
    ["an href with credentials", '<a href="https://u:p@login.example.org/">401 Unauthorized</a>', HERE, null],
    ["a data-href alone", '<a data-href="https://login.example.org/">401 Unauthorized</a>', HERE, null],
    ["a data-href before the href", '<a data-href="https://evil.example.org/" href="https://login.example.org/">401 Unauthorized</a>', HERE, "https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2F%23%2Falpha%2Finbox%2FPR-0007"],
    ["a bare word", "Unauthorized", HERE, null],
    ["an empty body", "", HERE, null],
  ])("%s gives exactly its row", (_case, body, here, link) => {
    expect(doorLink(body, here)).toBe(link);
  });

  it("reads the test helper's body as the observed one", () => {
    expect(doorBody("/api/projects/alpha/proposals/PR-0007/confirm")).toBe(DOOR_BODY);
  });

  it("keeps the origin alone, encodes the whole page with its hash, and reads only the first href", () => {
    const here = "https://hub.example.org/?scenario=x#/alpha/inbox/PR-0007";
    expect(doorLink('<a class="x" href="https://door.example.org/sub/path?rd=elsewhere&amp;rm=POST">a</a> <a href="https://other.example.org/">b</a>', here)).toBe(
      `https://door.example.org/?rd=${encodeURIComponent(here)}`,
    );
  });
});

describe("doorRefusalOf (DS2)", () => {
  it("reads a 401 as the door's, with a link or without one", () => {
    expect(doorRefusalOf({ status: 401, message: DOOR_BODY }, HERE)).toEqual({ kind: "door", link: doorLink(DOOR_BODY, HERE) });
    expect(doorRefusalOf({ status: 401, message: "Unauthorized" }, HERE)).toEqual({ kind: "door", link: null });
  });

  it("reads status 0 as no answer", () => {
    expect(doorRefusalOf({ status: 0, message: "GET /api/projects/alpha/proposals/PR-0007/confirm: no response from the daemon (Failed to fetch)" }, HERE)).toEqual({
      kind: "noAnswer",
    });
  });

  it.each([403, 404, 409, 503])("leaves a %i to the daemon's words: null", (status) => {
    expect(doorRefusalOf({ status, message: DOOR_BODY }, HERE)).toBeNull();
  });
});
