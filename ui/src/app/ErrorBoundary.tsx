import { Component, type ReactNode } from "react";

interface Props {
  fallback: (error: Error, reset: () => void) => ReactNode;
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches a render error below it and shows its fallback instead; the rest of the page lives on.
 * React reports the error itself (the root's onCaughtError).
 */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: unknown): State {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  reset = (): void => {
    this.setState({ error: null });
  };

  override render(): ReactNode {
    return this.state.error === null ? this.props.children : this.props.fallback(this.state.error, this.reset);
  }
}
