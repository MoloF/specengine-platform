// Every chrome string the workspaces bring (docs/features/workspaces.md "The UI", D12): the switcher,
// the module bar, the config notes, the nav's wait and the Board's flows line. A string around data
// is a function of that data, so ui-languages (RM-0082) turns this one module into catalog keys
// without touching a component. Section labels stay in src/app/sections.ts. Names, titles, roots
// and notes are the project's data: shown as served, never here.

export const WORKSPACE_TEXT = {
  /** The switcher's label. */
  workspaceLabel: "Workspace",
  /** The switcher's description: a lens, never an identity or a right (D6). */
  workspaceHint: "Orders the sections for one trade. It changes no right, and every section stays in the palette.",
  /** The module bar's group name. */
  modulesLabel: "Modules",
  /** The bar's first button: no module, the whole project. */
  allModules: "All modules",
  /** The config notes' title. */
  notesTitle: "Config notes",
  /** The disclosure's button: the title and how many lines it holds. */
  notesButton: (count: number) => `Config notes (${String(count)})`,
  /** The disclosure's line above a failed read's message, no answer kept: the nav lists every section. */
  readFailed: "The workspaces could not be read: every section is shown.",
  /** The same, the last answer kept: the nav still lists its workspace's sections. */
  readFailedKept: "The workspaces could not be read again: showing the last read.",
  /** The nav's skeleton while the workspaces are read. */
  navPending: "Reading the workspaces",
  /** The Board's line when the workspace hides some of its flows. */
  flowsShown: (shown: number, all: number) => `Showing ${String(shown)} of ${String(all)} card flows.`,
  /** The Board's button showing every flow for the visit. */
  showAllFlows: "Show all flows",
  /** The same button once every flow is shown. */
  showWorkspaceFlows: "Show this workspace's flows",
};

export type WorkspaceText = typeof WORKSPACE_TEXT;
