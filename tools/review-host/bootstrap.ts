import { connectEvents, type EventSocket } from './events.ts';
import {
  CONNECTION_REFUSED, createSession, type ReviewConfig, type ReviewHandle, type ReviewMountOptions,
} from './session.ts';

const LAUNCH_SESSION_ROUTE = '/review-launch/session';

export interface BrowserElement {
  hidden: boolean;
  textContent: string | null;
  value: string;
  addEventListener(name: string, listener: (event: { preventDefault(): void }) => void): void;
  focus(): void;
}

interface BrowserDocument {
  getElementById(id: string): BrowserElement | null | undefined;
}

interface BrowserLocation {
  readonly origin: string;
  readonly search: string;
  readonly hash: string;
  readonly pathname: string;
}

interface FetchResponse {
  readonly ok: boolean;
  json(): Promise<unknown>;
}

interface BootstrapDependencies {
  readonly document: BrowserDocument;
  readonly location: BrowserLocation;
  readonly history: { replaceState(data: null, unused: string, url: string): void };
  readonly fetch: (path: string, init: {
    method?: 'POST';
    headers: Record<string, string>;
    body?: string;
    redirect: 'error';
    cache: 'no-store';
  }) => Promise<FetchResponse>;
  readonly mount: (root: BrowserElement, options: ReviewMountOptions) => ReviewHandle;
  /** Opens the host's event socket. */
  readonly openEvents: () => EventSocket;
  /** Calls the listener when the page is online or active again. */
  readonly onWake: (listener: () => void) => void;
}

/** Tells the person that the page does not show later saves until it reconnects. */
export const LIVE_UPDATES_PAUSED =
  'Live updates stopped. They start again when this page is active, or select Open / Refresh.';

/** Tells the person that a reviewer setting changed and how to apply it. */
export const SETTINGS_CHANGED =
  'Reviewer settings changed. Select Open / Refresh to apply them; unsaved drafts close.';

const sameActors = (left: ReviewConfig, right: ReviewConfig) =>
  left.dispositionActorIds.length === right.dispositionActorIds.length &&
  left.dispositionActorIds.every((id, index) => right.dispositionActorIds[index] === id);

function element(document: BrowserDocument, id: string): BrowserElement {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing browser element: ${id}`);
  return found;
}

function reviewConfig(value: unknown, origin: string): ReviewConfig {
  if (typeof value !== 'object' || value === null ||
    !('endpoint' in value) || typeof value.endpoint !== 'string' || value.endpoint !== origin ||
    !('repositoryId' in value) || typeof value.repositoryId !== 'string' ||
    !('scope' in value) || typeof value.scope !== 'string' ||
    !('dispositionActorIds' in value) || !Array.isArray(value.dispositionActorIds) ||
    !value.dispositionActorIds.every(id => typeof id === 'string')) {
    throw new Error('Invalid configuration');
  }
  return {
    endpoint: value.endpoint,
    repositoryId: value.repositoryId,
    scope: value.scope,
    dispositionActorIds: value.dispositionActorIds,
  };
}

function sessionBearer(value: unknown): string {
  if (typeof value !== 'object' || value === null ||
    !('bearer' in value) || typeof value.bearer !== 'string' || value.bearer === '') {
    throw new Error('Invalid launch session');
  }
  return value.bearer;
}

/** Takes the launch code out of the address bar and exchanges it once for the page session. */
export function bootstrapReviewPage(dependencies: BootstrapDependencies) {
  // A browser refuses `fetch` when it is called as a method of another object.
  const { fetch, location } = dependencies;
  const root = element(dependencies.document, 'root');
  const selection = element(dependencies.document, 'selection');
  const requirement = element(dependencies.document, 'requirement');
  const status = element(dependencies.document, 'status');
  const searchParams = new URLSearchParams(location.search);
  const linkedRoot = searchParams.get('root');
  const linkedFocus = searchParams.get('focus') ?? undefined;
  const code = new URLSearchParams(location.hash.slice(1)).get('launch');
  if (location.hash !== '') {
    dependencies.history.replaceState(null, '', `${location.pathname}${location.search}`);
  }
  let configuration: (() => Promise<ReviewConfig>) | undefined;
  let current: ReviewConfig | undefined;
  const events = connectEvents({
    open: dependencies.openEvents,
    refresh: change => {
      session.refresh(change);
      if (change._tag !== 'Everything' || !configuration) return;
      // The configuration can change with the settings, so a full read reads it too.
      void configuration().then(next => {
        if (current && !sameActors(current, next)) status.textContent = SETTINGS_CHANGED;
      }, () => {});
    },
    showRecord: (rootId, focusId) => {
      const search = new URLSearchParams({ root: rootId, ...(focusId === undefined ? {} : { focus: focusId }) });
      dependencies.history.replaceState(null, '', `${location.pathname}?${search}`);
      return session.show(rootId, focusId);
    },
    live: () => session.restoreStatus(),
    paused: () => { status.textContent = LIVE_UPDATES_PAUSED; },
  });
  dependencies.onWake(events.wake);
  const session = createSession({
    mount: options => dependencies.mount(root, options),
    connected: config => {
      current = config;
      // Subscribe before the page reads, so no save between them is lost.
      events.start();
      selection.hidden = linkedRoot !== null;
      if (linkedRoot !== null) session.open(linkedRoot, linkedFocus);
      else requirement.focus();
    },
    status: message => { status.textContent = message; },
  });
  selection.addEventListener('submit', event => {
    event.preventDefault();
    session.open(requirement.value.trim());
    events.wake();
  });

  void (async () => {
    let bearer = '';
    if (code) {
      const launch = await fetch(LAUNCH_SESSION_ROUTE, {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ code }), redirect: 'error', cache: 'no-store',
      });
      if (!launch.ok) throw new Error('Launch refused');
      bearer = sessionBearer(await launch.json());
    }
    configuration = async () => {
      const response = await fetch('/review-config', {
        headers: bearer ? { authorization: `Bearer ${bearer}` } : {}, redirect: 'error', cache: 'no-store',
      });
      if (!response.ok) throw new Error('Access refused');
      return reviewConfig(await response.json(), location.origin);
    };
    await session.connect(bearer, configuration);
  })().catch(() => { status.textContent = CONNECTION_REFUSED; });
}
