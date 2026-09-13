import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, realpathSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { portableFile } from './portable-file.js';

test('canonical implementation coordinates use an aliased project root', (t) => {
  const temporary = mkdtempSync(join(realpathSync(tmpdir()), 'portable-root-'));
  t.after(() => rmSync(temporary, { recursive: true, force: true }));
  const root = join(temporary, 'project');
  const alias = join(temporary, 'alias');
  mkdirSync(root);
  symlinkSync(root, alias, 'junction');
  assert.equal(portableFile(join(root, 'src', 'code.ts'), alias), 'src/code.ts');
  assert.equal(portableFile(join(alias, 'src', 'code.ts'), alias), 'src/code.ts');
  assert.throws(() => portableFile(join(temporary, 'outside.ts'), alias));
});
