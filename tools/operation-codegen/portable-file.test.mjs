import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import ts from 'typescript';
const source = await readFile(new URL('../../packages/provenance/src/portable-file.ts', import.meta.url), 'utf8');
const javascript = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
const { portableFile } = await import('data:text/javascript;base64,' + Buffer.from(javascript).toString('base64'));

test('local file coordinates become portable paths beneath the explicit project root', () => {
  const root = resolve('fixture-project');
  assert.equal(portableFile(join(root, 'src', 'check.ts'), root), 'src/check.ts');
  assert.equal(portableFile('src/check.ts', root), 'src/check.ts');
  assert.throws(() => portableFile('../outside.ts', root), /outside/);
  assert.throws(() => portableFile(resolve('outside.ts'), root), /outside/);
  assert.throws(() => portableFile('src/check.ts', undefined), /localRoot/);
});
