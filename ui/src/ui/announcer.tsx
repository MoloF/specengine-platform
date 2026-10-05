import { createContext, use, useCallback, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export type Urgency = "polite" | "assertive";

type Announce = (message: string, urgency?: Urgency) => void;

const AnnounceContext = createContext<Announce>(() => undefined);

/** Marks the live regions' container (`data-live-region` below): a dialog never makes it inert. */
export const LIVE_REGION_ATTRIBUTE = "data-live-region";

interface Spoken {
  text: string;
  /** Bumped on every announcement, so the same words said twice are a new node, spoken again. */
  count: number;
}

/**
 * The page's live regions, outside the app's root: a result announced in the commit that closes a
 * dialog is spoken even though the root is still inert in that commit. Visually hidden; views show
 * their own visible copy.
 */
export function Announcer({ children }: { children: ReactNode }) {
  const [spoken, setSpoken] = useState<{ urgency: Urgency } & Spoken>({ urgency: "polite", text: "", count: 0 });
  const announce = useCallback<Announce>((text, urgency = "polite") => {
    setSpoken((current) => ({ urgency, text, count: current.count + 1 }));
  }, []);
  const region = (urgency: Urgency) => (
    <div aria-live={urgency} aria-atomic="true">
      {spoken.urgency === urgency && spoken.text !== "" && <p key={spoken.count}>{spoken.text}</p>}
    </div>
  );
  return (
    <AnnounceContext value={announce}>
      {children}
      {createPortal(
        <div className="sr-only" data-live-region="">
          {region("polite")}
          {region("assertive")}
        </div>,
        document.body,
      )}
    </AnnounceContext>
  );
}

/** Speaks a result through the page's live region; `assertive` for what went wrong. */
export function useAnnounce(): Announce {
  return use(AnnounceContext);
}
