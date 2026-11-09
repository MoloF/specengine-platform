import type { RevisionChange } from "../api/types";
import { useTranslate } from "../i18n";
import { revisionText } from "./revisions";

/**
 * The review's `revisions`, one node per line as `spec review` writes them, in the daemon's order
 * (`docs/features/rev-discipline.md` "UI"): `to` as sent; a refused raise reads "refused", its
 * reason under it in the daemon's words.
 */
export function RevisionList({ entries }: { entries: readonly RevisionChange[] }) {
  const t = useTranslate();
  return (
    <ul className="revision-list">
      {entries.map((entry, index) => (
        <li key={`${String(index)}-${entry.id}`}>
          <span className="mono revision-line">{revisionText(entry, t)}</span>
          {entry.refused !== null && (
            <span className="revision-refused" lang="en">
              {entry.refused}
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}
