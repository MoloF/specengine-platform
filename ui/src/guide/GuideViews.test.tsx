import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { GuidePage } from "../api/types";
import { guideHash, sectionHash } from "../app/routes";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import appCss from "../styles/app.css?raw";
import { aGuideIndex, aGuidePage, aGuideRule, aGuideTopic } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// docs/features/specengine-guide.md "The UI", GU14, on the stub client: Documents' View group, "Project
// documents | SpecEngine guide", kept per project for the page's life; the guide's shelf listing the
// served topics, each its title (code spans as code), summary and rule count, the title opening its
// route; a topic's page read only, its text under lang=en, its rules anchors with Copy citation copying
// `guide:<topic>#<ID>`; a rule's route focusing the rule, clear of the sticky header, and a link to the
// rule already named focusing it again; the none, failure and an older daemon's 404 states in the
// daemon's words with Retry; the chrome in Russian, under the viewer's `lang` inside the English text,
// the guide's text as served; Documents' shelf set to the guide only from the shelf or the back link,
// never by a card's citation. Neither view reads the project's library, nor offers a task, a split, a
// Flow or an edit act.

const TOPICS = [
  aGuideTopic("start", { title: "Adopting SpecEngine", summary: "SpecEngine keeps a project's spec in its git.", rules: 6 }),
  aGuideTopic("records", { title: "Records, the spec tree and canon", summary: "Every statement has one ID.", rules: 2 }),
  aGuideTopic("auto", { title: "The `[auto]` mode", summary: "A rule in `specengine.toml` lands a pack.", rules: 1 }),
];

const RECORDS: GuidePage = aGuidePage("records", [aGuideRule("GD-RECORDS-ONE-FILE", { title: "An ID is defined in one file", line: 61 }), aGuideRule("GD-RECORDS-LATIN-IDS", { title: "IDs are Latin", line: 65 })], {
  title: "Records, the spec tree and canon",
  summary: "Every statement has one ID.",
});

function client(): StubClient {
  const stub = stubClient();
  stub.getGuide.mockResolvedValue(aGuideIndex(TOPICS));
  stub.getGuideTopic.mockImplementation((topic) =>
    topic === "records" ? Promise.resolve(structuredClone(RECORDS)) : Promise.reject(new ClientError({ status: 404, message: `spec: no guide topic \`${topic}\`; topics: start, records, auto` })),
  );
  return stub;
}

/** The View group's two buttons, by name, with whether each is pressed. */
function shelves(): [string, string | null][] {
  return within(screen.getByRole("group", { name: "View" }))
    .getAllByRole("button")
    .map((button) => [button.textContent, button.getAttribute("aria-pressed")]);
}

async function openShelf(stub: StubClient): Promise<HTMLElement> {
  renderApp(stub, "#/alpha/documents");
  fireEvent.click(await screen.findByRole("button", { name: "SpecEngine guide" }));
  return screen.findByRole("list", { name: "3 topics" });
}

function stubClipboard(writeText: (text: string) => Promise<void>) {
  const spy = vi.fn(writeText);
  Object.defineProperty(navigator, "clipboard", { value: { writeText: spy }, configurable: true });
  return spy;
}

beforeAll(async () => {
  await import("../documents/DocumentsView");
  await import("./GuideTopicView");
  await import("../markdown/Markdown");
});

