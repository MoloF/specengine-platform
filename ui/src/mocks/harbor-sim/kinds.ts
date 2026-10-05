/** harbor-sim's node kinds: project vocabulary the core never knows (ADR-0031). */
export const nodeKinds = ["domain", "mechanic", "rule"] as const;

/** harbor-sim's spec statuses, front-matter vocabulary shown raw (ADR-0031). */
export const specStatuses = ["accepted", "proposed", "draft"] as const;
