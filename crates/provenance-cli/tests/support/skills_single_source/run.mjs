import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, cpSync, rmSync, readdirSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const [mode, source, cargo, baseBinary, pdf] = process.argv.slice(2);
const names = ['provenance-fork-tournament', 'provenance-grounded-writing', 'provenance-shaping', 'provenance-swarm-backtrace'];
const temporary = mkdtempSync(join(tmpdir(), 'skills-single-source-'));
const checkout = join(temporary, 'checkout');
const target = join(temporary, 'target');
const env = { ...process.env, CARGO_TARGET_DIR: target, PROVENANCE_STE100_INDEX_DIR: join(temporary, 'indexes') };
function run(command, args, cwd = checkout, extra = {}) {
  console.log(`${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, { cwd, env: { ...env, ...extra }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, `${command}: ${result.stdout}\n${result.stderr}`);
  return result.stdout;
}
function fixture() {
  mkdirSync(checkout);
  const files = run('git', ['ls-files', '-z'], source).split('\0').filter(Boolean);
  for (const file of files) {
    const destination = join(checkout, file);
    mkdirSync(resolve(destination, '..'), { recursive: true });
    cpSync(join(source, file), destination);
  }
  for (const directory of ['contracts/operations', 'packages/provenance/src/generated', 'crates/provenance-http-client/src/generated']) {
    if (existsSync(join(source, directory))) cpSync(join(source, directory), join(checkout, directory), { recursive: true });
  }
  run('git', ['init', '-q']);
  run('git', ['config', 'user.name', 'Fixture']);
  run('git', ['config', 'user.email', 'fixture@example.invalid']);
  run('git', ['add', '-A']);
  run('git', ['commit', '-qm', 'Fixture sources']);
}
function init(binary, directory) {
  return run(binary, ['init', '--path', directory, '--scope', 'default', '--path-prefix', '.', '--ste-pdf', pdf]);
}
function payload(path) {
  return readFileSync(path, 'utf8').replace(/^<!-- Installed by provenance [^\n]+ -->\n/m, '');
}
function checkInstalled(directory, expected) {
  for (const name of names) for (const root of ['.agents/skills', '.claude/skills']) {
    assert.equal(payload(join(directory, root, name, 'SKILL.md')), expected[name], `${root}/${name} payload`);
  }
}
function edit(sentence) {
  const expected = {};
  for (const name of names) {
    const file = join(checkout, 'skills', name, 'SKILL.md');
    writeFileSync(file, readFileSync(file, 'utf8') + `\n${sentence} ${name}.\n`);
    expected[name] = readFileSync(file, 'utf8');
  }
  return expected;
}
function prepare() {
  run('node', ['tools/skills/prepare.mjs']);
}
function build(directory = checkout) {
  run(cargo, ['build', '--locked', '-p', 'provenance-cli', '--bin', 'provenance'], directory);
  return join(target, 'debug', process.platform === 'win32' ? 'provenance.exe' : 'provenance');
}
function checkCli(binary, expected) {
  const listed = JSON.parse(run(binary, ['skills', 'list', '--format', 'json']));
  assert.deepEqual(listed.map(skill => skill.name), names);
  for (const name of names) assert.equal(run(binary, ['skills', 'show', name]), expected[name]);
  const install = mkdtempSync(join(temporary, 'install-'));
  run(binary, ['skills', 'install', '--copy'], install);
  checkInstalled(install, expected);
  const project = join(temporary, `init-${readdirSync(temporary).length}`);
  const first = init(binary, project);
  const second = init(binary, project);
  checkInstalled(project, expected);
  rmSync(project, { recursive: true, force: true });
  assert.equal(init(baseBinary, project), first, 'fresh init summary must match the unchanged build');
  assert.equal(init(baseBinary, project), second, 'repeat init summary must match the unchanged build');
}
try {
  fixture();
  if (mode === 'clean') {
    // Normal onboarding files exist before the fixture commit; only skill outputs are absent.
    init(baseBinary, checkout);
    for (const name of names) for (const root of ['.agents/skills', '.claude/skills']) {
      rmSync(join(checkout, root, name), { recursive: true, force: true });
    }
    run('git', ['add', '-A']);
    run('git', ['commit', '--allow-empty', '-qm', 'Prepared onboarding without installed skills']);
    init(baseBinary, checkout);
    assert.equal(run('git', ['status', '--porcelain', '--untracked-files=all']), '', 'init must leave the prepared checkout clean');
    const second = init(baseBinary, checkout);
    assert.match(second, /No change\./);
    assert.equal(init(baseBinary, checkout), second);
    const expected = Object.fromEntries(names.map(name => [name, readFileSync(join(checkout, 'skills', name, 'SKILL.md'), 'utf8')]));
    checkInstalled(checkout, expected);
    edit('This sentence checks source preparation');
    prepare();
    const changes = run('git', ['status', '--porcelain', '--untracked-files=all']).trimEnd().split('\n');
    assert.deepEqual(changes.map(line => line.slice(3)).sort(), names.map(name => `skills/${name}/SKILL.md`).sort());
  } else {
    run('npm', ['ci', '--prefix', 'tools/operation-codegen']);
    run('node', ['tools/operation-codegen/ensure-generated.mjs']);
    let expected = edit('This sentence checks the first source edit');
    let binary = build();
    checkCli(binary, expected);
    expected = edit('This sentence checks the rebuild trigger');
    binary = build();
    checkCli(binary, expected);
    for (const name of names) {
      const install = mkdtempSync(join(temporary, 'external-'));
      const home = mkdtempSync(join(temporary, 'home-'));
      run(process.platform === 'win32' ? 'npx.cmd' : 'npx', ['--yes', 'skills@1.7.2', 'add', checkout, '--skill', name, '--agent', 'codex', '--copy', '--yes'], install, { HOME: home, USERPROFILE: home });
      assert.equal(readFileSync(join(install, '.agents/skills', name, 'SKILL.md'), 'utf8'), expected[name]);
    }
    await import('./package.mjs').then(module => module.checkPackage({ checkout, temporary, target, cargo, expected, run, prepare, checkCli, names }));
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
