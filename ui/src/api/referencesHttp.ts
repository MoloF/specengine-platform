import { ClientError, type ReferencesOptions, type UploadProgress } from "./client";
import { errorBodyOf, projectPath, queryOf, refusedReasonOf, request, segment } from "./http";
import type { BlobStored, Reference, ReferenceAddBody, ReferenceList } from "./types";

// The references' requests (`docs/features/work-references.md` "HTTP"), `HttpClient`'s in a chunk of
// their own, reached by its dynamic import alone. The read is judged: a `{id, reason}` 404 or 409
// rejects with its reason, an older daemon's unknown route in its words. The attach a JSON body, the
// delete none: 200 the reference, a refusal (an error body 400 or 403, `{id, reason}` 404 or 409)
// rejecting verbatim. The upload is the binary door's one PUT: the bytes as they are, typed
// `application/octet-stream`, never a form, through an XMLHttpRequest, so its progress is the
// browser's own count of the body sent; the browser adds `Content-Length`, `Origin` and
// `Sec-Fetch-Site`, which the door requires.

/** What the binary door takes: the bytes as they are, never multipart (`docs/features/work-references.md` "HTTP"). */
export const OCTET_STREAM = "application/octet-stream";

/** How many times a 429 is tried again, each after its `Retry-After` (`docs/features/work-references.md` "UI"). */
export const UPLOAD_RETRIES = 5;

/** The wait when a 429 names none or an unreadable one: the daemon's own `Retry-After: 5`. */
const RETRY_AFTER_S = 5;

/** The wait before the one try again of an upload cut with no answer (status 0), as a daemon refusing early may reset it. */
export const BROKEN_RETRY_S = 5;

/** The longest wait a `Retry-After` is taken for. */
const RETRY_AFTER_MAX_S = 60;

/** Too many uploads at once: the slots' answer. */
const TOO_MANY = 429;

/** A project's references; one reference's delete is under it. */
function referencesPath(project: string): string {
  return `${projectPath(project)}/references`;
}

/** `spec ref list --json`: `on`, then `before` and `limit` when given. */
export async function getReferences(project: string, options: ReferencesOptions, signal?: AbortSignal): Promise<ReferenceList> {
  const query = queryOf([
    ["on", options.on],
    ["before", options.before],
    ["limit", options.limit],
  ]);
  return (await request("GET", `${referencesPath(project)}${query}`, { signal, judged: true })) as ReferenceList;
}

export async function addReference(project: string, body: ReferenceAddBody): Promise<Reference> {
  return (await request("POST", referencesPath(project), { body: JSON.stringify(body) })) as Reference;
}

export async function deleteReference(project: string, id: string): Promise<Reference> {
  return (await request("DELETE", `${referencesPath(project)}/${segment(id)}`)) as Reference;
}

/** What one PUT came to: its status, its body as text, and its `Retry-After`. */
interface Answer {
  status: number;
  statusText: string;
  text: string;
  retryAfter: string | null;
}

/** The error Cancel ends an upload with, as an aborted `fetch` gives one: never a ClientError, since the daemon was not at fault. */
function cancelled(): DOMException {
  return new DOMException("The upload was cancelled", "AbortError");
}

/** One PUT of the bytes, its progress reported as the browser sends them; `signal` aborts it. */
function sendOnce(url: string, blob: Blob, onProgress: UploadProgress, signal: AbortSignal): Promise<Answer> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(cancelled());
      return;
    }
    const xhr = new XMLHttpRequest();
    const abort = () => {
      xhr.abort();
    };
    const settle = () => {
      signal.removeEventListener("abort", abort);
    };
    xhr.open("PUT", url);
    // Set by hand: the browser would type a Blob by its own `type` (an image's), which the door refuses.
    xhr.setRequestHeader("Content-Type", OCTET_STREAM);
    xhr.setRequestHeader("Accept", "application/json");
    xhr.responseType = "text";
    xhr.upload.onprogress = (event) => {
      onProgress(event.loaded, event.lengthComputable ? event.total : blob.size);
    };
    xhr.onload = () => {
      settle();
      resolve({ status: xhr.status, statusText: xhr.statusText, text: xhr.responseText, retryAfter: xhr.getResponseHeader("Retry-After") });
    };
    xhr.onerror = () => {
      settle();
      reject(new ClientError({ status: 0, message: `PUT ${url}: the upload broke off, no answer from the daemon` }));
    };
    xhr.onabort = () => {
      settle();
      reject(cancelled());
    };
    signal.addEventListener("abort", abort, { once: true });
    // The Blob itself: never a FormData, never read into memory here.
    xhr.send(blob);
  });
}

/** Waits `seconds`, or rejects at once when `signal` aborts. */
function waitFor(seconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(cancelled());
      return;
    }
    const onAbort = () => {
      clearTimeout(timer);
      reject(cancelled());
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, seconds * 1000);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

/** A 429's wait in seconds: its `Retry-After` as a whole number, else the daemon's 5. */
export function retryAfterOf(header: string | null): number {
  const seconds = header === null ? Number.NaN : Number(header.trim());
  return Number.isInteger(seconds) && seconds >= 0 ? Math.min(seconds, RETRY_AFTER_MAX_S) : RETRY_AFTER_S;
}

/** A refused PUT as a ClientError: the error body's message, a `{id, reason}`'s reason, else the text as sent. */
function refusalOf(url: string, answer: Answer): ClientError {
  const value = parsedOrNull(answer.text);
  const body = errorBodyOf(value);
  if (body !== null) {
    return new ClientError({ status: answer.status, message: body.message });
  }
  const reason = refusedReasonOf(value);
  if (reason !== null) {
    return new ClientError({ status: answer.status, message: reason });
  }
  const said = answer.text === "" ? `PUT ${url} answered ${String(answer.status)} ${answer.statusText} with an empty body` : answer.text;
  return new ClientError({ status: answer.status, message: said });
}

function parsedOrNull(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return null;
  }
}

/**
 * The binary door's PUT (`docs/features/work-references.md` "HTTP"): 200 the stored blob; a 429
 * waits its `Retry-After` and goes again, at most UPLOAD_RETRIES times, its progress starting over;
 * an upload cut with no answer (status 0: a daemon refusing early may reset the connection) goes
 * again once after BROKEN_RETRY_S, then says it broke off; any other refusal rejects in the daemon's
 * words. Cancel (`signal`) aborts the request or the wait.
 */
export async function putBlob(url: string, blob: Blob, onProgress: UploadProgress, signal: AbortSignal): Promise<BlobStored> {
  let busy = 0;
  let broken = false;
  for (;;) {
    let answer: Answer;
    try {
      answer = await sendOnce(url, blob, onProgress, signal);
    } catch (error) {
      if (broken || !(error instanceof ClientError) || error.status !== 0) {
        throw error;
      }
      broken = true;
      onProgress(0, blob.size);
      await waitFor(BROKEN_RETRY_S, signal);
      continue;
    }
    if (answer.status >= 200 && answer.status < 300) {
      const stored = parsedOrNull(answer.text);
      if (stored === null || typeof stored !== "object") {
        throw new ClientError({ status: answer.status, message: `PUT ${url} answered ${String(answer.status)} without a JSON document: ${answer.text}` });
      }
      return stored as BlobStored;
    }
    if (answer.status !== TOO_MANY || busy >= UPLOAD_RETRIES) {
      throw refusalOf(url, answer);
    }
    busy += 1;
    onProgress(0, blob.size);
    await waitFor(retryAfterOf(answer.retryAfter), signal);
  }
}
