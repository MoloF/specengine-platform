import type { TraceCounts } from "../api/types";
import { countParts } from "../flow/labels";

/**
 * A bar drawn from served counts, a part per state with something in it. It adds nothing a line does
 * not say: wherever it stands, the parts stand beside it in words, so no state goes by colour alone.
 */
export function TraceBar({ counts, className }: { counts: TraceCounts; className?: string }) {
  return (
    <span className={className === undefined ? "trace-bar" : `trace-bar ${className}`} aria-hidden="true">
      {countParts(counts).map((part) => (
        <span key={part.key} className={`trace-bar-part tone-${part.look.tone}${part.hollow ? " is-hollow" : ""}`} style={{ flexGrow: part.count }} />
      ))}
    </span>
  );
}
