function operations(document) {
  return Object.entries(document.paths).flatMap(([path, item]) =>
    ['get', 'post', 'patch'].flatMap(method => item[method] ? [{ path, method, op: item[method] }] : []));
}
const ref = schema => schema.$ref.split('/').at(-1);
const propertyName = parameter => parameter.in === 'header'
  ? parameter.name.toLowerCase().replaceAll('-', '_') : parameter.name;
const pascal = value => value.split(/[^a-zA-Z0-9]+/).filter(Boolean)
  .map(part => part[0].toUpperCase() + part.slice(1)).join('');
const queryVariants = op => op['x-provenance-query-variants'] ?? [];
const variantStem = (op, variant) => `${pascal(op.operationId)}${variant.selector === null ? 'Base' : pascal(variant.selector)}`;
const schemaName = schema => ref(schema);

function tsType(parameter) {
  const schema = parameter.schema;
  if (typeof schema.const === 'string') return `'${schema.const.replaceAll('\\', '\\\\').replaceAll("'", "\\'")}'`;
  if (Array.isArray(schema.enum) && schema.enum.every(value => typeof value === 'string')) {
    return schema.enum.map(value => `'${value.replaceAll('\\', '\\\\').replaceAll("'", "\\'")}'`).join(' | ');
  }
  if (schema.type === 'integer' || schema.type === 'number') return 'number';
  if (schema.type === 'boolean') return 'boolean';
  if (schema.type === 'array') return 'string[]';
  return 'string';
}

function tsFields(parameters, required = parameter => parameter.required) {
  return parameters.map(parameter =>
    `${JSON.stringify(propertyName(parameter))}${required(parameter) ? '' : '?'}: ${tsType(parameter)}`);
}

function tsCall(op) {
  const fields = tsFields(op.parameters ?? []);
  if (op.requestBody) {
    const request = ref(op.requestBody.content['application/json'].schema);
    fields.push(`data: components['schemas']['${request}']['data']`);
  }
  return `{ ${fields.join('; ')} }`;
}

function queryTypeDeclarations(op) {
  return queryVariants(op).flatMap(variant => {
    const stem = variantStem(op, variant);
    const fields = tsFields(variant.parameters);
    if (variant.selector === null) fields.push('"query"?: undefined');
    return [
      `export type ${stem}Input = { ${fields.join('; ')} };`,
      `export type ${stem}Success = components['schemas']['${schemaName(variant.success)}'];`,
      `export type ${stem}Failure = components['schemas']['${schemaName(variant.failure)}'];`,
    ];
  });
}

function queryImplementationCall(op, variants) {
  const required = parameter => variants.every(variant => variant.parameters.some(candidate =>
    candidate.name === parameter.name
      && candidate.in === parameter.in
      && candidate.required));
  return `{ ${tsFields(op.parameters ?? [], required).join('; ')} }`;
}

function tsRequest(path, method, op) {
  let url = `this.baseUrl + '${path}'`;
  for (const parameter of (op.parameters ?? []).filter(parameter => parameter.in === 'path')) {
    url += `.replace('{${parameter.name}}', encodeURIComponent(String(call[${JSON.stringify(propertyName(parameter))}])))`;
  }
  const query = (op.parameters ?? []).filter(parameter => parameter.in === 'query');
  const queryCode = query.length ? `
    const query = new URLSearchParams();
${query.map(parameter => `    if (call[${JSON.stringify(propertyName(parameter))}] !== undefined) query.set(${JSON.stringify(parameter.name)}, ${parameter.schema.type === 'array' ? `call[${JSON.stringify(propertyName(parameter))}]!.join(',')` : `String(call[${JSON.stringify(propertyName(parameter))}])`});`).join('\n')}
    if (query.size) url.search = query.toString();` : '';
  const headers = (op.parameters ?? []).filter(parameter => parameter.in === 'header');
  const headerCode = [`'content-type': 'application/json'`, ...headers.map(parameter =>
    `${JSON.stringify(parameter.name)}: String(call[${JSON.stringify(propertyName(parameter))}])`)].join(', ');
  const body = op.requestBody ? `, body: JSON.stringify({ data: call.data })` : '';
  return `    const url = new URL(${url});${queryCode}
    const response = await send(this.fetcher, url, {
      method: '${method.toUpperCase()}', headers: { ${headerCode} }${body}, signal: options.signal,
    }, '${op.operationId}', ${op['x-operation-mutates'] === true});`;
}

