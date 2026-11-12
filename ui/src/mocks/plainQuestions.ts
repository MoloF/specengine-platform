import { proposal, stamp, type MockProject } from "./build";
import { logItemEvent, type MockBoard } from "./cards";
import { TIDE_FILE } from "./harbor-sim/fixtures";

// The `plain-questions` scenario (docs/features/plain-questions.md "Acceptance criteria", PQ12,
// PQ14): every read as `normal`, but harbor-sim's queue holds two more open questions on C-0001,
// "Write the tide table foreword". PR-0059 is an agent's, raised with all four new fields: a gist, why
// it matters, two priced options and its recommendation; its own text is the agent's, IDs, paths and
// code names and all. PR-0060 is a person's, raised from the page, with none of them: it renders as an
// item did before. Both lead the round (high severity) and stand in the task's Activity.

/** The agent's question with a gist. */
export const PLAIN_QUESTION = "PR-0059";

/** The person's question without one. */
export const PERSON_QUESTION = "PR-0060";

/** The task both are raised for. */
export const PLAIN_CARD = "C-0001";

/** PR-0059's fields as the agent sent them: the four new ones and the question's own. */
export const PLAIN_FIELDS = {
  text: "Does RULE-TIDE-WINDOW apply when sim::tide::entry_window_system() runs before tide_table::parse_sample() has loaded the day's table (docs/spec/tides/tide-cycle.md)?\nAt 00:00 the system reads an empty table and lets every ship in.",
  gist: "Should a deep ship wait at the anchorage until the day's tide table is read?",
  matters: "The foreword tells skippers which one holds, and two agent tasks build on the answer.\nA wrong guess lets a deep ship ground at low water.",
  options: [
    { label: "Wait at the anchorage", effect: "Deep ships hold until the day's table is read.", price: "None: as built." },
    { label: "Enter on yesterday's table", effect: "Ships keep moving through midnight.", price: "A grounding risk on spring tides; the tide window rewritten." },
  ],
  recommendation: 0,
  working_answer: "Deep ships wait at the anchorage until the day's table is read.",
  price_of_other: "The tide window rule rewritten and the foreword's first page redone.",
} as const;

/** PR-0060's texts, a person's, without a gist. */
export const PERSON_FIELDS = {
  text: "Does the foreword name the chart datum by its local name?",
  working_answer: "Yes, with the international name after it in brackets.",
  price_of_other: "One line of the foreword rewritten.",
} as const;

/** Adds the two questions to harbor-sim's queue and their raising events to its board. */
export function plainQuestionsScenario(harbor: MockProject, board: MockBoard | undefined, at: number): void {
  const asked = stamp(at, 45);
  const askedByPerson = stamp(at, 35);
  harbor.proposals.push(
    proposal({
      id: PLAIN_QUESTION,
      project: harbor.project.slug,
      kind: "question",
      severity: "high",
      target_id: "RULE-TIDE-WINDOW",
      target_path: TIDE_FILE,
      target_ids: ["RULE-TIDE-WINDOW", "MEC-TIDE-TABLES"],
      summary: PLAIN_FIELDS.text,
      gist: PLAIN_FIELDS.gist,
      matters: PLAIN_FIELDS.matters,
      options: PLAIN_FIELDS.options.map((option) => ({ ...option })),
      recommendation: PLAIN_FIELDS.recommendation,
      working_answer: PLAIN_FIELDS.working_answer,
      price_of_other: PLAIN_FIELDS.price_of_other,
      author: { type: "agent", role: "rust-developer", model: "claude-opus-5-5", run: "R-2290" },
      card_id: PLAIN_CARD,
      created_at: asked,
    }),
    proposal({
      id: PERSON_QUESTION,
      project: harbor.project.slug,
      kind: "question",
      severity: "high",
      target_id: "MEC-TIDE-TABLES",
      target_ids: ["MEC-TIDE-TABLES"],
      summary: PERSON_FIELDS.text,
      working_answer: PERSON_FIELDS.working_answer,
      price_of_other: PERSON_FIELDS.price_of_other,
      author: { type: "human", role: null, model: null, run: null },
      card_id: PLAIN_CARD,
      created_at: askedByPerson,
    }),
  );
  if (board !== undefined) {
    logItemEvent(board, PLAIN_QUESTION, "proposal.created", { as: "agent" }, asked);
    logItemEvent(board, PERSON_QUESTION, "proposal.created", { as: "Mira Holt" }, askedByPerson);
  }
}
