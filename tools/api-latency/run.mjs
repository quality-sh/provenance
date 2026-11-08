// Times every catalog operation, porcelain command and MCP tool action of
// one `provenance` binary on scratch clones of one repository.
//
//   node tools/api-latency/run.mjs --bin target/release/provenance \
//     --source . --work "$RUNNER_TEMP/api-latency" --out results.json
//
// Each way in gets its own clone, so the writes of one way do not change the
// input of another. report.mjs ranks the results.
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { helpCommands, mcpToolActions, operationsFromCatalog, possibleValues } from './catalog.mjs';
import { COMMAND_ARGUMENTS, NOT_CALLED, cliWriteArguments, commandVariants, fillArguments, mcpWriteArguments, orderLeaves, verbOperation } from './commands.mjs';
import { Planner, Unresolved, describe } from './requests.mjs';
import { findKey } from './body.mjs';
import { CliWay, HttpWay, McpWay, run } from './ways.mjs';

const { values: options } = parseArgs({
  options: {
    bin: { type: 'string' },
    source: { type: 'string', default: '.' },
    work: { type: 'string' },
    out: { type: 'string', default: 'results.json' },
    reads: { type: 'string', default: '20' },
    writes: { type: 'string', default: '10' },
    ways: { type: 'string', default: 'HTTP,MCP,CLI' },
    filter: { type: 'string', default: '' },
  },
});
const bin = resolve(options.bin);
const work = resolve(options.work);
const reads = Number(options.reads);
const writes = Number(options.writes);
const filter = options.filter;

/** Routes that take a body but change nothing. */
const READ_ONLY_POSTS = ['/statement-checks', '/authoring-plans'];

const calls = [];
const unmeasured = [];
const log = message => process.stderr.write(`${message}\n`);

function skip(way, call, reason) {
  unmeasured.push({ way, call, reason });
  log(`  not measured: ${way} ${call}: ${reason}`);
}

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

async function git(args, cwd) {
  const result = await run('git', args, { cwd });
  if (result.code !== 0) {
    throw new Error(`git ${args.join(' ')}: ${result.stderr}`);
  }
  return result.stdout.trim();
}

async function clone(name, actor) {
  const directory = join(work, name);
  await rm(directory, { recursive: true, force: true });
  await git(['clone', '--quiet', '--no-hardlinks', resolve(options.source), directory]);
  await git(['config', 'user.name', 'API latency'], directory);
  await git(['config', 'user.email', 'api-latency@example.invalid'], directory);
  if (actor) {
    await git(['config', 'provenance.reviewer', actor], directory);
  }
  return directory;
}

async function context(repo, operations, http) {
  const manifest = JSON.parse(await readFile(join(repo, '.provenance/state/manifest.json'), 'utf8'));
  const actor = manifest.disposition_actor_ids?.[0] ?? 'api_latency';
  const base = await git(['rev-parse', 'HEAD~1'], repo).catch(() => git(['rev-parse', 'HEAD'], repo));
  const bindings = await http.call({ method: 'get', path: '/verification-bindings', query: { limit: 1 } });
  const rules = await http.call({ method: 'get', path: '/rules', query: { limit: 1, exclude_terminal: true } });
  const skills = await run(bin, ['skills', 'list'], { cwd: repo });
  return {
    actor,
    text: 'This text is for a test.',
    text2: 'This text is for a second test.',
    idText: 'api_latency',
    fields: { actor, declared_by: actor, made_by: actor, approved_by: actor, statement: 'The system keeps a record.', decision: 'deferred' },
    query: { base, head: 'HEAD', file: findKey(bindings.data, 'file') ?? 'Cargo.toml' },
    word: 'rule',
    base,
    statement: rules.data?.items?.[0]?.statement ?? 'The system keeps a record.',
    skill: findKey(parseJson(skills.stdout), 'name') ?? 'provenance',
    operations,
  };
}