export function typescriptClient(document, compatibility) {
  const routes = operations(document).filter(({ op }) => op.operationId !== 'metadata');
  const failureTypes = [...new Set(routes.flatMap(({ op }) => queryVariants(op).length
    ? queryVariants(op).map(variant => schemaName(variant.failure))
    : [ref(op.responses['400'].content['application/json'].schema)]))];
  const queryTypes = routes.flatMap(({ op }) => queryTypeDeclarations(op));
  const methods = routes.map(({ path, method, op }) => {
    const success = ref(op.responses['200'].content['application/json'].schema);
    const failure = ref(op.responses['400'].content['application/json'].schema);
    const variants = queryVariants(op);
    if (variants.length) {
      const overloads = variants.map(variant => {
        const stem = variantStem(op, variant);
        return `  ${op.operationId}(call: ${stem}Input, options?: { signal?: AbortSignal }): Promise<${stem}Success>;`;
      }).join('\n');
      const successUnion = variants.map(variant => `${variantStem(op, variant)}Success`).join(' | ');
      const failureUnion = variants.map(variant => `${variantStem(op, variant)}Failure`).join(' | ');
      const contracts = variants.map(variant => {
        const stem = variantStem(op, variant);
        const condition = variant.selector === null
          ? `call.query === undefined`
          : `call.query === ${JSON.stringify(variant.selector)}`;
        return `${condition} ? { success: validate.${schemaName(variant.success)}, failure: validate.${schemaName(variant.failure)} }`;
      }).join(' : ');
      return `${overloads}
  async ${op.operationId}(call: ${queryImplementationCall(op, variants)}, options: { signal?: AbortSignal } = {}): Promise<${successUnion}> {
    const contract = ${contracts} : undefined;
    if (contract === undefined) throw new TypeError('Invalid query selector');
${tsRequest(path, method, op)}
    const value = await readJson(response, '${op.operationId}', false, options.signal);
    if (!response.ok) {
      checked(value, contract.failure, '${op.operationId}', false);
      throw new OperationError<${failureUnion}>(response.status, value as ${failureUnion});
    }
    checked(value, contract.success, '${op.operationId}', false);
    return value as ${successUnion};
  }`;
    }
    return `  async ${op.operationId}(call: ${tsCall(op)}, options: { signal?: AbortSignal } = {}): Promise<components['schemas']['${success}']> {
${tsRequest(path, method, op)}
    const value = await readJson(response, '${op.operationId}', false, options.signal);
    if (!response.ok) {
      checked(value, validate.${failure}, '${op.operationId}', false);
      throw new OperationError(response.status, value as components['schemas']['${failure}']);
    }
    checked(value, validate.${success}, '${op.operationId}', false);
    return value as components['schemas']['${success}'];
  }`;
  }).join('\n');
  return `// Generated from OpenAPI. Do not edit.
import type { components } from './schema.js';
import * as validate from './validators.mjs';
import { send, readJson, checked, ConnectionError, OperationError, ProtocolMismatchError, IdentityMismatchError } from './runtime.js';
export { ConnectionError, MalformedResponseError, OperationError, ProtocolMismatchError, IdentityMismatchError, MAX_RESPONSE_BYTES } from './runtime.js';
export type { components } from './schema.js';
export const COMPATIBILITY = ${JSON.stringify(compatibility)} as const;
export const PROTOCOL_VERSION = COMPATIBILITY.wire;
export type OperationFailure = ${failureTypes.map(name => `components['schemas']['${name}']`).join(' | ')};
${queryTypes.join('\n')}
export class HttpClient {
  private constructor(private readonly baseUrl: string, private readonly fetcher: typeof fetch) {}
  static async connectWithBearer(baseUrl: string, bearer: string, fetcher: typeof fetch = fetch, options: { signal?: AbortSignal; repository?: string; scope?: string } = {}): Promise<HttpClient> {
    const origin = new URL(baseUrl).origin;
    const authenticated: typeof fetch = (input, init) => {
      const target = input instanceof Request ? input.url : input.toString();
      if (new URL(target).origin !== origin) throw new Error('Cross-origin credential request refused');
      const headers = new Headers(init?.headers); headers.set('authorization', 'Bearer ' + bearer);
      return fetcher(input, { ...init, headers, redirect: 'error' });
    };
    return HttpClient.connect(baseUrl, authenticated, options);
  }
  static async connect(baseUrl: string, fetcher: typeof fetch = fetch, options: { signal?: AbortSignal; repository?: string; scope?: string } = {}): Promise<HttpClient> {
    const url = new URL(baseUrl);
    if (!['http:', 'https:'].includes(url.protocol) || url.search || url.hash || url.username || url.password) throw new Error('Invalid HTTP host URL');
    const client = new HttpClient(baseUrl.replace(/\\/$/, ''), fetcher);
    const response = await send(fetcher, client.baseUrl + '/metadata', { redirect: 'error', signal: options.signal }, 'metadata', false);
    if (!response.ok) { await response.body?.cancel(); throw new ConnectionError(); }
    const value = await readJson(response, 'metadata', false, options.signal);
    checked(value, validate.MetadataSuccess, 'metadata', false);
    const metadata = (value as components['schemas']['MetadataSuccess']).data;
    const received = metadata.compatibility;
    if (received.wire !== COMPATIBILITY.wire || received.state !== COMPATIBILITY.state
      || received.review_journal !== COMPATIBILITY.review_journal
      || received.read_derivation !== COMPATIBILITY.read_derivation) {
      throw new ProtocolMismatchError(COMPATIBILITY, received);
    }
    const requestedIdentity = { repository: options.repository, scope: options.scope };
    const authorizedIdentity = { repository: metadata.repository, scope: metadata.scope };
    if ((options.repository !== undefined && options.repository !== metadata.repository)
      || (options.scope !== undefined && options.scope !== metadata.scope)) {
      throw new IdentityMismatchError(requestedIdentity, authorizedIdentity);
    }
    return client;
  }
${methods}
}
`;
}

