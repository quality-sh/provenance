import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { runInNewContext } from 'node:vm';
import { startFixtureHost } from './fixture-host.js';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const packageRoot = fileURLToPath(new URL('..', import.meta.url));
const manifest = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8'));
const temporary = mkdtempSync(join(tmpdir(), 'provenance-effect-install-'));
function run(command, args, cwd = root) {
  return execFileSync(command, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}
function npm(args, cwd) { return run(process.execPath, [process.env.npm_execpath, ...args], cwd); }
try {
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const cli = join(root, 'target/debug', `provenance${suffix}`);
  const hostBinary = join(root, 'target/debug', `existing-root-host-fixture${suffix}`);
  const packed = JSON.parse(npm(['pack', '--json', '--pack-destination', temporary], packageRoot));
  const archive = join(temporary, packed[0].filename);
  assert.ok(packed[0].files.some(file => file.path === 'dist/generated/effect-contract.js'));
  assert.ok(!packed[0].files.some(file => file.path.includes('node_modules/effect')));
  const repo = join(temporary, 'repository');
  run(cli, ['init', '--path', repo, '--scope', 'default', '--path-prefix', '.']);
  const host = await startFixtureHost({ root: repo, binary: hostBinary });
  try {
    for (const name of ['browser', 'promise']) {
      const application = join(temporary, name);
      mkdirSync(application);
      writeFileSync(join(application, 'package.json'), JSON.stringify({ name: `installed-${name}`, private: true, type: 'module', dependencies: {
        '@quality-sh/provenance': `file:${archive}`,
        ...(name === 'promise' ? {} : { effect: manifest.peerDependencies.effect }),
      } }));
      npm(['install', '--omit=optional', '--no-audit', '--no-fund'], application);
      if (name === 'promise') {
        assert.equal(existsSync(join(application, 'node_modules/effect')), false);
        writeFileSync(join(application, 'consumer.mjs'), `import {HttpClient} from '@quality-sh/provenance/client';
const client = await HttpClient.connectWithBearer(${JSON.stringify(host.environment.PROVENANCE_ENDPOINT)}, ${JSON.stringify(host.environment.PROVENANCE_TOKEN)});
const result = await client.checkStatement({request:{statement:'Install the cover.'}});
if(result.issue!==9) throw new Error('Invalid report');`);
        run(process.execPath, ['consumer.mjs'], application);
        continue;
      }
      const source = `import * as Effect from 'effect/Effect';
import * as Schema from 'effect/Schema';
import {Atom, AtomRegistry} from 'effect/unstable/reactivity';
import {EffectHttpClient, ProvenanceClient, CheckStatementSuccessOutput, ProvenanceApi} from '@quality-sh/provenance/effect';
export async function exercise(baseUrl: string, bearer: string, suffix = '') {
 const client = await Effect.runPromise(EffectHttpClient.connect({baseUrl,bearer}));
 const context = {repository:'fixture',scope:'default'};
 const statement = await Effect.runPromise(client.checkStatement({request:{statement:'Install the cover.'}}));
 Schema.decodeUnknownSync(CheckStatementSuccessOutput)(statement);
 const created = await Effect.runPromise(client.createRequirement({context,request:{scope_id:'default',status:'active',depends_on:[],supersedes:[],id:'req_${name.replaceAll('-', '_')}'+suffix,statement:'The record retains its fields.'}}));
 if(created.id!=='req_${name.replaceAll('-', '_')}'+suffix) throw new Error('Mutation failed');
 const runtime=Atom.runtime(ProvenanceClient.layer({baseUrl,bearer}));
 const atom=runtime.atom(Effect.flatMap(ProvenanceClient,sdk=>sdk.get({context,request:{node_type:'requirement',id:created.id}})));
 const registry=AtomRegistry.make();
 try { const result=await Effect.runPromise(AtomRegistry.getResult(registry,atom)); if(!result.found)throw new Error('Read failed'); }
 finally {registry.dispose();}
 let dispatched = 0;
 const uncertainClient = await Effect.runPromise(EffectHttpClient.connect({baseUrl,bearer,fetch:async(input,init)=>{
   const response=await fetch(input,init);
   if(init?.method==='POST' && input.toString().endsWith('/create-requirement')) {
     dispatched++; await response.arrayBuffer();
     return new Response(new ReadableStream());
   }
   return response;
 }}));
 const unknownId='req_unknown_${name.replaceAll('-', '_')}'+suffix;
 await Effect.runPromise(Effect.flip(uncertainClient.createRequirement({context,request:{scope_id:'default',status:'active',depends_on:[],supersedes:[],id:unknownId,statement:'The record retains its fields.'}}).pipe(Effect.timeout('1 second'))));
 const unresolved=uncertainClient.unresolvedWrites();
 if(dispatched!==1 || unresolved.length!==1 || unresolved[0].state!=='uncertain') throw new Error('Lost mutation outcome');
 const evidence=await Effect.runPromise(client.get({context,request:{node_type:'requirement',id:unknownId}}));
 if(!evidence.found) throw new Error('Expected the mutation to persist after response loss');
 uncertainClient.resolveWrite(unresolved[0].id);
 if(!ProvenanceApi) throw new Error('Missing contract');
}
`;
      writeFileSync(join(application, 'consumer.ts'), source);
      run(process.execPath, [join(packageRoot, 'node_modules/typescript/bin/tsc'), '--strict', '--skipLibCheck', '--target', 'es2022', '--module', 'nodenext', 'consumer.ts'], application);
      writeFileSync(join(application, 'run.mjs'), `import {exercise} from './consumer.js'; await exercise(${JSON.stringify(host.environment.PROVENANCE_ENDPOINT)},${JSON.stringify(host.environment.PROVENANCE_TOKEN)});`);
      run(process.execPath, ['run.mjs'], application);
      const bundle = await build({ entryPoints: [join(application, 'consumer.ts')], bundle: true, platform: 'browser', format: 'iife', globalName: 'Consumer', write: false, minify: true, metafile: true });
      const effectRoots = new Set(Object.keys(bundle.metafile.inputs).filter(path => path.includes('node_modules/effect/')).map(path => path.split('node_modules/effect/')[0]));
      assert.equal(effectRoots.size, 1, 'The application supplies one Effect runtime');
      assert.ok(!Object.keys(bundle.metafile.inputs).some(path => /engine-path|node:|typescript\/lib/.test(path)));
      const bytes = bundle.outputFiles[0].contents;
      const browser = {
        setTimeout, clearTimeout, setInterval, clearInterval, queueMicrotask,
        TextEncoder, TextDecoder, URL, URLSearchParams, AbortController, AbortSignal,
        Headers, Request, Response, ReadableStream, WritableStream, TransformStream,
        crypto, performance, atob, btoa, structuredClone, console,
        fetch: (input, init) => {
          assert.equal(new URL(input instanceof Request ? input.url : input.toString()).origin, new URL(host.environment.PROVENANCE_ENDPOINT).origin);
          return fetch(input, init);
        },
      };
      runInNewContext(bundle.outputFiles[0].text, browser, { timeout: 10_000 });
      await browser.Consumer.exercise(host.environment.PROVENANCE_ENDPOINT, host.environment.PROVENANCE_TOKEN, '_browser');
      assert.equal(browser.process, undefined);
      assert.equal(browser.require, undefined);
      console.log(`${name}: installed and browser-context read, mutation, and atom passed; browser bundle ${bytes.length} bytes, gzip ${gzipSync(bytes).length} bytes`);
    }
  } finally { await host.close(); }
} finally { rmSync(temporary, { recursive: true, force: true }); }
