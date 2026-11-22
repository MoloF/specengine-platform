import { render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { sentence } from "../test/text";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/comments.md "UI" over the mock as `pnpm dev` shows it (src/mocks/comments.ts):
// harbor-sim's task C-0003 holds a thread (an edited question naming two logins, an agent's reply
// under it, a deleted remark), MEC-MOORING a remark in Cyrillic, PR-0049 one; the `hub` scenario's
// login, mira, is mentioned twice and lit in the nav. The mock is imported here alone (policy AC-05).

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

describe("comments over the mock", () => {
  it("serves a thread page oldest first, a reply naming its comment, a tombstone with no text", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const list = await client.getComments("harbor-sim", { on: "C-0003" });
    expect(list.comments.map((comment) => [comment.id, comment.reply_to, comment.body === null])).toEqual([
      ["CM-0001", null, false],
      ["CM-0002", "CM-0001", false],
      ["CM-0003", null, true],
    ]);
    expect([list.on, list.total, list.next_before]).toEqual([{ kind: "card", id: "C-0003" }, 3, null]);
    await expect(client.getComments("harbor-sim", { on: "C-0999" })).rejects.toThrow("on: no card C-0999");
  });

  it("refuses as the daemon does: a reply to a reply, a stale version, another's edit", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    await expect(client.addComment("harbor-sim", { on: "C-0003", body: "x", reply_to: "CM-0002", as: "ivo" })).rejects.toThrow("reply_to: CM-0002 is a reply: a reply names a top-level comment");
    await expect(client.editComment("harbor-sim", "CM-0001", { body: "y", version: 1, as: "ivo" })).rejects.toThrow("CM-0001 is at version 2, not 1: nothing changed");
    await expect(client.editComment("harbor-sim", "CM-0001", { body: "y", version: 2, as: "lev" })).rejects.toThrow("refused: CM-0001 is ivo's: only its author edits it");
    const made = await client.addComment("harbor-sim", { on: "C-0003", body: "Agreed, @mira.", reply_to: "CM-0001", as: "lev" });
    expect([made.id, made.actor, made.mentions]).toEqual(["CM-0006", "lev", ["mira"]]);
  });

  it("draws C-0003's thread in its modal and lights mira's Mentions on the hub", async () => {
    renderMock("#/harbor-sim/board/C-0003", "hub");
    const dialog = await screen.findByRole("dialog", { name: "Night passage rules, plain words" });
    await within(dialog).findByText(sentence(/^Deleted by mira, /));
    expect(within(dialog).getByRole("region", { name: "Comments" }).querySelectorAll(".comment")).toHaveLength(3);
    const nav = await screen.findByRole("navigation", { name: "Sections" });
    const link = await within(nav).findByRole("link", { name: /^Mentions/ });
    await waitFor(() => {
      expect(link.querySelector(".nav-badge")?.getAttribute("data-unseen")).toBe("2");
    });
  });
});
