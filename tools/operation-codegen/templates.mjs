export function typescriptClient(document) {
  const version = document['x-protocol-version'];
  const routes = Object.entries(document.paths).filter(([, route]) => route.post);
  const failureTypes = [...new Set(routes.map(([, route]) => route.post.responses['400'].content['application/json'].schema.$ref.split('/').at(-1)))];
  const methods = routes.map(([path, route]) => {
    const op = route.post;
    const request = op.requestBody.content['application/json'].schema.$ref.split('/').at(-1);
    const success = op.responses['200'].content['application/json'].schema.$ref.split('/').at(-1);
    const failure = op.responses['400'].content['application/json'].schema.$ref.split('/').at(-1);
    const mutates = op['x-operation-mutates'] === true;
    return `  async ${op.operationId}(call: components['schemas']['${request}']): Promise<components['schemas']['${success}']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '${path}', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, '${op.operationId}', ${mutates});
    const value = await readJson(response, '${op.operationId}', ${mutates});
    if (!response.ok) {
      checked(value, validate.${failure}, '${op.operationId}', ${mutates});
      const failure = value as components['schemas']['${failure}'];
      if (failure.error.kind === 'uncertain_write' || (${mutates} && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('${op.operationId}', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.${success}, '${op.operationId}', ${mutates});
    return value as components['schemas']['${success}'];
  }`;
  }).join('\n');
  return `// Generated from OpenAPI. Do not edit.
import type { components } from './schema.js';
import * as validate from './validators.mjs';
import { send, readJson, checked, ConnectionError, OperationError, UncertainWriteError, ProtocolMismatchError } from './runtime.js';
export { ConnectionError, MalformedResponseError, OperationError, UncertainWriteError, ProtocolMismatchError, MAX_RESPONSE_BYTES } from './runtime.js';
export type { components } from './schema.js';
export const PROTOCOL_VERSION = ${version};
export type OperationFailure = ${failureTypes.map(name => `components['schemas']['${name}']`).join(' | ')};
export class HttpClient {
  private constructor(private readonly baseUrl: string, private readonly fetcher: typeof fetch) {}
  static async connectWithBearer(baseUrl: string, bearer: string, fetcher: typeof fetch = fetch): Promise<HttpClient> {
    const origin = new URL(baseUrl).origin;
    const authenticated: typeof fetch = (input, init) => {
      const target = input instanceof Request ? input.url : input.toString();
      if (new URL(target).origin !== origin) throw new Error('Cross-origin credential request refused');
      const headers = new Headers(init?.headers);
      headers.set('authorization', 'Bearer ' + bearer);
      return fetcher(input, { ...init, headers, redirect: 'error' });
    };
    return HttpClient.connect(baseUrl, authenticated);
  }
  static async connect(baseUrl: string, fetcher: typeof fetch = fetch): Promise<HttpClient> {
    const url = new URL(baseUrl);
    if (!['http:', 'https:'].includes(url.protocol) || url.search || url.hash || url.username || url.password) throw new Error('Invalid HTTP host URL');
    const client = new HttpClient(baseUrl.replace(/\\/$/, ''), fetcher);
    const response = await send(fetcher, client.baseUrl + '/metadata', {}, 'metadata', false);
    if (!response.ok) { await response.body?.cancel(); throw new ConnectionError(); }
    const value = await readJson(response, 'metadata', false);
    checked(value, validate.MetadataOutput, 'metadata', false);
    const metadata = value as { protocol_version: number };
    if (metadata.protocol_version !== PROTOCOL_VERSION) throw new ProtocolMismatchError(PROTOCOL_VERSION, metadata.protocol_version);
    return client;
  }
${methods}
}
`;
}

