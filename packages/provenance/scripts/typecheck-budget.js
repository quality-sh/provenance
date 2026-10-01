import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const MAX_MEMORY_KIB = 750_000;
export const MAX_INSTANTIATIONS = 300_000;

export function parseDiagnostics(output) {
  const memoryKiB = Number(output.match(/^Memory used:\s+(\d+)K$/m)?.[1]);
  const instantiations = Number(output.match(/^Instantiations:\s+(\d+)$/m)?.[1]);
  if (!Number.isSafeInteger(memoryKiB) || !Number.isSafeInteger(instantiations)) {
    throw new Error('TypeScript did not report memory and instantiation diagnostics.');
  }
  return { memoryKiB, instantiations };
}

export function checkBudgets({ memoryKiB, instantiations }) {
  const failures = [];
  if (memoryKiB > MAX_MEMORY_KIB) {
    failures.push(`memory ${memoryKiB} KiB exceeds ${MAX_MEMORY_KIB} KiB`);
  }
  if (instantiations > MAX_INSTANTIATIONS) {
    failures.push(`instantiations ${instantiations} exceed ${MAX_INSTANTIATIONS}`);
  }
  if (failures.length > 0) throw new Error(`TypeScript budget exceeded: ${failures.join('; ')}`);
}

export function writeDiagnostics(output, stream = process.stderr) {
  stream.write(output);
}

function run() {
  const root = dirname(dirname(fileURLToPath(import.meta.url)));
  const result = spawnSync(process.execPath, [
    join(root, 'node_modules/typescript/bin/tsc'),
    '-p', join(root, 'tsconfig.json'), '--extendedDiagnostics',
  ], { cwd: root, encoding: 'utf8' });
  writeDiagnostics(result.stdout);
  process.stderr.write(result.stderr);
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
  checkBudgets(parseDiagnostics(result.stdout));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) run();
