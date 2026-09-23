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
export class MalformedResponseError extends ClientError {
  readonly _tag = 'MalformedResponseError';
  constructor(cause?: unknown) { super('MalformedResponseError', 'Host response does not match the operation contract', cause); }
}
export class OperationError<F extends OperationFailure = OperationFailure> extends ClientError {
  readonly _tag = 'OperationError';
  constructor(readonly status: number, readonly failure: F) {
    super('OperationError', 'Operation failed');
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

export async function send(fetcher: typeof fetch, url: string | URL, init: RequestInit, _operation: string, _mutates: boolean): Promise<Response> {
  try { return await fetcher(url, { ...init, redirect: 'error' }); }
  catch (cause) { throw new ConnectionError(cause); }
}

export async function readJson(response: Response, _operation: string, _mutates: boolean, signal?: AbortSignal): Promise<unknown> {
  const malformed = (cause?: unknown) => new MalformedResponseError(cause);
  const reader = response.body?.getReader();
  if (!reader) throw malformed();
  let cancellation: Promise<void> | undefined;
  const abort = () => { cancellation ??= reader.cancel().catch(() => {}); };
  signal?.addEventListener('abort', abort, { once: true });
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    if (signal?.aborted) { await reader.cancel(); throw new ConnectionError(); }
    for (;;) {
      const { done, value } = await reader.read();
      if (signal?.aborted) throw new ConnectionError();
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
    throw new ConnectionError(cause);
  } finally { signal?.removeEventListener('abort', abort); await cancellation; reader.releaseLock(); }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); }
  catch (cause) { throw malformed(cause); }
}

export function checked(value: unknown, validate: (value: unknown) => boolean, _operation: string, _mutates: boolean): void {
  if (!validate(value)) throw new MalformedResponseError();
}
