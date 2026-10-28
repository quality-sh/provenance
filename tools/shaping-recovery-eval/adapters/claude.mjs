import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

export const name = 'claude';
export const model = process.env.SHAPING_CLAUDE_MODEL ?? 'claude-opus-5-5[1m]';
export const protocol = 'print stream-json; saved session resumed by --resume';
export const forcedCompaction = 'Print /compact emits a manual compact_boundary event; recovery is a separate case.';

export async function invoke({ repo, prompt, session, transcript, timeoutMs = 600_000 }) {
  const args = ['--print', '--output-format', 'stream-json', '--verbose', '--model', model,
    '--permission-mode', 'acceptEdits', '--allowedTools', 'Bash', 'Read', 'Write', 'Edit', 'Glob', 'Grep',
    ...(session ? ['--resume', session] : []), '--', prompt];
  const process = spawn('claude', args, { cwd: repo, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  process.stdout.on('data', chunk => { stdout += chunk; });
  process.stderr.on('data', chunk => { stderr += chunk; });
  const timer = setTimeout(() => process.kill('SIGTERM'), timeoutMs);
  const code = await new Promise((resolve, reject) => { process.on('error', reject); process.on('close', resolve); });
  clearTimeout(timer);
  writeFileSync(transcript, stdout);
  writeFileSync(`${transcript}.stderr`, stderr);
  const events = stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  const result = events.findLast(event => event.type === 'result');
  if (code !== 0 || !result || result.is_error) throw new Error(`Claude turn failed; see ${transcript}`);
  const id = result.session_id;
  if (!id) throw new Error('Claude did not return a session ID');
  const actual = events.find(event => event.type === 'system' && event.subtype === 'init')?.model;
  if (actual && actual !== model) throw new Error(`Claude model changed: ${actual}`);
  return { session: id, events, answer: result.result, actualModel: actual ?? model };
}
