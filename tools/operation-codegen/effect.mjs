import * as Effect from 'effect/Effect';
import * as OpenApiGenerator from '@effect/openapi-generator/OpenApiGenerator';
import { validators, responseSchemas } from './validators.mjs';

export async function effectFiles(document) {
  const warnings = [];
  const generated = await Effect.runPromise(Effect.flatMap(OpenApiGenerator.make, generator =>
    generator.generate(document, { name: 'ProvenanceApi', format: 'httpapi', onWarning: warning => warnings.push(warning) })));
  if (warnings.length) throw new Error(`Effect generator warnings: ${JSON.stringify(warnings)}`);
  const shared = new Set(responseSchemas(document).filter(name => Object.hasOwn(document.components.schemas, name)));
  const names = [];
  const source = generated.replace(/^export const (\w+) = (.+)$/gm, (line, name, expression) => {
    if (!Object.hasOwn(document.components.schemas, name)) return line;
    if (!shared.has(name)) names.push(name);
    const validation = `${shared.has(name) ? 'shared' : 'wire'}.${name}(value)`;
    const protocol = name === 'MetadataOutput' ? ` && (value as { protocol_version: unknown }).protocol_version === ${document['x-protocol-version']}` : '';
    return `export const ${name} = Schema.declare<${name}>((value): value is ${name} => ${validation}${protocol}, { identifier: '${name}' })`;
  });
  return {
    'effect-client.ts': effectClient(document),
    'effect-contract.ts': '// Generated from OpenAPI with the official Effect generator. Do not edit.\nimport * as wire from "./effect-validators.mjs";\nimport * as shared from "./validators.mjs";\n' + source,
    ...await validators(document, names, 'effect-validators', false),
  };
}

export function effectClient(document) {
  const routes = Object.values(document.paths).filter(route => route.post).map(route => route.post);
  const ref = schema => schema.$ref.split('/').at(-1);
  const methods = routes.map(op => {
    const input = ref(op.requestBody.content['application/json'].schema);
    const success = ref(op.responses['200'].content['application/json'].schema);
    const failure = ref(op.responses['400'].content['application/json'].schema);
    return `  ${op.operationId}(call: components['schemas']['${input}']): Effect.Effect<components['schemas']['${success}'], ClientFailure<components['schemas']['${failure}']>> {
    return this.runtime.run('${op.operationId}', ${op['x-operation-mutates'] === true}, call, (input, signal) => this.http.${op.operationId}(input, { signal }));
  }`;
  });
  return `// Generated from OpenAPI. Do not edit.
import * as Effect from 'effect/Effect';
import * as Context from 'effect/Context';
import * as Layer from 'effect/Layer';
import { HttpClient, type components } from './client.js';
import { ClientRuntime, requestEffect, connectionFailure, type ClientFailure } from '../effect-runtime.js';
export interface ClientOptions {
  readonly baseUrl: string;
  readonly bearer?: string;
  readonly fetch?: typeof fetch;
}
export class EffectHttpClient {
  private readonly runtime = new ClientRuntime();
  private constructor(private readonly http: HttpClient) {}
  static connect(options: ClientOptions): Effect.Effect<EffectHttpClient, ClientFailure> {
    return Effect.map(requestEffect(signal => options.bearer === undefined
      ? HttpClient.connect(options.baseUrl, options.fetch, { signal })
      : HttpClient.connectWithBearer(options.baseUrl, options.bearer, options.fetch, { signal }), connectionFailure), http => new EffectHttpClient(http));
  }
  unresolvedWrites() { return this.runtime.unresolvedWrites(); }
  resolveWrite(id: number): void { this.runtime.resolveWrite(id); }
${methods.join('\n')}
}
export class ProvenanceClient extends Context.Service<ProvenanceClient, EffectHttpClient>()('@quality-sh/provenance/EffectHttpClient') {
  static layer(options: ClientOptions) { return Layer.effect(ProvenanceClient, EffectHttpClient.connect(options)); }
}
`;
}
