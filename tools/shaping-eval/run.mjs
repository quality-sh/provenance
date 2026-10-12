import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile, mkdtemp, cp, readdir } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { score } from './score.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url));
const fixture = JSON.parse(await readFile(new URL('./fixtures/session.json', import.meta.url)));
const skillNames = ['provenance-shaping', 'provenance-grounded-writing'];
const baseCommit = 'd981e0bd10f00900772bfc49544caa43d17f3154';

export function command(executable, args, cwd, timeout = 120000) {
  return new Promise((resolveResult, reject) => {
    const child = spawn(executable, args, { cwd, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '', timedOut = false;
    child.stdout.on('data', b => { stdout += b; });
    child.stderr.on('data', b => { stderr += b; });
    const timer = setTimeout(() => {
      timedOut = true;
      try { if (child.pid) process.kill(-child.pid, 'SIGTERM'); }
      catch (error) { if (error.code !== 'ESRCH') reject(error); }
    }, timeout);
    child.on('error', error => { clearTimeout(timer); reject(error); });
    child.on('close', code => { clearTimeout(timer); resolveResult({ executable, args, code, timedOut, stdout, stderr }); });
  });
}
async function checked(executable, args, cwd) {
  const result = await command(executable, args, cwd);
  if (result.code !== 0 || result.timedOut) throw new Error(JSON.stringify(result));
  return result.stdout;
}
const cli = (workspace, args) => checked(process.env.PROVENANCE_BIN ?? 'provenance', [...args, '--format', 'json'], workspace);
async function create(workspace, collection, fields) {
  if (fields.statement) {
    const report = JSON.parse(await cli(workspace, ['statement-checks', 'create', '--statement', fields.statement]));
    if (report.data.findings.some(f => f.kind === 'violation')) throw new Error(JSON.stringify(report));
  }
  const args = [collection, 'create'];
  for (const [key, value] of Object.entries(fields)) args.push(`--${key.replaceAll('_', '-')}-json`, JSON.stringify(value));
  return cli(workspace, args);
}
export async function prepare(workspace, { variant }) {
  await checked('git', ['init', '-q'], workspace);
  await checked(process.env.PROVENANCE_BIN ?? 'provenance', ['init', '--path', '.', '--quiet'], workspace);
  await cli(workspace, ['skills', 'install', '--copy', '--force']);
  // Pin the evaluated guidance independently of the installed CLI build.
  for (const name of skillNames) {
    const content = variant === 'base'
      ? await checked('git', ['show', `${baseCommit}:skills/${name}/SKILL.md`], root)
      : await readFile(join(root, 'skills', name, 'SKILL.md'), 'utf8');
    await writeFile(join(workspace, '.agents/skills', name, 'SKILL.md'), content);
    await writeFile(join(workspace, '.claude/skills', name, 'SKILL.md'), content);
  }
  await mkdir(join(workspace, 'docs'), { recursive: true });
  const shapingDoc = variant === 'base'
    ? await checked('git', ['show', `${baseCommit}:docs/shaping.md`], root)
    : await readFile(join(root, 'docs/shaping.md'), 'utf8');
  await writeFile(join(workspace, 'docs/shaping.md'), shapingDoc);
  await create(workspace, 'sources', { id: 'src_owner_brief', name: 'Owner brief, health negative control', source_type: 'project_artifact', reference: 'session.json:negativeControl' });
  for (const [id, statement, refines] of [
    ['req_robot', 'The robot supports personal work goals.'],
    ['req_tasks', 'The robot rewards completed tasks.', 'req_robot'],
    ['req_health', 'The robot loses health when goals are neglected.', 'req_robot'],
    ['req_appearance', 'The robot has a friendly appearance.', 'req_robot'],
  ]) await create(workspace, 'requirements', { id, statement, ...(refines ? { refines } : {}) });
  await create(workspace, 'resolutions', { id: 'res_negative_control', title: 'Health floor', requirement_ids: ['req_health'], position: 'Use a zero health floor.', context: 'Health cannot express a value below zero.', rationale: 'A zero floor prevents negative health. A negative floor would require a separate meaning.', status: 'proposed' });
  await create(workspace, 'rules', { id: 'rule_negative_control', requirement_ids: ['req_health'], resolution_ids: ['res_negative_control'], statement: 'Health cannot decrease below 0.', source_document: 'Owner brief, health negative control', source_section: 'The owner requires a zero health floor independently of the proposed Resolution.', status: 'active' });
}

export async function snapshot(workspace) {
  const graph = {};
  for (const collection of ['requirements', 'rules', 'resolutions', 'questions', 'sources']) {
    graph[collection] = [];
    let cursor;
    do {
      const result = JSON.parse(await cli(workspace, [collection, 'list', '--limit', '200', ...(cursor ? ['--cursor', cursor] : [])]));
      graph[collection].push(...result.data.items);
      cursor = result.meta.has_more ? result.meta.next_cursor : null;
      if (result.meta.has_more && !cursor) throw new Error('Missing continuation cursor');
    } while (cursor);
  }
  graph.reviews = {};
  for (const collection of ['requirements', 'rules', 'resolutions']) for (const record of graph[collection]) {
    graph.reviews[record.id] = JSON.parse(await cli(workspace, [record.id, 'get', '--view', 'review']));
  }
  return graph;
}

function runnerArgs(runner, session, prompt) {
  if (runner.kind === 'codex') return session
    ? ['exec', 'resume', '--json', '--dangerously-bypass-approvals-and-sandbox', '-m', runner.model, session, prompt]
    : ['exec', '--json', '--dangerously-bypass-approvals-and-sandbox', '-m', runner.model, prompt];
  if (runner.kind === 'claude') return ['-p', '--output-format', 'stream-json', '--verbose', '--dangerously-skip-permissions', '--setting-sources', '', '--model', runner.model, ...(session ? ['--resume', session] : []), prompt];
  throw new Error('Only codex and claude runners are supported');
}
export function completedEvents(result) {
  const events = result.stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  if (result.code !== 0 || result.timedOut || events.some(e => e.type === 'error' || e.type === 'turn.failed' || e.is_error)) throw new Error('Runner failed');
  if (!events.some(e => e.type === 'turn.completed' || (e.type === 'result' && e.subtype === 'success'))) throw new Error('Runner output is incomplete');
  return events;
}
async function session(runner, workspace, output, prompts = [fixture.brief, ...fixture.responses]) {
  let sessionId;
  const transcript = [];
  for (const [turn, prompt] of prompts.entries()) {
    const result = await command(runner.executable, [...(runner.args ?? []), ...runnerArgs(runner, sessionId, prompt)], workspace, runner.timeoutMs ?? 600000);
    await writeFile(join(output, `turn-${turn}.json`), JSON.stringify(result, null, 2));
    let events;
    try { events = completedEvents(result); } catch (error) { throw new Error(`${error.message}; see ${output}/turn-${turn}.json`); }
    transcript.push({ role: 'owner', text: prompt }, ...events);
    sessionId ??= events.find(e => e.type === 'thread.started')?.thread_id ?? events.find(e => e.session_id)?.session_id;
    if (!sessionId) throw new Error('Runner did not return a resumable session id');
    await writeFile(join(output, `graph-${turn}.json`), JSON.stringify(await snapshot(workspace), null, 2));
  }
  return { transcript, sessionId };
}

async function modelEvidence(runner, sessionId, transcript) {
  if (runner.kind === 'claude') {
    const models = [...new Set(transcript.map(e => e.message?.model).filter(Boolean))];
    if (!models.length) throw new Error('Claude did not report its model');
    return { models, locator: 'transcript:assistant.message.model' };
  }
  const sessions = join(process.env.CODEX_HOME ?? join(process.env.HOME, '.codex'), 'sessions');
  const files = await readdir(sessions, { recursive: true });
  const file = files.find(f => f.endsWith(`${sessionId}.jsonl`));
  if (!file) throw new Error('Codex session metadata is missing');
  const events = (await readFile(join(sessions, file), 'utf8')).trim().split('\n').map(l => JSON.parse(l));
  const models = [...new Set(events.filter(e => e.type === 'turn_context').map(e => e.payload.model).filter(Boolean))];
  if (!models.length) throw new Error('Codex did not record its model');
  return { models, locator: `${file}:turn_context.model` };
}

async function main() {
  const flags = Object.fromEntries(process.argv.slice(2).reduce((pairs, arg, i, all) => i % 2 ? pairs : [...pairs, [arg.replace(/^--/, ''), all[i + 1]]], []));
  if (!['base', 'candidate'].includes(flags.variant) || !flags.config || !flags.output) throw new Error('Use --config PATH --variant base|candidate --output PATH');
  const config = JSON.parse(await readFile(flags.config));
  const output = resolve(flags.output);
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output, { recursive: false });
  await writeFile(join(output, 'config.json'), JSON.stringify(config, null, 2));
  for (const runner of config.runners) for (let run = 1; run <= (flags.probe === 'true' ? 1 : 3); run++) {
    const dir = join(output, `${runner.kind}-${run}`);
    await mkdir(dir);
    const workspace = await mkdtemp(join(tmpdir(), 'shaping-eval-'));
    await prepare(workspace, { variant: flags.variant });
    const skillHashes = {};
    for (const name of skillNames) skillHashes[name] = createHash('sha256').update(await readFile(join(workspace, '.agents/skills', name, 'SKILL.md'))).digest('hex');
    skillHashes['docs/shaping.md'] = createHash('sha256').update(await readFile(join(workspace, 'docs/shaping.md'))).digest('hex');
    await writeFile(join(dir, 'settings.json'), JSON.stringify({ runner, workspace, skillHashes, variant: flags.variant, baseCommit, cli: await checked(process.env.PROVENANCE_BIN ?? 'provenance', ['--version'], workspace) }, null, 2));
    console.log(`Start ${runner.kind} session ${run}`);
    const { transcript, sessionId } = await session(runner, workspace, dir, flags.probe === 'true' ? [
      'Probe only. Read the installed grounded-writing skill. Use the real Provenance CLI to create Source src_runner_probe with name Runner probe and source type project_artifact. Then stop. Work only in this repository.',
      'Resume probe. Read src_runner_probe through the real CLI. Report its id and stop. Do not recreate it.',
    ] : undefined);
    const identity = await modelEvidence(runner, sessionId, transcript);
    if (identity.models.some(model => model !== runner.model)) throw new Error(`Model selection changed: ${JSON.stringify(identity)}`);
    const graph = await snapshot(workspace);
    await writeFile(join(dir, 'graph.json'), JSON.stringify(graph, null, 2));
    const result = { transcript, model: identity, settings: runner, skillHashes, graph, ...score(graph) };
    await writeFile(join(dir, 'result.json'), JSON.stringify(result, null, 2));
    await cp(join(workspace, '.provenance'), join(dir, 'canonical'), { recursive: true });
    // A separate review packet hides variant, runner, model, and skill hashes.
    const blindId = randomUUID();
    await writeFile(join(dir, 'blind-key.json'), JSON.stringify({ blindId, variant: flags.variant, runner: runner.kind, run }));
    await mkdir(join(output, 'review'), { recursive: true });
    await writeFile(join(output, 'review', `${blindId}.json`), JSON.stringify({ graph, ownerResponses: fixture.responses, transcript: transcript.flatMap(e => e.item?.type === 'agent_message' ? [e.item.text] : e.message?.content?.filter(c => c.type === 'text').map(c => c.text) ?? []), expectedPlacement: fixture.expectedPlacement, ...score(graph) }, null, 2));
    console.log(`Complete ${runner.kind} session ${run}`);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
