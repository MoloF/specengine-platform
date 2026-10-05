/** The six sections of the nav, in order (07 §3 "Web UI — screens"), and the slice building each. */
export const SECTIONS = [
  { id: "inbox", label: "Inbox", slice: null, about: "Proposals waiting for your decision." },
  {
    id: "tasks",
    label: "Tasks",
    slice: "ui-tasks",
    about: "The task board from draft to accepted, with briefs, plans and runs.",
  },
  {
    id: "tree",
    label: "Spec tree",
    slice: "ui-tree-node",
    about: "The business-logic tree and each node's text, links and bindings.",
  },
  { id: "graph", label: "Graph", slice: "ui-graph", about: "Nodes and their links, with an impact mode." },
  { id: "health", label: "Health", slice: "ui-health-round", about: "What is left, drift, budgets and W metrics." },
  {
    id: "questions",
    label: "Questions",
    slice: "ui-health-round",
    about: "The question round: a sheet in the project's language and the pasted answers.",
  },
] as const;

export type SectionId = (typeof SECTIONS)[number]["id"];

export function isSectionId(value: string): value is SectionId {
  return SECTIONS.some((section) => section.id === value);
}
