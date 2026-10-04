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
  readonly search: string;
}

interface FetchResponse {
  readonly ok: boolean;
  json(): Promise<unknown>;
}

interface BootstrapDependencies {
  readonly document: BrowserDocument;
  readonly location: BrowserLocation;
  readonly fetch: (path: string, init: {
    headers: { authorization: string };
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

export function bootstrapReviewPage(dependencies: BootstrapDependencies) {
  const root = element(dependencies.document, 'root');
  const access = element(dependencies.document, 'access');
  const selection = element(dependencies.document, 'selection');
  const credential = element(dependencies.document, 'credential');
  const requirement = element(dependencies.document, 'requirement');
  const status = element(dependencies.document, 'status');
  const searchParams = new URLSearchParams(dependencies.location.search);
  const linkedRoot = searchParams.get('root');
  const linkedFocus = searchParams.get('focus') ?? undefined;
  const session = createSession({
    mount: options => dependencies.mount(root, options),
    connected: () => {
      access.hidden = true;
      selection.hidden = linkedRoot !== null;
      if (linkedRoot !== null) session.open(linkedRoot, linkedFocus);
      else requirement.focus();
    },
    status: message => { status.textContent = message; },
  });

  access.addEventListener('submit', event => {
    event.preventDefault();
    const bearer = credential.value;
    credential.value = '';
    void session.connect(bearer, async () => {
      const response = await dependencies.fetch('/review-config', {
        headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
      });
      if (!response.ok) throw new Error('Access refused');
      return reviewConfig(await response.json(), dependencies.location.origin);
    });
  });
  selection.addEventListener('submit', event => {
    event.preventDefault();
    session.open(requirement.value.trim());
  });
}