function rustType(parameter) {
  const type = parameter.schema.type === 'integer' ? 'u64' : parameter.schema.type === 'boolean' ? 'bool' : parameter.schema.type === 'array' ? '&[&str]' : '&str';
  return parameter.required ? type : `Option<${type}>`;
}

function rustVariantType(parameter) {
  const type = parameter.schema.type === 'integer' ? 'u64'
    : parameter.schema.type === 'boolean' ? 'bool'
      : parameter.schema.type === 'array' ? "&'a [&'a str]" : "&'a str";
  return parameter.required ? type : `Option<${type}>`;
}

function rustQueryStatement(parameter) {
  if (typeof parameter.schema.const === 'string') {
    return `request = request.query(&[(${JSON.stringify(parameter.name)}, ${JSON.stringify(parameter.schema.const)})]);`;
  }
  const name = propertyName(parameter);
  const value = parameter.schema.type === 'array' ? 'value.join(",")' : 'value.to_string()';
  return parameter.required
    ? `request = request.query(&[(${JSON.stringify(parameter.name)}, ${parameter.schema.type === 'array' ? `${name}.join(",")` : `${name}.to_string()`})]);`
    : `if let Some(value) = ${name} { request = request.query(&[(${JSON.stringify(parameter.name)}, ${value})]); }`;
}

function rustUrl(path, pathParameters) {
  if (pathParameters.length === 0) return `self.base_url.clone() + ${JSON.stringify(path)}`;
  const parameters = new Map(pathParameters.map(parameter => [parameter.name, parameter]));
  const values = [];
  const template = path.replace(/\{([^}]+)\}/g, (_match, name) => {
    const parameter = parameters.get(name);
    if (parameter === undefined) throw new Error(`Unbound path parameter: ${name}`);
    values.push(`runtime::path(${propertyName(parameter)})`);
    return '{}';
  });
  if (values.length !== pathParameters.length) throw new Error(`Unused path parameter: ${path}`);
  return `format!(${JSON.stringify(`{}${template}`)}, self.base_url, ${values.join(', ')})`;
}

