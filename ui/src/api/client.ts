import type { ApiError, Decision, DecisionResult, Inbox, NodeView, Project } from "./types";

/**
 * The one seam between the UI and SpecEngine (ADR-0033): methods named after the daemon's
 * endpoints (`docs/specs/specengine-platform/07-interfaces.md` "3. HTTP (daemon)"). Only the
 * bootstrap, src/main.tsx, picks the implementation. Every method rejects with a ClientError
 * carrying the daemon's status and message verbatim.
 */
export interface SpecEngineClient {
  /** Drives the permanent "Mock data" indicator. */
  readonly dataSource: "mock" | "daemon";
  /** MISSING ENDPOINT GET /api/projects (named for rust-developer; docs/features/ui-shell.md). */
  getProjects(): Promise<Project[]>;
  /** GET /api/projects/:p/inbox */
  getInbox(project: string): Promise<Inbox>;
  /** GET /api/projects/:p/nodes/:id */
  getNode(project: string, id: string): Promise<NodeView>;
  /** POST /api/projects/:p/proposals/:id/decision */
  decideProposal(project: string, id: string, decision: Decision): Promise<DecisionResult>;
}

/** HTTP 409: the proposal was decided elsewhere. */
export const DECIDED_ELSEWHERE = 409;

/** What a SpecEngineClient rejects with: the daemon's error body, its message verbatim. */
export class ClientError extends Error implements ApiError {
  readonly status: number;

  constructor(body: ApiError) {
    super(body.message);
    this.name = "ClientError";
    this.status = body.status;
  }
}

/** The status and the verbatim message of any rejection; status 0 when no response carried one. */
export function apiErrorOf(error: unknown): ApiError {
  if (error instanceof ClientError) {
    return { status: error.status, message: error.message };
  }
  if (error instanceof Error) {
    return { status: 0, message: error.message };
  }
  return { status: 0, message: String(error) };
}
