import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generatedPaths } from './artifacts.mjs';

export function checkSourceControl(root) {
  const result = spawnSync('git', ['ls-files', '-z', '--', ...generatedPaths, 'crates/provenance-cli/review-assets-generated'], { cwd: root, encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || 'Cannot inspect Git index');
  return result.stdout.split('\0').filter(Boolean);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const paths = checkSourceControl(resolve(dirname(fileURLToPath(import.meta.url)), '../..'));
  if (paths.length) {
    console.error(`Generated files must not be tracked. Remove them from the index with git rm --cached:\n${paths.join('\n')}`);
    process.exitCode = 1;
  }
}
