import { useMemo } from "react";
import type { TaskPackage } from "../api/types";
import { CopyButton } from "../ui/CopyButton";

/**
 * The Package tab: the answer exactly as the daemon sent it, every key, nulls and empty lists kept,
 * pretty-printed; whatever its `schema_version`. "Copy package" copies that text.
 */
export function PackagePanel({ id, answer }: { id: string; answer: TaskPackage }) {
  const text = useMemo(() => JSON.stringify(answer, null, 2), [answer]);
  return (
    <div className="task-panel">
      <p className="package-actions">
        <CopyButton text={text} label="Copy package" context={`of ${id} as JSON`} failed="Copy failed: select the text and copy it" />
      </p>
      <figure className="package-figure" aria-label={`Package of ${id} as JSON`}>
        <pre className="task-json">{text}</pre>
      </figure>
    </div>
  );
}
