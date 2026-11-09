---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-09
---

# UI languages: the chrome in English and Russian

## State

Not built. v4 step 1b `ui-languages` (RM-0082, `docs/features/ui-languages.md`) builds it. Until it ships the chrome is English only, each string sits in its view or module (1a's in `ui/src/workspace/text.ts`), letter hotkeys match `event.key`, and `crates/specengine-eval/tests/anonymity.rs` admits Russian text only in the fixture corpora `fixtures/spec-b` and `fixtures/token-calibration`.

<a id="the-rule"></a>
## The rule

The chrome, every word the UI writes itself, is English or Russian, each viewer's choice: one per browser, for every project, on a local daemon and on a hub alike. With no choice stored, a browser whose first language is Russian starts in Russian, any other in English. Spec content (titles, bodies, names, a project's vocabulary, workspace titles) shows as written, in any script. The daemon's, the CLI's and MCP's text (refusals, notes, gap causes, messages, commands) stays English and verbatim on a Russian screen: the UI never translates what it did not write.

Every chrome string is a key of one catalog, `ui/src/i18n/`. English, `en.ts`, is the source of the keys; Russian, `ru.ts`, is typed against it, so a missing key fails the build. `ru.ts` is the only file of the repository holding Russian text that `anonymity.rs` does not already exempt as a fixture corpus; keys, comments, tests and documentation stay English. The library is i18next with react-i18next, at exact pins, imported by no module outside `ui/src/i18n/`. A third language is a new decision. ADR-0072.
