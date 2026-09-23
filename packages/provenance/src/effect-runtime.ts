import * as Effect from 'effect/Effect';
import {
  ConnectionError, IdentityMismatchError, MalformedResponseError, OperationError, ProtocolMismatchError,
  type OperationFailure,
} from './generated/client.js';

export class InvalidRequestError extends Error {
  readonly _tag = 'InvalidRequestError';
  constructor() { super('Request cannot be serialized as JSON'); this.name = this._tag; }
}
export type ClientFailure<F extends OperationFailure = OperationFailure> =
  ConnectionError | IdentityMismatchError | MalformedResponseError | ProtocolMismatchError |
  OperationError<F> | InvalidRequestError;

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
  return cause instanceof ConnectionError || cause instanceof MalformedResponseError ||
    cause instanceof ProtocolMismatchError || cause instanceof IdentityMismatchError || cause instanceof OperationError
    ? cause : new ConnectionError(cause);
}

export class ClientRuntime {
  run<I, A, F extends OperationFailure>(_operation: string, _mutates: boolean, input: I,
    request: (input: I, signal: AbortSignal) => Promise<A>): Effect.Effect<A, ClientFailure<F>> {
    return Effect.suspend(() => {
      let snapshot: I;
      try { snapshot = JSON.parse(JSON.stringify(input)) as I; }
      catch { return Effect.fail(new InvalidRequestError()); }
      return requestEffect(signal => request(snapshot, signal), cause => connectionFailure(cause) as ClientFailure<F>);
    });
  }
}
