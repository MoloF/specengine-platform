import { useCallback, useEffect, useRef } from "react";

type Target = () => HTMLElement | null | undefined;

/**
 * Focus an element after the next commit: once a dialog has unmounted and the page is no longer
 * inert, or once the item to focus exists. Pair each request with a state change.
 */
export function useFocusLater(): (target: Target) => void {
  const pending = useRef<Target | null>(null);
  useEffect(() => {
    const take = pending.current;
    if (take === null) {
      return;
    }
    pending.current = null;
    take()?.focus();
  });
  return useCallback((target: Target) => {
    pending.current = target;
  }, []);
}
