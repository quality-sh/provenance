import { connection, type SdkSettings } from './settings.js';
import { portableFile } from './portable-file.js';
import type { DeclarationAddress } from './protocol.js';
import type { components } from './generated/schema.js';
import type { VerificationMethod } from './rules.js';

export interface VerifyOptions {
  method?: VerificationMethod;
  file?: string;
  url?: string;
  symbol?: string;
}
type VerificationRequest = components['schemas']['BeginVerificationRequest']['data'];
type DeclarationReference = NonNullable<VerificationRequest['declaration']>;
export type VerificationTarget =
  | { rule: NonNullable<VerificationRequest['rule']> }
  | { declaration: Omit<DeclarationReference, 'address'> & { address: DeclarationAddress } };

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
  const coordinates = 'declaration' in target
    ? { declaration: { ...target.declaration, address: [...target.declaration.address] } }
    : target;
  const run = await client.beginVerification({ data: {
    ...coordinates, key, method: options.method ?? 'examples',
    declared_by: settings.verificationOwner, file: relativeFile, symbol: options.symbol,
  } });
  try { await callback(); }
  catch (error) {
    try { await client.completeVerification({ run_id: run.data.id, data: { status: 'failed', error: serializeError(error) } }); }
    catch { /* Preserve the callback as the primary test failure. */ }
    throw error;
  }
  await client.completeVerification({ run_id: run.data.id, data: { status: 'passed' } });
}

function serializeError(error: unknown): string {
  if (error instanceof Error) return error.stack ?? `${error.name}: ${error.message}`;
  if (typeof error === 'string') return error;
  try { return JSON.stringify(error); }
  catch { return String(error); }
}
