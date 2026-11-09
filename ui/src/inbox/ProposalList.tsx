import type { InboxEntry } from "../api/types";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { formatAge, formatUtc } from "../ui/time";
import { severityLook, statusLook } from "./labels";
import { summaryOf } from "./summary";
import { targetsOf } from "./targets";
import { useTranslate } from "../i18n";

/**
 * The queue as one Tab stop: a listbox whose selected option alone is tabbable. Moving with the
 * keys belongs to the queue around it. A proposal with a staged choice stays listed, marked
 * Staged (`docs/canon/decision-staging.md` "UI").
 */
export function ProposalList({
  proposals,
  selectedId,
  now,
  onSelect,
}: {
  proposals: readonly InboxEntry[];
  selectedId: string;
  now: number;
  onSelect: (id: string) => void;
}) {
  const t = useTranslate();
  return (
    <div role="listbox" aria-label={t("inbox.listLabel")} className="queue-list">
      {proposals.map((proposal) => {
        const selected = proposal.id === selectedId;
        const status = statusLook(proposal.status, t);
        const targets = targetsOf(proposal);
        return (
          <div
            key={proposal.id}
            role="option"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            data-proposal={proposal.id}
            className="queue-item"
            onClick={() => {
              onSelect(proposal.id);
            }}
          >
            <span className="queue-item-top">
              <Badge name={t("inbox.severityName")} look={severityLook(proposal.severity, t)} />
              <span className="mono queue-item-id">{proposal.id}</span>
              <time className="queue-item-age" dateTime={proposal.created_at} title={formatUtc(proposal.created_at, t)}>
                {formatAge(proposal.created_at, now, t)}
              </time>
            </span>
            <span className="queue-item-summary">{summaryOf(proposal, t)}</span>
            <span className="queue-item-meta">
              <span className="mono">{proposal.kind}</span>
              <span aria-hidden="true"> | </span>
              <span className="mono">{targets.length === 0 ? t("inbox.noTarget") : targets.join(", ")}</span>
              {proposal.status !== "open" && (
                <>
                  <span aria-hidden="true"> | </span>
                  <Badge name={t("inbox.statusName")} look={status} />
                </>
              )}
              {proposal.staged_at !== null && (
                <>
                  <span aria-hidden="true"> | </span>
                  <span className="staged-mark" title={t("inbox.stagedAt", { at: proposal.staged_at })}>
                    <Icon name="terminal" />
                    {t("inbox.staged.mark")}
                  </span>
                </>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
