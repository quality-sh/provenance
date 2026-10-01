// Document-level transforms over the exported OpenAPI contract.
//
// Protocol truth stays generator-emitted: every emitted name, alias, and
// matcher derived from this module traces back to the OpenAPI document that
// the Rust catalog exported. The transforms never change what a schema
// accepts. They name structurally identical per-operation copies once and
// point every reference at the shared name. The comparison resolves all
// component references. A different shape stays per-operation.

const MAX_DEPTH = 64;

/** Component names used as operation request or response envelopes. */
export function envelopeNames(document) {
  const names = new Set();
  const add = schema => {
    if (typeof schema?.$ref === 'string') names.add(schema.$ref.split('/').at(-1));
  };
  for (const route of Object.values(document.paths)) {
    for (const operation of Object.values(route)) {
      if (operation === null || typeof operation !== 'object') continue;
      add(operation.requestBody?.content?.['application/json']?.schema);
      for (const response of Object.values(operation.responses ?? {})) {
        add(response.content?.['application/json']?.schema);
      }
      for (const variant of operation['x-provenance-query-variants'] ?? []) {
        add(variant.success);
        add(variant.failure);
      }
    }
  }
  return names;
}

/**
 * The family path of a component: the part after the longest envelope-name
 * prefix, e.g. `GetFailureOutputReadFailure` -> `ReadFailure`. Returns null
 * for envelopes themselves and for standalone names like `MetadataOutput`.
 */
export function familyOf(name, envelopes) {
  return envelopeOf(name, envelopes) === null ? null : name.slice(envelopeOf(name, envelopes).length);
}

/** The longest envelope-name prefix of a component, or null. */
export function envelopeOf(name, envelopes) {
  let best = null;
  for (const envelope of envelopes) {
    if (name.startsWith(envelope) && name.length > envelope.length
      && (best === null || envelope.length > best.length)) best = envelope;
  }
  return best;
}

function schemaShapes(schemas) {
  const cache = new Map();
  const active = new Set();
  const shape = (node, depth = 0) => {
    if (depth > MAX_DEPTH) throw new Error(`contract-families: schema nesting exceeds ${MAX_DEPTH} levels`);
    if (node === null || typeof node !== 'object') return JSON.stringify(node);
    if (Array.isArray(node)) return `[${node.map(item => shape(item, depth + 1)).join(',')}]`;
    const entries = [];
    for (const key of Object.keys(node).sort()) {
      if (key === '$ref' && typeof node[key] === 'string' && node[key].startsWith('#/components/schemas/')) {
        const target = node[key].split('/').at(-1);
        entries.push(`${JSON.stringify('$resolved')}:${named(target)}`);
      } else entries.push(`${JSON.stringify(key)}:${shape(node[key], depth + 1)}`);
    }
    return `{${entries.join(',')}}`;
  };
  const named = name => {
    if (cache.has(name)) return cache.get(name);
    if (active.has(name)) throw new Error(`contract-families: recursive $ref chain through ${name}`);
    if (!Object.hasOwn(schemas, name)) return JSON.stringify({ $ref: name });
    active.add(name);
    const result = shape(schemas[name]);
    active.delete(name);
    cache.set(name, result);
    return result;
  };
  return { node: shape, named };
}

function rewriteRefs(node, rename) {
  if (node === null || typeof node !== 'object') return node;
  if (Array.isArray(node)) return node.map(item => rewriteRefs(item, rename));
  const result = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === '$ref' && typeof value === 'string') {
      const target = value.split('/').at(-1);
      result[key] = rename.has(target) ? `${value.slice(0, value.length - target.length)}${rename.get(target)}` : value;
    } else result[key] = rewriteRefs(value, rename);
  }
  return result;
}

function constDiscriminant(variant) {
  // Inline variants carry the discriminant directly; ref conjunctions carry it
  // in sibling properties. Both forms make a flat discriminated union.
  const properties = variant?.properties;
  if (!properties || typeof properties !== 'object') return null;
  for (const [key, schema] of Object.entries(properties)) {
    if (schema && typeof schema === 'object' && 'const' in schema && typeof schema.const === 'string') return [key, schema.const];
  }
  return null;
}

