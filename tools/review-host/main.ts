import { HttpClient } from '@quality-sh/provenance/client';
import type { components } from '@quality-sh/provenance/client';
import { loadReviewStore, mountReview, DocumentUnavailableError } from 'review-renderer';
import type { CursorReviewStore, DocumentLoader } from 'review-renderer';
import { createSession } from './session.ts';

const root = document.getElementById('root')!;
const access = document.getElementById('access') as HTMLFormElement;
const selection = document.getElementById('selection') as HTMLFormElement;
const credential = document.getElementById('credential') as HTMLInputElement;
const requirement = document.getElementById('requirement') as HTMLInputElement;
const status = document.getElementById('status')!;
const session = createSession<CursorReviewStore>({
  failure: error => error instanceof DocumentUnavailableError ? error.message : undefined,
  mount: store => mountReview(root, { store }),
  status: message => { status.textContent = message; },
});
let load: (() => Promise<CursorReviewStore>) | undefined;

access.addEventListener('submit', async event => {
  event.preventDefault();
  const bearer = credential.value;
  credential.value = '';
  status.textContent = 'Connecting…';
  try {
    const response = await fetch('/review-config', {
      headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
    });
    if (!response.ok) throw new Error('Access refused');
    const config: unknown = await response.json();
    if (typeof config !== 'object' || config === null ||
      !('repositoryId' in config) || typeof config.repositoryId !== 'string' ||
      !('scope' in config) || typeof config.scope !== 'string') throw new Error('Invalid configuration');
    const client = await HttpClient.connectWithBearer(location.origin, bearer);
    const repository = config.repositoryId;
    const scope = config.scope;
    load = () => {
      const id = requirement.value.trim();
      const context = { repository, scope };
      const loaderFor = (id: string): DocumentLoader => cursor => client.readDocument({
        context: { ...context, freshness: cursor ? 'annotate_only' : 'catch_up' },
        request: { id, limit: 50, cursor },
      });
      const search = (request: components['schemas']['SearchRequestInput']['request']) => client.search({
        context: { ...context, freshness: request.cursor ? 'annotate_only' : 'catch_up' },
        request: { ...request, limit: 50 },
      });
      return loadReviewStore(loaderFor(id), { openDocument: loaderFor, search });
    };
    access.hidden = true;
    selection.hidden = false;
    status.textContent = `Read-only · ${config.repositoryId} / ${config.scope}`;
    requirement.focus();
  } catch {
    status.textContent = 'Connection refused. Enter the access token from this host session.';
  }
});
selection.addEventListener('submit', event => {
  event.preventDefault();
  if (load) void session.refresh(load);
});
