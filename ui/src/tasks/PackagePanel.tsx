import { useMemo } from "react";
import type { TaskPackage } from "../api/types";
import { useTranslate } from "../i18n";
import { CopyButton } from "../ui/CopyButton";

/**
 * The Package tab: the answer exactly as the daemon sent it, every key, nulls and empty lists kept,
 * pretty-printed; whatever its `schema_version`. "Copy package" copies that text.
 */
export function PackagePanel({ id, answer }: { id: string; answer: TaskPackage }) {
  const t = useTranslate();
  const text = useMemo(() => JSON.stringify(answer, null, 2), [answer]);
  return (
    <div className="task-panel">
      <p className="package-actions">
        <CopyButton text={text} label={t("tasks.package.copy")} context={t("tasks.package.copyContext", { id })} failed={t("tasks.package.copyFailed")} />
      </p>
      <figure className="package-figure" aria-label={t("tasks.package.label", { id })}>
        <pre className="task-json">{text}</pre>
      </figure>
    </div>
  );
}
