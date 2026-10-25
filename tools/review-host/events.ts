import { BROWSER_RESET_REASONS } from './session.ts';
import type { ChangedRecord, ReviewChange, RereadReason } from './session.ts';

/** The WebSocket parts that the event client uses. */
export interface EventSocket {
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  send(text: string): void;
  close(): void;
}

type Frame =
  | { readonly type: 'ready'; readonly epoch: string; readonly sequence: number }
  | {
    readonly type: 'changed'; readonly epoch: string; readonly sequence: number;
    readonly records: ReadonlyArray<ChangedRecord>;
  }
  | { readonly type: 'reset'; readonly epoch: string; readonly sequence: number; readonly reason: RereadReason }
  | { readonly type: 'show-record'; readonly requestId: string; readonly rootId: string; readonly focusId?: string };

/** Where the client is: the last frame it applied, or no feed. */
type Position =
  | { readonly _tag: 'Waiting' }
  | { readonly _tag: 'At'; readonly epoch: string; readonly sequence: number };

/**
 * The connection: open, closed with one immediate attempt still available,
 * or paused until the page becomes active again.
 */
type Connection =
  | { readonly _tag: 'Open'; readonly socket: EventSocket; readonly retry: boolean }
  | { readonly _tag: 'Paused' }
  | { readonly _tag: 'Stopped' };

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

const changedRecords = (scopes: unknown): ReadonlyArray<ChangedRecord> | undefined => {
  if (!Array.isArray(scopes)) return undefined;
  const records: ChangedRecord[] = [];
  for (const scope of scopes) {
    if (!isRecord(scope) || !Array.isArray(scope.records)) return undefined;
    for (const record of scope.records) {
      if (!isRecord(record) || typeof record.kind !== 'string' || typeof record.id !== 'string') return undefined;
      records.push({ kind: record.kind, id: record.id });
    }
  }
  return records;
};

/** Reads one server frame. A frame of another shape is a protocol fault. */
export function parseFrame(text: unknown): Frame | undefined {
  if (typeof text !== 'string') return undefined;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return undefined;
  }
  if (!isRecord(value)) return undefined;
  if (value.type === 'show-record') {
    if (typeof value.requestId !== 'string' || typeof value.rootId !== 'string' ||
      !(value.focusId === undefined || typeof value.focusId === 'string')) return undefined;
    return {
      type: 'show-record', requestId: value.requestId, rootId: value.rootId,
      ...(value.focusId === undefined ? {} : { focusId: value.focusId }),
    };
  }
  if (typeof value.epoch !== 'string' || typeof value.sequence !== 'number') return undefined;
  const { epoch, sequence } = value;
  switch (value.type) {
    case 'ready':
      return { type: 'ready', epoch, sequence };
    case 'reset':
      for (const reason of BROWSER_RESET_REASONS) {
        if (value.reason === reason) return { type: 'reset', epoch, sequence, reason };
      }
      return undefined;
    case 'changed': {
      const records = changedRecords(value.scopes);
      return records && { type: 'changed', epoch, sequence, records };
    }
    default:
      return undefined;
  }
}

/**
 * Keeps the review page current with the host's saved state. Ready, reset, a
 * new epoch, a sequence gap and a reconnect read everything again; a changed
 * frame reads its records again. After a close the client reconnects once at
 * once, then only when the page wakes or the person asks. It has no timer.
 */
export function connectEvents(view: {
  open(): EventSocket;
  refresh(change: ReviewChange): void;
  showRecord(rootId: string, focusId?: string): Promise<void>;
  live(): void;
  paused(): void;
}) {
  let position: Position = { _tag: 'Waiting' };
  let connection: Connection = { _tag: 'Stopped' };

  const apply = (frame: Exclude<Frame, { type: 'show-record' }>) => {
    const next = position._tag === 'At' && position.epoch === frame.epoch &&
      position.sequence + 1 === frame.sequence;
    position = { _tag: 'At', epoch: frame.epoch, sequence: frame.sequence };
    if (frame.type === 'ready') {
      view.live();
      view.refresh({ _tag: 'Everything', reason: 'ready' });
    } else if (frame.type === 'reset') {
      view.refresh({ _tag: 'Everything', reason: frame.reason });
    } else if (next) {
      view.refresh({ _tag: 'Records', records: frame.records });
    } else {
      view.refresh({ _tag: 'Everything', reason: 'sequence-gap' });
    }
  };

  const start = (retry: boolean) => {
    const socket = view.open();
    connection = { _tag: 'Open', socket, retry };
    socket.onmessage = event => {
      const frame = parseFrame(event.data);
      if (!frame) {
        socket.close();
        return;
      }
      if (frame.type !== 'show-record') {
        if (frame.type === 'ready') connection = { _tag: 'Open', socket, retry: true };
        apply(frame);
        return;
      }
      void view.showRecord(frame.rootId, frame.focusId).then(() => {
        if (connection._tag === 'Open' && connection.socket === socket) {
          socket.send(JSON.stringify({ type: 'ack', requestId: frame.requestId }));
        }
      });
    };
    socket.onclose = () => {
      if (connection._tag !== 'Open' || connection.socket !== socket) return;
      // Frames after the close are lost, so the next connection reads everything.
      position = { _tag: 'Waiting' };
      if (connection.retry) {
        start(false);
        return;
      }
      connection = { _tag: 'Paused' };
      view.paused();
    };
  };

  return {
    start: () => start(true),
    /** The page is online or active again, or the person asked to refresh. */
    wake: () => {
      if (connection._tag === 'Paused') start(false);
    },
    stop: () => {
      const current = connection;
      connection = { _tag: 'Stopped' };
      if (current._tag === 'Open') current.socket.close();
    },
  };
}
