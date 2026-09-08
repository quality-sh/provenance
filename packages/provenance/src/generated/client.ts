// Generated from OpenAPI. Do not edit.
import type { components } from './schema.js';
import * as validate from './validators.mjs';
import { send, readJson, checked, ConnectionError, OperationError, UncertainWriteError, ProtocolMismatchError } from './runtime.js';
export { ConnectionError, MalformedResponseError, OperationError, UncertainWriteError, ProtocolMismatchError, MAX_RESPONSE_BYTES } from './runtime.js';
export type { components } from './schema.js';
export const PROTOCOL_VERSION = 7;
export type OperationFailure = components['schemas']['AddSourceReferenceFailureOutput'] | components['schemas']['ApplyFailureOutput'] | components['schemas']['BeginVerificationFailureOutput'] | components['schemas']['CheckStatementFailureOutput'] | components['schemas']['CompleteVerificationFailureOutput'] | components['schemas']['CreateRequirementFailureOutput'] | components['schemas']['CreateResolutionFailureOutput'] | components['schemas']['CreateRuleFailureOutput'] | components['schemas']['CreateSourceFailureOutput'] | components['schemas']['EvidenceFailureOutput'] | components['schemas']['GetFailureOutput'] | components['schemas']['ImpactFailureOutput'] | components['schemas']['InfoFailureOutput'] | components['schemas']['NeighborsFailureOutput'] | components['schemas']['PlanFailureOutput'] | components['schemas']['ResolveSymbolFailureOutput'] | components['schemas']['SearchFailureOutput'] | components['schemas']['StaleFailureOutput'] | components['schemas']['TraceFailureOutput'] | components['schemas']['VerificationBindingsFailureOutput'] | components['schemas']['VerificationRunsFailureOutput'];
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
    const client = new HttpClient(baseUrl.replace(/\/$/, ''), fetcher);
    const response = await send(fetcher, client.baseUrl + '/metadata', {}, 'metadata', false);
    if (!response.ok) { await response.body?.cancel(); throw new ConnectionError(); }
    const value = await readJson(response, 'metadata', false);
    checked(value, validate.MetadataOutput, 'metadata', false);
    const metadata = value as { protocol_version: number };
    if (metadata.protocol_version !== PROTOCOL_VERSION) throw new ProtocolMismatchError(PROTOCOL_VERSION, metadata.protocol_version);
    return client;
  }
  async addSourceReference(call: components['schemas']['AddSourceReferenceRequestInput']): Promise<components['schemas']['AddSourceReferenceSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/add-source-reference', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'addSourceReference', true);
    const value = await readJson(response, 'addSourceReference', true);
    if (!response.ok) {
      checked(value, validate.AddSourceReferenceFailureOutput, 'addSourceReference', true);
      const failure = value as components['schemas']['AddSourceReferenceFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('addSourceReference', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.AddSourceReferenceSuccessOutput, 'addSourceReference', true);
    return value as components['schemas']['AddSourceReferenceSuccessOutput'];
  }
  async apply(call: components['schemas']['ApplyRequestInput']): Promise<components['schemas']['ApplySuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/apply', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'apply', true);
    const value = await readJson(response, 'apply', true);
    if (!response.ok) {
      checked(value, validate.ApplyFailureOutput, 'apply', true);
      const failure = value as components['schemas']['ApplyFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('apply', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.ApplySuccessOutput, 'apply', true);
    return value as components['schemas']['ApplySuccessOutput'];
  }
  async beginVerification(call: components['schemas']['BeginVerificationRequestInput']): Promise<components['schemas']['BeginVerificationSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/begin-verification', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'beginVerification', true);
    const value = await readJson(response, 'beginVerification', true);
    if (!response.ok) {
      checked(value, validate.BeginVerificationFailureOutput, 'beginVerification', true);
      const failure = value as components['schemas']['BeginVerificationFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('beginVerification', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.BeginVerificationSuccessOutput, 'beginVerification', true);
    return value as components['schemas']['BeginVerificationSuccessOutput'];
  }
  async checkStatement(call: components['schemas']['CheckStatementRequestInput']): Promise<components['schemas']['CheckStatementSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/check-statement', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'checkStatement', false);
    const value = await readJson(response, 'checkStatement', false);
    if (!response.ok) {
      checked(value, validate.CheckStatementFailureOutput, 'checkStatement', false);
      const failure = value as components['schemas']['CheckStatementFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('checkStatement', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CheckStatementSuccessOutput, 'checkStatement', false);
    return value as components['schemas']['CheckStatementSuccessOutput'];
  }
  async completeVerification(call: components['schemas']['CompleteVerificationRequestInput']): Promise<components['schemas']['CompleteVerificationSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/complete-verification', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'completeVerification', true);
    const value = await readJson(response, 'completeVerification', true);
    if (!response.ok) {
      checked(value, validate.CompleteVerificationFailureOutput, 'completeVerification', true);
      const failure = value as components['schemas']['CompleteVerificationFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('completeVerification', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CompleteVerificationSuccessOutput, 'completeVerification', true);
    return value as components['schemas']['CompleteVerificationSuccessOutput'];
  }
  async createRequirement(call: components['schemas']['CreateRequirementRequestInput']): Promise<components['schemas']['CreateRequirementSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/create-requirement', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'createRequirement', true);
    const value = await readJson(response, 'createRequirement', true);
    if (!response.ok) {
      checked(value, validate.CreateRequirementFailureOutput, 'createRequirement', true);
      const failure = value as components['schemas']['CreateRequirementFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('createRequirement', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CreateRequirementSuccessOutput, 'createRequirement', true);
    return value as components['schemas']['CreateRequirementSuccessOutput'];
  }
  async createResolution(call: components['schemas']['CreateResolutionRequestInput']): Promise<components['schemas']['CreateResolutionSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/create-resolution', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'createResolution', true);
    const value = await readJson(response, 'createResolution', true);
    if (!response.ok) {
      checked(value, validate.CreateResolutionFailureOutput, 'createResolution', true);
      const failure = value as components['schemas']['CreateResolutionFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('createResolution', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CreateResolutionSuccessOutput, 'createResolution', true);
    return value as components['schemas']['CreateResolutionSuccessOutput'];
  }
  async createRule(call: components['schemas']['CreateRuleRequestInput']): Promise<components['schemas']['CreateRuleSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/create-rule', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'createRule', true);
    const value = await readJson(response, 'createRule', true);
    if (!response.ok) {
      checked(value, validate.CreateRuleFailureOutput, 'createRule', true);
      const failure = value as components['schemas']['CreateRuleFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('createRule', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CreateRuleSuccessOutput, 'createRule', true);
    return value as components['schemas']['CreateRuleSuccessOutput'];
  }
  async createSource(call: components['schemas']['CreateSourceRequestInput']): Promise<components['schemas']['CreateSourceSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/create-source', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'createSource', true);
    const value = await readJson(response, 'createSource', true);
    if (!response.ok) {
      checked(value, validate.CreateSourceFailureOutput, 'createSource', true);
      const failure = value as components['schemas']['CreateSourceFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (true && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('createSource', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.CreateSourceSuccessOutput, 'createSource', true);
    return value as components['schemas']['CreateSourceSuccessOutput'];
  }
  async evidence(call: components['schemas']['EvidenceRequestInput']): Promise<components['schemas']['EvidenceSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/evidence', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'evidence', false);
    const value = await readJson(response, 'evidence', false);
    if (!response.ok) {
      checked(value, validate.EvidenceFailureOutput, 'evidence', false);
      const failure = value as components['schemas']['EvidenceFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('evidence', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.EvidenceSuccessOutput, 'evidence', false);
    return value as components['schemas']['EvidenceSuccessOutput'];
  }
  async get(call: components['schemas']['GetRequestInput']): Promise<components['schemas']['GetSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/get', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'get', false);
    const value = await readJson(response, 'get', false);
    if (!response.ok) {
      checked(value, validate.GetFailureOutput, 'get', false);
      const failure = value as components['schemas']['GetFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('get', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.GetSuccessOutput, 'get', false);
    return value as components['schemas']['GetSuccessOutput'];
  }
  async impact(call: components['schemas']['ImpactRequestInput']): Promise<components['schemas']['ImpactSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/impact', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'impact', false);
    const value = await readJson(response, 'impact', false);
    if (!response.ok) {
      checked(value, validate.ImpactFailureOutput, 'impact', false);
      const failure = value as components['schemas']['ImpactFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('impact', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.ImpactSuccessOutput, 'impact', false);
    return value as components['schemas']['ImpactSuccessOutput'];
  }
  async info(call: components['schemas']['InfoRequestInput']): Promise<components['schemas']['InfoSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/info', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'info', false);
    const value = await readJson(response, 'info', false);
    if (!response.ok) {
      checked(value, validate.InfoFailureOutput, 'info', false);
      const failure = value as components['schemas']['InfoFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('info', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.InfoSuccessOutput, 'info', false);
    return value as components['schemas']['InfoSuccessOutput'];
  }
  async neighbors(call: components['schemas']['NeighborsRequestInput']): Promise<components['schemas']['NeighborsSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/neighbors', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'neighbors', false);
    const value = await readJson(response, 'neighbors', false);
    if (!response.ok) {
      checked(value, validate.NeighborsFailureOutput, 'neighbors', false);
      const failure = value as components['schemas']['NeighborsFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('neighbors', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.NeighborsSuccessOutput, 'neighbors', false);
    return value as components['schemas']['NeighborsSuccessOutput'];
  }
  async plan(call: components['schemas']['PlanRequestInput']): Promise<components['schemas']['PlanSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/plan', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'plan', false);
    const value = await readJson(response, 'plan', false);
    if (!response.ok) {
      checked(value, validate.PlanFailureOutput, 'plan', false);
      const failure = value as components['schemas']['PlanFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('plan', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.PlanSuccessOutput, 'plan', false);
    return value as components['schemas']['PlanSuccessOutput'];
  }
  async resolveSymbol(call: components['schemas']['ResolveSymbolRequestInput']): Promise<components['schemas']['ResolveSymbolSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/resolve-symbol', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'resolveSymbol', false);
    const value = await readJson(response, 'resolveSymbol', false);
    if (!response.ok) {
      checked(value, validate.ResolveSymbolFailureOutput, 'resolveSymbol', false);
      const failure = value as components['schemas']['ResolveSymbolFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('resolveSymbol', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.ResolveSymbolSuccessOutput, 'resolveSymbol', false);
    return value as components['schemas']['ResolveSymbolSuccessOutput'];
  }
  async search(call: components['schemas']['SearchRequestInput']): Promise<components['schemas']['SearchSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/search', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'search', false);
    const value = await readJson(response, 'search', false);
    if (!response.ok) {
      checked(value, validate.SearchFailureOutput, 'search', false);
      const failure = value as components['schemas']['SearchFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('search', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.SearchSuccessOutput, 'search', false);
    return value as components['schemas']['SearchSuccessOutput'];
  }
  async stale(call: components['schemas']['StaleRequestInput']): Promise<components['schemas']['StaleSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/stale', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'stale', false);
    const value = await readJson(response, 'stale', false);
    if (!response.ok) {
      checked(value, validate.StaleFailureOutput, 'stale', false);
      const failure = value as components['schemas']['StaleFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('stale', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.StaleSuccessOutput, 'stale', false);
    return value as components['schemas']['StaleSuccessOutput'];
  }
  async trace(call: components['schemas']['TraceRequestInput']): Promise<components['schemas']['TraceSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/trace', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'trace', false);
    const value = await readJson(response, 'trace', false);
    if (!response.ok) {
      checked(value, validate.TraceFailureOutput, 'trace', false);
      const failure = value as components['schemas']['TraceFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('trace', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.TraceSuccessOutput, 'trace', false);
    return value as components['schemas']['TraceSuccessOutput'];
  }
  async verificationBindings(call: components['schemas']['VerificationBindingsRequestInput']): Promise<components['schemas']['VerificationBindingsSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/verification-bindings', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'verificationBindings', false);
    const value = await readJson(response, 'verificationBindings', false);
    if (!response.ok) {
      checked(value, validate.VerificationBindingsFailureOutput, 'verificationBindings', false);
      const failure = value as components['schemas']['VerificationBindingsFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('verificationBindings', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.VerificationBindingsSuccessOutput, 'verificationBindings', false);
    return value as components['schemas']['VerificationBindingsSuccessOutput'];
  }
  async verificationRuns(call: components['schemas']['VerificationRunsRequestInput']): Promise<components['schemas']['VerificationRunsSuccessOutput']> {
    const body = JSON.stringify(call);
    const response = await send(this.fetcher, this.baseUrl + '/v7/operations/verification-runs', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body,
    }, 'verificationRuns', false);
    const value = await readJson(response, 'verificationRuns', false);
    if (!response.ok) {
      checked(value, validate.VerificationRunsFailureOutput, 'verificationRuns', false);
      const failure = value as components['schemas']['VerificationRunsFailureOutput'];
      if (failure.error.kind === 'uncertain_write' || (false && ['internal', 'write_failed'].includes(failure.error.kind))) throw new UncertainWriteError('verificationRuns', undefined, failure);
      throw new OperationError(response.status, failure);
    }
    checked(value, validate.VerificationRunsSuccessOutput, 'verificationRuns', false);
    return value as components['schemas']['VerificationRunsSuccessOutput'];
  }
}
