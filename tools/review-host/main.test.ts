import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const source = await readFile(fileURLToPath(new URL('./main.ts', import.meta.url)), 'utf8');

test('the host passes the named configuration, token, and Requirement to the renderer', () => {
  assert.match(source, /mountReview\(root, options\)/);
  assert.match(source, /endpoint:\s*config\.endpoint/);
  assert.match(source, /repositoryId:\s*config\.repositoryId/);
  assert.match(source, /scope:\s*config\.scope/);
  assert.match(source, /dispositionActorIds:\s*config\.dispositionActorIds/);
  assert.match(source, /bearer/);
  assert.match(source, /session\.open\(requirement\.value\.trim\(\)\)/);
});

test('the host does not keep the old store adapter or SDK client', () => {
  assert.doesNotMatch(source, /loadReviewStore|CursorReviewStore|DocumentLoader|HttpClient/);
  assert.doesNotMatch(source, /@quality-sh\/provenance\/client/);
});

test('the host reads configuration with the bearer credential', () => {
  assert.match(source, /fetch\('\/review-config'/);
  assert.match(source, /authorization:\s*`Bearer \$\{bearer\}`/);
  assert.match(source, /credential\.value = ''/);
  assert.match(source, /value\.endpoint !== location\.origin/);
});