/** Time one call: cold, then `count` warm repeats. `prepare` gives each request; `follow` gives the read after a write. */
async function measure(way, label, kind, count, prepare, send, { restart = false, follow = null } = {}) {
  const fail = (step, reason) => skip(way.name, label, `${step}: ${reason}`);
  // Profiling (wip): the wrapper binary reads the row label to choose what it profiles.
  process.env.PROF_LABEL = label;
  try {
    return await measureRow(way, label, kind, count, prepare, send, { restart, follow }, fail);
  } finally {
    process.env.PROF_LABEL = '';
  }
}

async function measureRow(way, label, kind, count, prepare, send, { restart, follow }, fail) {
  let request;
  try {
    request = await prepare(0);
  } catch (error) {
    fail('setup', error instanceof Unresolved ? error.message : String(error));
    return;
  }
  if (restart) {
    await way.stop();
    await way.start();
  }
  const cold = await send(request);
  if (!cold.ok) {
    fail('the cold call failed', describe(cold.error));
    return;
  }
  const after = follow?.(request, cold);
  const afterCold = after ? await way.call(after) : null;
  const warm = [];
  const afterWarm = [];
  for (let index = 1; index <= count; index++) {
    try {
      request = await prepare(index);
    } catch (error) {
      fail(`setup of repeat ${index}`, error instanceof Unresolved ? error.message : String(error));
      return;
    }
    const result = await send(request);
    if (!result.ok) {
      fail(`repeat ${index} failed`, describe(result.error));
      return;
    }
    warm.push(result.ms);
    const next = follow?.(request, result);
    if (next) {
      const read = await way.call(next);
      if (read.ok) {
        afterWarm.push(read.ms);
      }
    }
  }
  calls.push({ way: way.name, call: label, kind, cold: cold.ms, warm });
  if (afterCold?.ok) {
    calls.push({ way: way.name, call: `${label}, then the first read`, kind: 'read after write', cold: afterCold.ms, warm: afterWarm });
  } else if (after) {
    skip(way.name, `${label}, then the first read`, `the read failed: ${describe(afterCold?.error)}`);
  }
  log(`  ${way.name} ${label}: cold ${cold.ms.toFixed(1)} ms`);
}

const selected = label => !filter || label.includes(filter);

async function routeRows(way, planner, operations, readRequests) {
  const restart = way.name !== 'CLI';
  const send = request => way.call(request);
  for (const op of operations.filter(item => !item.write && selected(item.label))) {
    if (op.method !== 'get') {
      await measure(way, op.label, 'read', reads, () => planner.writeRequest(send, op, {}, way.name), send, { restart });
      continue;
    }
    const request = readRequests.get(op.label);
    if (request) {
      await measure(way, op.label, 'read', reads, async () => request, send, { restart });
    }
  }
  for (const op of operations.filter(item => item.write && selected(item.label))) {
    await measure(way, op.label, 'write', writes, () => planner.writeRequest(send, op, {}, way.name), send, {
      restart,
      follow: (request, result) => planner.followRead(request, result),
    });
  }
}

/** The requests of the read verbs `get` (each view), `trace`, `discussions` and `discussion`. */
function readVerbs(ctx, readRequests) {
  const member = readRequests.get('GET /requirements/{id}');
  const discussions = [...readRequests.entries()].find(([label]) => /^GET \/[a-z-]+\/\{id\}\/discussions\/\{discussion_id\}$/.test(label))?.[1];
  const id = member ? decodeURIComponent(member.path.split('/')[2]) : null;
  const parts = discussions?.path.split('/').map(decodeURIComponent) ?? [];
  return { id, owner: parts[2] ?? null, discussion: parts[4] ?? null };
}