function rustVariantRequest(path, method, op, variant, operation) {
  const parameters = variant.parameters;
  const pathParameters = parameters.filter(parameter => parameter.in === 'path');
  const queryParameters = parameters.filter(parameter => parameter.in === 'query');
  const headerParameters = parameters.filter(parameter => parameter.in === 'header');
  let setup = `let url = ${rustUrl(path, pathParameters)};`;
  const changesRequest = queryParameters.length > 0 || headerParameters.length > 0;
  setup += `\n                let ${changesRequest ? 'mut ' : ''}request = self.http.${method}(url);`;
  for (const parameter of queryParameters) setup += `\n                ${rustQueryStatement(parameter)}`;
  for (const parameter of headerParameters) {
    setup += `\n                request = request.header(${JSON.stringify(parameter.name)}, ${propertyName(parameter)});`;
  }
  return `${setup}
                let response = request.send().await.map_err(|cause| runtime::connection("${operation}", false, cause))?;`;
}

function rustQueryMethod(path, method, op) {
  const variants = queryVariants(op);
  const operation = op.operationId.replace(/[A-Z]/g, character => '_' + character.toLowerCase());
  const operationStem = pascal(op.operationId);
  const input = `${operationStem}Input`;
  const output = `${operationStem}Output`;
  const failure = `${operationStem}Failure`;
  const imports = [...new Set(variants.flatMap(variant => [
    schemaName(variant.success), schemaName(variant.failure),
  ]))].sort();
  const inputVariants = variants.map(variant => {
    const name = variant.selector === null ? 'Base' : pascal(variant.selector);
    const fields = variant.parameters
      .filter(parameter => parameter.schema.const === undefined)
      .map(parameter => `${propertyName(parameter)}: ${rustVariantType(parameter)}`);
    return `    ${name} { ${fields.join(', ')} },`;
  }).join('\n');
  const outputs = variants.map(variant => {
    const name = variant.selector === null ? 'Base' : pascal(variant.selector);
    return `    ${name}(Box<${schemaName(variant.success)}>),`;
  }).join('\n');
  const failures = variants.map(variant => {
    const name = variant.selector === null ? 'Base' : pascal(variant.selector);
    return `    ${name}(${schemaName(variant.failure)}),`;
  }).join('\n');
  const arms = variants.map(variant => {
    const name = variant.selector === null ? 'Base' : pascal(variant.selector);
    const fields = variant.parameters.filter(parameter => parameter.schema.const === undefined)
      .map(parameter => propertyName(parameter));
    const pattern = fields.length ? ` { ${fields.join(', ')} }` : '';
    const helper = `${operation}_${variant.selector === null ? 'base' : variant.selector.replaceAll('-', '_')}`;
    return `            ${input}::${name}${pattern} => self.${helper}(${fields.join(', ')}).await`;
  }).join(',\n');
  const helpers = variants.map(variant => {
    const name = variant.selector === null ? 'Base' : pascal(variant.selector);
    const parameters = variant.parameters.filter(parameter => parameter.schema.const === undefined);
    const helperArguments = parameters.map(parameter => `${propertyName(parameter)}: ${rustType(parameter)}`);
    const helper = `${operation}_${variant.selector === null ? 'base' : variant.selector.replaceAll('-', '_')}`;
    const success = schemaName(variant.success);
    const failed = schemaName(variant.failure);
    return `    async fn ${helper}(&self${helperArguments.length ? `, ${helperArguments.join(', ')}` : ''}) -> Result<${output}, Error> {
        ${rustVariantRequest(path, method, op, variant, operation)}
        let status = response.status();
        let value = runtime::read_json(response, "${operation}", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "${failed}", "${operation}", false)?;
            let failure = runtime::decode(value, "${operation}", false)?;
            return Err(Error::Operation { status: status.as_u16(), failure: OperationFailure::${operationStem}(Box::new(${failure}::${name}(failure))) });
        }
        runtime::validate(&value, "${success}", "${operation}", false)?;
        Ok(${output}::${name}(Box::new(runtime::decode(value, "${operation}", false)?)))
    }`;
  }).join('\n');
  return `// Generated from OpenAPI. Do not edit.
use crate::types::{${imports.join(', ')}};
pub enum ${input}<'a> {
${inputVariants}
}
#[derive(Debug)]
pub enum ${output} {
${outputs}
}
#[derive(Debug, serde::Serialize)]
pub enum ${failure} {
${failures}
}
impl HttpClient {
    pub async fn ${operation}(&self, call: ${input}<'_>) -> Result<${output}, Error> {
        match call {
${arms}
        }
    }
${helpers}
}
`;
}
function rustArgs(op) {
  const args = (op.parameters ?? []).map(parameter => `${propertyName(parameter)}: ${rustType(parameter)}`);
  if (op.requestBody) args.push(`call: &${ref(op.requestBody.content['application/json'].schema)}`);
  return args.join(', ');
}
function rustRequest(path, method, op, operation) {
  const pathParameters = (op.parameters ?? []).filter(parameter => parameter.in === 'path');
  const queryParameters = (op.parameters ?? []).filter(parameter => parameter.in === 'query');
  const headerParameters = (op.parameters ?? []).filter(parameter => parameter.in === 'header');
  let setup = `let url = ${rustUrl(path, pathParameters)};`;
  const changesRequest = queryParameters.length > 0 || headerParameters.length > 0 || op.requestBody;
  setup += `\n        let ${changesRequest ? 'mut ' : ''}request = self.http.${method}(url);`;
  for (const parameter of queryParameters) {
    const name = propertyName(parameter);
    setup += `\n        if let Some(value) = ${name} { request = request.query(&[(${JSON.stringify(parameter.name)}, ${parameter.schema.type === 'array' ? 'value.join(",")' : 'value.to_string()'})]); }`;
  }
  for (const parameter of headerParameters) {
    setup += `\n        request = request.header(${JSON.stringify(parameter.name)}, ${propertyName(parameter)});`;
  }
  if (op.requestBody) setup += `\n        request = request.json(call);`;
  return `${setup}
        let response = request.send().await.map_err(|cause| runtime::connection("${operation}", false, cause))?;`;
}