describe("Documents' guide shelf (GU14 of specengine-guide)", () => {
  it("the View group starts at Project documents; SpecEngine guide lists the served topics in order, each its title linking its route, its summary, its topic and rule count, read with no project", async () => {
    const stub = client();
    renderApp(stub, "#/alpha/documents");
    await screen.findByRole("group", { name: "View" });
    expect(shelves()).toEqual([
      ["Project documents", "true"],
      ["SpecEngine guide", "false"],
    ]);
    fireEvent.click(screen.getByRole("button", { name: "SpecEngine guide" }));
    expect(shelves()).toEqual([
      ["Project documents", "false"],
      ["SpecEngine guide", "true"],
    ]);
    const list = await screen.findByRole("list", { name: "3 topics" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      "Adopting SpecEngineSpecEngine keeps a project's spec in its git.start6 rules",
      "Records, the spec tree and canonEvery statement has one ID.records2 rules",
      "The [auto] modeA rule in specengine.toml lands a pack.auto1 rule",
    ]);
    const links = within(list).getAllByRole("link");
    expect(links.map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("lang")])).toEqual([
      ["Adopting SpecEngine", guideHash("alpha", "start"), "en"],
      ["Records, the spec tree and canon", guideHash("alpha", "records"), "en"],
      ["The [auto] mode", guideHash("alpha", "auto"), "en"],
    ]);
    // A title's and a summary's code spans as code; the summary served English.
    expect(rows[2]?.querySelectorAll("code")).toHaveLength(2);
    expect(rows[0]?.querySelector(".guide-topic-summary")?.getAttribute("lang")).toBe("en");
    expect(stub.getGuide).toHaveBeenCalledTimes(1);
    expect(stub.getGuide.mock.calls[0]?.[0]).toBeInstanceOf(AbortSignal);
    // Nothing on the shelf writes: no button but the View group's.
    expect(within(list).queryAllByRole("button")).toEqual([]);
  });

  it("never reads the library while the guide shows: a page starting on it asks nothing of the project's documents", async () => {
    const stub = client();
    await openShelf(stub);
    const asked = stub.getLibrary.mock.calls.length;
    expect(asked).toBeLessThanOrEqual(1);
    // Back on Documents with the guide kept: no library read at all.
    act(() => {
      window.location.hash = sectionHash("alpha", "inbox");
    });
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    act(() => {
      window.location.hash = sectionHash("alpha", "documents");
    });
    await screen.findByRole("list", { name: "3 topics" });
    expect(shelves()[1]).toEqual(["SpecEngine guide", "true"]);
    expect(stub.getLibrary.mock.calls.length).toBe(asked);
  });

  it("keeps the shelf per project for the page's life: another project starts at its documents", async () => {
    const stub = client();
    await openShelf(stub);
    act(() => {
      window.location.hash = sectionHash("beta", "documents");
    });
    await waitFor(() => {
      expect(shelves()).toEqual([
        ["Project documents", "true"],
        ["SpecEngine guide", "false"],
      ]);
    });
    act(() => {
      window.location.hash = sectionHash("alpha", "documents");
    });
    await waitFor(() => {
      expect(shelves()[1]).toEqual(["SpecEngine guide", "true"]);
    });
  });

  it("shows a skeleton while the index is out, then the list", async () => {
    const stub = client();
    let answer: () => void = () => undefined;
    stub.getGuide.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = () => {
            resolve(aGuideIndex(TOPICS));
          };
        }),
    );
    renderApp(stub, "#/alpha/documents");
    fireEvent.click(await screen.findByRole("button", { name: "SpecEngine guide" }));
    expect(await screen.findByRole("status", { name: "Reading the guide" })).toBeTruthy();
    expect(document.querySelector(".guide-shelf")?.getAttribute("aria-busy")).toBe("true");
    await act(async () => {
      answer();
      await Promise.resolve();
    });
    expect(await screen.findByRole("list", { name: "3 topics" })).toBeTruthy();
    expect(document.querySelector(".guide-shelf")?.getAttribute("aria-busy")).toBe("false");
  });

  it("shows an older daemon's 404 in its words under lang=en with Retry, which reads again", async () => {
    const message = "no route /api/guide: the routes are /api/projects and the web UI";
    const stub = client();
    stub.getGuide.mockRejectedValueOnce(new ClientError({ status: 404, message }));
    renderApp(stub, "#/alpha/documents");
    fireEvent.click(await screen.findByRole("button", { name: "SpecEngine guide" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The guide could not be read");
    const shown = screen.getByText(message);
    expect(shown.getAttribute("lang")).toBe("en");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("list", { name: "3 topics" })).toBeTruthy();
    expect(screen.queryByText(message)).toBeNull();
    expect(stub.getGuide).toHaveBeenCalledTimes(2);
  });

  it("says so when the build serves no topic", async () => {
    const stub = client();
    stub.getGuide.mockResolvedValue(aGuideIndex([]));
    renderApp(stub, "#/alpha/documents");
    fireEvent.click(await screen.findByRole("button", { name: "SpecEngine guide" }));
    expect(await screen.findByRole("heading", { level: 2, name: "No guide topic" })).toBeTruthy();
    expect(screen.getByText("This daemon's build carries no guide page.")).toBeTruthy();
  });
});