function pascal(value) {
  return value.split(/[^a-zA-Z0-9]+/).filter(Boolean)
    .map(part => part[0].toUpperCase() + part.slice(1)).join('');
}

function sharedNameFor(family, body, envelopes) {
  if (family !== 'OperationError' || !Array.isArray(body.anyOf)) return null;
  const kinds = body.anyOf.flatMap(part => typeof part.$ref === 'string'
    ? [familyOf(part.$ref.split('/').at(-1), envelopes) ?? part.$ref.split('/').at(-1)] : [])
    .filter(name => name !== 'OperationFailure' && name.endsWith('Failure'));
  return kinds.length === 1 ? `${kinds[0].slice(0, -'Failure'.length)}OperationError` : null;
}

/**
 * Name identical response-side and request-side family components once.
 *
 * Response envelopes repeat their failure and payload families once per
 * operation, and request envelopes repeat their input families the same way.
 * Each structurally identical group gets one shared component and every
 * reference points at it. A family can contain multiple groups. This keeps
 * operation-specific narrowing while it shares all equal shapes. Request and
 * response shapes stay separate when their strictness differs: request
 * objects carry `additionalProperties: false`, while most response objects
 * stay open.
 *
 * Hoisted unions whose variants are inline discriminated objects keep their
 * variants as named shared components (`FailureVariant*`), so no generated
 * type copies a variant shape. Hoisted unions whose variants reference other
 * components keep their variant list untouched.
 */
export function hoistSharedFamilies(document) {
  const transformed = structuredClone(document);
  const schemas = transformed.components.schemas;
  const envelopes = envelopeNames(transformed);
  // Every envelope kind participates: identical input families are named once
  // exactly like identical failure and payload families.
  const familyEnvelope = name => envelopeOf(name, envelopes);

  const families = new Map();
  for (const name of Object.keys(schemas)) {
    if (familyEnvelope(name) === null) continue;
    const family = familyOf(name, envelopes);
    if (!families.has(family)) families.set(family, []);
    families.get(family).push(name);
  }

  const renames = new Map();
  const sharedBodies = new Map();
  const shapes = schemaShapes(schemas);
  for (const family of [...families.keys()].sort()) {
    const members = families.get(family).sort();
    if (members.length < 2) continue;
    const groups = new Map();
    for (const member of members) {
      const shape = shapes.named(member);
      if (!groups.has(shape)) groups.set(shape, []);
      groups.get(shape).push(member);
    }
    const shared = [...groups.values()].filter(group => group.length > 1)
      .sort((a, b) => b.length - a.length || a[0].localeCompare(b[0]));
    let index = 0;
    for (const group of shared) {
      const body = schemas[group[0]];
      const specific = sharedNameFor(family, body, envelopes);
      const candidates = specific === null
        ? index === 0 ? [family, `Shared${family}`] : [`Shared${family}`, `Shared${family}${index + 1}`]
        : [specific, `Shared${specific}`];
      let name = candidates.find(candidate => !Object.hasOwn(schemas, candidate) && !sharedBodies.has(candidate));
      while (name === undefined) {
        index++;
        const candidate = `Shared${specific ?? family}${index + 1}`;
        if (!Object.hasOwn(schemas, candidate) && !sharedBodies.has(candidate)) name = candidate;
      }
      for (const member of group) renames.set(member, name);
      sharedBodies.set(name, structuredClone(body));
      index++;
    }
  }
  if (renames.size === 0) return transformed;

  // Extract inline discriminated variants of hoisted unions into shared
  // variant components. Ref-conjunction unions keep their variant list. The
  // canonical normalized shape is used only for equality; the emitted
  // component body stays the variant's real schema.
  const variantBodies = new Map();
  for (const family of [...sharedBodies.keys()].sort()) {
    const body = sharedBodies.get(family);
    if (!Array.isArray(body.oneOf) || !body.oneOf.every(constDiscriminant)) continue;
    const entries = [];
    const seen = new Set();
    for (const variant of body.oneOf) {
      const [, value] = constDiscriminant(variant);
      const name = `FailureVariant${pascal(value)}`;
      const shape = shapes.node(variant);
      const existing = variantBodies.get(name);
      if (existing && existing.shape !== shape) {
        throw new Error(`contract-families: failure variant ${name} has two different wire shapes`);
      }
      variantBodies.set(name, { shape, body: structuredClone(variant) });
      if (!seen.has(name)) { seen.add(name); entries.push({ $ref: `#/components/schemas/${name}` }); }
    }
    body.oneOf = entries;
  }

  for (const family of [...sharedBodies.keys()].sort()) schemas[family] = sharedBodies.get(family);
  for (const name of [...variantBodies.keys()].sort()) {
    if (Object.hasOwn(schemas, name)) throw new Error(`contract-families: shared variant name ${name} already exists`);
    schemas[name] = variantBodies.get(name).body;
  }
  transformed.paths = rewriteRefs(transformed.paths, renames);
  transformed.components.schemas = rewriteRefs(schemas, renames);
  for (const member of [...renames.keys()].sort()) delete transformed.components.schemas[member];
  return transformed;
}

