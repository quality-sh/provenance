import { buildBinary } from './cargo.mjs';
import { ensureGenerated } from './ensure-generated.mjs';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { checkStatements } from './test-statements.mjs';
import { checkRecords } from './test-records.mjs';
import { checkEvidence } from './test-evidence.mjs';
import { checkWrites } from './test-writes.mjs';
import { checkDiscussions } from './test-discussions.mjs';
import { checkCreation } from './test-creation.mjs';

const family = process.argv[2];
const checks = { statements: checkStatements, records: checkRecords, evidence: checkEvidence, writes: checkWrites, creation: checkCreation, discussions: checkDiscussions };
if (!checks[family]) throw new Error('Expected statements, records, evidence, writes, creation, or discussions');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function run(args, env = process.env) {
  const child = spawnSync('cargo', args, { cwd: root, env, stdio: 'inherit' });
  if (child.status !== 0) throw new Error(`cargo ${args.join(' ')} failed`);
}
await ensureGenerated();
const binary = ['writes', 'creation', 'discussions'].includes(family) ? 'existing-root-host-fixture' : family === 'statements' ? 'statement-host-fixture' : 'records-host-fixture';
const hostBinary = buildBinary(root, ['--locked', '-p', 'provenance-transport', '--features', 'test-fixture', '--bin', binary], binary);
const temporary = await mkdtemp(join(tmpdir(), 'provenance-clients-'));
if (['writes', 'creation', 'discussions'].includes(family)) {
  const cliBinary = buildBinary(root, ['--locked', '-p', 'provenance-cli', '--bin', 'provenance'], 'provenance');
  const result = spawnSync(cliBinary, ['init', '--path', temporary, '--scope', 'default', '--path-prefix', '.'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  await writeFile(join(temporary, 'check.rs'), 'fn check() {}\n');
}
const host = spawn(hostBinary, [], { cwd: temporary, env: { ...process.env, PROVENANCE_FIXTURE_ROOT: temporary, PROVENANCE_FIXTURE_REPOSITORY_ID: 'fixture', PROVENANCE_FIXTURE_SCOPE: 'default', PROVENANCE_FIXTURE_TOKEN: 'fixture-secret' }, stdio: ['pipe', 'pipe', 'inherit'] });
try {
  const firstLine = await new Promise((resolve, reject) => {
    let output = '';
    const timeout = setTimeout(() => reject(new Error('Fixture did not start')), 30000);
    host.once('exit', code => { clearTimeout(timeout); reject(new Error(`Fixture exited: ${code}`)); });
    host.stdout.on('data', chunk => {
      output += chunk;
      if (output.includes('\n')) { clearTimeout(timeout); resolve(output.split('\n')[0].trim()); }
    });
  });
  const fixture = family === 'statements' ? { url: firstLine } : { ...JSON.parse(firstLine), root: temporary };
  await writeFile(join(temporary, 'package.json'), '{"type":"module"}');
  for (const name of ['client', 'runtime']) {
    const source = await readFile(join(root, `packages/provenance/src/generated/${name}.ts`), 'utf8');
    const script = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
    await writeFile(join(temporary, `${name}.js`), script);
  }
  await writeFile(join(temporary, 'validators.mjs'), await readFile(join(root, 'packages/provenance/src/generated/validators.mjs')));
  const module = await import(join(temporary, 'client.js'));
  await checks[family](module, fixture);
  run(['test', '--locked', '-p', 'provenance-http-client', '--test', family, '--', '--ignored'], {
    ...process.env, PROVENANCE_TEST_HOST: fixture.url, PROVENANCE_RECORDS_FIXTURE: JSON.stringify(fixture),
  });
  console.log(`Both generated clients passed against the real ${family} host.`);
} finally {
  const exited = host.exitCode === null ? once(host, 'exit') : Promise.resolve([host.exitCode]);
  host.stdin.end();
  const timeout = setTimeout(() => host.kill('SIGKILL'), 10000);
  try { const [code] = await exited; assert.equal(code, 0, 'Fixture must join cleanly'); }
  finally { clearTimeout(timeout); await rm(temporary, { recursive: true, force: true }); }
}
