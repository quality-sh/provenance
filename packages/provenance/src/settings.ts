import { HttpClient } from './client.js';

export interface ConfigureOptions {
  endpoint?: string;
  bearer?: string;
  repositoryId?: string;
  localRoot?: string;
  scope?: string;
  owner?: string;
  verificationOwner?: string;
}
export interface SdkSettings extends ConfigureOptions {
  scope: string;
  owner: string;
  verificationOwner: string;
}
const connections = new Map<string, Promise<HttpClient>>();

export function defaults(): SdkSettings {
  return {
    endpoint: process.env.PROVENANCE_ENDPOINT,
    bearer: process.env.PROVENANCE_TOKEN,
    repositoryId: process.env.PROVENANCE_REPOSITORY_ID,
    localRoot: process.env.PROVENANCE_LOCAL_ROOT,
    scope: process.env.PROVENANCE_SCOPE ?? 'default',
    owner: process.env.PROVENANCE_SPEC_OWNER ?? 'spec://typescript',
    verificationOwner: process.env.PROVENANCE_VERIFICATION_OWNER ?? 'ci://typescript',
  };
}

export function context(settings: SdkSettings) {
  if (!settings.endpoint || !settings.repositoryId || !settings.bearer) {
    throw new Error('Configure endpoint, bearer, and repositoryId explicitly; PROVENANCE_REPO is not an HTTP target');
  }
  return { repository: settings.repositoryId, scope: settings.scope };
}

export async function connection(settings: SdkSettings): Promise<HttpClient> {
  context(settings);
  const endpoint = settings.endpoint!;
  const bearer = settings.bearer!;
  const key = JSON.stringify([endpoint, bearer]);
  let pending = connections.get(key);
  if (!pending) {
    pending = HttpClient.connectWithBearer(endpoint, bearer);
    connections.set(key, pending);
  }
  try { return await pending; }
  catch (cause) {
    if (connections.get(key) === pending) connections.delete(key);
    throw cause;
  }
}
