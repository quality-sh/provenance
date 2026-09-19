import * as Effect from 'effect/Effect';
import * as OpenApiGenerator from '@effect/openapi-generator/OpenApiGenerator';
import { validators, responseSchemas } from './validators.mjs';
import { clientTypeSchema } from './typescript-schema.mjs';
import { hoistSharedFamilies, discriminatedUnions } from './contract-families.mjs';
import { renderNested, substitute, declaration, formatLongTypes } from './contract-render.mjs';

function pascal(value) {
  return value.split(/[^a-zA-Z0-9]+/).filter(Boolean)
    .map(part => part[0].toUpperCase() + part.slice(1)).join('');
}

export async function effectFiles(document) {
  // Family hoisting names structurally identical per-operation schemas once.
  // It runs on the raw document so the wire validators below keep the full
  // validation truth, including validation-only cross-field anyOf conditions.
  const contract = hoistSharedFamilies(document);
  // Client types project validation-only conditions out; runtime validation
  // below stays on the unprojected contract.
  const projected = clientTypeSchema(contract);
  const warnings = [];
  const generated = await runGenerator(projected, 'ProvenanceApi', warnings);
  const actionable = warnings.filter(warning => warning.code !== 'response-headers-ignored');
  if (actionable.length) throw new Error(`Effect generator warnings: ${JSON.stringify(actionable)}`);
  const shared = new Set(responseSchemas(contract).filter(name => Object.hasOwn(contract.components.schemas, name)));
  const names = [];
  const source = generated.replace(/^export const (\w+) = (.+)$/gm, (line, name, expression) => {
    if (!Object.hasOwn(contract.components.schemas, name)) return line;
    if (!shared.has(name)) names.push(name);
    const validation = `${shared.has(name) ? 'shared' : 'wire'}.${name}(value)`;
    return `export const ${name} = Schema.declare<${name}>((value): value is ${name} => ${validation}, { identifier: '${name}' })`;
  });

  // The pinned generator inlines every nested component into the envelope
  // types, which duplicated whole failure unions per operation. Render each
  // nested component once with the same generator, swap every inline
  // rendering for the component's name, and declare each used component.
  // Renderings are replaced only by names of byte-identical bodies, so the
  // rewritten types accept exactly what the generator originally emitted.
  const renderWarnings = [];
  const renderings = await Effect.runPromise(Effect.flatMap(OpenApiGenerator.make, generator =>
    Effect.tryPromise(() => renderNested(projected, document => Effect.runPromise(
      generator.generate(document, { name: 'ProvenanceRenderings', format: 'httpapi', onWarning: warning => renderWarnings.push(warning) }))))));
  const actionableRenderWarnings = renderWarnings.filter(warning => warning.code !== 'response-headers-ignored');
  if (actionableRenderWarnings.length) throw new Error(`Effect generator warnings while rendering families: ${JSON.stringify(actionableRenderWarnings)}`);
  // Operation-level schemas are already emitted under their own names; their
  // renderings must not participate, or substitution would collapse each
  // envelope's body into a self-reference.
  const emitted = new Set([...generated.matchAll(/^export const (\w+) =/gm)]
    .map(match => match[1]).filter(name => Object.hasOwn(contract.components.schemas, name)));
  for (const name of emitted) renderings.delete(name);
  const { rewritten } = substitute(source, renderings);
  // Declarations form a closed set: start from the names the envelope source
  // references plus every discriminated union, then add every name those
  // bodies reference, until nothing new appears. Non-canonical duplicates of
  // identical renderings stay undeclared — the canonical name carries them.
  // A union without a rendering (an empty projected body) stays inline.
  const declared = new Set(renderings.keys());
  const bodies = new Map();
  const queue = [...declared];
  while (queue.length > 0) {
    const name = queue.pop();
    const rendering = renderings.get(name);
    if (rendering === undefined) throw new Error(`contract-render: no rendering for declared name ${name}`);
    const body = substitute(rendering, renderings, rendering);
    bodies.set(name, body.rewritten);
    for (const referenced of body.used) {
      if (declared.has(referenced)) continue;
      if (!renderings.has(referenced)) continue;
      declared.add(referenced);
      queue.push(referenced);
    }
  }
  const declarations = [...bodies.entries()].sort(([a], [b]) => a.localeCompare(b))
    .map(([name, body]) => declaration(name, body))
    .join('\n');
  const assembled = formatLongTypes(rewritten + '\n'
    + '// Factored families: one declaration per shared component, bodies byte-equal\n'
    + '// to what the generator inlined before factoring.\n'
    + declarations + unionViews(contract));

  return {
    'effect-client.ts': effectClient(contract),
    'effect-contract.ts': '// Generated from OpenAPI with the official Effect generator. Do not edit.\n'
      + '// Identical per-operation schemas are named once here and composed per\n'
      + '// operation; discriminated union views (variant aliases and exhaustive\n'
      + '// matchers) are derived views of the same document.\n'
      + 'import * as wire from "./effect-validators.mjs";\n'
      + 'import * as shared from "./validators.mjs";\n'
      + assembled,
    ...await validators(contract, names, 'effect-validators', false),
  };
}

