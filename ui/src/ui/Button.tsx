import type { ButtonHTMLAttributes, Ref } from "react";
import { Icon, type IconName } from "./Icon";

// A button (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"): five appearances, two spacings, an
// optional icon before the label. It renders the global classes, so every `.button` of the UI takes
// the same metrics from one rule: 32 px, or 24 px compact. At most one primary stands per area (a
// dialog, a view's header acts, a side column); the caller keeps to it.

export type ButtonAppearance = "default" | "primary" | "subtle" | "warning" | "danger";

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> {
  /** "default" unless given. */
  appearance?: ButtonAppearance;
  /** 32 px, or 24 px compact. */
  spacing?: "default" | "compact";
  /** Before the label. */
  icon?: IconName;
  /** Layout only (e.g. `board-move`), never a look. */
  className?: string;
  ref?: Ref<HTMLButtonElement>;
}

/** Each appearance's class beside `button`; the default needs none. */
const APPEARANCE_CLASSES: Readonly<Record<ButtonAppearance, string | null>> = {
  default: null,
  primary: "button-primary",
  subtle: "button-quiet",
  warning: "button-warning",
  danger: "button-danger",
};

/** The classes a button of this look takes: `button`, its appearance's, `button-compact`, then the layout's. */
export function buttonClasses(appearance: ButtonAppearance = "default", spacing: "default" | "compact" = "default", className?: string): string {
  return ["button", APPEARANCE_CLASSES[appearance], spacing === "compact" ? "button-compact" : null, className ?? null].filter((part) => part !== null && part !== "").join(" ");
}

/** A button of the UI's look; its `type` is `button` unless given, so it never submits a form by accident. */
export function Button({ appearance = "default", spacing = "default", icon, className, type = "button", children, ref, ...rest }: ButtonProps) {
  return (
    <button ref={ref} type={type} className={buttonClasses(appearance, spacing, className)} {...rest}>
      {icon !== undefined && <Icon name={icon} />}
      {children}
    </button>
  );
}
