// Source-level factoring of the generated Effect contract.
//
// The pinned Effect generator names only operation-level schemas and inlines
// every nested component. These helpers recover the naming: they render each
// nested component once with the same generator, replace every inline
// rendering in the emitted source with the component's name, and emit one
// declaration per used component. A rendering is only ever replaced by the
// name of a component whose body is byte-identical, so every rewritten type
// accepts and rejects exactly what the generator originally emitted.

/**
 * Split `text` on `separator` occurrences that sit at bracket depth zero,
 * ignoring separators inside strings. Tracks (), [], {}, <>, '…' and "…".
 */
export function topLevelSplit(text, separator) {
  const parts = [];
  let depth = 0, part = '', quote = null;
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (quote !== null) {
      part += char;
      if (char === '\\') { part += text[++i] ?? ''; continue; }
      if (char === quote) quote = null;
      continue;
    }
    if (char === '\'' || char === '"') { quote = char; part += char; continue; }
    if (char === '(' || char === '[' || char === '{' || char === '<') depth++;
    if (char === ')' || char === ']' || char === '}' || char === '>') depth--;
    if (depth === 0 && text.startsWith(separator, i)) {
      parts.push(part);
      part = '';
      i += separator.length - 1;
      continue;
    }
    part += char;
  }
  parts.push(part);
  return parts;
}

/**
 * Render every non-operation component of `projected` exactly as the pinned
 * generator inlines it: one synthetic document whose responses each hold a
 * single `$ref` to one component, rendered by the same generator, then split
 * back into per-component type texts keyed by synthetic property name. Each
 * operation responds with a named wrapper component, because the generator
 * keeps component names but invents names for inline responses. One component
 * per wrapper keeps each emission independent: the generator collapses shared
 * subtrees to `Schema.Json` when one emission revisits them, which real
 * per-operation envelopes never do.
 */
export async function renderNested(projected, generate) {
  const schemas = structuredClone(projected.components.schemas);
  const names = Object.keys(schemas).filter(name => Object.keys(schemas[name]).length > 0).sort();
  const byProperty = new Map(names.map((name, index) => [`p${index}`, name]));
  const paths = {};
  for (const [key, name] of byProperty) {
    const wrapper = `ProvenanceRender${key}`;
    schemas[wrapper] = { type: 'object', properties: { [key]: { $ref: `#/components/schemas/${name}` } }, required: [key] };
    paths[`/render/${key}`] = { post: {
      operationId: `render${key}`,
      responses: { 200: { description: 'Rendering', content: { 'application/json': {
        schema: { $ref: `#/components/schemas/${wrapper}` },
      } } } },
    } };
  }
  const source = await generate({
    ...structuredClone(projected),
    components: { ...structuredClone(projected.components), schemas },
    paths,
  });
  const renderings = new Map();
  for (const line of source.split('\n')) {
    const match = line.match(/^export type ProvenanceRender(p\d+) = \{ readonly "(p\d+)": (.+) \} & \{ readonly \[x: string\]: Schema\.Json \}$/);
    if (!match) continue;
    const name = byProperty.get(match[2]);
    if (name === undefined) throw new Error(`contract-render: unknown rendering property ${match[2]}`);
    if (renderings.has(name)) throw new Error(`contract-render: rendered ${name} twice`);
    renderings.set(name, match[3]);
  }
  const missing = names.filter(name => !renderings.has(name));
  if (missing.length) throw new Error(`contract-render: generator did not render ${missing.slice(0, 5).join(', ')}`);
  return renderings;
}

/**
 * Replace every rendering with its component name, longest text first so a
 * rendering that contains another never shadows it. Only structured
 * renderings participate: objects (`{ … }`) and top-level unions (`… | …`).
 * Bare literals like `9` or `'ASD-STE100'` are never substituted — they carry
 * no factoring value and replacing them would corrupt string literals and
 * numbers elsewhere in the source. Identical renderings map to one shared
 * name (the lexicographically smallest); the other names are never
 * substituted and their declarations are skipped. `exceptRendering` drops one
 * rendering (and every name sharing it) from substitution entirely, so a
 * declaration body is never swallowed by its own — or a sibling's — identical
 * span. Returns the rewritten text, the set of names that actually appear,
 * and the canonical-name map over all rendered components.
 */
export function substitute(text, renderings, exceptRendering = null) {
  const byText = new Map();
  for (const name of [...renderings.keys()].sort()) {
    const rendering = renderings.get(name);
    if (rendering === exceptRendering) continue;
    if (!isStructured(rendering)) continue;
    if (!byText.has(rendering)) byText.set(rendering, name);
  }
  const entries = [...byText.entries()].sort((a, b) => b[0].length - a[0].length);
  if (entries.length === 0) {
    // An empty alternation would match everywhere; nothing to substitute.
    return { rewritten: text, used: new Set(), canonical: new Map() };
  }
  const index = new Map(entries);
  const used = new Set();
  const pattern = new RegExp(entries.map(([rendering]) =>
    rendering.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|'), 'g');
  const rewritten = text.replace(pattern, match => {
    const name = index.get(match);
    used.add(name);
    return name;
  });
  return { rewritten, used, canonical: new Map([...renderings.keys()].map(name =>
    [name, byText.get(renderings.get(name))]).filter(([, canonical]) => canonical !== undefined)) };
}

/** Objects and top-level unions are structured; bare literals are not. */
function isStructured(rendering) {
  return rendering.startsWith('{ ') || topLevelSplit(rendering, ' | ').length > 1;
}

/** True when `text` is a union whose members all sit at top level. */
function isUnion(text) {
  return topLevelSplit(text, ' | ').length > 1;
}

/**
 * One `export type NAME = …` declaration for a rendering, with the union
 * broken one-alternative-per-line when the single line would run long.
 */
export function declaration(name, rendering, limit = 200) {
  if (isUnion(rendering) && rendering.length > limit) {
    const members = topLevelSplit(rendering, ' | ');
    return [`export type ${name} =`, ...members.map(member => `  | ${member}`)].join('\n');
  }
  return `export type ${name} = ${rendering}`;
}

const INDEX_SUFFIX = ' & { readonly [x: string]: Schema.Json }';

/**
 * Reformat `export type` lines whose single-line body would run long: wide
 * objects put one property per line, unions one alternative per line. The
 * emitted text is the same type, only differently laid out. Envelope lines the
 * generator already emitted are reformatted exactly like factored
 * declarations.
 */
export function formatLongTypes(source, limit = 300) {
  return source.split('\n').map(line => {
    const match = line.match(/^(export type \w+ =) (.+)$/);
    if (!match || line.length <= limit) return line;
    const [, head, body] = match;
    let inner = body;
    let suffix = '';
    if (body.endsWith(INDEX_SUFFIX)) {
      inner = body.slice(0, body.length - INDEX_SUFFIX.length);
      suffix = INDEX_SUFFIX;
    }
    if (inner.startsWith('{ ') && inner.endsWith(' }')) {
      const properties = topLevelSplit(inner.slice(2, -2), ', ');
      if (properties.length > 1) {
        return [`${head} {`, ...properties.map(property => `  ${property},`), `}${suffix}`].join('\n');
      }
    }
    if (isUnion(body)) {
      return [`${head}`, ...topLevelSplit(body, ' | ').map(member => `  | ${member}`)].join('\n');
    }
    return line;
  }).join('\n');
}