async function runGenerator(document, name, warnings) {
  return Effect.runPromise(Effect.flatMap(OpenApiGenerator.make, generator =>
    generator.generate(document, { name, format: 'httpapi', onWarning: warning => warnings.push(warning) })));
}

export function effectClient(document) {
  const routes = Object.values(document.paths).flatMap(route => ['get', 'post', 'patch'].flatMap(method => route[method] ? [route[method]] : []))
    .filter(op => op.operationId !== 'metadata' && op.responses?.['200']?.content?.['application/json'] && op.responses?.['400']?.content?.['application/json']);
  const ref = schema => schema.$ref.split('/').at(-1);
  const queryVariants = op => op['x-provenance-query-variants'] ?? [];
  const variantStem = (op, variant) => `${pascal(op.operationId)}${variant.selector === null ? 'Base' : pascal(variant.selector)}`;
  const queryInputs = new Set();
  const queryContracts = new Set();
  const methods = routes.map(op => {
    const success = ref(op.responses['200'].content['application/json'].schema);
    const failure = ref(op.responses['400'].content['application/json'].schema);
    const variants = queryVariants(op);
    if (variants.length) {
      const overloads = variants.map(variant => {
        const stem = variantStem(op, variant);
        queryInputs.add(`${stem}Input`);
        for (const suffix of ['Success', 'Failure']) queryContracts.add(`${stem}${suffix}`);
        return `  ${op.operationId}(call: ${stem}Input): Effect.Effect<${stem}Success, ClientFailure<${stem}Failure>>;`;
      }).join('\n');
      const inputs = variants.map(variant => `${variantStem(op, variant)}Input`).join(' | ');
      const successes = variants.map(variant => `${variantStem(op, variant)}Success`).join(' | ');
      const failures = variants.map(variant => `${variantStem(op, variant)}Failure`).join(' | ');
      const branches = variants.map(variant => {
        const stem = variantStem(op, variant);
        const condition = variant.selector === null
          ? 'call.query === undefined'
          : `call.query === ${JSON.stringify(variant.selector)}`;
        return `    if (${condition}) return this.runtime.run<${stem}Input, ${stem}Success, ${stem}Failure>('${op.operationId}', false, call, (input, signal) => this.http.${op.operationId}(input, { signal }));`;
      }).join('\n');
      return `${overloads}
  ${op.operationId}(call: ${inputs}): Effect.Effect<${successes}, ClientFailure<${failures}>> {
${branches}
    return Effect.die(new TypeError('Invalid query selector'));
  }`;
    }
    return `  ${op.operationId}(call: Parameters<HttpClient['${op.operationId}']>[0]): Effect.Effect<components['schemas']['${success}'], ClientFailure<components['schemas']['${failure}']>> {
    return this.runtime.run('${op.operationId}', ${op['x-operation-mutates'] === true}, call, (input, signal) => this.http.${op.operationId}(input, { signal }));
  }`;
  });
  const queryTypes = [...queryInputs, ...queryContracts];
  return `// Generated from OpenAPI. Do not edit.
import * as Effect from 'effect/Effect';
import * as Context from 'effect/Context';
import * as Layer from 'effect/Layer';
import { HttpClient, type components${queryTypes.map(name => `, type ${name}`).join('')} } from './client.js';
${queryInputs.size ? `export type { ${[...queryInputs].join(', ')} } from './client.js';` : ''}
import { ClientRuntime, requestEffect, connectionFailure, type ClientFailure } from '../effect-runtime.js';
export interface ClientOptions {
  readonly baseUrl: string;
  readonly bearer?: string;
  readonly fetch?: typeof fetch;
  readonly repository?: string;
  readonly scope?: string;
}
export class EffectHttpClient {
  private readonly runtime = new ClientRuntime();
  private constructor(private readonly http: HttpClient) {}
  static connect(options: ClientOptions): Effect.Effect<EffectHttpClient, ClientFailure> {
    return Effect.map(requestEffect(signal => options.bearer === undefined
      ? HttpClient.connect(options.baseUrl, options.fetch, { signal, repository: options.repository, scope: options.scope })
      : HttpClient.connectWithBearer(options.baseUrl, options.bearer, options.fetch, { signal, repository: options.repository, scope: options.scope }), connectionFailure), http => new EffectHttpClient(http));
  }
${methods.join('\n')}
}
export class ProvenanceClient extends Context.Service<ProvenanceClient, EffectHttpClient>()('@quality-sh/provenance/EffectHttpClient') {
  static layer(options: ClientOptions) { return Layer.effect(ProvenanceClient, EffectHttpClient.connect(options)); }
}
`;
}

