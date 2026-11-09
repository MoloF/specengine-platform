import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DataSource } from "../app/DataSource";
import { enT, ruT } from "../test/text";
import { switchLanguage } from "./index";
import { en } from "./en";
import type * as RuModule from "./ru";

// docs/features/ui-languages.md "Rules and edge cases": a key with no Russian value at run time
// (impossible past `tsc`, LG-1) falls back to English and writes nothing to the console. Here the
// Russian catalog the instance loads lacks `app.mockData`; the setup fails a test that writes to
// the console.

vi.mock("./ru", async (importOriginal) => {
  const actual = await importOriginal<typeof RuModule>();
  const ru = structuredClone(actual.ru);
  Reflect.deleteProperty(ru.app, "mockData");
  return { ru };
});

describe("a Russian value missing at run time", () => {
  it("shows the English value in Russian, the console untouched", () => {
    expect(ruT("app.mockData")).toBe(en.app.mockData);
    switchLanguage("ru");
    render(<DataSource dataSource="mock" scenario={null} />);
    expect(screen.getByText(en.app.mockData)).toBeTruthy();
    expect(document.documentElement.lang).toBe("ru");
  });

  it("leaves every other key Russian", () => {
    expect(ruT("app.overview")).not.toBe(enT("app.overview"));
  });
});
