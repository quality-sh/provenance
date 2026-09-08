import { buildBinary } from './cargo.mjs';
import { spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { typescriptFiles } from './typescript.mjs';
import { responseSchemas } from './validators.mjs';
import { compareTrees } from './inventory.mjs';
import { rustClientFiles } from './templates.mjs';
import { generatedDirectories as directories, recordGeneration } from './artifacts.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
}
async function generate(temporary, generator) {
  for (const directory of directories) await mkdir(join(temporary, directory), { recursive: true });
  run(generator, ['export', join(temporary, directories[0])]);
  const openapiPath = join(temporary, directories[0], 'openapi.json');
  const document = JSON.parse(await readFile(openapiPath, 'utf8'));
  const tsDir = join(temporary, directories[1]);
  for (const [path, source] of Object.entries(await typescriptFiles(document))) await writeFile(join(tsDir, path), source);
  const rustDir = join(temporary, directories[2]);
  run(generator, ['rust', openapiPath, rustDir]);
  await writeFile(join(rustDir, 'responses.json'), JSON.stringify({ components: document.components, response_schemas: responseSchemas(document) }) + '\n');
  const clientFiles = rustClientFiles(document);
  for (const [path, source] of Object.entries(clientFiles)) {
    await mkdir(dirname(join(rustDir, path)), { recursive: true });
    await writeFile(join(rustDir, path), source);
  }
  // Format generated connection and operation templates before comparison.
  run('rustfmt', ['--edition', '2021', ...Object.keys(clientFiles).map(path => join(rustDir, path))]);
  const oversized = await compareTrees(rustDir, rustDir);
  if (oversized.length) throw new Error(oversized.join('\n'));
  await recordGeneration(root, temporary);
}

const temporary = await mkdtemp(join(tmpdir(), 'provenance-codegen-'));
try {
  const generator = buildBinary(root, ['--locked', '--quiet', '-p', 'provenance-codegen', '--all-targets'], 'provenance-codegen');
  const first = join(temporary, 'first');
  await generate(first, generator);
  if (process.argv.includes('--check')) {
    const second = join(temporary, 'second');
    await generate(second, generator);
    const errors = (await Promise.all(directories.map(async directory =>
      (await compareTrees(join(first, directory), join(second, directory))).map(error => `${directory}: ${error}`)))).flat();
    if (errors.length) throw new Error(errors.join('\n'));
    console.log('Operation generation is deterministic; Rust outputs satisfy the file size limit.');
  } else {
    for (const directory of directories) {
      await rm(join(root, directory), { recursive: true, force: true });
      await cp(join(first, directory), join(root, directory), { recursive: true });
    }
  }
} finally { await rm(temporary, { recursive: true, force: true }); }
