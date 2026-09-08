export function typescriptClient(document) {
  const version = document['x-protocol-version'];
  const methods = Object.entries(document.paths).filter(([, route]) => route.post).map(([path, route]) => {
    const op = route.post;
    const request = op.requestBody.content['application/json'].schema.$ref.split('/').at(-1);
    const success = op.responses['200'].content['application/json'].schema.$ref.split('/').at(-1);
    const failure = op.responses['400'].content['application/json'].schema.$ref.split('/').at(-1);
    return `  async ${op.operationId}(call: components['schemas']['${request}']): Promise<components['schemas']['${success}']> {
    const response = await this.fetcher(this.baseUrl + '${path}', {
      method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json' }, body: JSON.stringify(call),
    });
    if (!response.ok) throw new OperationError(response.status, await response.json() as components['schemas']['${failure}']);
    return await response.json() as components['schemas']['${success}'];
  }`;
  }).join('\n');
  const failureTypes = [...new Set(Object.values(document.paths).filter(route => route.post).map(route => route.post.responses['400'].content['application/json'].schema.$ref.split('/').at(-1)))];
  return `// Generated from OpenAPI. Do not edit.
import type { components } from './schema.js';
export type { components } from './schema.js';
export const PROTOCOL_VERSION = ${version};
export class ProtocolMismatchError extends Error {
  constructor(readonly requested: number, readonly supported: number) { super('Incompatible operation protocol'); }
}
export type OperationFailure = ${failureTypes.map(name => `components['schemas']['${name}']`).join(' | ')};
export class OperationError extends Error {
  constructor(readonly status: number, readonly failure: OperationFailure) { super('Operation refused'); }
}
export class HttpClient {
  private constructor(private readonly baseUrl: string, private readonly fetcher: typeof fetch) {}
  static async connectWithBearer(baseUrl: string, bearer: string, fetcher: typeof fetch = fetch): Promise<HttpClient> {
    const authenticated: typeof fetch = (input, init) => {
      const headers = new Headers(init?.headers);
      headers.set('authorization', 'Bearer ' + bearer);
      return fetcher(input, { ...init, headers });
    };
    return HttpClient.connect(baseUrl, authenticated);
  }
  static async connect(baseUrl: string, fetcher: typeof fetch = fetch): Promise<HttpClient> {
    const url = new URL(baseUrl);
    if (!['http:', 'https:'].includes(url.protocol) || url.search || url.hash || url.username || url.password) throw new Error('Invalid HTTP host URL');
    const client = new HttpClient(baseUrl.replace(/\\/$/, ''), fetcher);
    const response = await fetcher(client.baseUrl + '/metadata', { redirect: 'error' });
    if (!response.ok) throw new Error('Cannot read engine metadata: ' + response.status);
    const metadata = await response.json() as { engine_version: string; protocol_version: number };
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
    return [`operations/${method}.rs`, `// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn ${method}(&self, call: &${request}) -> Result<${success}, Error> {
        let response = self.http.post(format!("{}${path}", self.base_url))
            .json(call).send().await.map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: ${failure} = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation { status: status.as_u16(), failure: OperationFailure::${variant}(Box::new(failure)) });
        }
        response.json().await.map_err(Error::Transport)
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
pub const PROTOCOL_VERSION: u32 = ${version};
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum OperationFailure {
${failureVariants}
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid HTTP host URL")]
    InvalidUrl,
    #[error("invalid bearer credential format")]
    InvalidCredentials,
    #[error("HTTP transport failed: {0}")]
    Transport(reqwest::Error),
    #[error("metadata request failed with status {0}")]
    MetadataStatus(u16),
    #[error("incompatible operation protocol: expected {expected}, received {received}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("operation refused with status {status}")]
    Operation { status: u16, failure: OperationFailure },
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
            http: reqwest::Client::builder().default_headers(headers).retry(reqwest::retry::never()).redirect(reqwest::redirect::Policy::none()).build().map_err(Error::Transport)? };
        let response = client.http.get(format!("{}/metadata", client.base_url)).send().await.map_err(Error::Transport)?;
        if !response.status().is_success() { return Err(Error::MetadataStatus(response.status().as_u16())); }
        let metadata: Metadata = response.json().await.map_err(Error::Transport)?;
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
