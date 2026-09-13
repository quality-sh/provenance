import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generationIsCurrent } from './artifacts.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export function runGenerator() {
  const result = spawnSync(process.execPath, [resolve(root, 'tools/operation-codegen/generate.mjs')], { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error('Operation generation failed. Install the pinned Rust toolchain and run npm ci --prefix tools/operation-codegen, then retry.');
}
export async function ensureGenerated() {
  if (!await generationIsCurrent(root)) runGenerator();
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await ensureGenerated();
