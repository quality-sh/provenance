// Release-check aid: the verify scripts run a real review host on a new fixture repository.
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

/** Writes records into the fixture repository before the host starts. */
export interface FixtureRepository {
  /** Runs `<collection> create --stdin` with the body. */
  create(collection: string, body: object): void;
}

/** The first line that the review host writes: its origin and the bearer for its operations. */
export interface FixtureHost {
  readonly endpoint: string;
  readonly bearer: string;
}

/**
 * Copies the CLI into a new directory, makes a repository with the synthetic STE dictionary,
 * writes the seed records, and starts `review` on it. The host stops and the directory goes
 * after `use` completes or fails.
 */
export async function withFixtureHost<T>(
  binaryArg: string,
  seed: (repository: FixtureRepository) => void,
  use: (host: FixtureHost) => Promise<T>,
): Promise<T> {
  const work = await mkdtemp(join(tmpdir(), 'provenance-review-host-'));
  const binary = join(work, process.platform === 'win32' ? 'provenance.exe' : 'provenance');
  let host: ReturnType<typeof spawn> | undefined;
  try {
    await copyFile(resolve(binaryArg), binary);
    await chmod(binary, 0o755);
    const repository = join(work, 'repository');
    const steAssets = join(work, 'ste-assets');
    await mkdir(steAssets);
    await copyFile(
      join(root, 'packages/provenance/test/fixtures/synthetic-ste-dictionary.pdf'),
      join(steAssets, 'ASD-STE100_ISSUE9.pdf'),
    );
    const env = {
      ...process.env,
      PROVENANCE_STE100_ASSET_DIR: steAssets,
      PROVENANCE_STE100_INDEX_DIR: join(work, 'ste-indexes'),
    };
    execFileSync(binary, ['init', '--path', repository, '--scope', 'default', '--path-prefix', '.'], {
      cwd: work, encoding: 'utf8', timeout: 180_000, env,
    });
    seed({
      create(collection, body) {
        execFileSync(binary, [collection, 'create', '--repo', repository, '--scope', 'default', '--stdin'], {
          cwd: work, encoding: 'utf8', timeout: 30_000, env, input: JSON.stringify(body),
        });
      },
    });
    host = spawn(binary, ['review', '--repo', repository, '--repository-id', 'fixture', '--scope', 'default', '--no-open'], {
      cwd: work, env: { ...process.env, PATH: work }, stdio: ['ignore', 'pipe', 'inherit'],
    });
    const exited = once(host, 'exit');
    const lines = createInterface({ input: host.stdout! });
    const [line] = await Promise.race([
      once(lines, 'line', { signal: AbortSignal.timeout(30_000) }),
      exited.then(() => { throw new Error('Review host exited before startup'); }),
    ]);
    lines.close();
    const result = await use(JSON.parse(line));
    host.kill('SIGTERM');
    await exited;
    return result;
  } finally {
    if (host && host.exitCode === null && host.signalCode === null) {
      const exited = once(host, 'exit');
      host.kill();
      await exited;
    }
    await rm(work, { recursive: true, force: true });
  }
}
