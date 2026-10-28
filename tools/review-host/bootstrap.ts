import { connectEvents, type EventSocket } from './events.ts';
import {
  CONNECTION_REFUSED, createSession, type ConfigReviewer, type ReviewConfig, type ReviewHandle,
  type ReviewMountOptions,
} from './session.ts';

const LAUNCH_SESSION_ROUTE = '/review-launch/session';

/** The local storage key of the page session. Local storage is scoped to this host's origin. */
export const SESSION_KEY = 'provenance-review-session';

/** The subprotocol name the event socket offers beside the page session. */
export const EVENT_PROTOCOL = 'provenance-review';

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
  /** The page's own-origin storage; the browser passes localStorage. */
  readonly storage: {
    getItem(key: string): string | null;
    setItem(key: string, value: string): void;
    removeItem(key: string): void;
  };
  /** Opens the host's event socket with the page session. */
  readonly openEvents: (session: string) => EventSocket;
  /** Calls the listener when the page is online or active again. */
  readonly onWake: (listener: () => void) => void;
}

/** Tells the person that the page does not show later saves until it reconnects. */
export const LIVE_UPDATES_PAUSED =
  'Live updates stopped. Focus this page to reconnect. Unsaved drafts stay open.';

/** Tells the person that a reviewer setting changed and how to apply it. */
export const SETTINGS_CHANGED =
  'Reviewer settings changed. Save or discard your drafts, then reload this page to apply the settings.';

const sameActors = (left: ReviewConfig, right: ReviewConfig) =>
  left.dispositionActorIds.length === right.dispositionActorIds.length &&
  left.dispositionActorIds.every((id, index) => right.dispositionActorIds[index] === id);

function element(document: BrowserDocument, id: string): BrowserElement {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing browser element: ${id}`);
  return found;
}

/** The reviewer that the host reports; the page decides only as this person. */
// @provenance rule: rule_review_host_reports_reviewer
function configReviewer(value: unknown): ConfigReviewer {
  if (typeof value !== 'object' || value === null ||
    !('actorId' in value) || typeof value.actorId !== 'string' ||
    !('allowed' in value) || typeof value.allowed !== 'boolean') {
    throw new Error('Invalid reviewer');
  }
  return { actorId: value.actorId, allowed: value.allowed };
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
    // The page decides only as the reviewer that the host reports, so the app never invents one.
    ...('reviewer' in value && value.reviewer !== undefined ? { reviewer: configReviewer(value.reviewer) } : {}),
  };
}

function launchSession(value: unknown): string {
  if (typeof value !== 'object' || value === null ||
    !('session' in value) || typeof value.session !== 'string' || value.session === '') {
    throw new Error('Invalid launch session');
  }
  return value.session;
}

/** Keeps the page session in this origin's storage and sends it only as a bearer to this origin. */
// @provenance rule: rule_review_page_session_stays_on_its_origin
async function startPageSession(
  page: Pick<BootstrapDependencies, 'fetch' | 'storage'> & { origin: string },
  code: string | null,
) {
  const { fetch, storage } = page;
  let bearer: string;
  if (code) {
    const launch = await fetch(LAUNCH_SESSION_ROUTE, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ code }), redirect: 'error', cache: 'no-store',
    });
    if (!launch.ok) throw new Error('Launch refused');
    bearer = launchSession(await launch.json());
    storage.setItem(SESSION_KEY, bearer);
  } else {
    // A reload or a new tab on this origin reuses the stored session.
    bearer = storage.getItem(SESSION_KEY) ?? '';
  }
  const configuration = async () => {
    const response = await fetch('/review-config', {
      headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
    });
    if (!response.ok) {
      // A session from an earlier host run is useless, so the page forgets it.
      if (storage.getItem(SESSION_KEY) === bearer) storage.removeItem(SESSION_KEY);
      throw new Error('Access refused');
    }
    return reviewConfig(await response.json(), page.origin);
  };
  return { bearer, configuration };
}

/** Takes the launch code out of the address bar and exchanges it once for the page session. */
export function bootstrapReviewPage(dependencies: BootstrapDependencies) {
  // A browser refuses `fetch` when it is called as a method of another object.
  const { fetch, location, storage } = dependencies;
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
  let firstReady = true;
  let pageSession = '';
  const events = connectEvents({
    open: () => dependencies.openEvents(pageSession),
    refresh: change => {
      session.refresh(change);
      if (change._tag !== 'Everything' || !configuration) return;
      if (change.reason === 'ready' && firstReady) {
        firstReady = false;
        return;
      }
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
    const access = await startPageSession({ fetch, storage, origin: location.origin }, code);
    pageSession = access.bearer;
    configuration = access.configuration;
    if (pageSession === '') throw new Error('No page session');
    await session.connect(pageSession, configuration);
  })().catch(() => { status.textContent = CONNECTION_REFUSED; });
}
