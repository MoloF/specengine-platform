import { useCallback, useEffect, useRef, useState } from "react";

type Target = () => HTMLElement | null | undefined;

/**
 * Focus an element after the next commit: once a dialog has unmounted and the page is no longer
 * inert, or once the item to focus exists. Each request brings its own commit: a state change
 * that changes nothing (Home on the first row, End on the last) renders nothing, and a request
 * left pending would fire on some later, unrelated render (a keystroke in a filter) and steal focus.
 */
export function useFocusLater(): (target: Target) => void {
  const pending = useRef<Target | null>(null);
  const [, setRequests] = useState(0);
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
    setRequests((count) => count + 1);
  }, []);
}
