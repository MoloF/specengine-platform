import { useEffect, useRef } from "react";
import { focusIsLost } from "./focus";

/**
 * After a successful Retry the Retry button that had focus is gone and focus falls to the body,
 * where no region's keys reach. Call the returned function when Retry is pressed; once `settled`
 * (the read's data) arrives, focus goes to `target` if it is still lost.
 */
export function useRetryFocus(settled: unknown, target: () => HTMLElement | null | undefined): () => void {
  const retried = useRef(false);
  const take = useRef(target);
  useEffect(() => {
    take.current = target;
  });
  useEffect(() => {
    if (!retried.current || settled === undefined) {
      return;
    }
    retried.current = false;
    if (focusIsLost()) {
      take.current()?.focus();
    }
  }, [settled]);
  return () => {
    retried.current = true;
  };
}
