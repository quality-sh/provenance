import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { lintFixture, catalogNamesFromDocument } from './grammar-lint.mjs';

const fixture = JSON.parse(await readFile(new URL('./legacy-operation-coverage.json', import.meta.url), 'utf8'));

// The 16 review operations from PR #273 (branch codex/r4-transport-host),
// read from crates/provenance-store/src/operations/catalog/review.rs there.
const PR273_OPERATIONS = [
  'create-review-requirement',
  'save-requirement',
  'submit-requirement-review',
  'decide-requirement-review',
  'withdraw-requirement-review',
  'write-discussion',
  'requirement-edit-state',
  'requirement-decision-state',
  'review-history',
  'review-evidence',
  'review-discussions',
  'review-discussion-messages',
  'requirement-save-receipt',
  'requirement-creation-receipt',
  'discussion-receipt',
  'requirement-review-receipt',
];

test('every legacy operation is mapped exactly once and the fixture lints clean', () => {
  assert.deepEqual(lintFixture(fixture), []);
});

test('the coverage accounting is 74 catalog plus 16 review operations in nine buckets', () => {
  assert.equal(fixture.catalog_size, 74);
  assert.equal(fixture.review_size, 16);
  assert.equal(fixture.operations.length, 90);
  const sources = {};
  const buckets = {};
  for (const operation of fixture.operations) {
    sources[operation.source] = (sources[operation.source] ?? 0) + 1;
    buckets[operation.bucket] = (buckets[operation.bucket] ?? 0) + 1;
  }
  assert.deepEqual(sources, { catalog: 74, pr273: 16 });
  assert.deepEqual(buckets, fixture.accounting);
  assert.deepEqual(Object.values(fixture.accounting).reduce((a, b) => a + b, 0), 90);
});

test('the PR #273 review operations all appear', () => {
  const mapped = new Set(fixture.operations.filter(o => o.source === 'pr273').map(o => o.legacy));
  for (const name of PR273_OPERATIONS) assert.ok(mapped.has(name), `${name} is missing from the fixture`);
  assert.equal(mapped.size, PR273_OPERATIONS.length);
});

test('the legacy surface collapses; no operation survives as its own route', () => {
  const routePaths = new Set(fixture.routes.map(route => `${route.method} ${route.path}${route.query ? `?query=${route.query}` : ''}`));
  assert.equal(routePaths.size, fixture.routes.length);
  for (const route of fixture.routes) {
    assert.ok(!/^\/v\d+\//.test(route.path), `${route.id} carries a version prefix`);
    assert.ok(route.path.replace(/[[\]]/g, '').split('/').every(s => !s.startsWith('{') || fixture.variables.includes(s.replaceAll(/[{}]/g, ''))), `${route.id} has an undeclared path parameter`);
  }
});

test('receipt operations stay internal and declare no route', () => {
  const receipts = fixture.operations.filter(o => o.bucket === 'receipt');
  assert.equal(receipts.length, 4);
  for (const receipt of receipts) {
    assert.equal(receipt.internal, true);
    assert.deepEqual(receipt.routes, []);
  }
  const publicPaths = fixture.routes.map(r => r.path);
  assert.ok(publicPaths.every(p => !p.includes('receipt')), 'receipt handling must stay out of the public route table');
});

test('the catalog seam accepts the live catalog shape and rejects drift', () => {
  const catalogNames = fixture.operations.filter(o => o.source === 'catalog').map(o => o.legacy);
  const document = {
    paths: Object.fromEntries([
      ...catalogNames.map(name => [`/v9/operations/${name}`, {}]),
      ['/metadata', {}],
    ]),
  };
  assert.deepEqual(catalogNamesFromDocument(document), catalogNames);
  assert.deepEqual(lintFixture(fixture, { catalogNames: catalogNamesFromDocument(document) }), []);
  const drifted = { paths: Object.fromEntries([...catalogNames.slice(1).map(name => [`/v9/operations/${name}`, {}]), ['/metadata', {}]]) };
  const errors = lintFixture(fixture, { catalogNames: catalogNamesFromDocument(drifted) });
  assert.ok(errors.some(e => e.includes('is not in the live catalog')), errors.join('; '));
  assert.ok(errors.some(e => e.includes(`'${catalogNames[0]}'`)), errors.join('; '));
});
