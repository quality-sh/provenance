import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validators } from './validators.mjs';

const ref = name => ({ $ref: `#/components/schemas/${name}` });
const json = schema => ({ content: { 'application/json': { schema } } });

function contract() {
  return {
    openapi: '3.1.0',
    paths: {
      '/replies': {
        post: {
          operationId: 'createReply',
          parameters: [{ name: 'If-Match', in: 'header', schema: ref('Revision') }],
          requestBody: json(ref('ReplyRequest')),
          responses: { 200: { description: 'Saved', ...json(ref('ReplyReceipt')) } },
        },
      },
    },
    components: { schemas: {
      ReplyReceipt: {
        type: 'object',
        properties: { data: ref('Reply'), meta: ref('ResponseMeta') },
        required: ['data', 'meta'],
        additionalProperties: false,
      },
      ResponseMeta: {
        type: 'object',
        properties: { next_cursor: { type: ['string', 'null'] } },
        additionalProperties: false,
      },
      Reply: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          version: { type: 'integer', format: 'uint64' },
          kind: { enum: ['reply'] },
          channel: { const: 'web' },
          author: ref('Author'),
          labels: { type: 'object', additionalProperties: { type: 'string' } },
          changes: { type: 'array', items: { type: 'object', properties: { field: { type: 'string' } }, required: ['field'], additionalProperties: false } },
        },
        required: ['id', 'version', 'kind', 'author'],
        additionalProperties: false,
      },
      Author: {
        type: 'object',
        properties: { name: { type: 'string' } },
        required: ['name'],
        additionalProperties: false,
      },
      ReplyRequest: {
        type: 'object',
        properties: { text: { type: 'string' }, author: ref('RequestAuthor') },
        required: ['text'],
        additionalProperties: false,
      },
      RequestAuthor: {
        type: 'object',
        properties: { name: { type: 'string' } },
        additionalProperties: false,
      },
      Revision: { type: 'string' },
    } },
  };
}

async function load(document, names) {
  const files = await validators(document, names, 'open', false);
  return import(`data:text/javascript;base64,${Buffer.from(files['open.mjs']).toString('base64')}`);
}

const receipt = () => ({
  data: { id: 'msg_1', version: 2, kind: 'reply', channel: 'web', author: { name: 'Ben' }, labels: { area: 'web' }, changes: [{ field: 'text' }] },
  meta: { next_cursor: null },
});

test('a response validator accepts an unknown field at every depth', async () => {
  const document = contract();
  const original = structuredClone(document);
  const wire = await load(document, ['ReplyReceipt', 'ReplyRequest']);
  assert.equal(wire.ReplyReceipt(receipt()), true);
  for (const add of [
    body => { body.added = 1; },
    body => { body.meta.added = 1; },
    body => { body.data.added = 1; },
    body => { body.data.author.added = 1; },
    body => { body.data.changes[0].added = 1; },
  ]) {
    const body = receipt();
    add(body);
    assert.equal(wire.ReplyReceipt(body), true, JSON.stringify(body));
  }
  assert.deepEqual(document, original, 'the exported document stays closed');
});

test('a response validator still refuses a missing or wrongly typed field', async () => {
  const wire = await load(contract(), ['ReplyReceipt']);
  for (const change of [
    body => { delete body.meta; },
    body => { delete body.data.version; },
    body => { delete body.data.author.name; },
    body => { delete body.data.changes[0].field; },
    body => { body.data.version = '2'; },
    body => { body.data.kind = 'invented'; },
    body => { body.data.channel = 'cli'; },
    body => { body.data.labels.area = 3; },
    body => { body.meta.next_cursor = 4; },
  ]) {
    const body = receipt();
    change(body);
    assert.equal(wire.ReplyReceipt(body), false, JSON.stringify(body));
  }
});

test('a request validator compiled in the same call still refuses an unknown field', async () => {
  const wire = await load(contract(), ['ReplyReceipt', 'ReplyRequest']);
  assert.equal(wire.ReplyRequest({ text: 'Agreed.', author: { name: 'Ben' } }), true);
  assert.equal(wire.ReplyRequest({ text: 'Agreed.', added: 1 }), false);
  assert.equal(wire.ReplyRequest({ text: 'Agreed.', author: { name: 'Ben', added: 1 } }), false);
});

const sharedMessage = /^A request and a response share these schema components: Author\./;

test('a component shared by a request and a response stops generation and names the component', async () => {
  const document = contract();
  document.components.schemas.ReplyRequest.properties.author = ref('Author');
  await assert.rejects(validators(document, ['ReplyReceipt'], 'open', false), { message: sharedMessage });
});

test('a request property named like a data keyword still reaches its component', async () => {
  const document = contract();
  document.components.schemas.ReplyRequest.properties.default = ref('Author');
  await assert.rejects(validators(document, ['ReplyReceipt'], 'open', false), { message: sharedMessage });
});

test('a query variant parameter is a request root', async () => {
  const document = contract();
  document.paths['/replies'].post['x-provenance-query-variants'] = [
    { selector: null, parameters: [{ name: 'author', in: 'query', schema: ref('Author') }], success: ref('ReplyReceipt'), failure: ref('ReplyReceipt') },
  ];
  await assert.rejects(validators(document, ['ReplyReceipt'], 'open', false), { message: sharedMessage });
});