export function rustClientFiles(document) {
  const imports = new Set();
  for (const route of Object.values(document.paths)) {
    if (!route.post) continue;
    imports.add(route.post.requestBody.content['application/json'].schema.$ref.split('/').at(-1));
    for (const status of ['200', '400']) imports.add(route.post.responses[status].content['application/json'].schema.$ref.split('/').at(-1));
  }
  const version = document['x-protocol-version'];
  const methods = Object.fromEntries(Object.entries(document.paths).filter(([,route])=>route.post).map(([path, route]) => {
    const op = route.post;
    const request = op.requestBody.content['application/json'].schema.$ref.split('/').at(-1);
    const success = op.responses['200'].content['application/json'].schema.$ref.split('/').at(-1);
    const failure = op.responses['400'].content['application/json'].schema.$ref.split('/').at(-1);
    const variant = op.operationId[0].toUpperCase() + op.operationId.slice(1);
    const method = op.operationId.replace(/[A-Z]/g, c => '_' + c.toLowerCase());
    const mutates = op['x-operation-mutates'] === true;
    return [`operations/${method}.rs`, `// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn ${method}(&self, call: &${request}) -> Result<${success}, Error> {
        let response = self.http.post(format!("{}${path}", self.base_url))
            .json(call).send().await.map_err(|cause| runtime::connection("${method}", ${mutates}, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "${method}", ${mutates}).await?;
        if !status.is_success() {
            runtime::validate(&value, "${failure}", "${method}", ${mutates})?;
            let uncertain = runtime::uncertain_kind(&value, ${mutates});
            let failure: ${failure} = runtime::decode(value, "${method}", ${mutates})?;
            let failure = OperationFailure::${variant}(Box::new(failure));
            if uncertain { return Err(runtime::uncertain("${method}", failure)); }
            return Err(Error::Operation { status: status.as_u16(), failure });
        }
        runtime::validate(&value, "${success}", "${method}", ${mutates})?;
        runtime::decode(value, "${method}", ${mutates})
    }
}
`];
  }));
  const failureVariants = Object.values(document.paths).filter(route => route.post).map(route => {
    const op = route.post;
    const name = op.operationId[0].toUpperCase() + op.operationId.slice(1);
    const type = op.responses['400'].content['application/json'].schema.$ref.split('/').at(-1);
    return `    ${name}(Box<${type}>),`;
  }).join('\n');
  const connection = `// Generated from OpenAPI. Do not edit.
use crate::types::{${[...imports].sort().join(', ')}};
use crate::{runtime, Error};
pub const PROTOCOL_VERSION: u32 = ${version};
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum OperationFailure {
${failureVariants}
}
#[derive(serde::Deserialize)]
struct Metadata { protocol_version: u32 }
#[derive(Clone)]
pub struct HttpClient { base_url: String, http: reqwest::Client }
impl HttpClient {
    pub async fn connect(base_url: &str) -> Result<Self, Error> {
        Self::connect_with_headers(base_url, reqwest::header::HeaderMap::new()).await
    }
    pub async fn connect_with_bearer(base_url: &str, bearer: &str) -> Result<Self, Error> {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {bearer}"))
            .map_err(|_| Error::InvalidCredentials)?;
        value.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, value);
        Self::connect_with_headers(base_url, headers).await
    }
    async fn connect_with_headers(base_url: &str, headers: reqwest::header::HeaderMap) -> Result<Self, Error> {
        let url = reqwest::Url::parse(base_url).map_err(|_| Error::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https") || url.query().is_some() || url.fragment().is_some()
            || !url.username().is_empty() || url.password().is_some() { return Err(Error::InvalidUrl); }
        let client = Self { base_url: base_url.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder().default_headers(headers).retry(reqwest::retry::never()).redirect(reqwest::redirect::Policy::none()).build().map_err(|cause| runtime::connection("metadata", false, cause))? };
        let response = client.http.get(format!("{}/metadata", client.base_url)).send().await.map_err(|cause| runtime::connection("metadata", false, cause))?;
        if !response.status().is_success() { return Err(runtime::metadata_status()); }
        let value = runtime::read_json(response, "metadata", false).await?;
        runtime::validate(&value, "MetadataOutput", "metadata", false)?;
        let metadata: Metadata = runtime::decode(value, "metadata", false)?;
        if metadata.protocol_version != PROTOCOL_VERSION {
            return Err(Error::ProtocolMismatch { expected: PROTOCOL_VERSION, received: metadata.protocol_version });
        }
        Ok(client)
    }
}
${Object.keys(methods).map(path => `include!("${path}");`).join('\n')}
`;
  return { 'client.rs': connection, ...methods };
}
