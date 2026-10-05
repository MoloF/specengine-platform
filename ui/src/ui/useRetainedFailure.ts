import { useState } from "react";

/**
 * The failure to show for a read: its current error, else, while a retry runs, the error before it.
 * Retrying a read that never succeeded clears its error (the query goes back to pending); keeping
 * the old one up keeps the error panel, and the Retry that has focus, in place until the answer.
 */
export function useRetainedFailure(error: Error | null, retrying: boolean): Error | null {
  const [last, setLast] = useState<Error | null>(null);
  if (error !== null && error !== last) {
    // Storing information from previous renders (react.dev, useState): a new error replaces the kept one.
    setLast(error);
  }
  return error ?? (retrying ? last : null);
}
