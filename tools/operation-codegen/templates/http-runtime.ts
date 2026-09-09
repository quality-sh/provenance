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
  constructor(cause?: unknown) { super('ConnectionError', 'Cannot complete the HTTP connection', cause); }
}
export class MalformedResponseError extends ClientError {
  constructor(cause?: unknown) { super('MalformedResponseError', 'Host response does not match the operation contract', cause); }
}
export class UncertainWriteError extends ClientError {
  constructor(readonly operation: string, cause?: unknown, readonly failure?: OperationFailure) {
    super('UncertainWriteError', 'Write outcome is uncertain; inspect repository state before retrying', cause);
  }
}
export class OperationError extends ClientError {
  constructor(readonly status: number, readonly failure: OperationFailure) {
    super('OperationError', 'Operation failed');
  }
}
export class ProtocolMismatchError extends ClientError {
  constructor(readonly requested: number, readonly supported: number) {
    super('ProtocolMismatchError', 'Incompatible operation protocol');
  }
}

export async function send(fetcher: typeof fetch, url: string, init: RequestInit, operation: string, mutates: boolean): Promise<Response> {
  try { return await fetcher(url, { ...init, redirect: 'error' }); }
  catch (cause) { throw mutates ? new UncertainWriteError(operation, cause) : new ConnectionError(cause); }
}

export async function readJson(response: Response, operation: string, mutates: boolean): Promise<unknown> {
  const malformed = (cause?: unknown) => mutates ? new UncertainWriteError(operation, cause) : new MalformedResponseError(cause);
  const reader = response.body?.getReader();
  if (!reader) throw malformed();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
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
    throw mutates ? new UncertainWriteError(operation, cause) : new ConnectionError(cause);
  } finally { reader.releaseLock(); }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); }
  catch (cause) { throw malformed(cause); }
}

export function checked(value: unknown, validate: (value: unknown) => boolean, operation: string, mutates: boolean): void {
  if (!validate(value)) throw mutates ? new UncertainWriteError(operation) : new MalformedResponseError();
}
