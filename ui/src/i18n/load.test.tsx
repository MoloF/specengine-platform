import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { en } from "./en";
import type * as IndexModule from "./index";
import type * as RuModule from "./ru";
import { ru } from "./ru";

// LS-6 of docs/features/ui-languages-sweep.md: Russian is a chunk of its own, loaded on demand. Each
// test takes a fresh instance (a fresh module graph) over a mocked `./ru` whose arrival the test
// holds back, fails or counts: `reads` counts each read of its catalog, one per import that lands.
// The probe shows one key and the Language select, as the shell does; the bootstrap's gate is
// main.tsx's `readyLanguage().then(render)`.

/** The mocked chunk: held until `arrive()`, failing while `failing`, counting its imports and the reads of its catalog. */
interface Chunk {
  imports: number;
  reads: number;
  failing: boolean;
  arrive: () => void;
  held: Promise<void>;
}

const chunk: Chunk = {
  imports: 0,
  reads: 0,
  failing: false,
  arrive: () => undefined,
  held: Promise.resolve(),
};

function holdChunk(): void {
  chunk.held = new Promise<void>((resolve) => {
    chunk.arrive = resolve;
  });
}

/** A fresh instance and the views on it, `./ru` mocked as `chunk` says; nothing imported yet. */
async function fresh() {
  vi.resetModules();
  vi.doMock("./ru", async (): Promise<typeof RuModule> => {
    chunk.imports += 1;
    await chunk.held;
    if (chunk.failing) {
      throw new Error("chunk ru failed to load");
    }
    return {
      get ru() {
        chunk.reads += 1;
        return ru;
      },
    };
  });
  const i18n: typeof IndexModule = await import("./index");
  const { LanguageSwitcher } = await import("../app/LanguageSwitcher");
  function Probe() {
    const t = i18n.useTranslate();
    return (
      <main>
        <h1>{t("app.overview")}</h1>
        <LanguageSwitcher />
      </main>
    );
  }
  return { i18n, Probe };
}

/** Lets the module loader and every settled promise run, React's updates inside act; no timer is read. */
async function flush() {
  await act(async () => {
    for (let step = 0; step < 20; step += 1) {
      await new Promise<void>((resolve) => {
        const channel = new MessageChannel();
        channel.port1.onmessage = () => {
          channel.port1.close();
          resolve();
        };
        channel.port2.postMessage(null);
      });
    }
  });
}

function heading(): string | null {
  return screen.queryByRole("heading", { level: 1 })?.textContent ?? null;
}

function select(): HTMLSelectElement {
  return screen.getByRole<HTMLSelectElement>("combobox");
}

afterEach(() => {
  chunk.imports = 0;
  chunk.reads = 0;
  chunk.failing = false;
  chunk.held = Promise.resolve();
  vi.doUnmock("./ru");
  vi.resetModules();
});

describe("the start (R1, R2)", () => {
  it("imports nothing and renders at once when the page starts in English", async () => {
    const { i18n, Probe } = await fresh();
    let rendered = false;
    await act(() =>
      i18n.readyLanguage().then(() => {
        rendered = true;
        render(<Probe />);
      }),
    );
    expect(rendered).toBe(true);
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect([chunk.imports, chunk.reads]).toEqual([0, 0]);
  });

  it("renders nothing until Russian's chunk lands, then Russian first, lang ru", async () => {
    window.localStorage.setItem("specengine.language", "ru");
    holdChunk();
    const { i18n, Probe } = await fresh();
    let rendered = false;
    void i18n.readyLanguage().then(() => {
      rendered = true;
      render(<Probe />);
    });
    await flush();
    expect(rendered).toBe(false);
    expect(document.body.textContent).toBe("");
    chunk.arrive();
    await flush();
    expect(rendered).toBe(true);
    expect(heading()).toBe(ru.app.overview);
    expect(document.documentElement.lang).toBe("ru");
    expect(chunk.reads).toBe(1);
  });

  it("past the wait renders English, then turns Russian when the chunk lands", async () => {
    window.localStorage.setItem("specengine.language", "ru");
    holdChunk();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const { i18n, Probe } = await fresh();
    void i18n.readyLanguage().then(() => {
      render(<Probe />);
    });
    await flush();
    expect(heading()).toBeNull();
    vi.advanceTimersByTime(i18n.LANGUAGE_WAIT_MS - 1);
    await flush();
    expect(heading()).toBeNull();
    vi.advanceTimersByTime(1);
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect(i18n.LANGUAGE_WAIT_MS).toBe(2000);
    chunk.arrive();
    await flush();
    expect(heading()).toBe(ru.app.overview);
    expect(document.documentElement.lang).toBe("ru");
  });

  it("keeps English when the viewer chose it while the late chunk was on its way", async () => {
    window.localStorage.setItem("specengine.language", "ru");
    holdChunk();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const { i18n, Probe } = await fresh();
    void i18n.readyLanguage().then(() => {
      render(<Probe />);
    });
    vi.advanceTimersByTime(i18n.LANGUAGE_WAIT_MS);
    await flush();
    expect(heading()).toBe(en.app.overview);
    fireEvent.change(select(), { target: { value: "en" } });
    chunk.arrive();
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect(window.localStorage.getItem("specengine.language")).toBe("en");
  });
});

