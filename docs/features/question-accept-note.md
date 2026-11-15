---
class: spec
status: shipped
shipped: 2026-11-14
scope: [ui]
ref: RM-0116; the owner's report of 2026-11-14 from the hosted hub (0.46.0, his game project), written in Russian, paraphrased; scheduled the same day ("do all the fixes and release a new version"); patch release 0.46.1
adrs: []
---

# question-accept-note: a question's Accept says the note is not the answer, and takes an own answer (RM-0116)

## Why

The owner, 2026-11-14: on a question proposal, the Inbox's Accept dialog has one text field, and it is only a decision note. The dialog did not say so. He typed an answer there, and Accept decided the working answer with his words as a note in the record. His two ideas, an "Own answer" in this dialog as the Questions screen has, and a warning when the note diverges from the working answer, are both built, small: the own answer is the remedy, the warning catches the slip.

## Description and interactions

On an Accept whose item offers an own answer (by structure, `answersOf`, never by kind: a question or a decision, never an update, a create or a discrepancy), the dialog offers each option, "Take the working answer" and "Own answer"; an item with options is never preselected, one without checks the working answer on open, today's one-click Accept. Own answer opens a required "Your answer" with the Questions screen's byte count, sent trimmed as the stage's `answer`. The note becomes "Decision note (optional)", described as kept beside the answer and not the answer. A trimmed note unequal to a picked working answer shows a warning: the first Accept stages nothing and focuses "Use the note as my answer", which moves the note into Your answer. Every other Accept and every Reject is unchanged. Canon: `docs/canon/plain-questions.md` "UI", `decision-staging.md` "UI", `proposal-decision.md`.

## Data

No daemon change: the stage already takes `answer` (`--answer T`), judged as today (2 048 bytes). Stage bodies: an option `{decision: "approve", option: N, answer: null, canon: null, editorial: null, note}`; the working answer `option: null, answer: null`; Own answer `option: null, answer: <trimmed>`; `note` trimmed, else null. Strings, both catalogs: `inbox.dialog.ownAnswer` "Own answer", `inbox.dialog.writeAnswer` "Write your answer first.", `inbox.decision.accept.noteField` "Decision note (optional)", `inbox.decision.accept.noteHint` "Kept in the decision's record beside the answer. It is not the answer: to answer in your own words, choose Own answer.", `inbox.dialog.noteNotAnswer` "Your note is not an answer. Accept records the working answer, with this note beside it.", `inbox.dialog.useNoteAsAnswer` "Use the note as my answer"; `round.takeWorking`, `round.yourAnswer` reused.

## Rules and edge cases

Own answer is offered where the item takes one and staged only when picked and written; the first Accept under the warning stages nothing and announces it, the second stages; agent text shows through `AskedText`, `shownText` and `shownSlot`, never as markup.

## Acceptance criteria

All but QA10 are vitest under `ui/`, in `src/inbox/DecisionDialog.test.tsx` unless named. Fixtures: Q1, a question with no options, working answer `Use 2 slots`, price `a rebalance`; Q2, two options and a price; D1, a discrepancy with options, no price; U1, an update. Evidence: the tests citing each ID; the UI review, ACCEPT after iteration 2; the whole UI suite, lint, typecheck and the build green.

- [x] QA1 — **Who offers it.** Q2's Accept lists option 1, option 2, "Take the working answer" and "Own answer", none checked, Accept off; D1's has no Own answer; U1's has no answer radio and keeps "Note for the record (optional)". Mutation: Own answer by `answersOf` alone, so U1 gains it.
- [x] QA2 — **No options.** Q1 opens with "Take the working answer" checked beside "Own answer"; an immediate Accept sends `option: null, answer: null, note: null`. Mutation: nothing checked on open.
- [x] QA3 — **Own answer staged.** Q1, Own answer `  Use 3 slots  `, note `per the playtest`: `answer: "Use 3 slots"`, `note: "per the playtest"`, `option: null`, `canon: null`. Mutation: the answer sent as the note.
- [x] QA4 — **Empty own answer.** Blank text sends nothing; "Write your answer first." under Your answer, focus there. Mutation: an empty `answer` staged.
- [x] QA5 — **The note's words.** On Q1 and Q2 the label is "Decision note (optional)", its description holds "It is not the answer". Mutation: the hint dropped, or shown on U1.
- [x] QA6 — **The warning.** Q1, working answer checked, note `Use 3 slots`: the warning and its button; the first Accept calls no `onSubmit`, focuses the button, the announcer holds the warning; the second stages the working answer with the note. None for `  Use 2 slots `, an empty note, an option on Q2, or Own answer. Mutations: the first Accept stages; the warning on an option pick.
- [x] QA7 — **Use the note as my answer.** One click: Your answer `Use 3 slots`, the note empty, Own answer checked, focus in Your answer; the next Accept stages `answer: "Use 3 slots"`, `note: null`. Mutation: the note kept as well.
- [x] QA8 — **A revision while open.** With Own answer and text, a version still offering it keeps the pick and both texts; one that no longer does (Q1 revised to D1's shape) clears the pick, keeps the texts. Mutation: Your answer cleared on revision.
- [x] QA9 — **Through the view** (`InboxView.test.tsx`, the mock client): an own answer staged from the Inbox reaches `stageDecision` with the body above and the card's staged choice shows it; `catalog.test.ts`, `policy.test.ts`, the UI suite, lint, typecheck and the build pass.
- [x] QA10 — **[look]** The orchestrator's, 2026-11-14, Chrome at 1440x900 on PR-0048 of the mock, both languages: the radios, the note's label and hint, the warning and "Use the note as my answer"; the hint and the warning share one grid cell with a hidden placeholder, so the actions never move (a small blank under the hint accepted); the Russian reads well.

## Out of scope

Reject, Needs clarification, Defer; the Questions screen and a card's Answer…; `--canon` from the UI; comparing the note with an option's label; any daemon, CLI or MCP change.

## Implementation

| Module | What it does |
|---|---|
| UI `src/inbox/DecisionDialog.tsx` | the answer radios, Your answer, the note's words, the warning and its hold, the stage's `answer` |
| UI `src/round/DraftText.tsx` (new), `round/RoundItemCard.tsx` | the byte-counted answer field, shared by the sheet and the dialog |
| UI `src/i18n/`, `src/styles/app.css` | the six keys, the warning's cell |
| tests | `inbox/DecisionDialog.test.tsx` (new), `InboxView.test.tsx`, `ItemLead.test.tsx`, `mocks/decision.smoke.test.tsx` |

**Departures**, accepted: a revision keeps a working-answer pick too while still offered, and re-arms the warning's hold; an item without options gets the sheet's radio group legend, "Answer <id>" (`round.answerLegend`, the ID for screen readers alone); the Russian for Own answer is the Questions screen's word for it (`round.yourAnswer`'s); `inbox.dialog.workingRecorded` is gone, no view using it.

**Release**: the patch 0.46.1, `docs/features/board-subtasks.md` "Implementation".
