import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

if (process.argv[2] !== 'statements') throw new Error('Only Phase 1 statements are implemented');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function run(args, env = process.env) {
  const child = spawnSync('cargo', args, { cwd: root, env, stdio: 'inherit' });
  if (child.status !== 0) throw new Error(`cargo ${args.join(' ')} failed`);
}
run(['build', '-p', 'provenance-transport', '--features', 'test-fixture', '--bin', 'statement-host-fixture']);
const host = spawn(join(root, 'target/debug/statement-host-fixture'), [], { cwd: root, stdio: ['pipe', 'pipe', 'inherit'] });
const temporary = await mkdtemp(join(tmpdir(), 'provenance-clients-'));
try {
  const url = await new Promise((resolve, reject) => {
    let output = '';
    const timeout = setTimeout(() => reject(new Error('Fixture did not start')), 30000);
    host.once('exit', code => { clearTimeout(timeout); reject(new Error(`Fixture exited: ${code}`)); });
    host.stdout.on('data', chunk => {
      output += chunk;
      if (output.includes('\n')) { clearTimeout(timeout); resolve(output.split('\n')[0].trim()); }
    });
  });
  const source = await readFile(join(root, 'packages/provenance/src/generated/client.ts'), 'utf8');
  const script = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  await writeFile(join(temporary, 'client.mjs'), script);
  const { HttpClient, OperationError, PROTOCOL_VERSION } = await import(join(temporary, 'client.mjs'));
  const client = await HttpClient.connect(url);
  for (const statement of ['Install the cover.', 'Stop; wait.', 'Café; stop.']) {
    const call = { request: { statement } };
    const raw = await fetch(`${url}/v${PROTOCOL_VERSION}/operations/check-statement`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(call) });
    assert.equal(raw.status, 200);
    assert.deepEqual(await client.checkStatement(call), await raw.json());
  }
  await assert.rejects(client.checkStatement({ request: {} }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.status, 400);
    assert.equal(error.failure.operation, 'check-statement');
    assert.equal(error.failure.error.kind, 'invalid_input');
    return true;
  });
  run(['test', '-p', 'provenance-http-client', '--test', 'statements', '--', '--ignored'], { ...process.env, PROVENANCE_TEST_HOST: url });
  console.log('Both generated clients passed against the real statement host.');
} finally {
  const exited = once(host, 'exit');
  host.stdin.end();
  const timeout = setTimeout(() => host.kill('SIGKILL'), 10000);
  try { const [code] = await exited; assert.equal(code, 0, 'Fixture must join cleanly'); }
  finally { clearTimeout(timeout); await rm(temporary, { recursive: true, force: true }); }
}
