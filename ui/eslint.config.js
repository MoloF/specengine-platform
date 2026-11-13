// Flat config (ui/README.md "Gates"): typescript-eslint and react-hooks only, no template extras.
// The seam (docs/features/ui-shell.md "Data"): only src/main.tsx picks the mock, only src/api/ reads
// the provisional types; app code imports domain types from src/api/types.ts. Markdown
// (docs/features/ui-markdown.md, ADR-0036): only src/markdown/ imports react-markdown or remark-gfm.
// Languages (docs/features/ui-languages.md D3, ADR-0072): only src/i18n/ imports i18next or react-i18next.
// The corpus map (docs/features/corpus-map.md "Acceptance criteria", CM11; ADR-0080): only src/map/ imports d3-force.
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig, globalIgnores } from "eslint/config";
import tseslint from "typescript-eslint";

const MOCKS = {
  regex: "(^|/)mocks(/|$)",
  message: "Only src/main.tsx picks the client implementation; app code talks to SpecEngineClient.",
};
const QUERY = {
  name: "@tanstack/react-query",
  message: "Query hooks live in src/api/ and call only SpecEngineClient.",
};
const MARKDOWN = {
  // The packages and any of their subpaths: react-markdown, remark-gfm, remark-gfm/lib/....
  regex: "^(react-markdown|remark-gfm)(/|$)",
  message: "Only src/markdown/ renders markdown (ADR-0036): import its Prose, never the packages.",
};
const I18N = {
  // The packages and any of their subpaths: i18next, react-i18next, i18next/...; never i18next-foo.
  regex: "^(i18next|react-i18next)(/|$)",
  message: "Only src/i18n/ imports i18next (ADR-0072): read `t` from its useTranslate(), or take a Translate argument.",
};
const D3 = {
  // The layout engine and any of its subpaths: d3-force, d3-force/...; never another d3 package.
  regex: "^d3-force(/|$)",
  message: "Only src/map/ lays out the corpus map with d3-force (ADR-0080).",
};
const PROVISIONAL = {
  // With or without an extension: ./provisional, ../api/provisional.js, ./provisional.ts.
  regex: "(^|/)provisional(\\.[jt]sx?)?$",
  message: "Import domain types from src/api/types.ts: provisional types are replaced, never imported directly.",
};

export default defineConfig([
  globalIgnores(["dist/", "coverage/", "node_modules/"]),
  {
    files: ["**/*.{ts,tsx}"],
    extends: [tseslint.configs.strictTypeChecked, reactHooks.configs.flat["recommended-latest"]],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      "no-alert": "error",
      "no-restricted-globals": ["error", "alert", "confirm", "prompt"],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
      "no-restricted-imports": ["error", { paths: [QUERY], patterns: [MOCKS, PROVISIONAL, MARKDOWN, I18N, D3] }],
    },
  },
  {
    files: ["src/main.tsx", "src/mocks/**"],
    rules: { "no-restricted-imports": ["error", { paths: [QUERY], patterns: [PROVISIONAL, MARKDOWN, I18N, D3] }] },
  },
  {
    files: ["src/api/**"],
    rules: { "no-restricted-imports": ["error", { patterns: [MOCKS, MARKDOWN, I18N, D3] }] },
  },
  {
    files: ["src/markdown/**"],
    rules: { "no-restricted-imports": ["error", { paths: [QUERY], patterns: [MOCKS, PROVISIONAL, I18N, D3] }] },
  },
  {
    files: ["src/i18n/**"],
    rules: { "no-restricted-imports": ["error", { paths: [QUERY], patterns: [MOCKS, PROVISIONAL, MARKDOWN, D3] }] },
  },
  {
    files: ["src/map/**"],
    rules: { "no-restricted-imports": ["error", { paths: [QUERY], patterns: [MOCKS, PROVISIONAL, MARKDOWN, I18N] }] },
  },
]);
