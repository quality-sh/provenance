import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import standaloneCode from 'ajv/dist/standalone/index.js';
import { build } from 'esbuild';
import { operations, queryVariants } from './shared.mjs';

const schemaName = schema => schema.$ref.split('/').at(-1);

export function operationResponseSchemas(document) {
  return new Map(operations(document).map(({ op }) => {
    const names = new Set();
    for (const response of Object.values(op.responses ?? {})) {
      const schema = response.content?.['application/json']?.schema;
      if (schema?.$ref) names.add(schemaName(schema));
    }
    for (const variant of queryVariants(op)) {
      names.add(schemaName(variant.success));
      names.add(schemaName(variant.failure));
    }
    return [op.operationId, [...names].sort()];
  }));
}

export async function operationValidators(document, prefix = 'validators') {
  const files = {};
  for (const [operation, names] of operationResponseSchemas(document)) {
    Object.assign(files, await validators(document, names, `${prefix}/${operation}`, false));
  }
  return files;
}

export function responseSchemas(document) {
  const names = new Set();
  for (const route of Object.values(document.paths)) {
    for (const operation of Object.values(route)) {
      if (!operation?.responses) continue;
      for (const response of Object.values(operation.responses)) {
        names.add(schemaName(response.content['application/json'].schema));
      }
      for (const variant of operation['x-provenance-query-variants'] ?? []) {
        names.add(schemaName(variant.success));
        names.add(schemaName(variant.failure));
      }
    }
  }
  return [...names].sort();
}

const componentPrefix = '#/components/schemas/';
// Keywords whose value is data, not a schema: a key named like a keyword
// inside them is not a keyword.
const dataKeywords = new Set(['enum', 'const', 'default', 'examples', 'example']);
// Keywords whose value maps names to schemas.
const schemaMaps = new Set(['properties', 'patternProperties', 'dependentSchemas', '$defs', 'definitions']);

function collectReferences(node, names) {
  if (Array.isArray(node)) for (const item of node) collectReferences(item, names);
  else if (node && typeof node === 'object') {
    if (typeof node.$ref === 'string' && node.$ref.startsWith(componentPrefix)) names.add(node.$ref.slice(componentPrefix.length));
    for (const [key, value] of Object.entries(node)) if (!dataKeywords.has(key)) collectReferences(value, names);
  }
}

function closure(document, roots) {
  const reached = new Set();
  const pending = [...roots];
  while (pending.length) {
    const name = pending.pop();
    if (reached.has(name) || !Object.hasOwn(document.components?.schemas ?? {}, name)) continue;
    reached.add(name);
    const next = new Set();
    collectReferences(document.components.schemas[name], next);
    pending.push(...next);
  }
  return reached;
}

function directionClosures(document) {
  const requestRoots = new Set();
  for (const route of Object.values(document.paths ?? {})) {
    collectReferences(route?.parameters, requestRoots);
    for (const operation of Object.values(route ?? {})) {
      if (!operation?.responses) continue;
      collectReferences(operation.parameters, requestRoots);
      collectReferences(operation.requestBody, requestRoots);
    }
  }
  const responses = closure(document, document.paths ? responseSchemas(document) : []);
  const requests = closure(document, requestRoots);
  return { responses, requests, shared: [...responses].filter(name => requests.has(name)).sort() };
}

/**
 * The components that a response can reach. A component that a request can
 * also reach stops generation, so that opening responses never opens a
 * request.
 */
export function responseClosure(document) {
  const { responses, shared } = directionClosures(document);
  if (shared.length) throw new Error(`A request and a response share these schema components: ${shared.join(', ')}. Response validators ignore unknown fields and request validators refuse them, so each component must belong to one side.`);
  return responses;
}

// Response validators ignore unknown fields: a closed object becomes open.
// A map's value schema, required fields, types, enums and constants stay.
function openObjects(node) {
  if (Array.isArray(node)) { for (const item of node) openObjects(item); return; }
  if (!node || typeof node !== 'object') return;
  for (const keyword of ['additionalProperties', 'unevaluatedProperties']) if (node[keyword] === false) delete node[keyword];
  for (const [key, value] of Object.entries(node)) {
    if (dataKeywords.has(key)) continue;
    if (schemaMaps.has(key) && value && typeof value === 'object' && !Array.isArray(value)) for (const schema of Object.values(value)) openObjects(schema);
    else openObjects(value);
  }
}

// A hoisted document names identical request and response schemas once, so
// it can share a component that the source contract keeps apart. There, a
// shared component stays closed; the source contract went through the check.
function validationSchemas(document, hoisted) {
  const schemas = structuredClone(document.components.schemas);
  const { responses, requests } = hoisted ? directionClosures(document) : { responses: responseClosure(document), requests: new Set() };
  for (const name of responses) if (!requests.has(name)) openObjects(schemas[name]);
  return schemas;
}

export async function validators(document, names = responseSchemas(document), prefix = 'validators', _compatibility = true, { hoisted = false } = {}) {
  const schemas = validationSchemas(document, hoisted);
  const ajv = new Ajv({ code: { source: true, esm: true }, strict: true, allowUnionTypes: true });
  addFormats(ajv);
  for (const format of ['int64', 'int32', 'uint64', 'uint32', 'uint16', 'uint8', 'uint', 'int', 'float', 'double']) ajv.addFormat(format, true);
  ajv.addKeyword({ keyword: 'components', schemaType: 'object', valid: true });
  ajv.addKeyword({ keyword: 'x-provenance-model-family', schemaType: 'string', valid: true });
  ajv.addKeyword({ keyword: 'x-provenance-validation-only-any-of', schemaType: 'boolean', valid: true });
  const id = 'urn:provenance:operation-responses';
  ajv.addSchema({ $id: id, components: { schemas } });
  const exports = Object.fromEntries(names.map(name => [name, `${id}#/components/schemas/${name}`]));
  const code = standaloneCode(ajv, exports);
  const bundled = await build({ stdin: { contents: code, resolveDir: import.meta.dirname, sourcefile: 'validators.js' }, bundle: true, platform: 'browser', format: 'esm', write: false, minify: true, legalComments: 'none' });
  return {
    [`${prefix}.mjs`]: '// Generated from OpenAPI. Do not edit.\n' + bundled.outputFiles[0].text,
    [`${prefix}.d.mts`]: '// Generated from OpenAPI. Do not edit.\nexport {};\n'
      + names.map(name => `export declare function ${name}(value: unknown): boolean;`).join('\n') + '\n',
  };
}
