import { HttpClient } from '@quality-sh/provenance/client';
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
    const repository = config.repositoryId;
    const scope = config.scope;
    const client = await HttpClient.connectWithBearer(
      location.origin, bearer, fetch, { repository, scope },
    );
    load = () => {
      const id = requirement.value.trim();
      const loaderFor = (id: string): DocumentLoader => async cursor => {
        const answer = await client.getRequirementDocument({ id, limit: 50, cursor });
        return { ...answer.data, ...answer.meta };
      };
      const search = async (request: { text: string; cursor?: string }) => {
        const answer = await client.listRequirements({
          query: 'search', text: request.text, limit: 50, cursor: request.cursor,
        });
        return { nodes: answer.data.items, ...answer.meta };
      };
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
