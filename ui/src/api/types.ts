// The only place app code takes domain types from: the provisional ones today, the types generated
// from Rust (src/api/generated/) once `spec serve` exists (ui/README.md "Contract seam").
export type * from "./provisional";
// The check's known verdicts as a value: the Health screen labels a verdict through this tuple,
// never by spelling it (docs/features/ui-health.md "Rules and edge cases").
export { KNOWN_CHECK_VERDICTS } from "./provisional";
