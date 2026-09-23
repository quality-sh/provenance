import test from 'node:test';
import assert from 'node:assert/strict';
import { rustClientFiles, typescriptClient } from './templates.mjs';
import { allocateEnums } from './rust-parameters.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });

const response = () => ({
  200: { content: { 'application/json': { schema: reference('FixtureSuccess') } } },
  400: { content: { 'application/json': { schema: reference('FixtureFailure') } } },
});

const document = () => ({
  paths: {
    // A declared closed selection on a query parameter and on a path parameter.
    '/fixtures/{id}/view/{side}': { get: {
      operationId: 'viewFixture', 'x-operation-mutates': false,
      parameters: [
        { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
        { name: 'side', in: 'path', required: true, schema: { type: 'string', enum: ['before', 'after'] } },
        { name: 'direction', in: 'query', required: false, schema: { type: 'string', enum: ['out', 'in', 'both'] } },
      ],
      responses: response(),
    } },
    '/viewpoints': { get: {
      operationId: 'listViewpoints', 'x-operation-mutates': false,
      parameters: [
        { name: 'direction', in: 'query', required: true, schema: { type: 'string', enum: ['out', 'in', 'both'] } },
        { name: 'view', in: 'query', required: false, schema: { type: 'string', enum: ['before', 'after'] } },
      ],
      responses: response(),
    } },
  },
});

const compatibility = { wire: 9, state: 2, review_journal: 1, read_derivation: 1 };

test('declared parameter enums render as closed TypeScript unions', () => {
  const source = typescriptClient(document(), compatibility);
  assert.match(source, /"side": 'before' \| 'after'/);
  assert.match(source, /"direction"\?: 'out' \| 'in' \| 'both'/);
  assert.match(source, /"direction": 'out' \| 'in' \| 'both'/);
});

test('declared parameter enums render as closed Rust types with exact wire encoding', () => {
  const files = rustClientFiles(document(), compatibility);
  const source = Object.values(files).join('\n');
  // One shared enum per value set; the second `view` value set reuses the name
  // of the side set because the values match.
  assert.match(source, /pub enum Side \{\s+Before,\s+After,\s+\}/);
  assert.match(source, /pub enum Direction \{\s+Out,\s+In,\s+Both,\s+\}/);
  // Invented values have no variant to name.
  assert.doesNotMatch(source, /Sideways|Sideway|Invented/);
  // Required and optional positions keep their Rust shapes.
  assert.match(files['operations/view_fixture.rs'], /side: Side/);
  assert.match(files['operations/view_fixture.rs'], /direction: Option<Direction>/);
  assert.match(files['operations/view_fixture.rs'], /runtime::path\(side\.as_str\(\)\)/);
  assert.match(files['operations/view_fixture.rs'], /value\.as_str\(\)/);
  assert.match(files['operations/list_viewpoints.rs'], /direction: Direction/);
  assert.match(files['operations/list_viewpoints.rs'], /direction\.as_str\(\)/);
  // The generated module carries the closed enum.
  assert.match(files['parameters.rs'], /Self::Before => "before"/);
  assert.match(files['parameters.rs'], /Self::Both => "both"/);
});

test('identical value sets share one enum and distinct sets get distinct names', () => {
  const files = rustClientFiles(document(), compatibility);
  const parameters = files['parameters.rs'];
  for (const name of ['Side', 'Direction']) {
    const count = [...parameters.matchAll(new RegExp(`pub enum ${name} \\{`, 'g'))].length;
    assert.equal(count, 1, `${name} is emitted once`);
  }
  assert.match(parameters, /pub enum Side \{/);
  assert.match(parameters, /pub enum Direction \{/);
});

test('query variants allocate enums only for parameters their methods use', () => {
  const document = { paths: { '/query': { get: {
    operationId: 'queryRecords',
    parameters: [{ name: 'query', in: 'query', schema: { type: 'string', enum: ['search', 'trace'] } }],
    'x-provenance-query-variants': [{
      parameters: [{ name: 'direction', in: 'query', schema: { type: 'string', enum: ['in', 'out'] } }],
    }],
  } } } };
  const enums = allocateEnums(document);
  assert.deepEqual([...enums.values()], ['Direction']);
});
