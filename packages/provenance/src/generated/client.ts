// Generated from OpenAPI. Do not edit.
import type { components } from './schema.js';
export type { components } from './schema.js';
export const PROTOCOL_VERSION = 7;
export class ProtocolMismatchError extends Error {
  constructor(readonly requested: number, readonly supported: number) { super('Incompatible operation protocol'); }
}
export class OperationError extends Error {
  constructor(readonly status: number, readonly failure: components['schemas']['CheckStatementFailureOutput']) { super('Operation refused'); }
}
export class HttpClient {
  private constructor(private readonly baseUrl: string, private readonly fetcher: typeof fetch) {}
  static async connect(baseUrl: string, fetcher: typeof fetch = fetch): Promise<HttpClient> {
    const url = new URL(baseUrl);
    if (!['http:', 'https:'].includes(url.protocol) || url.search || url.hash || url.username || url.password) throw new Error('Invalid HTTP host URL');
    const client = new HttpClient(baseUrl.replace(/\/$/, ''), fetcher);
    const response = await fetcher(client.baseUrl + '/metadata', { redirect: 'error' });
    if (!response.ok) throw new Error('Cannot read engine metadata: ' + response.status);
    const metadata = await response.json() as { engine_version: string; protocol_version: number };
    if (metadata.protocol_version !== PROTOCOL_VERSION) throw new ProtocolMismatchError(PROTOCOL_VERSION, metadata.protocol_version);
    return client;
  }
  async checkStatement(call: components['schemas']['CheckStatementRequestInput']): Promise<components['schemas']['CheckStatementSuccessOutput']> {
    const response = await this.fetcher(this.baseUrl + '/v7/operations/check-statement', {
      method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json' }, body: JSON.stringify(call),
    });
    if (!response.ok) throw new OperationError(response.status, await response.json() as components['schemas']['CheckStatementFailureOutput']);
    return await response.json() as components['schemas']['CheckStatementSuccessOutput'];
  }
}
