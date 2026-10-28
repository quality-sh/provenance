import { existsSync, mkdirSync, writeFileSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { randomUUID, randomInt } from 'node:crypto';
import assert from 'node:assert/strict';
import { cli, prepareFixture, readGraph, prepareBinary } from './fixture.mjs';
import { readEvidence, eventEvidence } from './score.mjs';
import * as codex from './adapters/codex.mjs';
import * as claude from './adapters/claude.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const args = process.argv.slice(2);
function option(name, fallback) {
  const index = args.indexOf(name);
  return index < 0 ? fallback : args[index + 1];
}
const output = resolve(option('--output', `/tmp/shaping-eval-${Date.now()}`));
mkdirSync(output, { recursive: true });
const scenario = JSON.parse(readFileSync(join(here, 'scenario.json'), 'utf8'));
const agents = option('--agents', 'codex,claude').split(',').map(name => {
  const adapter = { codex, claude }[name];
  if (!adapter) throw new Error(`Unknown host: ${name}`);
  return adapter;
});
function save(path, value) { writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`); }
function version(name) {
  const result = spawnSync(name, ['--version'], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`${name} version probe failed`);
  return result.stdout.trim();
}

async function probe(adapter, binary) {
  const dir = join(output, `probe-${adapter.name}`), repo = join(dir, 'repo');
  mkdirSync(dir, { recursive: true });
  prepareFixture(binary, repo, scenario, args.includes('--candidate-guidance'));
  const first = await adapter.invoke({ repo, transcript: join(dir, 'first.jsonl'),
    prompt: `Run ${binary} questions question_threshold get --repo ${repo} --scope default --format json. Report the stored resolution_method and include PROBE_OK. Do not change records.` });
  if (!first.answer.includes('PROBE_OK') || !first.answer.includes('research')) throw new Error(`${adapter.name} did not read the public record`);
  const resumed = await adapter.invoke({ repo, session: first.session, transcript: join(dir, 'resumed.jsonl'),
    prompt: 'Which resolution method did that record have? Include RESUME_OK. Do not read files or run a command.' });
  if (!resumed.answer.includes('RESUME_OK') || !resumed.answer.includes('research') || resumed.session !== first.session) throw new Error(`${adapter.name} saved-session resume failed`);
  let compaction;
  if (adapter.name === 'claude') {
    const compacted = await adapter.invoke({ repo, session: first.session, transcript: join(dir, 'compact.jsonl'), prompt: '/compact' });
    assert.ok(compacted.events.some(event => event.type === 'system' && event.subtype === 'compact_boundary'), 'no real compaction event');
    cli(binary, repo, ['topics', 'topic_pay', 'release']);
    cli(binary, repo, ['topics', 'topic_pay', 'claim', '--actor', 'other-worker']);
    const current = cli(binary, repo, ['questions', 'question_threshold', 'get']);
    cli(binary, repo, ['questions', 'question_threshold', 'update', '--if-match', current.data.edit.etag, '--stdin'], { resolution_method: 'verify' });
    const before = readGraph(binary, repo);
    const recovered = await adapter.invoke({ repo, session: first.session, transcript: join(dir, 'compact-recovery.jsonl'),
      prompt: `Resume Topic topic_pay and Question question_threshold from stored state. Establish the stored claim and method before any work. Use ${binary}. Do not change records. Report the stored Topic actor and Question method.` });
    const after = readGraph(binary, repo);
    assert.deepEqual(after, before, 'compaction recovery changed records');
    assert.ok(recovered.answer.includes('other-worker') && recovered.answer.includes('verify'), 'changed claim and method were not recovered');
    const evidence = { case: 'forced_compaction', before, after, answer: recovered.answer, tool_events: eventEvidence(recovered.events) };
    save(join(dir, 'compaction.json'), evidence);
    compaction = { supported: true, evidence: 'compaction.json' };
  } else {
    const compacted = await adapter.invoke({ repo, session: first.session, transcript: join(dir, 'compact-attempt.jsonl'), prompt: '/compact' });
    compaction = { supported: false, detail: 'exec resume /compact produced no compaction event; this protocol has no forced compaction control.' };
    if (compacted.events.some(event => /compact/.test(event.type ?? ''))) throw new Error('New compaction event requires an adapter update');
  }
  const result = { host: adapter.name, runner: version(adapter.name), model: adapter.model,
    protocol: adapter.protocol, session: first.session, transcripts: ['first.jsonl', 'resumed.jsonl'],
    forced_compaction: compaction };
  save(join(dir, 'probe.json'), result);
  return result;
}

async function trial(adapter, build, number, variant) {
  const dir = join(output, `${variant}-${adapter.name}-${number}`), repo = join(dir, 'repo');
  const resuming = args.includes('--resume') && existsSync(repo);
  if (existsSync(repo) && !resuming) throw new Error(`Refusing to reuse trial repository ${repo}`);
  if (resuming && existsSync(join(dir, 'report.json'))) return JSON.parse(readFileSync(join(dir, 'report.json'), 'utf8'));
  mkdirSync(dir, { recursive: true });
  if (!resuming) prepareFixture(build.binary, repo, scenario, variant === 'candidate');
  const turns = [];
  if (resuming) {
    for (let index = 0; existsSync(join(dir, `turn-${index}.json`)); index++) {
      const turn = JSON.parse(readFileSync(join(dir, `turn-${index}.json`), 'utf8'));
      if (turn.prompt !== scenario.turns[index]?.prompt) throw new Error('Owner script changed; cannot resume this trial');
      turns.push(turn);
    }
  }
  let session = turns.at(-1)?.session;
  for (const [index, turn] of scenario.turns.entries()) {
    if (index < turns.length) continue;
    const controller_action = turn.release_claim ? cli(build.binary, repo, ['questions', 'question_delivery', 'release']) : undefined;
    if (turn.restart) {
      if (!existsSync(join(repo, '.scenario/handoff.md'))) throw new Error(`Missing real handoff in ${repo}`);
      session = undefined;
    }
    const before = readGraph(build.binary, repo);
    const prompt = `${turn.prompt}\nUse ${build.binary} for every Provenance command. The repository is ${repo}.`;
    writeFileSync(join(dir, `turn-${index}.prompt.txt`), `${prompt}\n`);
    const result = await adapter.invoke({ repo, session, prompt, transcript: join(dir, `turn-${index}.jsonl`) });
    session = result.session;
    const after = readGraph(build.binary, repo);
    const evidence = { index, restarted: !!turn.restart, prompt: turn.prompt, session, controller_action,
      answer: result.answer, read_commands: readEvidence(result.events), tool_events: eventEvidence(result.events), before, after };
    save(join(dir, `turn-${index}.json`), evidence);
    turns.push(evidence);
    console.log(`${variant} ${adapter.name} trial ${number}: turn ${index + 1}/${scenario.turns.length}`);
  }
  const scored = 'pending blind judgement';
  const missingIds = 'pending blind judgement';
  const report = { host: adapter.name, runner: version(adapter.name), model: adapter.model, build, trial: number,
    variant, scenario: scenario.name, graph_findings: scored, missing_requirement_ids: missingIds,
    missing_accepted_decisions: 'pending blind graph judgement', reminders: 'pending blind judgement',
    method_departures: 'pending blind judgement', forced_compaction: 'not exercised', turns };
  save(join(dir, 'report.json'), report);
  return report;
}

function packets(reports) {
  const shuffled = [...reports];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = randomInt(i + 1); [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  const dir = join(output, 'blind-packets'); rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true });
  const mapping = [];
  for (const report of shuffled) {
    const id = randomUUID();
    mapping.push({ packet: id, host: report.host, model: report.model, trial: report.trial, variant: report.variant });
    const turns = report.turns.map(({ session, ...turn }) => turn);
    // Paths reveal run labels and are irrelevant to the graph and dialogue obligations.
    const text = JSON.stringify({ packet: id, scenario: scenario.name, expected_decisions: scenario.accepted_decisions,
      refused_decisions: scenario.refused_decisions, questions: scenario.questions, dependencies: scenario.dependencies, turns }, null, 2);
    const scrubbed = text.replaceAll(report.build.binary, '<provenance>').replaceAll(output, '<run-root>')
      .replaceAll(`${report.variant}-${report.host}-${report.trial}`, '<trial>');
    writeFileSync(join(dir, `packet-${id}.json`), `${scrubbed}\n`);
  }
  save(join(output, 'packet-mapping.json'), mapping);
  writeFileSync(join(dir, 'rubric.md'), `# Blind shaping evaluation\n\nScore each packet without inferring its host or variant. Return the packet ID and evidence for each finding.\n\n1. Missing accepted decisions: compare the expected facts with the final graph only. A stored Requirement, Resolution position, or Question answer must state the accepted fact. An ID alone is not evidence. Give one count per absent fact. Do not use dialogue promises as saved state.\n2. Prerequisite violations: read graph snapshots and the command order. Count each selection or resolution before all explicit prerequisites are answered. Use the independent dependency list. A claim conflict permits discovery but prevents work. Resolve same-turn order from command evidence; report unknown order separately.\n3. Reminders: count owner reminders that return the session to shaping or its method. Separate scheduled handoff instructions and ordinary answers from corrective reminders.\n4. Method departures: compare each selected Question's stored method with the actual work. Count departures with turn numbers and evidence. Do not treat Topic ownership as a Question method.\n5. Fresh reads: for each turn, check that the stored Topic claim and selected Question method were read before work. A fresh complete Topic summary can supply method and claim data. A handoff, prior dialogue, or unchecked read cannot replace stored recovery reads. Report missing reads separately.\n6. Refused decisions: check the final graph for the two refused ideas. A record of refusal is permitted. An obligation that adopts a refused idea is a failure.\n\nReturn uncertain cases separately. Structural findings are review aids, not the final semantic scores. Missing accepted decisions and prerequisite violations require zero in every candidate trial. Reminder and departure thresholds require owner approval after this baseline. Forced compaction is not part of these packets.\n`);
  return dir;
}

function mixedPackets(baseline, candidate, compaction) {
  const entries = [];
  for (const directory of [baseline, candidate]) {
    const mapping = JSON.parse(readFileSync(join(directory, 'packet-mapping.json'), 'utf8'));
    for (const key of mapping) {
      entries.push({ key, packet: JSON.parse(readFileSync(join(directory, 'blind-packets', `packet-${key.packet}.json`), 'utf8')) });
    }
  }
  const evidence = JSON.parse(readFileSync(compaction, 'utf8'));
  const compactEvent = readFileSync(join(dirname(compaction), 'compact.jsonl'), 'utf8').trim().split('\n')
    .map(line => JSON.parse(line)).find(event => event.type === 'system' && event.subtype === 'compact_boundary');
  if (!compactEvent) throw new Error('Missing retained manual compaction event');
  entries.push({ key: { variant: 'candidate', host: 'claude', case: 'forced_compaction' }, packet: {
    case: 'forced_compaction', scenario: scenario.name, expected_decisions: [], refused_decisions: [],
    dependencies: scenario.dependencies, questions: scenario.questions,
    compaction_event: { type: compactEvent.type, subtype: compactEvent.subtype, compact_metadata: compactEvent.compact_metadata },
    turns: [{ index: 0, prompt: 'Resume Topic topic_pay and Question question_threshold from stored state. Establish the stored claim and method before any work. Do not change records. Report the stored Topic actor and Question method.',
      before: evidence.before, after: evidence.after, answer: evidence.answer, tool_events: evidence.tool_events }],
  } });
  if (entries.length !== 14) throw new Error('Expected twelve trials and two compaction cases');
  for (let index = entries.length - 1; index > 0; index--) {
    const other = randomInt(index + 1); [entries[index], entries[other]] = [entries[other], entries[index]];
  }
  const directory = join(output, 'blind-packets');
  mkdirSync(directory, { recursive: true });
  if (readdirSync(directory).length) throw new Error('Mixed packet destination must be empty');
  const mapping = [];
  for (const entry of entries) {
    const id = randomUUID();
    mapping.push({ ...entry.key, original_packet: entry.key.packet, packet: id });
    entry.packet.packet = id;
    let content = JSON.stringify(entry.packet, null, 2);
    content = content.replace(/\/home\/ben\/[^\s"\\]+/g, '<local-path>')
      .replace(/\/tmp\/[^\s"\\]+/g, '<local-path>')
      .replace(/gpt-6-astra|claude-opus-5-5\[1m\]|codex|claude|baseline|candidate/gi, '<identity>')
      .replace(/"name": "Bash"/g, '"name": "shell"')
      .replace(/"name": "Read"/g, '"name": "read_file"')
      .replace(/"name": "Write"/g, '"name": "write_file"')
      .replace(/"name": "Edit"/g, '"name": "edit_file"')
      .replace(/"name": "Glob"/g, '"name": "find_files"')
      .replace(/"name": "Grep"/g, '"name": "search_files"');
    writeFileSync(join(directory, `packet-${id}.json`), `${content}\n`);
  }
  save(join(output, 'packet-mapping.json'), mapping);
  const rubric = readFileSync(join(baseline, 'blind-packets/rubric.md'), 'utf8')
    .replace('Reminder and departure thresholds require owner approval after this baseline. Forced compaction is not part of these packets.', 'The owner gate below applies to this mixed set.')
    .replace('Keep its result separate from the six long-conversation trials. Its candidate counterpart remains required.', 'Keep both compaction results separate from the twelve conversation trials.');
  writeFileSync(join(directory, 'rubric.md'), `${rubric}\nOwner gate, 2026-10-12: require zero missed fresh Topic claim and Question method reads before every work turn, with unfiltered output; zero missing accepted decisions, prerequisite violations, and corrective reminders. Method departures must not increase. A direct owner choice can settle a prototype Question when the stored answer explains why. Compare all anonymous packets with this same rule. Do not infer identities. Report note-only turns separately. Score the two forced-compaction cases separately.\n`);
  console.log(`Mixed blind packets: ${directory}`);
}

try {
  if (args.includes('--mix-with')) {
    mixedPackets(resolve(option('--mix-with')), resolve(option('--candidate-packets')), resolve(option('--compaction-evidence')));
  } else if (args.includes('--packets-only')) {
    const reports = readdirSync(output, { withFileTypes: true })
      .filter(entry => entry.isDirectory() && existsSync(join(output, entry.name, 'report.json')))
      .map(entry => {
        const dir = join(output, entry.name);
        const report = JSON.parse(readFileSync(join(dir, 'report.json'), 'utf8'));
        for (const turn of report.turns) {
          const events = readFileSync(join(dir, `turn-${turn.index}.jsonl`), 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
          turn.tool_events = eventEvidence(events);
        }
        return report;
      });
    if (!reports.length) throw new Error('No completed trial reports for blind packets');
    console.log(`Blind packets: ${packets(reports)}`);
  } else if (args.includes('--probe')) {
    const binary = option('--cli', process.env.SHAPING_EVAL_CLI ?? 'provenance');
    const results = [];
    for (const adapter of agents) results.push(await probe(adapter, binary));
    save(join(output, 'probe.json'), results);
    console.log(`Probe passed; evidence: ${output}`);
  } else {
    const trials = Number(option('--trials', '3'));
    if (!Number.isInteger(trials) || trials < 1) throw new Error('--trials must be a positive integer');
    const base = prepareBinary(option('--base', 'd981e0bd10f00900772bfc49544caa43d17f3154'), option('--base-cli'), root);
    const candidate = prepareBinary(option('--candidate', 'HEAD'), option('--candidate-cli'), root);
    save(join(output, 'builds.json'), { base, candidate });
    for (const adapter of agents) {
      const saved = join(output, `probe-${adapter.name}`, 'probe.json');
      if (args.includes('--resume') && existsSync(saved)) {
        const record = JSON.parse(readFileSync(saved, 'utf8'));
        if (record.model !== adapter.model || record.runner !== version(adapter.name)) throw new Error('Runner identity changed; cannot resume');
      } else await probe(adapter, base.binary);
    }
    const stage = option('--stage', 'baseline');
    if (!['baseline', 'candidate'].includes(stage)) throw new Error('--stage must be baseline or candidate');
    if (stage === 'candidate' && !option('--approved-thresholds')) throw new Error('Candidate trials require --approved-thresholds after baseline approval');
    if (stage === 'candidate') {
      const thresholds = JSON.parse(readFileSync(option('--approved-thresholds'), 'utf8'));
      if (!thresholds.approval || !Number.isInteger(thresholds.reminders) || !Number.isInteger(thresholds.method_departures)) throw new Error('Missing owner threshold approval');
      save(join(output, 'thresholds.json'), thresholds);
    }
    const reports = [];
    // Separate repositories and sessions let the hosts run independently.
    await Promise.all(agents.map(async adapter => {
      for (let number = 1; number <= trials; number++) reports.push(await trial(adapter, stage === 'baseline' ? base : candidate, number, stage));
    }));
    const destination = packets(reports);
    save(join(output, 'checkpoint.json'), { stage, trials, blind_packets: destination,
      remaining: stage === 'baseline' ? 'Blind scores and owner threshold approval; then paired candidate trials.' : 'Blind scores and final release gates.' });
    console.log(`Checkpoint: publish ${destination} alone, without packet-mapping.json, for blind scoring. Stop after publication.`);
  }
} catch (error) {
  save(join(output, 'stop.json'), { error: error.message, evidence: output });
  console.error(error.message);
  process.exitCode = 1;
}
