// Grammar linter for the frozen v2 operation surface.
//
// Generation passes the live OpenAPI and MCP documents to this module. The
// checks reject route shapes and wire schemas that violate
// docs/api-contract-v2.md.

const METHODS = new Set(['GET', 'POST', 'PATCH']);
const COLLECTIONS = new Set([
  'sources', 'requirements', 'resolutions', 'rules', 'domains', 'boundaries',
  'topics', 'questions', 'contributions', 'synthesis-packets', 'proposals',
  'verification-runs', 'verification-bindings', 'discussion-containers',
  'messages', 'assertions', 'dispositions',
]);
const COMPUTE_COLLECTIONS = new Set([
  'statement-checks', 'authoring-plans', 'authoring-changes',
]);
const ACTIONS = new Set([
  'claim', 'release', 'close', 'answer', 'submit', 'decide', 'withdraw',
  'begin-verification', 'complete-verification',
]);
const QUERIES = new Set([
  'search', 'stale', 'impact', 'trace', 'neighbors', 'resolve-symbol',
]);
const BASE_STATUSES = [400, 401, 403, 404, 405, 500, 503];

// Edge vocabulary: names that describe a relationship, never a resource.
const EDGE_SEGMENTS = new Set([
  'refines', 'refined-by', 'depends-on', 'depended-on-by', 'supersedes',
  'superseded-by', 'spawned-by', 'contradicts', 'contradicted-by',
  'source-references', 'source-refs', 'edges', 'relations', 'relationships',
]);

// Parent-owned child addresses the contract declares.
const CHILD_SEGMENTS = new Set([
  'discussions', 'messages', 'discussion-containers', 'legacy-messages',
  'assertions', 'dispositions', 'history', 'evidence', 'document',
  'submissions',
]);

const BARE_VERBS = new Set([
  'create', 'update', 'delete', 'get', 'list', 'set', 'clear', 'add', 'remove',
  'upsert', 'post', 'read', 'save', 'write', 'check', 'plan', 'apply', 'begin',
  'complete', 'resolve', 'search', 'stale', 'impact', 'trace', 'neighbors',
  'operations', 'query',
]);
const LEADING_VERBS = new Set([
  'create', 'update', 'delete', 'get', 'list', 'set', 'clear', 'add', 'remove',
  'upsert', 'post', 'read', 'save', 'write', 'check', 'plan', 'apply', 'begin',
  'complete', 'resolve', 'submit', 'decide', 'withdraw', 'claim', 'release',
  'close', 'answer', 'search', 'stale', 'impact', 'trace', 'neighbors',
]);

function pathVariables(path) {
  return [...path.matchAll(/\{([a-z0-9_]+)\}/g)].map(match => match[1]);
}

