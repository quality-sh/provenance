import { connection, context, type SdkSettings } from './settings.js';
import { portableFile } from './portable-file.js';
import type { DeclarationAddress } from './protocol.js';
import type { VerificationMethod } from './rules.js';

export interface VerifyOptions {
  method?: VerificationMethod;
  file?: string;
  url?: string;
  symbol?: string;
}
export type VerificationTarget =
  | { rule: string }
  | { declaration: { declared_by: string; address: DeclarationAddress } };

export async function runVerification(
  settings: SdkSettings,
  target: VerificationTarget,
  key: string,
  callback: () => unknown | Promise<unknown>,
  options: VerifyOptions,
  file: string,
): Promise<void> {
  const relativeFile = portableFile(file, settings.localRoot);
  const client = await connection(settings);
  const selected = context(settings);
  const coordinates = 'declaration' in target
    ? { declaration: { ...target.declaration, address: [...target.declaration.address] } }
    : target;
  const run = await client.beginVerification({ context: selected, request: {
    ...coordinates, key, method: options.method ?? 'examples',
    declared_by: settings.verificationOwner, file: relativeFile, symbol: options.symbol,
  } });
  try { await callback(); }
  catch (error) {
    try { await client.completeVerification({ context: selected, request: { run: run.id, status: 'failed', error: serializeError(error) } }); }
    catch { /* Preserve the callback as the primary test failure. */ }
    throw error;
  }
  await client.completeVerification({ context: selected, request: { run: run.id, status: 'passed' } });
}

function serializeError(error: unknown): string {
  if (error instanceof Error) return error.stack ?? `${error.name}: ${error.message}`;
  if (typeof error === 'string') return error;
  try { return JSON.stringify(error); }
  catch { return String(error); }
}
