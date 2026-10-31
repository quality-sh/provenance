/** Structural gate cases. Each one reads one anonymous packet and returns its findings. */

// The scorer splits each shell command into pipelines of words, outside quotes. Heredoc bodies
// and comments are script text, not calls. `$(`, backticks and parentheses also start a new pipeline,
// also inside double quotes. `nested` holds the open substitutions and the quote that each one ends in.
export function pipelines(command) {
  const out = [];
  let stages = [], words = [], word = null, quote = null, heredocs = [];
  const nested = [];
  const endWord = () => { if (word) words.push(word); word = null; };
  const endStage = () => { endWord(); if (words.length) stages.push(words); words = []; };
  const endPipeline = () => { endStage(); if (stages.length) out.push(stages); stages = []; };
  const append = (text, quoted = false) => { word ??= { text: '', quoted: false }; word.text += text; word.quoted ||= quoted; };
  for (let i = 0; i < command.length; i++) {
    const c = command[i], next = command[i + 1];
    if (quote === '"' && (c === '`' || (c === '$' && next === '('))) {
      endPipeline(); nested.push({ close: c === '`' ? '`' : ')', quote }); quote = null;
      if (c === '$') i++;
      continue;
    }
    if (quote) {
      if (c === quote) quote = null;
      else if (c === '\\' && quote === '"' && next !== undefined) append(command[++i], true);
      else append(c, true);
      continue;
    }
    if (c === "'" || c === '"') { quote = c; append('', true); continue; }
    if (c === '\\') { if (next !== '\n' && next !== undefined) append(next); i++; continue; }
    if (c === '#' && !word) { while (i + 1 < command.length && command[i + 1] !== '\n') i++; continue; }
    if (c === '<' && next === '<' && command[i + 2] !== '<') {
      const match = /^<<-?\s*(['"]?)([\w.-]+)\1/.exec(command.slice(i));
      if (match) { heredocs.push(match[2]); i += match[0].length - 1; continue; }
    }
    if (c === '\n') {
      endPipeline();
      for (const end of heredocs) {
        let line;
        do {
          const stop = command.indexOf('\n', i + 1);
          line = command.slice(i + 1, stop < 0 ? command.length : stop);
          i = stop < 0 ? command.length : stop;
        } while (i < command.length && line.trim() !== end);
      }
      heredocs = [];
      continue;
    }
    if (c === '|' && next !== '|') { endStage(); if (next === '&') i++; continue; }
    if (c === '&' && (next === '>' || command[i - 1] === '>')) { append(c); continue; }
    if (c === ')' || c === '`') {
      endPipeline();
      const open = nested.at(-1);
      if (open?.close === c) { nested.pop(); quote = open.quote; append('', !!quote); }
      else if (c === '`') nested.push({ close: c, quote: null });
      continue;
    }
    if (c === '(' || (c === '$' && next === '(')) { endPipeline(); nested.push({ close: ')', quote: null }); if (c === '$') i++; continue; }
    if (';&|'.includes(c)) { endPipeline(); if (next === c) i++; continue; }
    if (/\s/.test(c)) { endWord(); continue; }
    append(c);
  }
  endPipeline();
  return out;
}

const keyword = /^(do|then|else|elif|if|while|until|!|\{)$/;
// A prefix runs the next word as a program. These options of a prefix take a value word.
const prefix = /^(env|command|exec|time|nohup|xargs)$/;
const prefixValue = /^-([nILPdsaE]|-max-args|-replace|-max-lines|-max-procs|-delimiter|-arg-file|-eof|u|C|S|-unset|-chdir|-split-string)$/;
const program = /^(?:\S*\/)?<?provenance>?$|^\$\{?\w+\}?$/;
const writeVerbs = new Set(['create', 'update', 'answer', 'claim', 'release', 'submit', 'close', 'discuss', 'reply']);
const outputFilter = /^(grep|rg|jq|sed|head|tail|awk|cut|sort|wc|tr|python3?|node)$/;

// A CLI call is one pipeline stage that runs the binary or a shell variable that holds it, as in
// `$P questions question_rate answer`, also after a prefix such as `env` or `xargs`. Its action is the first write verb or `get` in the first
// four words that are not options. An option is an unquoted word, so an answer text that contains
// `--help` is not a help request.
function invocations(command) {
  const loops = new Map();
  const calls = [];
  for (const stages of pipelines(command)) {
    stages.forEach((stage, at) => {
      let start = 0, prefixed = false;
      while (start < stage.length) {
        const text = stage[start].text;
        if (keyword.test(text) || /^\w+=/.test(text)) start++;
        else if (prefix.test(text)) { prefixed = true; start++; }
        else if (prefixed && text.startsWith('-')) start += prefixValue.test(text) ? 2 : 1;
        else break;
      }
      const [head, ...rest] = stage.slice(start);
      if (head?.text === 'for' && rest[1]?.text === 'in') loops.set(rest[0].text, rest.slice(2).map(word => word.text));
      if (!head || !program.test(head.text)) return;
      const options = rest.filter(word => !word.quoted && word.text.startsWith('-')).map(word => word.text);
      const positional = rest.filter(word => word.quoted || !word.text.startsWith('-')).map(word => word.text).slice(0, 4);
      const help = options.some(option => option === '--help' || option === '-h') || positional[0] === 'help';
      const action = positional.find(word => writeVerbs.has(word) || word === 'get');
      const filtered = stages.slice(at + 1).some(later => outputFilter.test(later[0]?.text ?? ''));
      calls.push({ words: rest.map(word => word.text), positional, options, action, help, filtered });
    });
  }
  return calls.map(call => ({ ...call, write: !call.help && writeVerbs.has(call.action), read: !call.help && call.action === 'get', loops }));
}

// A call names a record by its ID, or by a loop variable such as `for q in ...; do $P questions $q get`.
const names = (call, id) => call.words.includes(id)
  || call.positional.some(word => (call.loops.get(/^\$\{?(\w+)\}?$/.exec(word)?.[1]) ?? []).includes(id));
const isWrite = command => invocations(command).some(call => call.write);
const writes = (command, id) => invocations(command).some(call => call.write && names(call, id));

// A read counts only in a command with no write, and only in a call whose output is not filtered.
function freshRead(command, id, view) {
  const calls = invocations(command);
  return !calls.some(call => call.write)
    && calls.some(call => call.read && !call.filtered && names(call, id) && (!view || view.test(call.words.join(' '))));
}

function shellCommands(turn) {
  const out = [];
  for (const event of turn.tool_events ?? []) {
    if (event.type === 'shell') out.push(event.command);
    if (event.type === 'tool_call' && ['Bash', 'shell'].includes(event.name)) out.push(event.input?.command ?? '');
  }
  return out;
}

const commands = turn => shellCommands(turn).filter(command => invocations(command).length);

const live = record => !['archived', 'abandoned', 'rejected', 'superseded'].includes(record.status);
const byId = list => new Map(list.map(record => [record.id, record]));

/** A Question claimed or answered while an oracle prerequisite is still open at the end of the turn. */
export function prerequisiteOrder(packet) {
  const findings = [];
  for (const turn of packet.turns) {
    const before = byId(turn.before.questions), after = byId(turn.after.questions);
    for (const question of after.values()) {
      const prior = before.get(question.id);
      const claimed = question.claimed_by && question.claimed_by !== prior?.claimed_by && question.claimed_by !== 'other-worker';
      const answered = question.status === 'answered' && prior?.status !== 'answered';
      if (!claimed && !answered) continue;
      for (const prerequisite of packet.dependencies[question.id] ?? []) {
        if (after.get(prerequisite)?.status !== 'answered') {
          findings.push({ turn: turn.index, question: question.id, action: answered ? 'answered' : 'claimed', open_prerequisite: prerequisite });
        }
      }
    }
  }
  return findings;
}

/** Each accepted fact is stored under its Requirement ID and states the fact. */
export function acceptedDecisions(packet) {
  const final = byId(packet.turns.at(-1).after.requirements);
  return packet.expected_decisions.flatMap(decision => {
    const record = final.get(decision.id);
    const text = `${record?.statement ?? ''} ${record?.description ?? ''}`;
    if (!record || !live(record)) return [{ decision: decision.id, finding: 'missing' }];
    return decision.terms.every(term => text.includes(term)) ? [] : [{ decision: decision.id, finding: 'does not state the fact', statement: record.statement }];
  });
}

/** A refused idea appears only as a refusal, never as an obligation or a selected position. */
export function refusedDecisions(packet) {
  const final = packet.turns.at(-1).after;
  const negation = /\b(not|no|never|without|only|reject\w*|exclud\w*|refus\w*)\b/i;
  const texts = [
    ...final.requirements.filter(live).map(record => [record.id, record.statement]),
    ...final.rules.filter(live).map(record => [record.id, record.statement]),
    ...final.resolutions.filter(live).map(record => [record.id, record.position]),
  ];
  return packet.refused_terms.flatMap(terms => texts
    .filter(([, text]) => terms.every(term => text?.toLowerCase().includes(term)) && !negation.test(text))
    .map(([id, text]) => ({ id, text })));
}

/** A work turn reads the Topic and each changed Question in a read-only, unfiltered command before it writes. */
export function freshReads(packet) {
  const findings = [];
  for (const turn of packet.turns) {
    const list = commands(turn);
    const firstWrite = list.findIndex(command => isWrite(command));
    if (firstWrite < 0) continue;
    if (!list.slice(0, firstWrite).some(command => freshRead(command, 'topic_pay'))) findings.push({ turn: turn.index, missing: 'topic_pay' });
    const before = byId(turn.before.questions);
    for (const question of turn.after.questions) {
      const prior = before.get(question.id);
      if (!prior || JSON.stringify(prior) === JSON.stringify(question)) continue;
      const writeAt = list.findIndex(command => writes(command, question.id));
      const until = writeAt < 0 ? firstWrite : writeAt;
      const read = list.slice(0, until).some(command => freshRead(command, question.id)
        || freshRead(command, 'topic_pay', /--view[= ]children\b/));
      if (!read) findings.push({ turn: turn.index, missing: question.id });
    }
  }
  return findings;
}

/** Failure 1: the requirements view opens in the first turn, before the first question, and its link is in the reply. */
export function viewBeforeFirstQuestion(packet) {
  const first = packet.turns[0];
  const findings = [];
  if (!first.opened_pages?.length) findings.push({ turn: 0, finding: 'no page opened before the first question' });
  if (!/http:\/\/127\.0\.0\.1:\d+/.test(first.answer ?? '')) findings.push({ turn: 0, finding: 'no page link in the first reply' });
  // Codex exec ends the processes of a turn when the turn ends, so a stopped host is not evidence;
  // a command that stops the host is.
  for (const turn of packet.turns) {
    for (const command of shellCommands(turn)) {
      if (/\b(p?kill|killall)\b/.test(command)) findings.push({ turn: turn.index, finding: 'the agent stopped the review host', command });
    }
  }
  return findings;
}

/** Failure 5: the first view shows the anchor inside its parent's tree, and no view uses the anchor as its root. */
export function anchorInsideParentTree(packet) {
  const findings = [];
  if (!(packet.turns[0].opened_pages ?? []).some(page => page.root === packet.anchor_parent)) {
    findings.push({ turn: 0, finding: `no page opened with root ${packet.anchor_parent}` });
  }
  for (const turn of packet.turns) {
    for (const page of turn.opened_pages ?? []) {
      if (page.root === packet.anchor) findings.push({ turn: turn.index, finding: 'page opened with the anchor as root', url: page.url });
    }
  }
  return findings;
}

function subtree(requirements, anchor) {
  const inside = new Set([anchor]);
  for (let grew = true; grew;) {
    grew = false;
    for (const record of requirements) {
      if (!inside.has(record.id) && (record.refines ?? []).some(parent => inside.has(parent))) { inside.add(record.id); grew = true; }
    }
  }
  return inside;
}

/** Failure 2: each new Rule is under a Requirement below the anchor, and names the Resolution that
 * spawned that Requirement. Each choice is a Resolution that spawns its Requirement. */
export function treeBranchesByDecision(packet) {
  const final = packet.turns.at(-1).after;
  const fixture = new Set(packet.fixture_records);
  const requirements = byId(final.requirements), resolutions = byId(final.resolutions);
  const inside = subtree(final.requirements, packet.anchor);
  inside.delete(packet.anchor);
  const findings = [];
  for (const rule of final.rules.filter(record => live(record) && !fixture.has(record.id))) {
    const parents = rule.requirement_ids ?? [];
    if (parents.includes(packet.anchor)) findings.push({ rule: rule.id, finding: 'Rule directly under the anchor' });
    else if (!parents.length || !parents.every(id => inside.has(id))) findings.push({ rule: rule.id, finding: 'Rule outside the anchor tree' });
    for (const resolution of new Set(parents.map(id => requirements.get(id)?.spawned_by).filter(Boolean))) {
      if (!(rule.resolution_ids ?? []).includes(resolution)) findings.push({ rule: rule.id, finding: `Rule does not name ${resolution}` });
    }
    for (const resolution of rule.resolution_ids ?? []) {
      if (!resolutions.has(resolution)) findings.push({ rule: rule.id, finding: `Rule names the missing Resolution ${resolution}` });
    }
  }
  for (const decision of packet.expected_decisions) {
    const record = requirements.get(decision.id);
    if (!record) continue;
    if (!inside.has(record.id)) findings.push({ requirement: record.id, finding: 'not below the anchor' });
    if (decision.choice && !resolutions.has(record.spawned_by)) findings.push({ requirement: record.id, finding: 'choice not spawned by a Resolution' });
  }
  return findings;
}

// The records that the session created, by default only the live ones.
const created = (packet, kinds, keep = live) => {
  const fixture = new Set(packet.fixture_records);
  const final = packet.turns.at(-1).after;
  return kinds.flatMap(kind => (final[kind] ?? []).filter(record => keep(record) && !fixture.has(record.id)).map(record => ({ kind, id: record.id })));
};
const reviewEntry = (review, { kind, id }) => review.find(entry => entry.kind === kind && entry.id === id);

/** Failure 3: every new Resolution has a review entry that shows a spawned Requirement and a produced Rule. */
export function resolutionShowsDownstream(packet) {
  const review = packet.turns.at(-1).review ?? [];
  return created(packet, ['resolutions']).flatMap(record => {
    const entry = reviewEntry(review, record);
    if (!entry) return [{ resolution: record.id, finding: 'no review entry' }];
    return entry.impact_rules?.length && entry.spawned?.length ? [] : [{ resolution: record.id, impact_rules: entry.impact_rules, spawned: entry.spawned }];
  });
}

/** Failure 4: every record that the session created has its own review submission, whatever its status. */
export function everyRecordSubmitted(packet) {
  const review = packet.turns.at(-1).review;
  if (!review) return [{ finding: 'no final review state' }];
  return created(packet, ['requirements', 'resolutions', 'rules', 'boundaries', 'sources'], () => true)
    .filter(record => { const entry = reviewEntry(review, record); return !entry?.pending && !entry?.decisions; });
}

export const cases = {
  prerequisite_order: prerequisiteOrder,
  accepted_decisions: acceptedDecisions,
  refused_decisions: refusedDecisions,
  fresh_reads: freshReads,
  view_before_first_question: viewBeforeFirstQuestion,
  anchor_inside_parent_tree: anchorInsideParentTree,
  tree_branches_by_decision: treeBranchesByDecision,
  resolution_shows_downstream: resolutionShowsDownstream,
  every_record_submitted: everyRecordSubmitted,
};

export function scorePacket(packet) {
  const results = Object.fromEntries(Object.entries(cases).map(([name, check]) => [name, check(packet)]));
  return { packet: packet.packet, pass: Object.values(results).every(findings => !findings.length), cases: results };
}
