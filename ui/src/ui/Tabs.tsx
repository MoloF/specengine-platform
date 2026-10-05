import { useRef, type KeyboardEvent, type ReactNode } from "react";
import { hasModifier } from "./keys";

export interface TabSpec<T extends string> {
  id: T;
  label: ReactNode;
}

function tabId(base: string, id: string): string {
  return `${base}-tab-${id}`;
}

function panelId(base: string, id: string): string {
  return `${base}-panel-${id}`;
}

/**
 * WAI-ARIA tabs with manual activation: Left and Right (wrapping), Home and End move focus among
 * the tabs; Enter or Space (the button's own click) or a click selects. One tab stop: the selected
 * tab. Keys act only on the tab list and never with Ctrl, Alt or Cmd.
 */
export function Tabs<T extends string>({
  label,
  base,
  tabs,
  selected,
  onSelect,
}: {
  label: string;
  base: string;
  tabs: readonly TabSpec<T>[];
  selected: T;
  onSelect: (id: T) => void;
}) {
  const list = useRef<HTMLDivElement>(null);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event)) {
      return;
    }
    const buttons = Array.from(list.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? []);
    const at = buttons.findIndex((button) => button === event.target);
    if (at < 0) {
      return;
    }
    let next: number;
    if (event.key === "ArrowRight") {
      next = (at + 1) % buttons.length;
    } else if (event.key === "ArrowLeft") {
      next = (at - 1 + buttons.length) % buttons.length;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = buttons.length - 1;
    } else {
      return;
    }
    event.preventDefault();
    buttons[next]?.focus();
  }

  return (
    <div ref={list} role="tablist" aria-label={label} className="tabs" onKeyDown={onKeyDown}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          id={tabId(base, tab.id)}
          aria-selected={tab.id === selected}
          aria-controls={panelId(base, tab.id)}
          tabIndex={tab.id === selected ? 0 : -1}
          className="tab"
          onClick={() => {
            onSelect(tab.id);
          }}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

/** One tab's panel; hidden, not removed, when another tab is selected (its state is kept). */
export function TabPanel({ base, id, selected, children }: { base: string; id: string; selected: boolean; children: ReactNode }) {
  return (
    <div role="tabpanel" id={panelId(base, id)} aria-labelledby={tabId(base, id)} hidden={!selected} tabIndex={0} className="tab-panel">
      {children}
    </div>
  );
}
