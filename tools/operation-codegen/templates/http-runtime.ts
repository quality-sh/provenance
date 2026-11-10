// Generated from the checked HTTP runtime template. Do not edit.
import type { OperationFailure } from './client.js';

export const MAX_RESPONSE_BYTES = 16 * 1024 * 1024;
class ClientError extends Error {
  #cause: unknown;
  constructor(name: string, message: string, cause?: unknown) {
    super(message);
    this.name = name;
    this.#cause = cause;
  }
}
export class ConnectionError extends ClientError {
  readonly _tag = 'ConnectionError';
  constructor(cause?: unknown) { super('ConnectionError', 'Cannot complete the HTTP connection', cause); }
}
export class OutcomeUnknownError extends ClientError {
  readonly _tag = 'OutcomeUnknownError';
  constructor(cause?: unknown) {
    super('OutcomeUnknownError', 'The outcome of the write request is not known. Read the current state before you send the write again', cause);
  }
}
export class MalformedResponseError extends ClientError {
  readonly _tag = 'MalformedResponseError';
  constructor(cause?: unknown) { super('MalformedResponseError', 'Host response does not match the operation contract', cause); }
}
export class OperationError<F = OperationFailure> extends ClientError {
  readonly _tag = 'OperationError';
  constructor(readonly status: number, readonly failure: F) {
    const error = failure && typeof failure === 'object' && 'error' in failure ? failure.error : undefined;
    const kind = error && typeof error === 'object' && 'kind' in error && typeof error.kind === 'string' ? error.kind : 'Operation failed';
    const message = error && typeof error === 'object' && 'message' in error && typeof error.message === 'string' ? error.message : undefined;
    super('OperationError', message ? `${kind}: ${message}` : kind);
  }
}
export class ProtocolMismatchError extends ClientError {
  readonly _tag = 'ProtocolMismatchError';
  constructor(readonly requested: unknown, readonly supported: unknown) {
    super('ProtocolMismatchError', 'Incompatible compatibility tuple');
  }
}
export class IdentityMismatchError extends ClientError {
  readonly _tag = 'IdentityMismatchError';
  constructor(readonly requested: unknown, readonly authorized: unknown) {
    super('IdentityMismatchError', 'Host metadata does not match the requested repository and scope');
  }
}

// System error codes that show that the connection did not open. Linux can also
// report EHOSTUNREACH or ENETUNREACH on a socket that is already open, so a code
// counts only when the failed system call is the connect or the name lookup.
const UNSENT_CODES = new Set(['ECONNREFUSED', 'ENOTFOUND', 'EAI_AGAIN', 'EAI_NONAME', 'EHOSTUNREACH', 'ENETUNREACH', 'EADDRNOTAVAIL']);
const UNSENT_SYSCALLS = new Set(['connect', 'getaddrinfo']);
// undici sets this code only when its connect step times out.
const CONNECT_TIMEOUT = 'UND_ERR_CONNECT_TIMEOUT';
// undici puts the socket error at most two levels down: TypeError, then the cause,
// then the errors of an AggregateError. The limit also stops a cause chain that
// refers to itself.
const MAX_CAUSE_DEPTH = 4;
function notSent(cause: unknown, depth = 0): boolean {
  if (!cause || typeof cause !== 'object' || depth > MAX_CAUSE_DEPTH) return false;
  const code = 'code' in cause ? cause.code : undefined;
  if (code === CONNECT_TIMEOUT) return true;
  const syscall = 'syscall' in cause ? cause.syscall : undefined;
  if (typeof code === 'string' && UNSENT_CODES.has(code) && typeof syscall === 'string' && UNSENT_SYSCALLS.has(syscall)) return true;
  if ('errors' in cause && Array.isArray(cause.errors) && cause.errors.length > 0) {
    return cause.errors.every(error => notSent(error, depth + 1));
  }
  return 'cause' in cause && notSent(cause.cause, depth + 1);
}

// After dispatch, a write possibly went to the host, so its outcome is unknown.
// Only a read of current state can show its result, so the client does not send
// it again.
function lostResponse(write: boolean, cause?: unknown): ConnectionError | OutcomeUnknownError {
  return write ? new OutcomeUnknownError(cause) : new ConnectionError(cause);
}

// A write fails as not sent only when the cause shows that the connection did not open.
// @provenance rule: rule_effect_sdk_retains_unknown_writes
function transportFailure(write: boolean, cause: unknown): ConnectionError | OutcomeUnknownError {
  return notSent(cause) ? new ConnectionError(cause) : lostResponse(write, cause);
}

export async function send(fetcher: typeof fetch, url: string | URL, init: RequestInit, write = false): Promise<Response> {
  if (init.signal?.aborted) throw new ConnectionError(init.signal.reason);
  try { return await fetcher(url, { ...init, redirect: 'error' }); }
  catch (cause) { throw transportFailure(write, cause); }
}

// @provenance rule: rule_sdk_read_interruption_releases_resources
export async function readJson(response: Response, signal?: AbortSignal, write = false): Promise<unknown> {
  const malformed = (cause?: unknown) => new MalformedResponseError(cause);
  const reader = response.body?.getReader();
  if (!reader) throw malformed();
  let cancellation: Promise<void> | undefined;
  const abort = () => { cancellation ??= reader.cancel().catch(() => {}); };
  signal?.addEventListener('abort', abort, { once: true });
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    if (signal?.aborted) { await reader.cancel(); throw lostResponse(write); }
    for (;;) {
      const { done, value } = await reader.read();
      if (signal?.aborted) throw lostResponse(write);
      if (done) break;
      length += value.byteLength;
      if (length > MAX_RESPONSE_BYTES) {
        await reader.cancel();
        throw malformed();
      }
      chunks.push(value);
    }
  } catch (cause) {
    if (cause instanceof ClientError) throw cause;
    // The response headers arrived, so the request went to the host.
    throw lostResponse(write, cause);
  } finally { signal?.removeEventListener('abort', abort); await cancellation; reader.releaseLock(); }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); }
  catch (cause) { throw malformed(cause); }
}

export function checked(value: unknown, validate: (value: unknown) => boolean): void {
  if (!validate(value)) throw new MalformedResponseError();
}