describe("a chunk that fails (R3)", () => {
  it("shows English, keeps the stored choice and says so; the next choice imports again", async () => {
    window.localStorage.setItem("specengine.language", "ru");
    chunk.failing = true;
    const { i18n, Probe } = await fresh();
    await act(() =>
      i18n.readyLanguage().then(() => {
        render(<Probe />);
      }),
    );
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect(window.localStorage.getItem("specengine.language")).toBe("ru");
    expect(screen.getByRole("status").textContent).toBe(en.language.failed);
    expect(select().value).toBe("en");

    chunk.failing = false;
    fireEvent.change(select(), { target: { value: "ru" } });
    expect(screen.queryByRole("status")).toBeNull();
    await flush();
    // The failed import is not kept: the choice imports again, and that one lands.
    expect([chunk.imports, chunk.reads]).toEqual([2, 1]);
    expect(heading()).toBe(ru.app.overview);
    expect(document.documentElement.lang).toBe("ru");
  });

  it("says so after a choice whose chunk fails, until the next choice", async () => {
    chunk.failing = true;
    const { Probe } = await fresh();
    render(<Probe />);
    fireEvent.change(select(), { target: { value: "ru" } });
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(screen.getByRole("status").textContent).toBe(en.language.failed);
    expect(window.localStorage.getItem("specengine.language")).toBe("ru");
    fireEvent.change(select(), { target: { value: "en" } });
    expect(screen.queryByRole("status")).toBeNull();
  });
});

describe("a choice before the chunk is in (R4, R5)", () => {
  it("keeps the language and lang while pending, the select showing Russian busy, focus kept; imports once for two choices", async () => {
    holdChunk();
    const { Probe } = await fresh();
    render(<Probe />);
    select().focus();
    fireEvent.change(select(), { target: { value: "ru" } });
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect(select().value).toBe("ru");
    expect(select().getAttribute("aria-busy")).toBe("true");
    expect(document.activeElement).toBe(select());

    fireEvent.change(select(), { target: { value: "ru" } });
    chunk.arrive();
    await flush();
    expect(heading()).toBe(ru.app.overview);
    expect(document.documentElement.lang).toBe("ru");
    expect(select().getAttribute("aria-busy")).toBeNull();
    expect(document.activeElement).toBe(select());
    // One import for both choices (M9: unmemoised, each choice imports and reads again).
    expect(chunk.reads).toBe(1);

    // Loaded: back and forth at once, no import more.
    fireEvent.change(select(), { target: { value: "en" } });
    expect(heading()).toBe(en.app.overview);
    fireEvent.change(select(), { target: { value: "ru" } });
    expect(heading()).toBe(ru.app.overview);
    expect(chunk.reads).toBe(1);
  });

  it("lets the last choice win: English chosen during the load stays when Russian lands", async () => {
    holdChunk();
    const { Probe } = await fresh();
    render(<Probe />);
    fireEvent.change(select(), { target: { value: "ru" } });
    await flush();
    fireEvent.change(select(), { target: { value: "en" } });
    expect(select().value).toBe("en");
    expect(select().getAttribute("aria-busy")).toBeNull();
    chunk.arrive();
    await flush();
    expect(heading()).toBe(en.app.overview);
    expect(document.documentElement.lang).toBe("en");
    expect(window.localStorage.getItem("specengine.language")).toBe("en");
  });

  it("names each option in its own language before Russian's chunk is in", async () => {
    holdChunk();
    const { i18n, Probe } = await fresh();
    render(<Probe />);
    expect([...select().options].map((option) => [option.value, option.textContent, option.lang])).toEqual([
      ["en", "English", "en"],
      ["ru", "\u0420\u0443\u0441\u0441\u043a\u0438\u0439", "ru"],
    ]);
    expect(i18n.languageName("ru")).toBe("\u0420\u0443\u0441\u0441\u043a\u0438\u0439");
    expect(chunk.reads).toBe(0);
  });
});

describe("loadLanguage", () => {
  it("is English at once, Russian once per page, and never rejects", async () => {
    chunk.failing = true;
    const { i18n } = await fresh();
    await expect(i18n.loadLanguage("en")).resolves.toBe(true);
    await expect(i18n.loadLanguage("ru")).resolves.toBe(false);
    chunk.failing = false;
    await expect(i18n.loadLanguage("ru")).resolves.toBe(true);
    await expect(i18n.loadLanguage("ru")).resolves.toBe(true);
    expect(chunk.reads).toBe(1);
  });
});
