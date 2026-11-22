import { useIdentity } from "../api/queries";
import { useActing, useHubLogin } from "../board/acting";
import { fitsLogin, type Acting } from "./labels";

// Who the page acts as for comments (docs/features/comments.md "UI"): a hub's login once read, else
// the Board's acting name as typed, which nothing verifies (ADR-0057). The daemon judges every write.

/** The page's login, or the acting name as typed: what a comment is posted under, and what is one's own. */
export function useCommentActing(): Acting {
  const login = useHubLogin();
  const [name] = useActing();
  return { login, name };
}

/**
 * Whose mentions the page lists, once the identity read has answered (`known`): on a hub its login's,
 * asked with no `login` (`asked` null), kept seen under it; locally the acting name's when it fits
 * the login grammar, asked and kept under it, else every mention, kept under the project alone.
 */
export function useMentionsWho(): { known: boolean; asked: string | null; seen: string | null } {
  const identity = useIdentity();
  const { login, name } = useCommentActing();
  const known = identity.data !== undefined || identity.error !== null;
  if (login !== null) {
    return { known, asked: null, seen: login };
  }
  const typed = name.trim();
  return fitsLogin(typed) ? { known, asked: typed, seen: typed } : { known, asked: null, seen: null };
}