/**
 * Component names reachable from any operation request body. Request types are
 * emitted without the open-object index signature because the contract refuses
 * unknown fields on requests; response types keep it for forward compatibility.
 */
export function requestSideNames(document) {
  const schemas = document.components.schemas;
  const roots = [];
  for (const route of Object.values(document.paths)) {
    if (!route.post) continue;
    const schema = route.post.requestBody?.content?.['application/json']?.schema;
    if (schema?.$ref) roots.push(schema.$ref.split('/').at(-1));
  }
  const seen = new Set();
  const visit = node => {
    if (node === null || typeof node !== 'object') return;
    if (Array.isArray(node)) { for (const item of node) visit(item); return; }
    if (typeof node.$ref === 'string') {
      const target = node.$ref.split('/').at(-1);
      const rest = { ...node };
      delete rest.$ref;
      visit(rest);
      if (seen.has(target)) return;
      seen.add(target);
      visit(schemas[target]);
      return;
    }
    for (const value of Object.values(node)) visit(value);
  };
  for (const root of [...new Set(roots)].sort()) { seen.add(root); visit(schemas[root]); }
  return seen;
}

/**
 * Flat oneOf unions with a shared const discriminant, e.g. the failure
 * families (`kind`), graph nodes (`node_type`), and document entries (`kind`).
 * Variants may be inline objects, ref conjunctions, or references to variant
 * components; the discriminant is found through all three forms. Each entry
 * names the union component (post-hoisting, so shared families appear once
 * under their shared name), the discriminant key, and its variant consts in
 * stable order.
 */
export function discriminatedUnions(document) {
  const schemas = document.components.schemas;
  const unions = [];
  for (const name of Object.keys(schemas).sort()) {
    const body = schemas[name];
    if (!Array.isArray(body.oneOf) || body.oneOf.length < 2) continue;
    let discriminant = null;
    const consts = new Set();
    let flat = true;
    for (const variant of body.oneOf) {
      if (variant === false) continue; // an empty alternative adds no variant
      const found = resolveDiscriminant(variant, schemas, 0);
      if (!found || (discriminant !== null && found[0] !== discriminant)) { flat = false; break; }
      discriminant = found[0];
      consts.add(found[1]);
    }
    if (!flat || discriminant === null || consts.size < 2) continue;
    unions.push({ name, discriminant, consts: [...consts].sort() });
  }
  return unions;
}

function resolveDiscriminant(variant, schemas, depth) {
  if (depth > MAX_DEPTH) return null;
  const direct = constDiscriminant(variant);
  if (direct) return direct;
  if (typeof variant?.$ref === 'string') {
    const target = schemas[variant.$ref.split('/').at(-1)];
    if (target === undefined) return null;
    const siblings = { ...variant };
    delete siblings.$ref;
    const nested = resolveDiscriminant(target, schemas, depth + 1);
    if (nested && constDiscriminant(siblings) === null) return nested;
    if (!nested) return constDiscriminant(siblings);
    return nested[0] === constDiscriminant(siblings)?.[0] ? nested : null;
  }
  return null;
}
