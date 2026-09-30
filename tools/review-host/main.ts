import { mountReview } from 'review-renderer';
import type { ReviewConfig } from './session.ts';
import { createSession } from './session.ts';

const root = document.getElementById('root')!;
const access = document.getElementById('access') as HTMLFormElement;
const selection = document.getElementById('selection') as HTMLFormElement;
const credential = document.getElementById('credential') as HTMLInputElement;
const requirement = document.getElementById('requirement') as HTMLInputElement;
const status = document.getElementById('status')!;

function reviewConfig(value: unknown): ReviewConfig {
  if (typeof value !== 'object' || value === null ||
    !('endpoint' in value) || typeof value.endpoint !== 'string' || value.endpoint !== location.origin ||
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

const session = createSession({
  mount: options => mountReview(root, options),
  connected: () => {
    access.hidden = true;
    selection.hidden = false;
    requirement.focus();
  },
  status: message => { status.textContent = message; },
});

access.addEventListener('submit', event => {
  event.preventDefault();
  const bearer = credential.value;
  credential.value = '';
  void session.connect(bearer, async () => {
    const response = await fetch('/review-config', {
      headers: { authorization: `Bearer ${bearer}` }, redirect: 'error', cache: 'no-store',
    });
    if (!response.ok) throw new Error('Access refused');
    const config = reviewConfig(await response.json());
    return {
      endpoint: config.endpoint,
      repositoryId: config.repositoryId,
      scope: config.scope,
      dispositionActorIds: config.dispositionActorIds,
    };
  });
});

selection.addEventListener('submit', event => {
  event.preventDefault();
  session.open(requirement.value.trim());
});
