import type { ShownNode } from "../api/types";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";

// One holder's facts, shared by the Spec tree's node pane and the document view's Technical details
// (docs/features/doc-modal.md "Acceptance criteria", DM-12).

/** One holder's header facts: kind, place, status, rev, size, span hash, flags. */
export function Facts({ holder }: { holder: ShownNode }) {
  const t = useTranslate();
  return (
    <dl className="facts node-facts">
      <div className="fact">
        <dt>{t("tree.node.kind")}</dt>
        <dd className="mono" lang={holder.kind === null ? undefined : "en"}>
          {holder.kind ?? "-"}
        </dd>
      </div>
      <div className="fact">
        <dt>{t("tree.node.where")}</dt>
        <dd className="mono">
          {holder.path}:{holder.line}-{holder.end_line}
        </dd>
      </div>
      {holder.status !== null && (
        <div className="fact">
          <dt>{t("tree.node.status")}</dt>
          <dd>
            <span className="status-tag" lang="en">
              {holder.status}
            </span>
          </dd>
        </div>
      )}
      {holder.rev !== null && (
        <div className="fact">
          <dt>{t("tree.node.rev")}</dt>
          <dd className="mono">{holder.rev}</dd>
        </div>
      )}
      <div className="fact">
        <dt>{t("tree.node.size")}</dt>
        <dd>{t("tree.binding.tokens", { count: holder.tokens_est })}</dd>
      </div>
      <div className="fact fact-wide">
        <dt>{t("tree.node.spanHash")}</dt>
        <dd className="mono">{holder.span_hash}</dd>
      </div>
      {(holder.archived || !holder.utf8) && (
        <div className="fact">
          <dt>{t("tree.node.flags")}</dt>
          <dd className="flags">
            {holder.archived && (
              <span className="plain-tag">
                <Icon name="archived" />
                {t("tree.row.archived")}
              </span>
            )}
            {!holder.utf8 && <span className="plain-tag">{t("tree.node.notUtf8")}</span>}
          </dd>
        </div>
      )}
    </dl>
  );
}
