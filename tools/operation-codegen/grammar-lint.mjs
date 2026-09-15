// Grammar linter for the frozen v2 operation surface.
//
// The checks implement the grammar rules in docs/api-contract-v2.md. The
// coverage fixture (legacy-operation-coverage.json) declares the frozen route
// table and maps every legacy operation to it exactly once. Generation fails
// on any violation, so the fixture cannot drift from the catalog and the
// declared routes cannot violate the contract grammar.
//
// All functions are pure and return arrays of error strings. An empty array
// means the fixture satisfies the contract.

const ID_GRAMMAR = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;
const METHODS = new Set(['GET', 'POST', 'PATCH']);
const RESPONSE_KINDS = new Set(['resource', 'items', 'result']);

// Edge vocabulary: names that describe a relationship, never a resource.
// A route segment from this list is a relationship route and is rejected.
const EDGE_SEGMENTS = new Set([
  'refines', 'refined-by', 'depends-on', 'depended-on-by', 'supersedes',
  'superseded-by', 'spawned-by', 'contradicts', 'contradicted-by',
  'source-references', 'source-refs', 'edges', 'relations', 'relationships',
]);

// Parent-owned child addresses the contract declares. A collection name in a
// non-root path segment is only legal here.
const CHILD_SEGMENTS = new Set([
  'discussions', 'messages', 'discussion-containers', 'legacy-messages',
  'assertions', 'dispositions', 'history', 'evidence', 'document',
  'submissions',
]);

// Bare verbs that name no resource. They are rejected as path segments.
const BARE_VERBS = new Set([
  'create', 'update', 'delete', 'get', 'list', 'set', 'clear', 'add', 'remove',
  'upsert', 'post', 'read', 'save', 'write', 'check', 'plan', 'apply', 'begin',
  'complete', 'resolve', 'search', 'stale', 'impact', 'trace', 'neighbors',
  'operations', 'query',
]);

// Verbs that start a multi-word segment, for example `/create-source`.
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
  const flat = path.replace(/\[\/?/g, '/').replace(/\]/g, '');
  return flat.split('/').filter(Boolean).filter(segment => !/^\{/.test(segment));
}

function routeIdentity(route) {
  const anonymous = route.path.replace(/\{[a-z0-9_]+\}/g, '{}');
  return `${route.method} ${anonymous}${route.query ? `?query=${route.query}` : ''}`;
}

function generatedName(route) {
  const words = route.path
    .replace(/[{}[\]]/g, ' ')
    .split(/[^a-zA-Z0-9]+/)
    .filter(Boolean)
    .map(word => word[0].toUpperCase() + word.slice(1));
  return (route.method + words.join('') + (route.query ? route.query.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join('') : '')).replace(/^./, c => c.toLowerCase());
}

