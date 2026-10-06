import type { TaskNotFound, TaskPackage } from "../api/types";

/** The package version this screen reads (`docs/features/task-package.md` "Data": `schema_version` 1). */
export const READ_SCHEMA_VERSION = 1;

/** The exit-1 document of an unknown T (the daemon's 404): it carries no `schema_version`. */
export function isTaskNotFound(answer: TaskPackage | TaskNotFound): answer is TaskNotFound {
  return !("schema_version" in answer);
}
