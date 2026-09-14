import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { build } from 'esbuild';
import * as Schema from 'effect/Schema';
import * as generator from './typescript.mjs';
import { validators } from './validators.mjs';

// @provenance rule: rule_sdk_bindings_derive_from_openapi
// @provenance verification: conformance
test('Effect schemas retain production wire values and reject contract violations', async () => {
  assert.equal(typeof generator.effectFiles, 'function');
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/fixtures.openapi.json', import.meta.url)));
  const names = Object.keys(document.components.schemas);
  for (const name of names) document.paths[`/fixture/${name}`] = { get: { operationId: name, responses: {
    200: { description: 'Fixture', content: { 'application/json': { schema: { $ref: `#/components/schemas/${name}` } } } },
  } } };
  const directory = await mkdtemp(join(import.meta.dirname, '.effect-test-'));
  try {
    for (const [name, source] of Object.entries(await generator.effectFiles(document))) await writeFile(join(directory, name), source);
    const shared = await validators(document, names, 'validators', false);
    await writeFile(join(directory, 'validators.mjs'), shared['validators.mjs']);
    await build({ entryPoints: [join(directory, 'effect-contract.ts')], outfile: join(directory, 'contract.mjs'), bundle: true, packages: 'external', format: 'esm' });
    const schemas = await import(join(directory, 'contract.mjs'));
    for (const [name, value] of Object.entries(document['x-wire-fixtures'])) {
      assert.deepEqual(Schema.decodeUnknownSync(schemas[name])(value), value, name);
      assert.deepEqual(Schema.encodeSync(schemas[name])(value), value, name);
    }
    const decode = (name, value) => Schema.decodeUnknownSync(schemas[name])(value);
    assert.throws(() => decode('CheckStatementInput', { statement: 'Stop.', extra: true }));
    const evidence = structuredClone(document['x-wire-fixtures'].EvidenceOutput);
    delete evidence.stale;
    assert.throws(() => decode('EvidenceOutput', evidence));
    assert.throws(() => decode('GraphNodeOutput', { node_type: 'invented' }));
    assert.throws(() => decode('ReportOutput', { ...document['x-wire-fixtures'].ReportOutput, issue: 8 }));
    for (const limit of [0, 201, null, '50']) assert.throws(() => decode('SearchInput', { text: 'x', limit }));
    assert.deepEqual(decode('SearchInput', { text: 'x' }), { text: 'x' });
    assert.ok(schemas.ProvenanceApi);
    const matchers = Object.entries(schemas).filter(([name]) => name.startsWith('match'));
    assert.ok(matchers.length > 0);
    // An exhaustive matcher dispatches on the wire discriminant, accepts the
    // `_` fallback for subsets, and throws on an unknown discriminant when no
    // fallback is given. The fixture document names its node union per the
    // GetOutput operation, when present.
    const match = schemas.matchGetOutputGraphNode;
    if (match !== undefined) {
      const node = document['x-wire-fixtures'].GraphNodeOutput;
      const marker = Symbol('marker');
      assert.equal(match(node, { [node.node_type]: () => marker }), marker);
      assert.equal(match(node, { _: () => marker }), marker);
      assert.throws(() => match({ ...node, node_type: 'invented' }, { [node.node_type]: () => marker }));
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
