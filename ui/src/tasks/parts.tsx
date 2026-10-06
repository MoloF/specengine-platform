import type { ReactNode } from "react";
import { formatAge, formatUtc } from "../ui/time";
import { useNow } from "../ui/useNow";

/** A part of a task's tab under its own h2 (the task's ID and title are the h1). */
export function Part({ title, children, count }: { title: string; children: ReactNode; count?: number }) {
  return (
    <section className="task-part">
      <h2 className="task-part-title">
        {title}
        {count !== undefined && <span className="task-part-count">{count}</span>}
      </h2>
      {children}
    </section>
  );
}

/** A stored UTC time as the Inbox shows times: the age, the exact UTC beside it. */
export function When({ at }: { at: string }) {
  const now = useNow();
  return (
    <time dateTime={at} title={formatUtc(at)}>
      {formatAge(at, now)} <span className="muted">({formatUtc(at)})</span>
    </time>
  );
}