export function rustClientFiles(document, compatibility) {
  const routes = operations(document).filter(({ op }) => op.operationId !== 'metadata');
  const imports = new Set(['MetadataSuccess']);
  for (const { op } of routes) {
    if (op.requestBody) imports.add(ref(op.requestBody.content['application/json'].schema));
    if (!queryVariants(op).length) {
      imports.add(ref(op.responses['200'].content['application/json'].schema));
      imports.add(ref(op.responses['400'].content['application/json'].schema));
    }
  }
  const methods = Object.fromEntries(routes.map(({ path, method, op }) => {
    if (queryVariants(op).length) {
      const name = op.operationId.replace(/[A-Z]/g, c => '_' + c.toLowerCase());
      return [`operations/${name}.rs`, rustQueryMethod(path, method, op)];
    }
    const success = ref(op.responses['200'].content['application/json'].schema);
    const failure = ref(op.responses['400'].content['application/json'].schema);
    const variant = op.operationId[0].toUpperCase() + op.operationId.slice(1);
    const name = op.operationId.replace(/[A-Z]/g, c => '_' + c.toLowerCase());
    return [`operations/${name}.rs`, `// Generated from OpenAPI. Do not edit.
#[allow(clippy::too_many_arguments)]
impl HttpClient {
    pub async fn ${name}(&self, ${rustArgs(op)}) -> Result<${success}, Error> {
        ${rustRequest(path, method, op, name)}
        let status = response.status();
        let value = runtime::read_json(response, "${name}", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "${failure}", "${name}", false)?;
            let failure: ${failure} = runtime::decode(value, "${name}", false)?;
            return Err(Error::Operation { status: status.as_u16(), failure: OperationFailure::${variant}(Box::new(failure)) });
        }
        runtime::validate(&value, "${success}", "${name}", false)?;
        runtime::decode(value, "${name}", false)
    }
}
`];
  }));
  const failures = routes.map(({ op }) => {
    const variant = op.operationId[0].toUpperCase() + op.operationId.slice(1);
    if (queryVariants(op).length) return `    ${variant}(Box<${variant}Failure>),`;
    return `    ${variant}(Box<${ref(op.responses['400'].content['application/json'].schema)}>),`;
  }).join('\n');
  const c = compatibility;
  const connection = `// Generated from OpenAPI. Do not edit.
use crate::types::{${[...imports].sort().join(', ')}};
use crate::{runtime, Error};
pub const PROTOCOL_VERSION: u32 = ${c.wire};
pub const COMPATIBILITY: (u32, u32, u32, u32) = (${c.wire}, ${c.state}, ${c.review_journal}, ${c.read_derivation});
#[derive(Debug, serde::Serialize)] #[serde(untagged)]
pub enum OperationFailure { ${failures} }
#[derive(Clone)] pub struct HttpClient { base_url: String, http: reqwest::Client }
impl HttpClient {
    pub async fn connect(base_url: &str) -> Result<Self, Error> { Self::connect_with_headers(base_url, reqwest::header::HeaderMap::new(), None).await }
    pub async fn connect_with_identity(base_url: &str, repository: &str, scope: &str) -> Result<Self, Error> {
        Self::connect_with_headers(base_url, reqwest::header::HeaderMap::new(), Some((repository, scope))).await
    }
    pub async fn connect_with_bearer(base_url: &str, bearer: &str) -> Result<Self, Error> {
        Self::connect_with_bearer_and_identity(base_url, bearer, None).await
    }
    pub async fn connect_with_bound_identity(base_url: &str, bearer: &str, repository: &str, scope: &str) -> Result<Self, Error> {
        Self::connect_with_bearer_and_identity(base_url, bearer, Some((repository, scope))).await
    }
    async fn connect_with_bearer_and_identity(base_url: &str, bearer: &str, identity: Option<(&str, &str)>) -> Result<Self, Error> {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {bearer}")).map_err(|_| Error::InvalidCredentials)?;
        value.set_sensitive(true); let mut headers = reqwest::header::HeaderMap::new(); headers.insert(reqwest::header::AUTHORIZATION, value);
        Self::connect_with_headers(base_url, headers, identity).await
    }
    async fn connect_with_headers(base_url: &str, headers: reqwest::header::HeaderMap, identity: Option<(&str, &str)>) -> Result<Self, Error> {
        let url = reqwest::Url::parse(base_url).map_err(|_| Error::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https") || url.query().is_some() || url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() { return Err(Error::InvalidUrl); }
        let client = Self { base_url: base_url.trim_end_matches('/').to_owned(), http: reqwest::Client::builder().default_headers(headers).retry(reqwest::retry::never()).redirect(reqwest::redirect::Policy::none()).build().map_err(|cause| runtime::connection("metadata", false, cause))? };
        let response = client.http.get(format!("{}/metadata", client.base_url)).send().await.map_err(|cause| runtime::connection("metadata", false, cause))?;
        if !response.status().is_success() { return Err(runtime::metadata_status()); }
        let value = runtime::read_json(response, "metadata", false).await?; runtime::validate(&value, "MetadataSuccess", "metadata", false)?;
        let metadata: MetadataSuccess = runtime::decode(value, "metadata", false)?;
        let received = &metadata.data.compatibility;
        if (received.wire, received.state, received.review_journal, received.read_derivation) != COMPATIBILITY { return Err(Error::CompatibilityMismatch); }
        if let Some((repository, scope)) = identity {
            if metadata.data.repository.as_deref() != Some(repository) || metadata.data.scope.as_deref() != Some(scope) { return Err(Error::IdentityMismatch); }
        }
        Ok(client)
    }
}
${Object.keys(methods).map(path => `include!("${path}");`).join('\n')}
`;
  return { 'client.rs': connection, ...methods };
}
