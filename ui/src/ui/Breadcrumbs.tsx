import type { MouseEvent } from "react";
import { useTranslate } from "../i18n";

// A breadcrumb trail (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"): a `nav` named "Breadcrumbs"
// holding an `ol`. Every item but the last is a link, its `href` from routes.ts; the last is the
// current item, text with `aria-current="page"` and no `href`. Separators are `/`, hidden from
// assistive technology. Each link is at least 24 px tall.

export interface Crumb {
  key: string;
  label: string;
  /** Every crumb but the last; the last is current and takes none. */
  href?: string;
  /** An ID, in the monospace face. */
  mono?: boolean;
  /** A plain click follows it in place (a card's link replaces its entry); any other click is the browser's. */
  onFollow?: () => void;
}

/** A plain click with the primary button; a modified one is the browser's. */
function isPlainClick(event: MouseEvent<HTMLElement>): boolean {
  return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

export function Breadcrumbs({ items }: { items: readonly Crumb[] }) {
  const t = useTranslate();
  return (
    <nav className="breadcrumbs" aria-label={t("ui.breadcrumbs")}>
      <ol className="breadcrumbs-list">
        {items.map((item, index) => {
          const current = index === items.length - 1;
          const text = <bdi className={item.mono === true ? "mono" : undefined}>{item.label}</bdi>;
          const follow = item.onFollow;
          return (
            <li key={item.key} className="breadcrumbs-item">
              {index > 0 && (
                <span className="breadcrumbs-separator" aria-hidden="true">
                  /
                </span>
              )}
              {current ? (
                <span className="breadcrumbs-current" aria-current="page">
                  {text}
                </span>
              ) : item.href === undefined ? (
                <span className="breadcrumbs-text">{text}</span>
              ) : (
                <a
                  className="breadcrumbs-link"
                  href={item.href}
                  onClick={
                    follow === undefined
                      ? undefined
                      : (event) => {
                          if (isPlainClick(event)) {
                            event.preventDefault();
                            follow();
                          }
                        }
                  }
                >
                  {text}
                </a>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
