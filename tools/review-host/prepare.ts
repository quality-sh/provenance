import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [sdkArg, outputArg] = process.argv.slice(2);
if (!sdkArg || !outputArg) throw new Error('Usage: node prepare.ts SDK_PACKAGE NEW_OUTPUT');
const pinPath = join(root, 'tools/review-assets.json');
const pin = JSON.parse(await readFile(pinPath, 'utf8'));
const work = await mkdtemp(join(tmpdir(), 'provenance-renderer-'));
try {
  let archive = process.env.PROVENANCE_REVIEW_ARCHIVE;
  if (!archive) {
    archive = join(work, pin.archive);
    if (pin.url) {
      if (new URL(pin.url).protocol !== 'https:') throw new Error('Renderer URL must use HTTPS');
      const response = await fetch(pin.url, { signal: AbortSignal.timeout(120_000) });
      if (!response.ok) throw new Error(`Renderer download failed: ${response.status}`);
      await writeFile(archive, new Uint8Array(await response.arrayBuffer()));
    } else {
      const run = JSON.parse(execFileSync('gh', ['run', 'view', String(pin.run), '--repo', pin.repository,
        '--json', 'conclusion,headSha'], { encoding: 'utf8' }));
      if (run.conclusion !== 'success' || run.headSha !== pin.commit) throw new Error('Renderer run does not match the successful pinned commit');
      execFileSync('gh', ['run', 'download', String(pin.run), '--repo', pin.repository,
        '--name', pin.artifact, '--dir', work], { stdio: 'inherit' });
    }
  }
  const bytes = await readFile(archive);
  if (createHash('sha256').update(bytes).digest('hex') !== pin.sha256) throw new Error('Renderer archive checksum mismatch');
  // Only the exact archive approved by the pin reaches the extractor.
  const renderer = join(work, 'renderer');
  await mkdir(renderer);
  execFileSync('tar', ['-xzf', resolve(archive), '-C', renderer], { stdio: 'inherit' });
  execFileSync(process.execPath, [join(root, 'tools/review-host/build.ts'), renderer,
    resolve(sdkArg), resolve(outputArg), pinPath], { stdio: 'inherit' });
} finally {
  await rm(work, { recursive: true, force: true });
}
