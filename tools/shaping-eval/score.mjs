import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

/** Evaluate saved Question states with an independent prerequisite oracle. */
export function scoreGraph(turns, scenario) {
  const prerequisite_violations = [];
  for (const [turn, { before, after }] of turns.entries()) {
    const prior = new Map(before.map(question => [question.id, question]));
    for (const question of after) {
      if (question.status !== 'answered' || prior.get(question.id)?.status === 'answered') continue;
      for (const prerequisite of scenario.dependencies[question.id] ?? []) {
        if (prior.get(prerequisite)?.status !== 'answered') {
          prerequisite_violations.push({ turn, question: question.id, prerequisite });
        }
      }
    }
  }
  const last = new Map((turns.at(-1)?.after ?? []).map(question => [question.id, question]));
  return {
    prerequisite_violations,
    unanswered_accepted_questions: scenario.accepted.filter(id => last.get(id)?.status !== 'answered' || !last.get(id)?.answer),
    within_turn_order_requires_judge: true,
  };
}

/** Commands are evidence of fresh reads; selection and method compliance need a blind judge. */
export function readEvidence(events) {
  const commands = [];
  for (const event of events) {
    if (event.type === 'item.completed' && event.item?.type === 'command_execution') commands.push(event.item.command);
    for (const block of event.message?.content ?? []) {
      if (block.type === 'tool_use' && block.name === 'Bash') commands.push(block.input.command);
    }
  }
  return commands.filter(command => /provenance/.test(command) && /\b(get|list|api)\b/.test(command));
}

/** Keep command and tool results for the blind order and method checks. */
export function eventEvidence(events) {
  const evidence = [];
  for (const event of events) {
    if (event.type === 'item.completed' && event.item?.type === 'command_execution') {
      evidence.push({ type: 'shell', command: event.item.command, output: event.item.aggregated_output, exit_code: event.item.exit_code });
    }
    for (const block of event.message?.content ?? []) {
      if (block.type === 'tool_use') evidence.push({ type: 'tool_call', id: block.id, name: block.name, input: block.input });
      if (block.type === 'tool_result') evidence.push({ type: 'tool_result', id: block.tool_use_id, output: block.content, is_error: block.is_error });
    }
  }
  return evidence;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (!process.argv.includes('--stdin')) throw new Error('Use --stdin with a JSON object containing turns and scenario');
  const input = JSON.parse(readFileSync(0, 'utf8'));
  process.stdout.write(`${JSON.stringify(scoreGraph(input.turns, input.scenario))}\n`);
}