async function commandRows(way, planner, ctx, readRequests) {
  const top = await run(bin, ['--help'], { cwd: way.repo });
  const leaves = [];
  for (const name of helpCommands(top.stdout)) {
    const sub = await run(bin, [name, '--help'], { cwd: way.repo });
    const children = helpCommands(sub.stdout);
    leaves.push(...(children.length ? children.map(child => `${name} ${child}`) : [name]));
  }
  const values = { ...ctx, scratch: way.scratch };
  const artifacts = possibleValues((await run(bin, ['validate', '--help'], { cwd: way.repo })).stdout, '<ARTIFACT>');
  for (const artifact of artifacts) {
    const [first] = (await way.call({ method: 'get', path: `/${artifact}s`, query: { limit: 1 } })).data?.items ?? [];
    await writeFile(join(way.scratch, `${artifact}.json`), JSON.stringify(first ?? {}));
  }
  const exported = await exportFile(way.repo, way.scratch);
  for (const leaf of orderLeaves(leaves).filter(item => selected(`provenance ${item}`))) {
    if (NOT_CALLED[leaf]) {
      skip(way.name, `provenance ${leaf}`, NOT_CALLED[leaf]);
      continue;
    }
    if (leaf === 'import' && exported) {
      skip(way.name, `provenance ${leaf}`, exported);
      continue;
    }
    const variants = commandVariants(COMMAND_ARGUMENTS[leaf] ?? [leaf.split(' ')], artifacts);
    for (const variant of variants) {
      const args = fillArguments(variant, values);
      const label = `provenance ${variant.join(' ')}`;
      const count = ['init', 'materialize', 'import'].includes(leaf) ? writes : reads;
      await measure(way, label, 'command', count, async () => args, request => way.command(request));
    }
  }
  const help = await run(bin, ['record_id', '--help'], { cwd: way.repo });
  const { id, owner, discussion } = readVerbs(ctx, readRequests);
  for (const verb of possibleValues(help.stdout, '[ACTION]')) {
    const label = `provenance <id> ${verb}`;
    if (!selected(label)) {
      continue;
    }
    const op = verbOperation(verb, ctx.operations);
    if (op) {
      const send = args => way.command(args);
      await measure(way, `${label} (${op.label})`, 'command', writes, async () => cliWriteArguments(verb, op, await planner.writeRequest(c => way.call(c), op, {}, way.name)), send, {
        follow: () => null,
      });
      continue;
    }
    const target = { get: id, trace: id, discussions: owner, discussion }[verb];
    if (!target) {
      skip(way.name, label, 'no record or discussion to read');
      continue;
    }
    const views = verb === 'get' ? possibleValues(help.stdout, '--view') : [null];
    for (const view of views) {
      const args = [target, verb, '--format', 'json', ...(view ? ['--view', view] : [])];
      await measure(way, view ? `${label} --view ${view}` : label, 'command', reads, async () => args, request => way.command(request));
    }
  }
}

/** Write the export file that import reads. Gives null, or the reason why import has no input. */
async function exportFile(repo, directory) {
  const result = await run(bin, ['export', '--output', join(directory, 'export.json')], { cwd: repo });
  return result.code === 0 ? null : `import needs an export file, and export failed: ${result.stderr.trim().slice(0, 200)}`;
}

/** Arguments for the MCP tools, by tool or tool and action. */
function mcpArguments(call, ctx, readRequests, scratch) {
  const { id, owner, discussion } = readVerbs(ctx, readRequests);
  const table = {
    get: { target: id },
    trace: { target: id },
    search: { text: ctx.word },
    api: { limit: 200 },
    check: {},
    'discussions discussions': { action: 'discussions', target: owner },
    'discussions discussion': { action: 'discussion', target: discussion },
    'change materialize': { action: 'materialize' },
    'change import': { action: 'import', input: join(scratch, 'export.json'), dry_run: true },
  };
  return table[call.action ? `${call.tool} ${call.action}` : call.tool] ?? (call.action ? { action: call.action } : {});
}

