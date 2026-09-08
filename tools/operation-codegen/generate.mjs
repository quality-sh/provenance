import { spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import openapiTS, { astToString } from 'openapi-typescript';
import { compareTrees } from './inventory.mjs';
import { rustClientFiles, typescriptClient } from './templates.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
}
const directories = ['contracts/operations', 'packages/provenance/src/generated', 'crates/provenance-http-client/src/generated'];
const temporary = await mkdtemp(join(tmpdir(), 'provenance-codegen-'));
try {
  for (const directory of directories) await mkdir(join(temporary, directory), { recursive: true });
  run('cargo', ['build', '--locked', '--quiet', '-p', 'provenance-codegen', '--all-targets']);
  const generator = join(root, 'target/debug/provenance-codegen');
  run(generator, ['export', join(temporary, directories[0])]);
  const openapiPath = join(temporary, directories[0], 'openapi.json');
  const document = JSON.parse(await readFile(openapiPath, 'utf8'));
  const tsDir = join(temporary, directories[1]);
  await writeFile(join(tsDir, 'schema.ts'), astToString(await openapiTS(document, { defaultNonNullable: false })));
  await writeFile(join(tsDir, 'client.ts'), typescriptClient(document));
  const rustDir = join(temporary, directories[2]);
  run(generator, ['rust', openapiPath, rustDir]);
  const clientFiles = rustClientFiles(document);
  for (const [path, source] of Object.entries(clientFiles)) {
    await mkdir(dirname(join(rustDir, path)), { recursive: true });
    await writeFile(join(rustDir, path), source);
  }
  // rustfmt all outputs so drift compares the repository's normal format.
  run('rustfmt', ['--edition', '2021', ...Object.keys(clientFiles).map(path => join(rustDir, path))]);
  const oversized = await compareTrees(rustDir, rustDir);
  if (oversized.length) throw new Error(oversized.join('\n'));
  if (process.argv.includes('--check')) {
    const errors = (await Promise.all(directories.map(async directory =>
      (await compareTrees(join(temporary, directory), join(root, directory))).map(error => `${directory}: ${error}`)))).flat();
    if (errors.length) throw new Error(errors.join('\n'));
    console.log('Operation definitions and both clients match.');
  } else {
    for (const directory of directories) {
      await rm(join(root, directory), { recursive: true, force: true });
      await cp(join(temporary, directory), join(root, directory), { recursive: true });
    }
  }
} finally { await rm(temporary, { recursive: true, force: true }); }