function routeErrors(route, fixture, legacyNames, seenIds, seenIdentities, seenNames) {
  const errors = [];
  const where = `route ${route.id ?? '(unnamed)'}`;
  if (typeof route.id !== 'string' || !ID_GRAMMAR.test(route.id)) errors.push(`${where}: id is missing or violates kebab-case grammar`);
  if (seenIds.has(route.id)) errors.push(`${where}: duplicate route id`);
  seenIds.add(route.id);
  if (!METHODS.has(route.method)) errors.push(`${where}: method ${route.method} is not GET, POST, or PATCH`);
  if (typeof route.path !== 'string' || !route.path.startsWith('/')) errors.push(`${where}: path is missing or does not start with /`);
  if (route.path && /^\/v\d+\//.test(route.path)) errors.push(`${where}: versioned URL prefix is rejected`);
  if (route.path && route.path.includes('/operations/')) errors.push(`${where}: legacy operations path is rejected`);

  const segments = route.path.split('/').filter(Boolean);
  const frozenWords = new Set([...fixture.collections, ...(fixture.compute_collections ?? []), ...fixture.actions, ...CHILD_SEGMENTS]);
  if (segments.includes('{repository}') || segments.includes('{scope}') || segments.includes('repositories') || segments.includes('scopes')) {
    errors.push(`${where}: repository or scope path prefix is rejected; identity binds at connection`);
  }
  if (segments[0] === 'query') errors.push(`${where}: /query subroutes are rejected; queries are GET parameters on their owning path`);
  if (segments[0] !== undefined && !segments[0].startsWith('{') && segments[0].includes('-') && LEADING_VERBS.has(segments[0].split('-')[0]) && !fixture.actions.includes(segments[0])) {
    errors.push(`${where}: verb-led route '${segments[0]}' is rejected`);
  }

  for (const segment of concreteSegments(route.path)) {
    if (EDGE_SEGMENTS.has(segment)) errors.push(`${where}: relationship route segment '${segment}' is rejected; relationships are PATCH fields`);
    if (BARE_VERBS.has(segment)) errors.push(`${where}: legacy verb segment '${segment}' is rejected`);
    if (legacyNames.has(segment) && !frozenWords.has(segment)) errors.push(`${where}: segment '${segment}' repeats a legacy operation name`);
    if (fixture.collections.includes(segment) && segments.indexOf(segment) > 0 && !CHILD_SEGMENTS.has(segment)) {
      errors.push(`${where}: collection '${segment}' as a subresource is a relationship route and is rejected`);
    }
  }
  for (const segment of concreteSegments(route.path).slice(1)) {
    if (!CHILD_SEGMENTS.has(segment) && !fixture.actions.includes(segment)) {
      errors.push(`${where}: subresource segment '${segment}' is not a declared child address or action`);
    }
  }

  const variables = pathVariables(route.path.replace(/[\[\]]/g, ''));
  for (const variable of variables) {
    if (!fixture.variables.includes(variable)) errors.push(`${where}: path parameter {${variable}} is not declared`);
  }

  if (route.method === 'GET' && route.body) errors.push(`${where}: GET bodies are rejected`);
  if ('request' in route && route.request !== 'typed') {
    errors.push(`${where}: request must be the typed request; required-null or empty request slots are rejected`);
  }
  if (route.response !== undefined && !RESPONSE_KINDS.has(route.response)) {
    errors.push(`${where}: response kind '${route.response}' is rejected; use resource, items, or result under the one envelope`);
  }
  if (route.response === 'array' || route.response === 'raw') errors.push(`${where}: raw array responses are rejected; lists use data.items`);
  if (route.response === 'items' && route.path.includes('{message_id}')) {
    errors.push(`${where}: a single-message read must not ship the items envelope; a path carrying {message_id} reads one message`);
  }
  if ('envelope' in route && route.envelope !== 'standard') errors.push(`${where}: flattened or non-standard envelopes are rejected`);
  if (route.mcp_only || ('mcp_envelope' in route && route.mcp_envelope !== 'standard')) errors.push(`${where}: MCP-only wrappers are rejected; MCP uses the same envelope`);

  if ('action' in route) {
    if (!fixture.actions.includes(route.action)) errors.push(`${where}: action '${route.action}' is not declared`);
    if (route.method !== 'POST') errors.push(`${where}: actions are POSTs`);
    if (route.action && !route.path.endsWith(`/${route.action}`)) errors.push(`${where}: path does not end with its declared action`);
  }
  if (route.query !== undefined) {
    if (!fixture.queries.includes(route.query)) errors.push(`${where}: query '${route.query}' is not declared`);
    if (route.method !== 'GET') errors.push(`${where}: queries are GETs`);
  }

  if (!('mutates' in route)) {
    if (route.method === 'POST') errors.push(`${where}: a POST must declare mutates explicitly (a read POST declares MUTATES=false)`);
  } else if (typeof route.mutates !== 'boolean') {
    errors.push(`${where}: mutates must be a boolean`);
  } else if (route.method === 'GET' && route.mutates) {
    errors.push(`${where}: mutating GETs are rejected`);
  } else if (route.method === 'PATCH' && !route.mutates) {
    errors.push(`${where}: a PATCH is a write and must declare mutates true`);
  }

  const statuses = route.statuses;
  if (!Array.isArray(statuses) || statuses.length === 0) {
    errors.push(`${where}: statuses must be a non-empty array; a status is declared with its failure variant`);
  } else {
    if (!statuses.every(status => Number.isInteger(status) && status >= 100 && status <= 599)) errors.push(`${where}: statuses must be HTTP status integers`);
    if (JSON.stringify(statuses) !== JSON.stringify([...statuses].sort((a, b) => a - b))) errors.push(`${where}: statuses must be sorted and unique`);
    for (const base of fixture.base_statuses) {
      // A connection-scoped metadata read has no resource identity to miss,
      // so the 404 base requirement does not apply to it.
      if (base === 404 && route.path === '/metadata') continue;
      if (!statuses.includes(base)) errors.push(`${where}: base status ${base} is not declared`);
    }
    if (route.mutates === true && !statuses.includes(409)) errors.push(`${where}: a mutating route must declare 409`);
  }

  const firstRaw = segments[0];
  if (firstRaw !== undefined && !firstRaw.startsWith('{') && firstRaw !== 'metadata' && !fixture.collections.includes(firstRaw) && !(fixture.compute_collections ?? []).includes(firstRaw)) {
    errors.push(`${where}: root segment '${firstRaw}' is not a declared collection`);
  }

  const identity = routeIdentity(route);
  if (seenIdentities.has(identity)) errors.push(`${where}: duplicate binding ${identity}`);
  seenIdentities.set(identity, route.id);
  const name = generatedName(route);
  if (seenNames.has(name)) errors.push(`${where}: generated name '${name}' collides with route ${seenNames.get(name)}`);
  seenNames.set(name, route.id);
  return errors;
}

export function lintRoutes(fixture) {
  const errors = [];
  const legacyNames = new Set((fixture.operations ?? []).map(operation => operation.legacy));
  const seenIds = new Set();
  const seenIdentities = new Map();
  const seenNames = new Map();
  for (const route of fixture.routes ?? []) errors.push(...routeErrors(route, fixture, legacyNames, seenIds, seenIdentities, seenNames));
  return errors;
}

export function coverageErrors(fixture, catalogNames = null) {
  const errors = [];
  const operations = fixture.operations;
  if (!Array.isArray(operations)) return ['fixture: operations array is missing'];
  const accounting = fixture.accounting ?? {};
  const expectedTotal = (fixture.catalog_size ?? 0) + (fixture.review_size ?? 0);

  const seen = new Map();
  const bucketCounts = {};
  const sourceCounts = {};
  operations.forEach((operation, index) => {
    const name = operation.legacy;
    if (typeof name !== 'string' || !ID_GRAMMAR.test(name)) {
      errors.push(`operation ${JSON.stringify(name)}: legacy name is missing or violates kebab-case grammar`);
      return;
    }
    if (seen.has(name)) errors.push(`operation ${name}: double-mapped (also mapped at index ${seen.get(name)})`);
    seen.set(name, index);
    const bucket = operation.bucket;
    if (!(bucket in accounting)) errors.push(`operation ${name}: bucket '${bucket}' is not in the accounting`);
    bucketCounts[bucket] = (bucketCounts[bucket] ?? 0) + 1;
    sourceCounts[operation.source] = (sourceCounts[operation.source] ?? 0) + 1;
    const internal = operation.internal === true;
    if (!Array.isArray(operation.routes) || (!internal && operation.routes.length === 0)) {
      errors.push(`operation ${name}: unmapped; declare its routes or mark it internal with a note`);
    }
    if (internal && operation.routes.length > 0) errors.push(`operation ${name}: internal operations declare no routes`);
  });

  if (operations.length !== expectedTotal) errors.push(`fixture: ${operations.length} operations mapped, expected ${expectedTotal}`);
  for (const [source, size] of [['catalog', fixture.catalog_size], ['pr273', fixture.review_size]]) {
    if ((sourceCounts[source] ?? 0) !== size) errors.push(`fixture: ${sourceCounts[source] ?? 0} '${source}' operations, expected ${size}`);
  }
  for (const [bucket, count] of Object.entries(accounting)) {
    if ((bucketCounts[bucket] ?? 0) !== count) errors.push(`fixture: bucket '${bucket}' has ${bucketCounts[bucket] ?? 0} operations, expected ${count}`);
  }

  const routesById = new Map((fixture.routes ?? []).map(route => [route.id, route]));
  const referenced = new Set();
  for (const operation of operations) {
    for (const id of operation.routes ?? []) {
      if (!routesById.has(id)) errors.push(`operation ${operation.legacy}: unknown route '${id}'`);
      referenced.add(id);
    }
  }
  for (const route of fixture.routes ?? []) {
    if (!referenced.has(route.id) && !route.capability) {
      errors.push(`route ${route.id}: no operation maps to it and no capability note explains it`);
    }
  }

  if (Array.isArray(catalogNames)) {
    const mapped = new Set(operations.filter(o => o.source === 'catalog').map(o => o.legacy));
    const live = new Set(catalogNames);
    for (const name of mapped) if (!live.has(name)) errors.push(`fixture: operation '${name}' is not in the live catalog; the fixture is stale`);
    for (const name of live) if (!mapped.has(name)) errors.push(`fixture: catalog operation '${name}' is unmapped; add it to the fixture`);
  }
  return errors;
}

export function lintFixture(fixture, { catalogNames = null } = {}) {
  return [...coverageErrors(fixture, catalogNames), ...lintRoutes(fixture)];
}

function operationEntries(document) {
  return Object.entries(document?.paths ?? {}).flatMap(([path, item]) =>
    ['get', 'post', 'patch'].flatMap(method => item?.[method] ? [{ path, method: method.toUpperCase(), operation: item[method] }] : []));
}

function matchesFixturePath(pattern, actual) {
  const variants = pattern.includes('[')
    ? [pattern.replace(/\[.*\]/, ''), pattern.replaceAll('[', '').replaceAll(']', '')]
    : [pattern];
  return variants.some(variant => {
    const expected = variant.split('/').filter(Boolean);
    const received = actual.split('/').filter(Boolean);
    return expected.length === received.length && expected.every((part, index) =>
      /^\{[a-z0-9_]+\}$/.test(part) ? received[index].length > 0 : part === received[index]);
  });
}

export function surfaceCoverageErrors(fixture, document) {
  const live = operationEntries(document);
  const errors = [];
  for (const entry of live) {
    if (!fixture.routes.some(route => route.method === entry.method && matchesFixturePath(route.path, entry.path))) {
      errors.push(`live route ${entry.method} ${entry.path}: no Phase 1 fixture pattern accounts for it`);
    }
  }
  for (const route of fixture.routes) {
    if (!live.some(entry => entry.method === route.method && matchesFixturePath(route.path, entry.path))) {
      errors.push(`route ${route.id}: no live catalog route implements this fixture pattern`);
    }
  }
  return errors;
}

function resolveSchema(document, schema) {
  const name = schema?.$ref?.match(/^#\/components\/schemas\/(.+)$/)?.[1];
  return name ? document.components?.schemas?.[name] : schema;
}

const FAILURE_STATUS = new Map([
  ['invalid_input', 400], ['protocol_mismatch', 400],
  ['unknown_operation', 404], ['unknown_target', 404], ['unknown_scope', 404],
  ['unauthenticated', 401], ['access_denied', 403], ['unavailable_needs', 503], ['internal', 500],
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
    let resolved = document;
    for (const part of reference.slice(2).split('/')) resolved = resolved?.[part];
    failureKinds(document, resolved, seen, kinds);
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
  const errors = [];
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
  }
  return errors;
}

/// Extracts the legacy catalog operation names from the exported OpenAPI
/// document. The legacy surface is one catch-all shape: `/v{version}/operations/{name}`,
/// plus `/metadata`.
export function catalogNamesFromDocument(document) {
  const names = Object.keys(document?.paths ?? {})
    .filter(path => path.includes('/operations/'))
    .map(path => path.slice(path.indexOf('/operations/') + '/operations/'.length));
  return names.length === 0 ? null : names;
}
