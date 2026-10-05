import {
  CONNECTION_REFUSED, createSession, type ReviewConfig, type ReviewMountOptions,
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
  readonly mount: (root: BrowserElement, options: ReviewMountOptions) => () => void;
}

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
  const session = createSession({
    mount: options => dependencies.mount(root, options),
    connected: () => {
      selection.hidden = linkedRoot !== null;
      if (linkedRoot !== null) session.open(linkedRoot, linkedFocus);
      else requirement.focus();
    },
    status: message => { status.textContent = message; },
  });
  selection.addEventListener('submit', event => {
    event.preventDefault();
    session.open(requirement.value.trim());
  });

  if (!code) {
    status.textContent = CONNECTION_REFUSED;
    return;
  }
  void (async () => {
    const launch = await fetch(LAUNCH_SESSION_ROUTE, {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ code }), redirect: 'error', cache: 'no-store',
    });
    if (!launch.ok) throw new Error('Launch refused');
    const bearer = sessionBearer(await launch.json());
    await session.connect(bearer, async () => {
      const response = await fetch('/review-config', {
        headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
      });
      if (!response.ok) throw new Error('Access refused');
      return reviewConfig(await response.json(), location.origin);
    });
  })().catch(() => { status.textContent = CONNECTION_REFUSED; });
}
