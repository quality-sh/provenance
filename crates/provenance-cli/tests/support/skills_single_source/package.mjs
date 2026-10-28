import assert from 'node:assert/strict';
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export function checkPackage({ checkout, temporary, target, cargo, expected, run, prepare, checkCli, names }) {
  for (const name of names) rmSync(join(checkout, 'crates/provenance-cli/skills', name), { recursive: true, force: true });
  prepare();
  // Dependency copies provide unpublished workspace crates without restoring CLI sources.
  const dependencies = join(temporary, 'dependencies');
  mkdirSync(dependencies);
  cpSync(join(checkout, 'crates'), join(dependencies, 'crates'), { recursive: true });
  cpSync(join(checkout, 'Cargo.toml'), join(dependencies, 'Cargo.toml'));
  cpSync(join(checkout, 'README.md'), join(dependencies, 'README.md'));
  cpSync(join(checkout, 'LICENSE'), join(dependencies, 'LICENSE'));
  cpSync(join(checkout, 'examples'), join(dependencies, 'examples'), { recursive: true });
  rmSync(join(dependencies, 'crates/provenance-cli'), { recursive: true, force: true });
  let manifest = readFileSync(join(dependencies, 'Cargo.toml'), 'utf8');
  manifest = manifest.replace(/\s*"crates\/provenance-cli",?/g, '');
  writeFileSync(join(dependencies, 'Cargo.toml'), manifest);
  const metadata = JSON.parse(run(cargo, ['metadata', '--no-deps', '--format-version', '1']));
  const patches = metadata.packages.filter(pkg => pkg.name !== 'provenance-cli' && pkg.manifest_path.startsWith(join(checkout, 'crates')));
  const config = join(temporary, 'patches.toml');
  writeFileSync(config, '[patch.crates-io]\n' + patches.map(pkg => `${pkg.name} = { path = ${JSON.stringify(join(dependencies, 'crates', pkg.name).replaceAll('\\', '/'))} }`).join('\n') + '\n');
  // Cargo verifies the CLI archive against a local overlay of the packaged workspace crates, so --locked checks the
  // committed lock. Cargo steps run offline: CI fetches the locked crates through Socket Firewall before the tests.
  const packages = new Map(metadata.packages.map(pkg => [pkg.name, pkg]));
  const selected = new Set();
  const pending = ['provenance-cli'];
  while (pending.length) {
    const name = pending.pop();
    if (selected.has(name)) continue;
    selected.add(name);
    for (const dependency of packages.get(name).dependencies) if (dependency.path && packages.has(dependency.name)) pending.push(dependency.name);
  }
  run(cargo, ['package', ...[...selected].flatMap(name => ['-p', name]), '--locked', '--offline', '--allow-dirty', '--registry', 'crates-io']);
  const version = metadata.packages.find(pkg => pkg.name === 'provenance-cli').version;
  const extracted = join(temporary, 'extracted');
  mkdirSync(extracted);
  const archive = join(target, 'package', `provenance-cli-${version}.crate`);
  run('tar', ['-xzf', archive, '-C', extracted]);
  const packaged = join(extracted, `provenance-cli-${version}`);
  for (const name of names) assert.equal(readFileSync(join(packaged, 'skills', name, 'SKILL.md'), 'utf8'), expected[name]);
  run(cargo, ['--config', config, 'update', '--workspace', '--offline'], packaged);
  run(cargo, ['--config', config, 'build', '--locked', '--offline', '--bin', 'provenance'], packaged);
  checkCli(join(target, 'debug', process.platform === 'win32' ? 'provenance.exe' : 'provenance'), expected);
}
