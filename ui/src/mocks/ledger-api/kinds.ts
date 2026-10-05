/** ledger-api's node kinds: project vocabulary the core never knows (ADR-0031). */
export const nodeKinds = ["service", "endpoint", "policy"] as const;

/** ledger-api's spec statuses, front-matter vocabulary shown raw (ADR-0031). */
export const specStatuses = ["accepted", "draft", "deprecated"] as const;
