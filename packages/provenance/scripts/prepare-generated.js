import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const [script, args] of [
  ['../../../tools/operation-codegen/ensure-generated.mjs', []],
  ['./generate-release-consumers.js', ['--runtime-only']],
]) {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL(script, import.meta.url)), ...args], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
