import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';

test('public browser client bundles without Node imports or runtime code generation', async () => {
  const output = await build({ entryPoints: [new URL('../../packages/provenance/src/client.ts', import.meta.url).pathname], bundle: true, platform: 'browser', format: 'esm', write: false });
  const source = output.outputFiles[0].text;
  assert.doesNotMatch(source, /\b(?:eval|Function)\s*\(/);
  assert.doesNotMatch(source, /(?:from|import)\s*[('"].*node:/);
  assert.match(source, /UncertainWriteError/);
});
