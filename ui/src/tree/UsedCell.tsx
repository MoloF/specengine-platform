import type { CatalogRow } from "../api/types";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";

/** A row's served uses: the core's counts and orphan flag, as a direction's table and Documents serve them alike. */
export type ServedUsage = Pick<CatalogRow, "used_by" | "mentioned_by" | "orphan">;

/**
 * What uses a row, as served (docs/features/usage-orphans.md "Acceptance criteria", UO11): the count;
 * its mentions, beside a 0 at full weight ("0 · 6 mentions", so a record cited only through mentions
 * never reads as unused), else small after a "+"; then a neutral Orphan badge where the core flags
 * one. The catalog's cell, Documents' too (docs/canon/spec-library.md "The screen").
 */
export function UsedCell({ row }: { row: ServedUsage }) {
  const t = useTranslate();
  const mentions = t("tree.catalog.mentions", { count: row.mentioned_by });
  return (
    <>
      {row.used_by}
      {row.mentioned_by > 0 &&
        (row.used_by === 0 ? (
          <>
            {" · "}
            <span className="catalog-used-cited">{mentions}</span>
          </>
        ) : (
          <>
            {" "}
            <span className="catalog-used-mentions">+ {mentions}</span>
          </>
        ))}
      {row.orphan === true && (
        <>
          {" "}
          <Badge look={{ label: t("tree.catalog.orphan"), tone: "usage-orphan", icon: "linkDangling" }} />
        </>
      )}
    </>
  );
}
