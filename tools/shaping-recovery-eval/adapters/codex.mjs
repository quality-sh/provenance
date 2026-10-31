import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

export const name = 'codex';
export const model = process.env.SHAPING_CODEX_MODEL ?? 'gpt-6-astra';
export const protocol = 'exec JSONL; saved thread resumed by exec resume';
export const forcedCompaction = 'No forced compaction command is exposed by exec help.';

export async function invoke({ repo, prompt, session, transcript, env, cache, timeoutMs = 600_000 }) {
  // The review host listens on loopback and keeps its launch key in the trial's user cache.
  const sandbox = cache ? ['-c', 'sandbox_workspace_write.network_access=true',
    '-c', `sandbox_workspace_write.writable_roots=${JSON.stringify([cache])}`] : [];
  const args = ['-a', 'never', 'exec', ...(session ? ['resume'] : []), '--json', '--ignore-rules',
    '--skip-git-repo-check', '-m', model, ...sandbox, ...(session ? [session] : ['-s', 'workspace-write', '-C', repo]), prompt];
  const process = spawn('codex', args, { cwd: repo, env, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  process.stdout.on('data', chunk => { stdout += chunk; });
  process.stderr.on('data', chunk => { stderr += chunk; });
  const timer = setTimeout(() => process.kill('SIGTERM'), timeoutMs);
  const code = await new Promise((resolve, reject) => { process.on('error', reject); process.on('close', resolve); });
  clearTimeout(timer);
  writeFileSync(transcript, stdout);
  writeFileSync(`${transcript}.stderr`, stderr);
  const events = stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  if (code !== 0 || !events.some(event => event.type === 'turn.completed')) throw new Error(`Codex turn failed; see ${transcript}`);
  const id = events.find(event => event.type === 'thread.started')?.thread_id ?? session;
  if (!id) throw new Error('Codex did not return a thread ID');
  return { session: id, events, answer: events.filter(event => event.item?.type === 'agent_message').map(event => event.item.text).join('\n') };
}
