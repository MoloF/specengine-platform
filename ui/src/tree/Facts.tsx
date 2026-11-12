import type { ShownNode } from "../api/types";
import { useTranslate, type Translate } from "../i18n";
import { Icon } from "../ui/Icon";
import type { Property } from "../ui/PropertyList";

// One holder's facts, one list of rows for both of its renderings (docs/features/design-sweep.md
// "Acceptance criteria", DS7): the Spec tree's node shows them as a PropertyList, the document view's
// Technical details as `dl.facts` (docs/features/doc-modal.md "Acceptance criteria", DM-12).

/** The row a fact spanning the whole line takes in `dl.facts`. */
const WIDE = "spanHash";

/** One holder's facts in order: kind, place, status, rev, size, span hash, flags; null where the holder has none. */
export function factsOf(holder: ShownNode, t: Translate): Property[] {
  return [
    {
      key: "kind",
      label: t("tree.node.kind"),
      value: (
        <span className="mono" lang={holder.kind === null ? undefined : "en"}>
          {holder.kind ?? "-"}
        </span>
      ),
    },
    {
      key: "where",
      label: t("tree.node.where"),
      value: (
        <span className="mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </span>
      ),
    },
    {
      key: "status",
      label: t("tree.node.status"),
      value:
        holder.status === null ? null : (
          <span className="status-tag" lang="en">
            {holder.status}
          </span>
        ),
    },
    { key: "rev", label: t("tree.node.rev"), value: holder.rev === null ? null : <span className="mono">{holder.rev}</span> },
    { key: "size", label: t("tree.node.size"), value: t("tree.binding.tokens", { count: holder.tokens_est }) },
    { key: WIDE, label: t("tree.node.spanHash"), value: <span className="mono">{holder.span_hash}</span> },
    {
      key: "flags",
      label: t("tree.node.flags"),
      value:
        holder.archived || !holder.utf8 ? (
          <span className="flags">
            {holder.archived && (
              <span className="plain-tag">
                <Icon name="archived" />
                {t("tree.row.archived")}
              </span>
            )}
            {!holder.utf8 && <span className="plain-tag">{t("tree.node.notUtf8")}</span>}
          </span>
        ) : null,
    },
  ];
}

/** One holder's facts as `dl.facts`, the document view's Technical details: `factsOf`'s rows, a null one left out. */
export function Facts({ holder }: { holder: ShownNode }) {
  const t = useTranslate();
  return (
    <dl className="facts node-facts">
      {factsOf(holder, t)
        .filter((fact) => fact.value !== null)
        .map((fact) => (
          <div key={fact.key} className={fact.key === WIDE ? "fact fact-wide" : "fact"}>
            <dt>{fact.label}</dt>
            <dd>{fact.value}</dd>
          </div>
        ))}
    </dl>
  );
}
