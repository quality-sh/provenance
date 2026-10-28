import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, rm, readFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const pinPath = fileURLToPath(new URL('../review-assets.json', import.meta.url));
const pin = JSON.parse(await readFile(pinPath, 'utf8'));

test('composition needs only the renderer and output paths', async t => {
  const work = await mkdtemp(join(tmpdir(), 'review-build-'));
  t.after(() => rm(work, { recursive: true, force: true }));
  const renderer = join(work, 'renderer');
  const output = join(work, 'output');
  await mkdir(join(renderer, 'types/browser'), { recursive: true });
  await writeFile(join(renderer, 'types/browser/main.d.ts'), [
    'export interface ReviewMountOptions { endpoint: string; repositoryId: string; scope: string;',
    'dispositionActorIds: ReadonlyArray<string>; bearer: string; rootId: string }',
    'export type ReviewChange = { readonly _tag: "Everything"; readonly reason: string }',
    '  | { readonly _tag: "Records"; readonly records: ReadonlyArray<{ kind: string; id: string }> };',
    'export interface ReviewHandle { dispose(): void; refresh(change: ReviewChange): void;',
    'showRecord(rootId: string, focusId?: string): Promise<void> }',
    'export declare function mountReview(element: Element, options: ReviewMountOptions): ReviewHandle;',
  ].join('\n'));
  await writeFile(join(renderer, 'review.js'),
    'export function mountReview() { return { dispose() {}, refresh() {}, showRecord: async () => {} }; }');
  await writeFile(join(renderer, 'build-info.json'), JSON.stringify({
    formatVersion: 1, commit: pin.commit, dirty: false,
  }));
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./build.ts', import.meta.url)), renderer, output, pinPath], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  await access(join(output, 'host.js'));
});

for (const [change, message] of [
  [{ commit: '0'.repeat(40) }, /Renderer identity/],
  [{ dirty: true }, /Renderer identity/],
] as const) {
  test(`composition refuses changed ${Object.keys(change)[0]}`, async t => {
    const work = await mkdtemp(join(tmpdir(), 'review-build-'));
    t.after(() => rm(work, { recursive: true, force: true }));
    const renderer = join(work, 'renderer');
    const output = join(work, 'output');
    await mkdir(join(renderer, 'types/browser'), { recursive: true });
    await writeFile(join(renderer, 'types/browser/main.d.ts'), 'export declare function mountReview(element: Element, options: unknown): () => void;');
    await writeFile(join(renderer, 'build-info.json'), JSON.stringify(Object.assign({
      formatVersion: 1, commit: pin.commit, dirty: false,
    }, change)));
    const result = spawnSync(process.execPath, [fileURLToPath(new URL('./build.ts', import.meta.url)), renderer, output, pinPath], { encoding: 'utf8' });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, message);
    await assert.rejects(access(output));
  });
}

// The pinned renderer's declarations import packages that the host does not install.
async function rendererWithReviewer(work: string, reviewer: string | undefined) {
  const renderer = join(work, 'renderer');
  await mkdir(join(renderer, 'types/browser'), { recursive: true });
  await mkdir(join(renderer, 'types/review-data'), { recursive: true });
  await writeFile(join(renderer, 'types/browser/main.d.ts'), [
    "import type { ConfigReviewer } from '../review-data/reviewer';",
    'export interface ReviewMountOptions { endpoint: string; repositoryId: string; scope: string;',
    'dispositionActorIds?: ReadonlyArray<string>; reviewer?: ConfigReviewer; bearer: string; rootId: string;',
    'focusId?: string }',
    'export type ReviewChange = { readonly _tag: "Everything"; readonly reason: string }',
    '  | { readonly _tag: "Records"; readonly records: ReadonlyArray<{ kind: string; id: string }> };',
    'export interface ReviewHandle { dispose(): void; refresh(change: ReviewChange): void;',
    'showRecord(rootId: string, focusId?: string): Promise<void> }',
    'export declare function mountReview(element: Element, options: ReviewMountOptions): ReviewHandle;',
  ].join('\n'));
  if (reviewer !== undefined) {
    await writeFile(join(renderer, 'types/review-data/reviewer.d.ts'), [
      "import * as Option from 'effect/Option';",
      reviewer,
      'export declare const reviewerActor: (reviewer: ConfigReviewer) => Option.Option<string>;',
    ].join('\n'));
  }
  await writeFile(join(renderer, 'review.js'),
    'export function mountReview() { return { dispose() {}, refresh() {}, showRecord: async () => {} }; }');
  await writeFile(join(renderer, 'build-info.json'), JSON.stringify({
    formatVersion: 1, commit: pin.commit, dirty: false,
  }));
  return renderer;
}

const goodReviewer = 'export interface ConfigReviewer { readonly actorId: string; readonly allowed: boolean }';
for (const [name, reviewer, refusal] of [
  ['a renderer package that the host does not install', goodReviewer, undefined],
  ['a missing renderer declaration file', undefined, /Cannot find module '\.\.\/review-data\/reviewer'/],
  ['a wrong ConfigReviewer shape', 'export interface ConfigReviewer { readonly actorId: number; readonly allowed: boolean }',
    /is not assignable/],
] as const) {
  test(`the host typecheck ${refusal ? 'fails on' : 'ignores'} ${name}`, async t => {
    const work = await mkdtemp(join(tmpdir(), 'review-build-'));
    t.after(() => rm(work, { recursive: true, force: true }));
    const renderer = await rendererWithReviewer(work, reviewer);
    const output = join(work, 'output');
    const result = spawnSync(process.execPath, [fileURLToPath(new URL('./build.ts', import.meta.url)), renderer, output, pinPath], { encoding: 'utf8' });
    if (!refusal) {
      assert.equal(result.status, 0, result.stderr);
      return;
    }
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Host typecheck failed/);
    assert.match(result.stderr, refusal);
  });
}
