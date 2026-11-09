import { useLayoutEffect, type RefObject } from "react";

/**
 * Keeps `--<name>` on a view that fills the viewport below the shell: its top on the page, measured
 * at once before the first paint, then again on resize and whenever the header or the main area
 * changes height, a frame later, since the shell above may wrap: the view's height is never a constant
 * (as Flow's studio measures its own).
 */
export function useViewTop(view: RefObject<HTMLElement | null>, name: string): void {
  useLayoutEffect(() => {
    let frame = 0;
    const set = () => {
      const element = view.current;
      if (element !== null) {
        const top = `${String(Math.max(0, Math.round(element.getBoundingClientRect().top + window.scrollY)))}px`;
        if (element.style.getPropertyValue(`--${name}`) !== top) {
          element.style.setProperty(`--${name}`, top);
        }
      }
    };
    const later = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(set);
    };
    set();
    window.addEventListener("resize", later);
    const observer = new ResizeObserver(later);
    const header = document.querySelector(".app-header");
    if (header !== null) {
      observer.observe(header);
    }
    const main = view.current?.parentElement;
    if (main !== null && main !== undefined) {
      observer.observe(main);
    }
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", later);
      observer.disconnect();
    };
  }, [view, name]);
}