/**
 * Emit derived views for every flat discriminated union: per-variant aliases
 * and an exhaustive matcher, named after the union's own (post-hoisting)
 * component name so views stay unique per emitted type. Without the `_`
 * fallback the handler keys must cover every variant; with it, any subset is
 * allowed and the fallback receives the union. Purely type-level and dispatch
 * — the wire keeps its const discriminants and no runtime representation
 * changes.
 */
export function unionViews(contract) {
  const schemas = contract.components.schemas;
  const unions = discriminatedUnions(contract);
  const reserved = new Set(Object.keys(schemas));
  // Alias names default to the brief form `XStale`; when that exact name is
  // already an emitted component — dense catalogs can carry a family member
  // spelled the same way — the alias deterministically falls back to
  // `XVariantStale`. A double collision is a generation error, never a silent
  // shadowing.
  const namesFor = unions.map(union => {
    const aliases = new Map(union.consts.map(value => {
      const preferred = `${union.name}${pascal(value)}`;
      const fallback = `${union.name}Variant${pascal(value)}`;
      return [value, !reserved.has(preferred) ? preferred
        : !reserved.has(fallback) ? fallback
        : null];
    }));
    if ([...aliases.values()].includes(null)) {
      throw new Error(`derived view names for ${union.name} collide with emitted names`);
    }
    return { union, aliases, matcher: `match${union.name}` };
  });
  for (const { union, aliases, matcher } of namesFor) {
    for (const name of [matcher, ...aliases.values()]) {
      if (reserved.has(name)) throw new Error(`derived view name ${name} collides with an emitted name`);
      reserved.add(name);
    }
  }
  const sections = namesFor.map(({ union, aliases, matcher }) => {
    const { name, discriminant, consts } = union;
    const literal = value => `'${value.replaceAll('\\', '\\\\').replaceAll("'", "\\'")}'`;
    const alias = value => aliases.get(value);
    const aliasLines = consts.map(value =>
      `export type ${alias(value)} = Extract<${name}, { ${discriminant}: ${literal(value)} }>`);
    const handlerLines = consts.map(value =>
      `  readonly ${validKey(value)}: (value: ${alias(value)}) => K;`);
    return [
      `/** Variants and exhaustive dispatch for the ${name} union, derived from the OpenAPI document. */`,
      ...aliasLines,
      `export function ${matcher}<K>(value: ${name}, handlers: {`,
      ...handlerLines,
      `}): K;`,
      `export function ${matcher}<K>(value: ${name}, handlers: {`,
      `  readonly [V in ${name}['${discriminant}']]?: (value: Extract<${name}, { ${discriminant}: V }>) => K;`,
      `} & { readonly _: (value: ${name}) => K }): K;`,
      `export function ${matcher}<K>(`,
      `  value: ${name},`,
      `  handlers: { readonly [V in ${name}['${discriminant}']]?: (value: Extract<${name}, { ${discriminant}: V }>) => K } & { readonly _?: (value: ${name}) => K },`,
      `): K {`,
      `  const handler = handlers[value.${discriminant}] as ((value: ${name}) => K) | undefined;`,
      `  if (handler) return handler(value);`,
      `  if (handlers._) return handlers._(value);`,
      `  throw new Error(\`unknown ${discriminant} for ${name}: \${String(value.${discriminant})}\`);`,
      `}`,
    ].join('\n');
  });
  if (sections.length === 0) return '';
  return '\n\n// Derived union views: per-variant aliases and exhaustive matchers.\n'
    + '// These are generated views of the schemas above; the wire keeps its\n'
    + '// discriminant fields and no runtime representation changes.\n'
    + sections.join('\n\n') + '\n';
}

/** Handler keys are the wire consts; quote any that are not plain identifiers. */
function validKey(value) {
  return /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(value) ? value : JSON.stringify(value);
}
