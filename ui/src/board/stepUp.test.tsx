import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import { ApiProvider } from "../api/provider";
import { aCard, aCardWrite } from "../test/builders";
import { jsonAnswer, stubFetch } from "../test/daemonStub";
import { doorAnswer, doorBody, standAt, TEST_ORIGIN } from "../test/doorStub";
import { stubClient, type StubClient } from "../test/stubClient";
import { Announcer } from "../ui/announcer";
import { ACTING_KEY } from "./acting";
import { AcceptCardDialog, ClearCardDialog, ClearGroupDialog } from "./CardDialogs";
import cardDialogsSource from "./CardDialogs.tsx?raw";

const PATHS = {
  accept: "/api/projects/alpha/cards/C-0001/accept",
  clear: "/api/projects/alpha/cards/C-0001/clear",
  group: "/api/projects/alpha/trace/clear",
};

// DS6 of docs/features/door-step-up.md "Acceptance criteria": the confirm class's three card writes,
// Accept, Clear and Clear all, over `fetch`, the hub's login door answering the POST 401 with its
// observed `text/html` body. The dialog shows the step-up notice and the door's login page in a new
// tab, no byte of the body; the fields keep their text and the submit stays on, so a second click is
// a second POST, whose 200 closes the dialog.

const HERE = "https://hub.example.org/#/alpha/board/C-0001";

/** The door's login page, back to HERE. */
const LOGIN = "https://login.example.org/?rd=https%3A%2F%2Fhub.example.org%2F%23%2Falpha%2Fboard%2FC-0001";

const STEP_UP_TITLE = "This action needs the second factor";

/** What the body carries and the page never shows. */
const BODY_BYTES = ["401 Unauthorized", "&amp;", "/api/projects/"];

/**
 * The three writes over HttpClient: first the door's 401 (or, `first` "none", no answer at all, as a
 * dropped hub or a restarting `spec serve` gives), then each POST as `then` answers; the reads the stub's own.
 */
function atTheDoor(path: string, then: () => Response, first: "door" | "none" = "door"): { client: StubClient; posts: () => number; open: () => void } {
  const http = new HttpClient();
  const client = stubClient();
  client.acceptCard.mockImplementation((project, id, body) => http.acceptCard(project, id, body));
  client.clearCard.mockImplementation((project, id, body) => http.clearCard(project, id, body));
  client.clearCardGroup.mockImplementation((project, body) => http.clearCardGroup(project, body));
  let door = true;
  const fetched = stubFetch(() => {
    if (door && first === "none") {
      throw new TypeError("Failed to fetch");
    }
    return door ? doorAnswer(doorBody(path, "POST")) : then();
  });
  return {
    client,
    posts: () => fetched.mock.calls.filter(([url, init]) => url === path && init?.method === "POST").length,
    open: () => {
      door = false;
    },
  };
}

/** A dialog as its caller holds it: open until its write is done. */
function Host({ dialog }: { dialog: (onDone: () => void) => ReactNode }) {
  const [open, setOpen] = useState(true);
  return open
    ? dialog(() => {
        setOpen(false);
      })
    : null;
}

function alone(client: StubClient, dialog: (onDone: () => void) => ReactNode) {
  render(
    <ApiProvider client={client}>
      <Announcer>
        <Host dialog={dialog} />
      </Announcer>
    </ApiProvider>,
  );
}

/** The notice in the dialog's alert: its title, the one link, no byte of the body, the submit on. */
async function expectStepUp(dialog: HTMLElement, submit: string) {
  const alert = await within(dialog).findByRole("alert");
  expect(alert.querySelector(".refusal-title")?.textContent).toBe(STEP_UP_TITLE);
  const links = within(alert).getAllByRole("link");
  expect(links.map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("target")])).toEqual([["Open the login page", LOGIN, "_blank"]]);
  expect(links[0]?.getAttribute("rel")?.split(" ")).toEqual(expect.arrayContaining(["noopener", "noreferrer"]));
  for (const bytes of BODY_BYTES) {
    expect(dialog.textContent).not.toContain(bytes);
  }
  const button = within(dialog).getByRole("button", { name: submit });
  expect([button.getAttribute("aria-disabled"), button.hasAttribute("disabled")]).toEqual(["false", false]);
}