describe("a guide topic's page (GU14 of specengine-guide)", () => {
  async function openTopic(stub: StubClient, hash = guideHash("alpha", "records")): Promise<HTMLElement> {
    renderApp(stub, hash);
    await screen.findByRole("heading", { level: 1, name: "Records, the spec tree and canon" });
    const article = document.querySelector<HTMLElement>(".guide-article");
    if (article === null) {
      throw new Error("no article");
    }
    await within(article).findByRole("heading", { level: 3, name: "An ID is defined in one file GD-RECORDS-ONE-FILE" });
    return article;
  }

  it("renders the page read only, its text under lang=en with its parts and rules, the title said once; the nav marks Documents; no task, split, Flow or edit act", async () => {
    const stub = client();
    const article = await openTopic(stub);
    expect(article.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("heading", { level: 1 }).getAttribute("lang")).toBe("en");
    expect(within(article).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["What it solves", "When to use it", "How", "Where it lives", "What not to do"]);
    // The title is the view's heading, not the text's again.
    expect(within(article).queryByRole("heading", { name: "Records, the spec tree and canon" })).toBeNull();
    expect(document.querySelector(".view-about")?.textContent).toBe("Every statement has one ID.");
    expect(document.querySelector(".guide-facts")?.textContent).toBe("Read onlyReviewed 2026-11-202 rules");
    expect(screen.getByRole("link", { name: "Documents" }).getAttribute("aria-current")).toBe("page");
    const main = screen.getByRole("main");
    // The only buttons in the view: each rule's Copy citation.
    expect(within(main).getAllByRole("button").map((button) => button.textContent)).toEqual(["Copy citation guide:records#GD-RECORDS-ONE-FILE", "Copy citation guide:records#GD-RECORDS-LATIN-IDS"]);
    for (const act of [/New task/, /Split/, /Open in Flow/, /Open in Spec tree/, /Edit/, /Propose/]) {
      expect(within(main).queryByRole("button", { name: act })).toBeNull();
      expect(within(main).queryByRole("link", { name: act })).toBeNull();
    }
    expect(stub.getGuideTopic).toHaveBeenCalledWith("records", expect.any(AbortSignal));
    expect(stub.getLibrary).not.toHaveBeenCalled();
    // Back to the shelf: Documents, where the guide is now the shelf shown.
    const back = screen.getByRole("link", { name: "SpecEngine guide" });
    expect(back.getAttribute("href")).toBe(sectionHash("alpha", "documents"));
  });

  it("makes each rule an anchor, its ID its id, listed under Rules with a link to its place", async () => {
    const article = await openTopic(client());
    expect(Array.from(article.querySelectorAll("[id]"), (element) => [element.tagName, element.id])).toEqual([
      ["H3", "GD-RECORDS-ONE-FILE"],
      ["H3", "GD-RECORDS-LATIN-IDS"],
    ]);
    const rules = screen.getByRole("navigation", { name: "Rules" });
    expect(within(rules).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["An ID is defined in one file", guideHash("alpha", "records", "GD-RECORDS-ONE-FILE")],
      ["IDs are Latin", guideHash("alpha", "records", "GD-RECORDS-LATIN-IDS")],
    ]);
  });

  it("Copy citation copies guide:<topic>#<ID> and says Copied", async () => {
    const writeText = stubClipboard(() => Promise.resolve());
    await openTopic(client());
    fireEvent.click(screen.getByRole("button", { name: "Copy citation guide:records#GD-RECORDS-LATIN-IDS" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith("guide:records#GD-RECORDS-LATIN-IDS");
    });
    expect(await screen.findByText("Copied", { selector: ".copy-status" })).toBeTruthy();
  });

  it("a rule's route brings that rule into view and focuses it once drawn; a rule the page lacks is said so", async () => {
    await openTopic(client(), guideHash("alpha", "records", "GD-RECORDS-LATIN-IDS"));
    await waitFor(() => {
      expect(document.activeElement?.id).toBe("GD-RECORDS-LATIN-IDS");
    });
    expect(screen.getByRole("link", { name: "IDs are Latin" }).getAttribute("aria-current")).toBe("location");
    act(() => {
      window.location.hash = guideHash("alpha", "records", "GD-RECORDS-ONE-FILE");
    });
    await waitFor(() => {
      expect(document.activeElement?.id).toBe("GD-RECORDS-ONE-FILE");
    });
    act(() => {
      window.location.hash = guideHash("alpha", "records", "GD-RECORDS-GONE");
    });
    expect((await screen.findByText(/This page has no rule/)).textContent).toBe("This page has no rule GD-RECORDS-GONE: this build's guide may have renamed or dropped it.");
  });

  it("an unknown topic's 404 is the CLI's line under lang=en with Retry", async () => {
    const stub = client();
    renderApp(stub, guideHash("alpha", "nope"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The guide topic nope could not be read");
    const message = screen.getByText("spec: no guide topic `nope`; topics: start, records, auto");
    expect(message.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("nope");
  });

  it("its back link opens Documents on the guide's shelf, for the page's life", async () => {
    await openTopic(client());
    fireEvent.click(screen.getByRole("link", { name: "SpecEngine guide" }));
    act(() => {
      window.location.hash = sectionHash("alpha", "documents");
    });
    await screen.findByRole("list", { name: "3 topics" });
    expect(shelves()).toEqual([
      ["Project documents", "false"],
      ["SpecEngine guide", "true"],
    ]);
  });

  it("reached from a card's citation, leaves Documents' shelf as it was (M: the topic setting the shelf on its visit)", async () => {
    await openTopic(client());
    // The nav's Documents, not the back link: the visit alone chose no shelf.
    fireEvent.click(screen.getByRole("link", { name: "Documents" }));
    act(() => {
      window.location.hash = sectionHash("alpha", "documents");
    });
    await screen.findByRole("group", { name: "View" });
    expect(shelves()).toEqual([
      ["Project documents", "true"],
      ["SpecEngine guide", "false"],
    ]);
  });

  it("reached from the shelf, Documents still opens on the guide", async () => {
    const stub = client();
    await openShelf(stub);
    fireEvent.click(screen.getByRole("link", { name: "Records, the spec tree and canon" }));
    await screen.findByRole("heading", { level: 1, name: "Records, the spec tree and canon" });
    act(() => {
      window.location.hash = sectionHash("alpha", "documents");
    });
    await screen.findByRole("list", { name: "3 topics" });
    expect(shelves()[1]).toEqual(["SpecEngine guide", "true"]);
  });

  it("scrolls a rule's heading clear of the sticky header: its scroll margin is the header's height and a space, twice the height where the header wraps", async () => {
    const article = await openTopic(client(), guideHash("alpha", "records", "GD-RECORDS-LATIN-IDS"));
    const heading = article.querySelector<HTMLElement>("#GD-RECORDS-LATIN-IDS");
    expect(heading?.matches(".markdown .md-anchor > .md-heading")).toBe(true);
    const css = appCss.replace(/\/\*[\s\S]*?\*\//g, "");
    expect(css).toMatch(/\n\.markdown \.md-anchor > \.md-heading \{[^}]*\bscroll-margin-top: calc\(var\(--nav-height\) \+ var\(--space-100\)\);/);
    expect(css).toMatch(/@media not all and \(min-width: 68\.75rem\) \{\s*\.markdown \.md-anchor > \.md-heading \{\s*scroll-margin-top: calc\(2 \* var\(--nav-height\) \+ var\(--space-100\)\);/);
    // The header wraps below the width where it stops wrapping: the second rule takes that query's exact complement (RT11 of release-tail).
    expect(css).toMatch(/@media \(min-width: 68\.75rem\) \{\s*\.app-header \{\s*flex-wrap: nowrap;/);
  });

  it("a plain click on a link to the rule already named, in the Rules list or a citation in the text, focuses it again; a modified click is the browser's", async () => {
    const stub = client();
    const text = `${RECORDS.text}\nSee guide:records#GD-RECORDS-LATIN-IDS again.\n`;
    stub.getGuideTopic.mockResolvedValue({ ...structuredClone(RECORDS), text });
    const article = await openTopic(stub, guideHash("alpha", "records", "GD-RECORDS-LATIN-IDS"));
    const rule = article.querySelector<HTMLElement>("#GD-RECORDS-LATIN-IDS");
    await waitFor(() => {
      expect(document.activeElement).toBe(rule);
    });
    const listed = within(screen.getByRole("navigation", { name: "Rules" })).getByRole("link", { name: "IDs are Latin" });
    const cited = await within(article).findByRole("link", { name: "guide:records#GD-RECORDS-LATIN-IDS" });
    for (const link of [listed, cited]) {
      screen.getByRole("heading", { level: 1 }).focus();
      fireEvent.click(link, { ctrlKey: true });
      expect(document.activeElement?.tagName).toBe("H1");
      fireEvent.click(link);
      await waitFor(() => {
        expect(document.activeElement).toBe(rule);
      });
    }
    // Another rule's link changes the hash, and the route focuses it.
    expect(within(screen.getByRole("navigation", { name: "Rules" })).getByRole("link", { name: "An ID is defined in one file" }).getAttribute("href")).toBe(guideHash("alpha", "records", "GD-RECORDS-ONE-FILE"));
  });

  it("speaks Russian in the chrome and keeps the guide's English as served", async () => {
    switchLanguage("ru");
    renderApp(client(), guideHash("alpha", "records"));
    await screen.findByRole("heading", { level: 1, name: "Records, the spec tree and canon" });
    const article = document.querySelector<HTMLElement>(".guide-article");
    await within(article as HTMLElement).findByRole("heading", { level: 3, name: "An ID is defined in one file GD-RECORDS-ONE-FILE" });
    expect(screen.getByRole("link", { name: ru.guide.back }).getAttribute("href")).toBe(sectionHash("alpha", "documents"));
    const copies = screen.getAllByRole("button", { name: new RegExp(`^${ru.guide.copyCitation} `) });
    expect(copies).toHaveLength(2);
    // The translated tool inside the English text says its own language; the rule's heading stays English.
    expect(copies.map((button) => button.closest("[lang]")?.getAttribute("lang"))).toEqual(["ru", "ru"]);
    expect(article?.querySelector("#GD-RECORDS-ONE-FILE")?.closest("[lang]")?.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("navigation", { name: ru.guide.page.rules })).toBeTruthy();
    expect(within(article as HTMLElement).getAllByRole("heading", { level: 2 })[0]?.textContent).toBe("What it solves");
  });
});
