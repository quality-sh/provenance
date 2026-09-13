import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, rm, readFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const pinPath = fileURLToPath(new URL('../review-assets.json', import.meta.url));
const pin = JSON.parse(await readFile(pinPath, 'utf8'));
for (const [change, message] of [
  [{ commit: '0'.repeat(40) }, /Renderer identity/],
  [{ dirty: true }, /Renderer identity/],
  [{ sdkSchemaSha256: '0'.repeat(64) }, /same generated SDK contract/],
] as const) {
  test(`composition refuses changed ${Object.keys(change)[0]}`, async t => {
    const work = await mkdtemp(join(tmpdir(), 'review-build-'));
    t.after(() => rm(work, { recursive: true, force: true }));
    const renderer = join(work, 'renderer');
    const sdk = join(work, 'sdk');
    const output = join(work, 'output');
    await mkdir(join(renderer, 'types/browser'), { recursive: true });
    await mkdir(join(sdk, 'dist/generated'), { recursive: true });
    await writeFile(join(renderer, 'types/browser/main.d.ts'), '');
    await writeFile(join(sdk, 'dist/generated/schema.d.ts'), '');
    await writeFile(join(renderer, 'build-info.json'), JSON.stringify(Object.assign({
      formatVersion: 1, commit: pin.commit, dirty: false,
    }, change)));
    const result = spawnSync(process.execPath, [fileURLToPath(new URL('./build.ts', import.meta.url)), renderer, sdk, output, pinPath], { encoding: 'utf8' });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, message);
    await assert.rejects(access(output));
  });
}
