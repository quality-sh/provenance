import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync, mkdtempSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

export function cli(binary, repo, args, input) {
  const result = spawnSync(binary, [...args, '--repo', repo, '--scope', 'default', '--format', 'json'],
    { input: input && JSON.stringify(input), encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.status !== 0) throw new Error(`${args.join(' ')}: ${result.stderr}`);
  return JSON.parse(result.stdout);
}

/** Reads a guidance file from the working tree, or from `commit` when one is given. */
function guidanceFile(root, relative, commit) {
  if (!commit) return readFileSync(join(root, relative));
  const shown = spawnSync('git', ['show', `${commit}:${relative}`], { cwd: root, maxBuffer: 16 * 1024 * 1024 });
  if (shown.status !== 0) throw new Error(`${commit}:${relative}: ${shown.stderr}`);
  return shown.stdout;
}

export function prepareFixture(binary, repo, scenario, candidate, guidanceCommit) {
  mkdirSync(repo, { recursive: true });
  // A reviewer makes the review page writable, as in a real session.
  const initialized = spawnSync(binary, ['init', '--path', repo, '--scope', 'default', '--path-prefix', '.',
    '--disposition-actor-id', scenario.reviewer], { encoding: 'utf8' });
  if (initialized.status !== 0) throw new Error(initialized.stderr);
  if (candidate) {
    const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
    for (const relative of ['skills/provenance-shaping/SKILL.md', 'docs/shaping.md']) {
      const destination = relative.startsWith('skills/') ? join(repo, '.agents', relative) : join(repo, relative);
      mkdirSync(dirname(destination), { recursive: true });
      writeFileSync(destination, guidanceFile(root, relative, guidanceCommit));
    }
  }
  cli(binary, repo, ['sources', 'create', '--stdin'], {
    id: 'source_pay_policy', name: 'Overtime example policy', source_type: 'project_artifact',
    reference: '.scenario/policy.txt: the weekly threshold is 38 hours; the owner chooses the rate and presentation.',
  });
  // The anchor has a parent, so a view rooted at the anchor hides where it belongs.
  cli(binary, repo, ['requirements', 'create', '--stdin'], { id: scenario.anchor_parent, statement: 'The system supports payroll for one owner.' });
  cli(binary, repo, ['requirements', 'create', '--stdin'], { id: 'req_pay', statement: 'The system calculates overtime pay.', refines: [scenario.anchor_parent] });
  cli(binary, repo, ['topics', 'create', '--stdin'], { id: 'topic_pay', requirement_id: 'req_pay', title: 'Overtime pay' });
  for (const question of scenario.questions) {
    const input = { id: question.id, topic_id: 'topic_pay', question: question.text, resolution_method: question.method };
    if (candidate) input.depends_on = scenario.dependencies[question.id] ?? [];
    cli(binary, repo, ['questions', 'create', '--stdin'], input);
  }
  cli(binary, repo, ['topics', 'topic_pay', 'claim', '--actor', 'eval-writer']);
  cli(binary, repo, ['questions', 'question_delivery', 'claim', '--actor', 'other-worker']);
  mkdirSync(join(repo, '.scenario'), { recursive: true });
  writeFileSync(join(repo, '.scenario', 'policy.txt'), 'Overtime applies after 38 hours in one week. The owner chooses the hourly rate and output format.\n');
  writeFileSync(join(repo, 'AGENTS.md'), `Read .agents/skills/provenance-shaping/SKILL.md in this repository for the shaping workflow. Use ${binary} for every Provenance command. Use actor eval-writer. Keep all work in this repository. Do not publish, send messages, or push.\n`);
  spawnSync('git', ['init', '-q'], { cwd: repo });
}

export function readGraph(binary, repo) {
  const graph = {};
  for (const kind of ['topics', 'questions', 'requirements', 'resolutions', 'sources', 'rules', 'boundaries']) {
    const result = cli(binary, repo, [kind, 'list', '--exclude-terminal', 'false']);
    if (result.meta?.has_more) throw new Error(`Incomplete ${kind} evidence`);
    graph[kind] = result.data.items ?? result.data;
    if (!Array.isArray(graph[kind])) throw new Error(`Unexpected ${kind} list response`);
  }
  return graph;
}

/** Reads the review state that the person sees for each record, and each Resolution's downstream records. */
export function readReview(binary, repo, graph) {
  const kinds = ['requirements', 'rules', 'resolutions', 'boundaries', 'sources'];
  const records = [];
  for (const kind of kinds) {
    for (const record of graph[kind]) {
      const review = cli(binary, repo, [record.id, 'get', '--view', 'review']).review?.decision;
      const entry = { kind, id: record.id, pending: !!review?.pending, decisions: review?.decisions?.length ?? 0 };
      if (kind === 'resolutions') {
        entry.impact_rules = (cli(binary, repo, ['resolutions', record.id, 'impact']).data.affected_rules ?? []).map(rule => rule.id);
        entry.spawned = graph.requirements.filter(requirement => requirement.spawned_by === record.id).map(requirement => requirement.id);
      }
      records.push(entry);
    }
  }
  return records;
}

export function artifact(binary, commit) {
  const result = spawnSync(binary, ['--version'], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr);
  return { binary: resolve(binary), commit, sha256: createHash('sha256').update(readFileSync(binary)).digest('hex'), cli_identity: result.stdout.trim() };
}

export function prepareBinary(ref, supplied, root) {
  const commit = spawnSync('git', ['rev-parse', ref], { cwd: root, encoding: 'utf8' });
  if (commit.status !== 0) throw new Error(commit.stderr);
  const sha = commit.stdout.trim();
  if (supplied) return artifact(supplied, sha);
  const dir = mkdtempSync(join(tmpdir(), 'shaping-eval-build-'));
  const checkout = spawnSync('git', ['worktree', 'add', '--detach', dir, sha], { cwd: root, encoding: 'utf8' });
  if (checkout.status !== 0) throw new Error(checkout.stderr);
  const build = spawnSync(join(homedir(), '.local/bin/cargo'), ['build'], {
    cwd: dir, env: { ...process.env, PROVENANCE_CI_ARTIFACT: '1' }, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  });
  writeFileSync(join(dir, 'build-output.log'), `${build.stdout}\n${build.stderr}`);
  if (build.status !== 0) throw new Error(`Artifact build failed; see ${dir}/build-output.log`);
  const binary = join(dir, 'target/debug/provenance');
  if (!existsSync(binary)) throw new Error(`No artifact at ${binary}`);
  return artifact(binary, sha);
}
