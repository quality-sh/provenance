import { createSession, type ReviewConfig, type ReviewMountOptions } from './session.ts';

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
  readonly pathname?: string;
  readonly search: string;
}

interface BrowserHistory {
  replaceState(state: unknown, unused: string, url?: string | URL | null): void;
}

interface FetchResponse {
  readonly ok: boolean;
  json(): Promise<unknown>;
  text(): Promise<string>;
}

interface BootstrapDependencies {
  readonly document: BrowserDocument;
  readonly history: BrowserHistory;
  readonly location: BrowserLocation;
  readonly fetch: (path: string, init: {
    method?: 'POST';
    headers?: { authorization?: string; 'content-type'?: string };
    body?: string;
    redirect: 'error';
    cache: 'no-store';
  }) => Promise<FetchResponse>;
  readonly mount: (root: BrowserElement, options: ReviewMountOptions) => () => void;
}

function launchCredential(value: unknown): string {
  if (typeof value !== 'object' || value === null ||
    !('bearer' in value) || typeof value.bearer !== 'string' || value.bearer.length === 0) {
    throw new Error('Invalid launch response');
  }
  return value.bearer;
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

export function bootstrapReviewPage(dependencies: BootstrapDependencies) {
  const root = element(dependencies.document, 'root');
  const selection = element(dependencies.document, 'selection');
  const requirement = element(dependencies.document, 'requirement');
  const status = element(dependencies.document, 'status');
  const searchParams = new URLSearchParams(dependencies.location.search);
  const code = searchParams.get('code');
  const linkedRoot = searchParams.get('root');
  const linkedFocus = searchParams.get('focus') ?? undefined;
  searchParams.delete('code');
  const visibleQuery = searchParams.toString();
  dependencies.history.replaceState(
    null, '', `${dependencies.location.pathname ?? '/'}${visibleQuery ? `?${visibleQuery}` : ''}`,
  );
  const session = createSession({
    mount: options => dependencies.mount(root, options),
    connected: () => {
      selection.hidden = linkedRoot !== null;
      if (linkedRoot !== null) session.open(linkedRoot, linkedFocus);
      else requirement.focus();
    },
    status: message => { status.textContent = message; },
  });

  if (code === null) {
    status.textContent = 'Connection refused. Run `provenance <record-id> get --review-link` to get a fresh link.';
  } else {
    void (async () => {
      const exchange = await dependencies.fetch('/review-launch/exchange', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ code }), redirect: 'error', cache: 'no-store',
      });
      if (!exchange.ok) throw new Error(await exchange.text());
      const bearer = launchCredential(await exchange.json());
    void session.connect(bearer, async () => {
      const response = await dependencies.fetch('/review-config', {
        headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
      });
      if (!response.ok) throw new Error('Access refused');
      return reviewConfig(await response.json(), dependencies.location.origin);
    });
    })().catch(() => {
      status.textContent = 'Connection refused. Run `provenance <record-id> get --review-link` to get a fresh link.';
    });
  }
  selection.addEventListener('submit', event => {
    event.preventDefault();
    session.open(requirement.value.trim());
  });
}
