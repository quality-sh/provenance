/** Structural gate cases. Each one reads one anonymous packet and returns its findings. */

// A write is a CLI call whose action follows the binary or its shell variable within three words,
// as in `$P questions question_rate answer`; script text that names an action is not a write.
const writeVerb = /(?:provenance>?|\$\{?\w+\}?)\s+(?:[\w.$<>{}-]+\s+){0,3}(create|update|answer|claim|release|submit|close|discuss|reply)\b/;
const outputFilter = /\|\s*(grep|rg|jq|sed|head|tail|awk|cut|sort|wc|tr|python3?|node)\b/;

// A read counts only in a call with no write, and only in a segment whose output is not filtered.
// The segment names the record, or reads it through a loop variable such as `for q in ...; do $P questions $q get`.
function freshRead(command, id) {
  if (writeVerb.test(command) || !command.includes(id)) return false;
  return command.split(/\n|;|&&|\|\|/).some(segment => /\bget\b/.test(segment) && !outputFilter.test(segment)
    && (segment.includes(id) || (/\$\{?\w+\}?\s+get\b/.test(segment) && new RegExp(`\\bin\\b[^;]*\\b${id}\\b`).test(command))));
}

function shellCommands(turn) {
  const out = [];
  for (const event of turn.tool_events ?? []) {
    if (event.type === 'shell') out.push(event.command);
    if (event.type === 'tool_call' && ['Bash', 'shell'].includes(event.name)) out.push(event.input?.command ?? '');
  }
  return out;
}

const commands = turn => shellCommands(turn).filter(command => /provenance/.test(command));

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
    const firstWrite = list.findIndex(command => writeVerb.test(command));
    if (firstWrite < 0) continue;
    if (!list.slice(0, firstWrite).some(command => freshRead(command, 'topic_pay'))) findings.push({ turn: turn.index, missing: 'topic_pay' });
    const before = byId(turn.before.questions);
    for (const question of turn.after.questions) {
      const prior = before.get(question.id);
      if (!prior || JSON.stringify(prior) === JSON.stringify(question)) continue;
      const writeAt = list.findIndex(command => writeVerb.test(command) && command.includes(question.id));
      const until = writeAt < 0 ? firstWrite : writeAt;
      const read = list.slice(0, until).some(command => freshRead(command, question.id)
        || (freshRead(command, 'topic_pay') && command.includes('--view children')));
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

/** Failure 2: no new Rule is directly under the anchor, and each choice is a Resolution that spawns its Requirement. */
export function treeBranchesByDecision(packet) {
  const final = packet.turns.at(-1).after;
  const fixture = new Set(packet.fixture_records);
  const findings = [];
  for (const rule of final.rules.filter(live)) {
    if (!fixture.has(rule.id) && (rule.requirement_ids ?? []).includes(packet.anchor)) findings.push({ rule: rule.id, finding: 'Rule directly under the anchor' });
  }
  const requirements = byId(final.requirements), resolutions = byId(final.resolutions);
  const inside = subtree(final.requirements, packet.anchor);
  for (const decision of packet.expected_decisions) {
    const record = requirements.get(decision.id);
    if (!record) continue;
    if (!inside.has(record.id) || record.id === packet.anchor) findings.push({ requirement: record.id, finding: 'not below the anchor' });
    if (decision.choice && !resolutions.has(record.spawned_by)) findings.push({ requirement: record.id, finding: 'choice not spawned by a Resolution' });
  }
  return findings;
}

/** Failure 3: every Resolution of the session shows a spawned Requirement and a produced Rule while it is proposed. */
export function resolutionShowsDownstream(packet) {
  const fixture = new Set(packet.fixture_records);
  const live = new Set(packet.turns.at(-1).after.resolutions.filter(record => !['archived', 'abandoned'].includes(record.status)).map(record => record.id));
  return (packet.turns.at(-1).review ?? [])
    .filter(entry => entry.kind === 'resolutions' && !fixture.has(entry.id) && live.has(entry.id))
    .filter(entry => !entry.impact_rules.length || !entry.spawned.length)
    .map(entry => ({ resolution: entry.id, impact_rules: entry.impact_rules, spawned: entry.spawned }));
}

/** Failure 4: every record that the session created has its own review submission. */
export function everyRecordSubmitted(packet) {
  const fixture = new Set(packet.fixture_records);
  const review = packet.turns.at(-1).review;
  if (!review) return [{ finding: 'no final review state' }];
  return review.filter(entry => !fixture.has(entry.id) && !entry.pending && !entry.decisions).map(entry => ({ kind: entry.kind, id: entry.id }));
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
