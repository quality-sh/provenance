import * as Effect from 'effect/Effect';
import * as Exit from 'effect/Exit';
import {
  ConnectionError, MalformedResponseError, OperationError, ProtocolMismatchError,
  UncertainWriteError, type OperationFailure,
} from './generated/client.js';

export class InvalidRequestError extends Error {
  readonly _tag = 'InvalidRequestError';
  constructor() { super('Request cannot be serialized as JSON'); this.name = this._tag; }
}
export class WriteCapacityError extends Error {
  readonly _tag = 'WriteCapacityError';
  constructor() { super('Resolve an outstanding write before another mutation'); this.name = this._tag; }
}
export type ClientFailure<F extends OperationFailure = OperationFailure> =
  ConnectionError | MalformedResponseError | ProtocolMismatchError | OperationError<F> |
  UncertainWriteError<F> | InvalidRequestError | WriteCapacityError;

/** Session memory for dispatched writes whose completion is not yet known. */
export interface UnresolvedWrite {
  readonly id: number;
  readonly operation: string;
  readonly state: 'pending' | 'uncertain';
}

/** Run one abortable request and wait for its cleanup when the fiber stops. */
// @provenance rule: rule_sdk_read_interruption_releases_resources
export function requestEffect<A, E>(run: (signal: AbortSignal) => Promise<A>, failure: (cause: unknown) => E): Effect.Effect<A, E> {
  return Effect.callback<A, E>((resume, signal) => {
    const pending = Promise.resolve().then(() => {
      if (signal.aborted) throw new ConnectionError();
      return run(signal);
    });
    const settled = pending.then(
      value => { resume(Effect.succeed(value)); },
      cause => { resume(Effect.fail(failure(cause))); },
    );
    return Effect.promise(() => settled);
  });
}

export function connectionFailure(cause: unknown): ClientFailure {
  return cause instanceof ConnectionError || cause instanceof MalformedResponseError || cause instanceof ProtocolMismatchError
    ? cause : new ConnectionError(cause);
}

// @provenance rule: rule_effect_sdk_retains_unknown_writes
class ClientRuntime {
  private nextId = 0;
  private readonly writes = new Map<number, UnresolvedWrite>();

  unresolvedWrites(): ReadonlyArray<UnresolvedWrite> {
    return [...this.writes.values()].map(write => ({ ...write }));
  }

  /** Remove uncertainty only after the application obtains independent evidence. */
  resolveWrite(id: number): void {
    if (this.writes.get(id)?.state === 'pending') throw new Error('Write is still pending');
    this.writes.delete(id);
  }

  run<I, A, F extends OperationFailure>(operation: string, mutates: boolean, input: I,
    request: (input: I, signal: AbortSignal) => Promise<A>): Effect.Effect<A, ClientFailure<F>> {
    return Effect.suspend(() => {
      let snapshot: I;
      try { snapshot = JSON.parse(JSON.stringify(input)) as I; }
      catch { return Effect.fail(new InvalidRequestError()); }
      let id: number | undefined;
      let confirmedRefusal = false;
      const effect = requestEffect(async signal => {
        if (mutates) {
          if (this.writes.size >= 128) throw new WriteCapacityError();
          id = ++this.nextId;
          this.writes.set(id, { id, operation, state: 'pending' });
        }
        return request(snapshot, signal);
      }, cause => {
        if (cause instanceof WriteCapacityError) return cause;
        if (cause instanceof OperationError) {
          confirmedRefusal = true;
          return cause as OperationError<F>;
        }
        if (id !== undefined || cause instanceof UncertainWriteError) {
          const failure = cause instanceof UncertainWriteError ? cause.failure as F | undefined : undefined;
          return new UncertainWriteError(operation, cause, failure, id);
        }
        return connectionFailure(cause) as ClientFailure<F>;
      });
      return Effect.onExit(effect, exit => Effect.sync(() => {
        if (id === undefined) return;
        if (Exit.isSuccess(exit) || (confirmedRefusal && !Exit.hasInterrupts(exit))) this.writes.delete(id);
        else this.writes.set(id, { id, operation, state: 'uncertain' });
      }));
    });
  }
}

export { ClientRuntime };
