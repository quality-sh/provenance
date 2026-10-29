import { test } from 'node:test';
import assert from 'node:assert/strict';
import { refusedFields, type Validator } from './renderer-reads-report.ts';

// Implementation aid: the report must name the field that a renderer validator refuses.
function validator(errors: Validator['errors']): Validator {
  const check: Validator = () => {
    check.errors = errors;
    return errors === undefined;
  };
  return check;
}

// A document entry without its statement, as the host could send it after a wire change.
const body = {
  data: { entries: [{ kind: 'member', node: { id: 'req_root', node_type: 'requirement', status: 'discovery' } }] },
};

test('a missing property is reported at its own path, beside the noise of the other union branches', () => {
  const refused = refusedFields(validator([
    { instancePath: '/data/entries/0/node', keyword: 'required', params: { missingProperty: 'statement' },
      message: "must have required property 'statement'" },
    { instancePath: '/data/entries/0/node/status', keyword: 'enum', message: 'must be equal to one of the allowed values' },
    { instancePath: '/data/entries/0/kind', keyword: 'const', message: 'must be equal to constant' },
    { instancePath: '/data/entries/0', keyword: 'anyOf', message: 'must match a schema in anyOf' },
  ]), body);
  assert.ok(refused.includes(
    "record req_root field /data/entries/0/node/statement: must have required property 'statement'",
  ), refused.join('\n'));
});

test('a refused body without errors gives a failure', () => {
  assert.deepEqual(refusedFields(validator(null), body), ['the validator refused the body and named no field']);
  assert.deepEqual(refusedFields(validator([]), body), ['the validator refused the body and named no field']);
});

test('an accepted body gives no failure', () => {
  assert.deepEqual(refusedFields(validator(undefined), body), []);
});
