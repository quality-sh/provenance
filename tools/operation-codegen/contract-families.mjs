// Document-level transforms over the exported OpenAPI contract.
//
// Protocol truth stays generator-emitted: every emitted name, alias, and
// matcher derived from this module traces back to the OpenAPI document that
// the Rust catalog exported. The transforms never change what a schema
// accepts; they only name structurally identical per-operation copies once and
// point every reference at the shared name. Equality is checked structurally,
// and any mismatch inside a family leaves the whole family per-operation, so a
// shared name can never lie about the wire.

const ENVELOPE_SUFFIXES = ['RequestInput', 'SuccessOutput', 'FailureOutput'];
const MAX_DEPTH = 64;

function envelopeSuffix(name) {
  return ENVELOPE_SUFFIXES.find(suffix => name.endsWith(suffix)) ?? null;
}

/** Full envelope component names, e.g. `GetFailureOutput`. */
export function envelopeNames(document) {
  return new Set(Object.keys(document.components.schemas)
    .filter(name => envelopeSuffix(name) !== null));
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

/** Stable structural string used only for equality, never emitted. */
function canonical(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
}

/**
 * Normalize a schema subtree for family equality: every `$ref` becomes a
 * reference to the target's family path (or its full name when the target has
 * no family). Siblings of a `$ref` are kept, because `{$ref, properties}` is a
 * conjunction, not a pure reference. Depth-bounded; cycles fail generation.
 */
function normalize(node, familyFor, depth, stack) {
  if (depth > MAX_DEPTH) throw new Error(`contract-families: schema nesting exceeds ${MAX_DEPTH} levels`);
  if (node === null || typeof node !== 'object') return node;
  if (Array.isArray(node)) return node.map(item => normalize(item, familyFor, depth + 1, stack));
  if (typeof node.$ref === 'string') {
    const target = node.$ref.split('/').at(-1);
    if (stack.includes(target)) throw new Error(`contract-families: recursive $ref chain through ${target}`);
    const rest = { ...node };
    delete rest.$ref;
    const normalized = { $family: familyFor(target) ?? `#${target}` };
    for (const key of Object.keys(rest).sort()) normalized[key] = normalize(rest[key], familyFor, depth + 1, stack);
    return normalized;
  }
  const result = {};
  for (const key of Object.keys(node).sort()) result[key] = normalize(node[key], familyFor, depth + 1, stack);
  return result;
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

/**
 * Name identical response-side and request-side family components once.
 *
 * Response envelopes repeat their failure and payload families once per
 * operation, and request envelopes repeat their input families the same way.
 * When every member of a family is structurally identical, one shared
 * component keeps the family name and every reference points at it. Families
 * with any structural difference stay per-operation, preserving
 * per-operation narrowing truth. Request and response shapes never merge with
 * each other: request objects carry `additionalProperties: false` (the wire
 * refuses unknown fields there) while response objects stay open, so the
 * structural comparison itself keeps the two sides apart.
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
  const familyFor = name => familyOf(name, envelopes);
  // Every envelope kind participates: identical input families are named once
  // exactly like identical failure and payload families.
  const familyEnvelope = name => {
    const envelope = envelopeOf(name, envelopes);
    return envelope !== null && ENVELOPE_SUFFIXES.some(suffix => envelope.endsWith(suffix)) ? envelope : null;
  };

  const families = new Map();
  for (const name of Object.keys(schemas)) {
    if (familyEnvelope(name) === null) continue;
    const family = familyOf(name, envelopes);
    if (!families.has(family)) families.set(family, []);
    families.get(family).push(name);
  }

  const renames = new Map();
  const sharedBodies = new Map();
  for (const family of [...families.keys()].sort()) {
    const members = families.get(family).sort();
    if (members.length < 2 || Object.hasOwn(schemas, family)) continue;
    const shape = body => canonical(normalize(body, familyFor, 0, []));
    const target = shape(schemas[members[0]]);
    if (!members.every(member => shape(schemas[member]) === target)) continue;
    for (const member of members) renames.set(member, family);
    sharedBodies.set(family, structuredClone(schemas[members[0]]));
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
      const shape = canonical(normalize(variant, familyFor, 0, []));
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
  // `$family` markers exist only inside equality keys; a leaked marker would
  // silently corrupt every downstream consumer of the transformed document.
  for (const [name, body] of Object.entries(transformed.components.schemas)) {
    if (canonicalContains(body, '$family')) throw new Error(`contract-families: internal $family marker leaked into ${name}`);
  }
  return transformed;
}

function canonicalContains(node, marker) {
  if (node === null || typeof node !== 'object') return false;
  if (Array.isArray(node)) return node.some(item => canonicalContains(item, marker));
  for (const [key, value] of Object.entries(node)) {
    if (key === marker || canonicalContains(value, marker)) return true;
  }
  return false;
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