async function toolRows(way, planner, ctx, readRequests) {
  const scratch = join(work, 'mcp-scratch');
  await mkdir(scratch, { recursive: true });
  const exported = await exportFile(way.repo, scratch);
  const send = request => way.tool(request.name, request.arguments);
  for (const call of mcpToolActions(await way.tools()).filter(item => selected(item.label))) {
    if (call.action === 'import' && exported) {
      skip(way.name, call.label, exported);
      continue;
    }
    const op = call.tool === 'change' ? verbOperation(call.action, ctx.operations) : null;
    const count = call.readOnly ? reads : writes;
    if (op) {
      await measure(way, `${call.label} (${op.label})`, 'tool', count, async () => ({
        name: call.tool,
        arguments: mcpWriteArguments(call.action, op, await planner.writeRequest(c => way.call(c), op, {}, way.name)),
      }), send, { restart: true });
      continue;
    }
    const args = mcpArguments(call, ctx, readRequests, scratch);
    const variants = call.tool === 'get' ? [null, 'children', 'trace', 'impact', 'review'] : [null];
    for (const view of variants) {
      const label = view ? `${call.label} view=${view}` : call.label;
      await measure(way, label, 'tool', count, async () => ({ name: call.tool, arguments: view ? { ...args, view } : args }), send, { restart: true });
    }
  }
}

let planner;

async function main() {
  await mkdir(work, { recursive: true });
  const planRepo = await clone('plan', null);
  const listing = await run(bin, ['api', '--format', 'json', '--limit', '200'], { cwd: planRepo });
  if (listing.code !== 0) {
    throw new Error(`the catalog listing failed: ${listing.stderr}`);
  }
  const operations = operationsFromCatalog(JSON.parse(listing.stdout), { readOnlyPosts: READ_ONLY_POSTS });
  log(`${operations.length} catalog operations`);
  const http = new HttpWay({ bin, repo: planRepo });
  await http.start();
  const ctx = await context(planRepo, operations, http);
  await git(['config', 'provenance.reviewer', ctx.actor], planRepo);
  planner = new Planner({ operations, context: ctx });
  const readRequests = new Map();
  // The read verbs of the porcelain and the MCP tools use these two reads, so they resolve with any filter.
  const needed = label => label === 'GET /requirements/{id}' || /^GET \/[a-z-]+\/\{id\}\/discussions\/\{discussion_id\}$/.test(label);
  for (const op of operations.filter(item => item.method === 'get' && (selected(item.label) || needed(item.label)))) {
    try {
      readRequests.set(op.label, await planner.read(request => http.call(request), op));
    } catch (error) {
      for (const way of options.ways.split(',')) {
        skip(way, op.label, error instanceof Unresolved ? error.message : String(error));
      }
    }
  }
  await http.stop();
  for (const item of planner.added) {
    log(`  added for ${item.call}: ${item.path}`);
  }
  for (const name of options.ways.split(',')) {
    const repo = join(work, name.toLowerCase());
    await rm(repo, { recursive: true, force: true });
    await cp(planRepo, repo, { recursive: true });
    const Way = { HTTP: HttpWay, MCP: McpWay, CLI: CliWay }[name];
    const way = new Way({ bin, repo });
    log(`${name}: ${repo}`);
    const startup = await way.start();
    if (name !== 'CLI') {
      calls.push({ way: name, call: 'process start', kind: 'start', cold: startup, warm: [] });
    }
    const first = await way.call({ method: 'get', path: '/requirements', query: { limit: 1 } });
    calls.push({ way: name, call: 'first read on a new clone', kind: 'start', cold: first.ms, warm: [] });
    const wayPlanner = new Planner({ operations, context: ctx });
    await routeRows(way, wayPlanner, operations, readRequests);
    if (name === 'CLI') {
      await commandRows(way, wayPlanner, ctx, readRequests);
    }
    if (name === 'MCP') {
      await toolRows(way, wayPlanner, ctx, readRequests);
    }
    await way.stop();
  }
  const commit = await git(['rev-parse', 'HEAD'], planRepo);
  const version = await run(bin, ['--version']);
  const meta = {
    commit: process.env.GITHUB_SHA ?? commit,
    input: commit,
    runner: process.env.RUNNER_DESCRIPTION ?? `${process.platform} ${process.arch}`,
    binary: version.stdout.trim(),
    reads,
    writes,
    added: planner.added,
  };
  await writeFile(options.out, `${JSON.stringify({ meta, calls, unmeasured }, null, 2)}\n`);
  log(`${calls.length} timed rows, ${unmeasured.length} not measured`);
}

await main();
