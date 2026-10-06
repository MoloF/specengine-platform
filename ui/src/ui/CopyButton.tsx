import { useEffect, useState, type ReactNode } from "react";
import { useAnnounce } from "./announcer";
import { Icon } from "./Icon";

/** Writes text to the clipboard; false when the browser refuses (no permission, no secure context). */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** How long "Copied" stays beside the button; a refusal stays until the next try. */
const COPIED_MS = 4000;

/**
 * A Copy button: the Clipboard API, then "Copied" beside it and in the page's polite live region;
 * a refusal says `failed` there instead, so the owner selects the text by hand. The accessible name
 * starts with the visible label (WCAG 2.5.3); `context` tells several Copy buttons apart.
 */
export function CopyButton({
  text,
  label = "Copy",
  context,
  failed,
}: {
  text: string;
  label?: string;
  context: ReactNode;
  failed: string;
}) {
  const announce = useAnnounce();
  const [state, setState] = useState<{ result: "idle" | "copied" | "failed"; count: number }>({ result: "idle", count: 0 });

  useEffect(() => {
    if (state.result !== "copied") {
      return;
    }
    const timer = setTimeout(() => {
      setState((current) => (current.count === state.count ? { ...current, result: "idle" } : current));
    }, COPIED_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [state]);

  async function copy() {
    const copied = await copyText(text);
    setState((current) => ({ result: copied ? "copied" : "failed", count: current.count + 1 }));
    announce(copied ? "Copied" : failed);
  }

  return (
    <span className="copy">
      <button
        type="button"
        className="button button-copy"
        onClick={() => {
          void copy();
        }}
      >
        <Icon name="copy" />
        <span>
          {label} <span className="sr-only">{context}</span>
        </span>
      </button>
      {state.result === "copied" && (
        <span className="copy-status" data-copy="copied">
          <Icon name="approved" />
          Copied
        </span>
      )}
      {state.result === "failed" && (
        <span className="copy-status copy-failed" data-copy="failed">
          <Icon name="alert" />
          {failed}
        </span>
      )}
    </span>
  );
}
