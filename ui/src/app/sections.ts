/** The seven sections of the nav, in order (07 §3 "Web UI — screens"), each with what it shows. */
export const SECTIONS = [
  { id: "inbox", label: "Inbox", about: "Proposals waiting for your decision." },
  {
    id: "tasks",
    label: "Tasks",
    about: "Tasks from draft to done: what waits for you, each task's plan, spec changes and runs.",
  },
  {
    id: "roadmap",
    label: "Roadmap",
    about: "The plan: its items by release, the MVP cut, the pool, what each item waits on and the tasks executing it.",
  },
  {
    id: "tree",
    label: "Spec tree",
    about: "The business-logic tree and each node's text, links, code bindings, bundle and proposals.",
  },
  { id: "graph", label: "Graph", about: "Nodes and their links, with an impact mode." },
  { id: "health", label: "Health", about: "What is left, drift, budgets and W metrics." },
  {
    id: "questions",
    label: "Questions",
    about: "The question round: a sheet in the project's language and the answers.",
  },
] as const;

export type SectionId = (typeof SECTIONS)[number]["id"];

export function isSectionId(value: string): value is SectionId {
  return SECTIONS.some((section) => section.id === value);
}
