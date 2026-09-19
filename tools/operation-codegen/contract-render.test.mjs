import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { topLevelSplit, substitute, declaration } from './contract-render.mjs';

test('topLevelSplit ignores separators nested in brackets, angles, and strings', () => {
  assert.deepEqual(topLevelSplit('a, b', ', '), ['a', 'b']);
  assert.deepEqual(topLevelSplit('{ a, b }, { c }, x', ', '), ['{ a, b }', '{ c }', 'x']);
  const union = `{ readonly "k": 'a' | 'b' } & { readonly [x: string]: Schema.Json } | string`;
  assert.deepEqual(topLevelSplit(union, ' | '), [
    `{ readonly "k": 'a' | 'b' } & { readonly [x: string]: Schema.Json }`,
    'string',
  ]);
  assert.deepEqual(topLevelSplit('ReadonlyArray<A | B> | C', ' | '), ['ReadonlyArray<A | B>', 'C']);
  assert.deepEqual(topLevelSplit(`'v9' | "x, y"`, ', '), [`'v9' | "x, y"`]);
});

test('substitute replaces only structured renderings and deduplicates by text', () => {
  const renderings = new Map([
    ['Big', `{ readonly "a": 1 } & { readonly [x: string]: Schema.Json }`],
    ['Small', '9'],
    ['Twin', '9'],
    ['CopyOfBig', `{ readonly "a": 1 } & { readonly [x: string]: Schema.Json }`],
  ]);
  const text = `v9 and { readonly "a": 1 } & { readonly [x: string]: Schema.Json } twice: { readonly "a": 1 } & { readonly [x: string]: Schema.Json }`;
  const { rewritten, used, canonical } = substitute(text, renderings);
  assert.match(rewritten, /v9 and Big twice: Big/);
  assert.deepEqual([...used].sort(), ['Big']);
  assert.equal(canonical.get('CopyOfBig'), 'Big');
  assert.equal(canonical.get('Big'), 'Big');
  assert.equal(canonical.has('Small'), false);
  // exceptRendering drops the whole identical group, not just one name.
  const exceptSelf = substitute(renderings.get('Big'), renderings, renderings.get('Big'));
  assert.equal(exceptSelf.rewritten, renderings.get('Big'));
});

test('substitute with no structured renderings leaves the text untouched', () => {
  const renderings = new Map([['Only', '9']]);
  const { rewritten, used } = substitute('keep 9', renderings);
  assert.equal(rewritten, 'keep 9');
  assert.equal(used.size, 0);
});

test('union declarations break one alternative per line when long', () => {
  const short = declaration('Short', 'A | B');
  assert.equal(short, 'export type Short = A | B');
  const long = declaration('Long', `${'A'.repeat(90)} | ${'B'.repeat(90)} | ${'C'.repeat(90)}`);
  assert.match(long, /^export type Long =\n  \| A{90}\n  \| B{90}\n  \| C{90}$/);
});

// Invariants of the real generated contract: no thousand-char type lines, the
// failure unions have generated exhaustive matchers, and every referenced
// factored name is declared.
test('generated Effect contract is factored and self-contained', async () => {
  const source = await readFile(new URL('../../packages/provenance/src/generated/effect-contract.ts', import.meta.url), 'utf8');
  const declared = new Set([...source.matchAll(/^export (?:type|const|function|class) (\w+)/gm)].map(match => match[1]));
  for (const match of source.matchAll(/\b(FailureVariant\w+)\b/g)) {
    assert.ok(declared.has(match[1]), `${match[1]} referenced but never declared`);
  }
  assert.match(source, /^export type UpdateSourceFailureWriteFailure =/m,
    'a write operation should retain its typed failure family');
  assert.match(source, /^export function matchUpdateSourceFailureWriteFailure</m,
    'typed failure families should have exhaustive matchers');
});