/** The second submit: one more POST, its 200 closing the dialog. */
async function retried(dialog: HTMLElement, submit: string, door: ReturnType<typeof atTheDoor>) {
  door.open();
  fireEvent.click(within(dialog).getByRole("button", { name: submit }));
  await waitFor(() => {
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  expect(door.posts()).toBe(2);
}

function field(dialog: HTMLElement, label: string): HTMLInputElement | HTMLTextAreaElement {
  return within(dialog).getByLabelText(label);
}

describe("a card write at the hub's login door (DS6 of door-step-up)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    standAt(TEST_ORIGIN);
  });

  it("Accept, a departure filled: the notice and its link, the fields kept, the submit on; a second submit is a second POST", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    standAt(HERE);
    const door = atTheDoor(PATHS.accept, () => jsonAnswer(200, aCardWrite("C-0001", { column: "done" })));
    const card = { id: "C-0001", title: "The card", revision: "4", assignee: "Ivo", reviewer: "Ann", targets: ["R-1"] };
    alone(door.client, (onDone) => <AcceptCardDialog project="alpha" card={card} onClose={() => undefined} onDone={onDone} />);
    const dialog = await screen.findByRole("dialog", { name: "Accept C-0001" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Departs from it" }));
    fireEvent.change(field(dialog, "Summary"), { target: { value: "Waits for rest" } });
    fireEvent.change(field(dialog, "What the result does"), { target: { value: "Regenerates at once" } });
    fireEvent.change(field(dialog, "What the brief says"), { target: { value: "Waits 1.5 s" } });
    fireEvent.change(field(dialog, "Note"), { target: { value: "Seen in play" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept task" }));
    await expectStepUp(dialog, "Accept task");
    expect([field(dialog, "Departs from").value, field(dialog, "Summary").value, field(dialog, "What the result does").value, field(dialog, "What the brief says").value, field(dialog, "Note").value]).toEqual([
      "R-1",
      "Waits for rest",
      "Regenerates at once",
      "Waits 1.5 s",
      "Seen in play",
    ]);
    expect(door.posts()).toBe(1);
    await retried(dialog, "Accept task", door);
  });

  it("Clear, a reason typed: the notice and its link, the reason kept, the submit on; a second submit is a second POST", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    standAt(HERE);
    const door = atTheDoor(PATHS.clear, () => jsonAnswer(200, aCardWrite("C-0001")));
    const card = aCard({ id: "C-0001", title: "The card", revision: "4" });
    alone(door.client, (onDone) => <ClearCardDialog project="alpha" card={card} on={null} onClose={() => undefined} onDone={onDone} />);
    const dialog = await screen.findByRole("dialog", { name: "Clear C-0001" });
    fireEvent.change(field(dialog, "Reason"), { target: { value: "The brief's change keeps this task" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    await expectStepUp(dialog, "Clear task");
    expect(field(dialog, "Reason").value).toBe("The brief's change keeps this task");
    expect(door.posts()).toBe(1);
    await retried(dialog, "Clear task", door);
  });

  it("Clear all on a node's group, a reason typed: the notice and its link, the reason kept, the submit on; a second submit is a second POST", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    standAt(HERE);
    const door = atTheDoor(PATHS.group, () => jsonAnswer(200, { group: "R-1", cleared: ["C-0001", "C-0002"], notes: [] }));
    alone(door.client, (onDone) => <ClearGroupDialog project="alpha" group="R-1" cards={2} onClose={() => undefined} onDone={onDone} />);
    const dialog = await screen.findByRole("dialog", { name: "Clear all under R-1" });
    fireEvent.change(field(dialog, "Reason"), { target: { value: "None of them is touched" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    await expectStepUp(dialog, "Clear all");
    expect(field(dialog, "Reason").value).toBe("None of them is touched");
    expect(door.posts()).toBe(1);
    await retried(dialog, "Clear all", door);
  });
});

// Status 0 on the same three writes (docs/features/door-step-up.md "Description and interactions",
// No answer): the client's line and the added one, no link; the dialog and every typed field stay.
// The notice is a static import of the card dialogs: an origin that did not answer serves no chunk
// either, so nothing is fetched as the refusal comes.
const NO_ANSWER_LINE = "If this action needs the second factor, sign in with it on the login page, then retry.";

/** Each write's dialog, its fields typed, and what they hold: [label, value]. */
const UNANSWERED: readonly (readonly [string, keyof typeof PATHS, string, string, (onDone: () => void) => ReactNode, readonly (readonly [string, string])[]])[] = [
  [
    "Accept, a departure filled",
    "accept",
    "Accept C-0001",
    "Accept task",
    (onDone) => <AcceptCardDialog project="alpha" card={{ id: "C-0001", title: "The card", revision: "4", assignee: "Ivo", reviewer: "Ann", targets: ["R-1"] }} onClose={() => undefined} onDone={onDone} />,
    [
      ["Summary", "Waits for rest"],
      ["What the result does", "Regenerates at once"],
      ["What the brief says", "Waits 1.5 s"],
    ],
  ],
  [
    "Clear, a reason typed",
    "clear",
    "Clear C-0001",
    "Clear task",
    (onDone) => <ClearCardDialog project="alpha" card={aCard({ id: "C-0001", title: "The card", revision: "4" })} on={null} onClose={() => undefined} onDone={onDone} />,
    [["Reason", "The brief's change keeps this task"]],
  ],
  [
    "Clear all, a reason typed",
    "group",
    "Clear all under R-1",
    "Clear all",
    (onDone) => <ClearGroupDialog project="alpha" group="R-1" cards={2} onClose={() => undefined} onDone={onDone} />,
    [["Reason", "None of them is touched"]],
  ],
];

describe("a card write with no answer (status 0 of door-step-up)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it.each(UNANSWERED)("%s: the client's words and the second factor's line, no link; the dialog and its fields kept, the submit on", async (_case, write, title, submit, dialogOf, typed) => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const door = atTheDoor(PATHS[write], () => jsonAnswer(200, write === "group" ? { group: "R-1", cleared: [], notes: [] } : aCardWrite("C-0001")), "none");
    alone(door.client, dialogOf);
    const dialog = await screen.findByRole("dialog", { name: title });
    if (write === "accept") {
      fireEvent.click(within(dialog).getByRole("radio", { name: "Departs from it" }));
    }
    for (const [label, value] of typed) {
      fireEvent.change(field(dialog, label), { target: { value } });
    }
    fireEvent.click(within(dialog).getByRole("button", { name: submit }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".refusal-title")?.textContent).toBe("The daemon did not take it.");
    expect(alert.querySelector(".verbatim")?.textContent).toBe(`POST ${PATHS[write]}: no response from the daemon (Failed to fetch)`);
    expect(alert.textContent).toContain(NO_ANSWER_LINE);
    expect(alert.querySelector("a")).toBeNull();
    expect(screen.getByRole("dialog", { name: title })).toBe(dialog);
    expect(typed.map(([label]) => [label, field(dialog, label).value])).toEqual(typed);
    const button = within(dialog).getByRole("button", { name: submit });
    expect([button.getAttribute("aria-disabled"), button.hasAttribute("disabled")]).toEqual(["false", false]);
    await retried(dialog, submit, door);
  });

  it("reaches the notice by a static import alone: no dynamic import of it to fail when the origin does not answer", () => {
    const staticImports = Array.from(cardDialogsSource.matchAll(/^import (?!type )[^;]*?from "([^"]+)";/gms), (match) => match[1]);
    expect(staticImports).toContain("../ui/Refusal");
    expect(cardDialogsSource).not.toMatch(/import\(\s*["']\.\.\/ui\/Refusal["']/);
  });
});