function concreteSegments(path) {
  return path.split('/').filter(Boolean).filter(segment => !/^\{/.test(segment));
}

function operationEntries(document) {
  return Object.entries(document?.paths ?? {}).flatMap(([path, item]) =>
    Object.entries(item ?? {}).flatMap(([method, operation]) => {
      const upper = method.toUpperCase();
      return METHODS.has(upper) ? [{ path, method: upper, operation }] : [];
    }));
}

function resolvePointer(root, reference) {
  if (typeof reference !== 'string' || !reference.startsWith('#/')) return null;
  let value = root;
  for (const raw of reference.slice(2).split('/')) {
    const part = raw.replaceAll('~1', '/').replaceAll('~0', '~');
    value = value?.[part];
  }
  return value ?? null;
}

function resolveSchema(root, schema) {
  return resolvePointer(root, schema?.$ref) ?? schema;
}

function schemaVariants(root, schema, seen = new Set()) {
  if (schema === null || typeof schema !== 'object') return [];
  if (typeof schema.$ref === 'string') {
    if (seen.has(schema.$ref)) return [];
    const resolved = resolvePointer(root, schema.$ref);
    return resolved ? schemaVariants(root, resolved, new Set([...seen, schema.$ref])) : [];
  }
  const union = schema.anyOf ?? schema.oneOf;
  return Array.isArray(union)
    ? union.flatMap(child => schemaVariants(root, child, seen))
    : [schema];
}

function envelopeErrors(root, schema, requiredFields, where) {
  const variants = schemaVariants(root, schema);
  if (variants.length === 0) return [`${where}: response schema is missing`];
  for (const variant of variants) {
    const required = new Set(variant.required ?? []);
    const properties = variant.properties ?? {};
    if (!requiredFields.every(field => required.has(field) && properties[field])) {
      return [`${where}: flattened or non-standard envelope is rejected; require ${requiredFields.join(' and ')}`];
    }
  }
  return [];
}

function routePathErrors(path) {
  const errors = [];
  const where = `path ${path}`;
  if (!path.startsWith('/')) errors.push(`${where}: path does not start with /`);
  if (/^\/v\d+(?:\/|$)/.test(path)) errors.push(`${where}: versioned URL prefix is rejected`);
  if (path.includes('/operations/')) errors.push(`${where}: legacy operations path is rejected`);

  const segments = path.split('/').filter(Boolean);
  if (segments.includes('{repository}') || segments.includes('{scope}')
      || segments.includes('repositories') || segments.includes('scopes')) {
    errors.push(`${where}: repository or scope path prefix is rejected; identity binds at connection`);
  }
  if (segments[0] === 'query') {
    errors.push(`${where}: /query subroutes are rejected; queries are GET parameters on their owning path`);
  }
  if (segments[0]?.includes('-') && LEADING_VERBS.has(segments[0].split('-')[0])
      && !COMPUTE_COLLECTIONS.has(segments[0])) {
    errors.push(`${where}: verb-led route '${segments[0]}' is rejected`);
  }

  for (const segment of concreteSegments(path)) {
    if (EDGE_SEGMENTS.has(segment)) {
      errors.push(`${where}: relationship route segment '${segment}' is rejected; relationships are PATCH fields`);
    }
    if (BARE_VERBS.has(segment)) errors.push(`${where}: legacy verb segment '${segment}' is rejected`);
  }
  for (const segment of concreteSegments(path).slice(1)) {
    if (COLLECTIONS.has(segment) && !CHILD_SEGMENTS.has(segment)) {
      errors.push(`${where}: collection '${segment}' as a subresource is a relationship route and is rejected`);
    } else if (!CHILD_SEGMENTS.has(segment) && !ACTIONS.has(segment)) {
      errors.push(`${where}: subresource segment '${segment}' is not a declared child address or action`);
    }
  }

  const root = segments[0];
  if (root !== undefined && root !== 'metadata'
      && !COLLECTIONS.has(root) && !COMPUTE_COLLECTIONS.has(root)) {
    errors.push(`${where}: root segment '${root}' is not a declared collection`);
  }
  return errors;
}

function operationShapeErrors(document, path, method, operation, seenIds, seenBindings) {
  const errors = [];
  const where = `${method} ${path}`;
  const operationId = operation?.operationId;
  if (typeof operationId !== 'string' || operationId.length === 0) {
    errors.push(`${where}: operationId is missing`);
  } else if (seenIds.has(operationId)) {
    errors.push(`${where}: operationId '${operationId}' collides with ${seenIds.get(operationId)}`);
  } else {
    seenIds.set(operationId, where);
  }

  const binding = `${method} ${path.replace(/\{[a-z0-9_]+\}/g, '{}')}`;
  if (seenBindings.has(binding)) errors.push(`${where}: duplicate binding ${binding}`);
  else seenBindings.add(binding);

  const parameters = Array.isArray(operation?.parameters) ? operation.parameters : [];
  const declared = new Set(parameters
    .filter(parameter => parameter?.in === 'path')
    .map(parameter => parameter.name));
  const variables = pathVariables(path);
  for (const variable of variables) {
    if (!declared.has(variable)) errors.push(`${where}: path parameter {${variable}} is not declared`);
  }
  for (const variable of declared) {
    if (!variables.includes(variable)) errors.push(`${where}: declared path parameter {${variable}} is not in the path`);
  }

  const segments = path.split('/').filter(Boolean);
  segments.forEach((segment, index) => {
    if (!ACTIONS.has(segment)) return;
    if (method !== 'POST') errors.push(`${where}: actions are POSTs`);
    if (index !== segments.length - 1) {
      errors.push(`${where}: action segment '${segment}' must be final`);
    }
  });

  if (method === 'GET' && operation?.requestBody !== undefined) {
    errors.push(`${where}: GET bodies are rejected`);
  }
  if (operation?.requestBody !== undefined) {
    const body = operation.requestBody?.content?.['application/json']?.schema;
    const requestErrors = envelopeErrors(document, body, ['data'], `${where} request`);
    if (operation.requestBody?.required !== true || requestErrors.length > 0) {
      errors.push(`${where}: request must use the required typed {data} envelope; required-null or empty request slots are rejected`);
    }
  }

  const mutates = operation?.['x-operation-mutates'];
  if (typeof mutates !== 'boolean') {
    errors.push(`${where}: x-operation-mutates must be declared explicitly`);
  } else if (method === 'GET' && mutates) {
    errors.push(`${where}: mutating GETs are rejected`);
  } else if (method === 'PATCH' && !mutates) {
    errors.push(`${where}: a PATCH is a write and must declare x-operation-mutates true`);
  }

  const query = parameters.find(parameter => parameter?.in === 'query' && parameter.name === 'query');
  if (query) {
    if (method !== 'GET') errors.push(`${where}: queries are GETs`);
    for (const name of query.schema?.enum ?? []) {
      if (!QUERIES.has(name)) errors.push(`${where}: query '${name}' is not declared`);
    }
  }

  const responses = operation?.responses;
  const statuses = Object.keys(responses ?? {}).map(Number).filter(status => status >= 400);
  if (statuses.length === 0) {
    errors.push(`${where}: failure statuses must be declared`);
  } else {
    for (const base of BASE_STATUSES) {
      if (!statuses.includes(base)) errors.push(`${where}: base status ${base} is not declared`);
    }
    if (mutates === true && !statuses.includes(409)) {
      errors.push(`${where}: a mutating route must declare 409`);
    }
  }

  const success = responses?.['200']?.content?.['application/json']?.schema;
  errors.push(...envelopeErrors(document, success, ['data', 'meta'], `${where} success`));
  const failure = Object.entries(responses ?? {})
    .find(([status]) => Number(status) >= 400)?.[1]
    ?.content?.['application/json']?.schema;
  errors.push(...envelopeErrors(document, failure, ['error', 'meta'], `${where} failure`));

  if (path.includes('{message_id}')) {
    for (const envelope of schemaVariants(document, success)) {
      const data = resolveSchema(document, envelope.properties?.data);
      if (data?.properties?.items) {
        errors.push(`${where}: a single-message read must not ship the items envelope`);
        break;
      }
    }
  }
  return errors;
}

export function routeGrammarErrors(document) {
  const errors = [];
  for (const [path, item] of Object.entries(document?.paths ?? {})) {
    errors.push(...routePathErrors(path));
    for (const method of Object.keys(item ?? {})) {
      const upper = method.toUpperCase();
      if (!METHODS.has(upper)) errors.push(`${upper} ${path}: method is not GET, POST, or PATCH`);
    }
  }
  const seenIds = new Map();
  const seenBindings = new Set();
  for (const { path, method, operation } of operationEntries(document)) {
    errors.push(...operationShapeErrors(document, path, method, operation, seenIds, seenBindings));
  }
  return errors;
}

const FAILURE_STATUS = new Map([
  ['invalid_input', 400], ['protocol_mismatch', 400],
  ['unknown_operation', 404], ['unknown_target', 404], ['unknown_scope', 404],
  ['unauthenticated', 401], ['access_denied', 403], ['method_not_allowed', 405],
  ['unavailable_needs', 503], ['internal', 500],
  ['resource_not_found', 404], ['read_failed', 500], ['file_access_denied', 403],
  ['file_unavailable', 503], ['git_unavailable', 503],
  ['cursor_invalid', 409], ['cursor_revision_changed', 409], ['page_budget_exceeded', 409],
  ['page_record_too_large', 409], ['document_root_missing', 409],
  ['document_catch_up_failed', 409], ['git_revision_not_found', 409], ['no_projection', 409],
  ['stale', 409], ['unit_unreadable', 409], ['schema_behind', 409], ['half_migrated', 409],
  ['write_failed', 500], ['record_ownership_conflict', 409], ['already_exists', 409],
  ['ownership_conflict', 409], ['already_complete', 409],
  ['schema_version', 400], ['invalid_commit_pin', 400], ['scope_mismatch', 400],
  ['empty_message_body', 400], ['unsupported_thread_parent', 400], ['statement_invalid', 400],
  ['invalid_declaration', 400], ['invalid_update', 400], ['missing_reference', 400],
  ['statement_rejected', 400], ['invalid_verification_target', 400], ['invalid_completion', 400],
]);

function failureKinds(document, schema, seen = new Set(), kinds = new Set()) {
  if (Array.isArray(schema)) {
    for (const child of schema) failureKinds(document, child, seen, kinds);
    return kinds;
  }
  if (schema === null || typeof schema !== 'object') return kinds;
  const reference = schema.$ref;
  if (typeof reference === 'string' && reference.startsWith('#/')) {
    if (seen.has(reference)) return kinds;
    seen.add(reference);
    failureKinds(document, resolvePointer(document, reference), seen, kinds);
    return kinds;
  }
  const kind = schema.properties?.kind?.const;
  if (typeof kind === 'string') kinds.add(kind);
  for (const child of Object.values(schema)) failureKinds(document, child, seen, kinds);
  return kinds;
}

function statusDriftErrors(document, path, method, operation) {
  const declared = new Set(Object.keys(operation.responses ?? {}).map(Number));
  const failure = Object.entries(operation.responses ?? {})
    .find(([status]) => Number(status) >= 400)?.[1]
    ?.content?.['application/json']?.schema;
  const errors = [];
  for (const kind of failureKinds(document, failure)) {
    const status = FAILURE_STATUS.get(kind);
    if (status === undefined) {
      errors.push(`${method} ${path}: failure variant '${kind}' has no runtime status mapping`);
    } else if (!declared.has(status)) {
      errors.push(`${method} ${path}: failure variant '${kind}' can produce undeclared status ${status}`);
    }
  }
  return errors;
}

export function documentGrammarErrors(document, mcp = null) {
  const errors = routeGrammarErrors(document);
  const banned = 'Invoke the shared operation.';
  for (const { path, method, operation } of operationEntries(document)) {
    const where = `${method} ${path}`;
    if (typeof operation.description !== 'string' || operation.description.trim().length < 20 || operation.description.includes(banned)) {
      errors.push(`${where}: tool-description-usefulness requires a specific resource-focused description`);
    }
    const body = resolveSchema(document, operation.requestBody?.content?.['application/json']?.schema);
    const data = resolveSchema(document, body?.properties?.data);
    const fields = Object.keys(data?.properties ?? {});
    for (const identity of ['repository', 'repo', 'scope', 'scope_id', 'collection']) {
      if (fields.includes(identity)) errors.push(`${where}: payload-identity-repetition rejects connection field '${identity}'`);
    }
    for (const identity of pathVariables(path)) {
      const createsImmutableChild = method === 'POST'
        && identity === 'id'
        && /^\/proposals\/\{id\}\/(assertions|dispositions)$/.test(path);
      if (createsImmutableChild) continue;
      if (fields.includes(identity)) errors.push(`${where}: payload-identity-repetition rejects path field '${identity}'`);
    }
    errors.push(...statusDriftErrors(document, path, method, operation));
  }
  for (const tool of mcp?.tools ?? []) {
    if (typeof tool.description !== 'string' || tool.description.trim().length < 20 || tool.description.includes(banned)) {
      errors.push(`MCP tool ${tool.name ?? '(unnamed)'}: tool-description-usefulness requires a specific resource-focused description`);
    }
    errors.push(...envelopeErrors(
      tool.outputSchema,
      tool.outputSchema,
      ['data', 'meta'],
      `MCP tool ${tool.name ?? '(unnamed)'} output`,
    ));
  }
  return errors;
}
