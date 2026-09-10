import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { mkdtemp, readFile, writeFile, mkdir, appendFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { chromium, expect } from '@playwright/test';
import { HttpClient } from '../../packages/provenance/dist/client.js';
import type { components } from '../../packages/provenance/dist/client.js';

type Thread = components['schemas']['ListThreadsSuccessOutputThread'];
type Message = components['schemas']['ListMessagesSuccessOutputMessage'];
const binary = process.env.PROVENANCE_REVIEW_TEST_BINARY;
if (!binary) throw new Error('Set PROVENANCE_REVIEW_TEST_BINARY to the CLI built with host assets');
async function host(repository: string) {
  const child = spawn(resolve(binary!), ['review', '--repo', repository, '--repository-id', 'A', '--scope', 'default'], { stdio: ['ignore', 'pipe', 'pipe'] });
  const lines = createInterface({ input: child.stdout });
  const [line] = await Promise.race([once(lines, 'line'), once(child, 'exit').then(() => { throw new Error('Host did not start'); })]);
  const config = JSON.parse(line as string);
  return { config, async close() { lines.close(); child.kill('SIGTERM'); await once(child, 'exit'); } };
}

test('real host renders complete saved documents and refuses failed refreshes', { timeout: 120_000 }, async () => {
  const repository = await mkdtemp(join(tmpdir(), 'provenance-review-document-'));
  execFileSync('git', ['init', '-q', repository]);
  execFileSync(binary!, ['init', '--path', repository, '--scope', 'default', '--path-prefix', '.', '--ste-onboarding', 'interactive'], { stdio: 'pipe' });
  const running = await host(repository);
  const browser = await chromium.launch({ headless: true });
  try {
    const { endpoint, bearer } = running.config;
    const client = await HttpClient.connectWithBearer(endpoint, bearer);
    const context = { repository: 'A', scope: 'default' };
    await client.createRequirement({ context, request: {
      scope_id: 'default', id: 'req_external', statement: 'External requirement', status: 'active', depends_on: [], supersedes: [],
    } });
    const root = await client.createRequirement({ context, request: {
      scope_id: 'default', id: 'req_browser_root', statement: 'Saved document root', status: 'active', depends_on: ['req_external'], supersedes: [],
    } });
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on('pageerror', () => errors.push('page error'));
    page.on('request', request => { if (request.url().includes(bearer)) errors.push('credential in URL'); });
    await page.goto(endpoint);
    await page.getByLabel('Local access token').fill('invalid');
    await page.getByRole('button', { name: 'Connect', exact: true }).click();
    await expect(page.getByRole('status')).toContainText('Connection refused');
    await page.getByLabel('Local access token').fill(bearer);
    await page.getByRole('button', { name: 'Connect', exact: true }).click();
    await expect(page.getByLabel('Requirement ID')).toBeVisible();
    await expect(page.getByLabel('Local access token')).toHaveValue('');
    await page.getByLabel('Requirement ID').fill(root.id);
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(root.statement);

    const shard = join(repository, '.provenance/state/scopes/default/requirements/req.jsonl');
    const children = Array.from({ length: 205 }, (_, i) => ({ ...root, id: `req_child_${i.toString().padStart(3, '0')}`, statement: `Saved child ${i}`, refines: root.id }));
    await appendFile(shard, children.map(row => JSON.stringify(row)).join('\n') + '\n');
    const threadDir = join(repository, '.provenance/state/scopes/default/threads');
    await mkdir(threadDir, { recursive: true });
    const threads: Thread[] = ['resolved', 'active'].map((status, i) => ({
      schema_version: root.schema_version, scope_id: 'default', id: `thread_independent_${i}`, parent: { node_type: 'requirement', node_id: root.id },
      status: status as Thread['status'], created_at: i + 1,
    }));
    const messages: Message[] = threads.map((thread, i) => ({ schema_version: root.schema_version, scope_id: 'default', id: `msg_independent_${i}`,
      thread_id: thread.id, role: 'user', body: `Independent discussion ${i}`, created_at: i + 1,
    }));
    await writeFile(join(threadDir, 'threads.jsonl'), threads.map(row => JSON.stringify(row)).join('\n') + '\n');
    await writeFile(join(threadDir, '2026-09.jsonl'), messages.map(row => JSON.stringify(row)).join('\n') + '\n');
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.locator('[data-record-id="req_child_204"]')).toBeVisible();
    await expect(page.locator('[data-record-id]')).toHaveCount(206);
    await page.locator('[data-record-id="req_browser_root"]').getByRole('button', { name: /^Discuss / }).first().click();
    await expect(page.getByText('Independent discussion 0', { exact: true })).toBeVisible();
    await expect(page.getByText('Independent discussion 1', { exact: true })).toBeVisible();
    await expect(page.getByText('Sequence 1', { exact: true })).toBeVisible();
    assert.equal(await page.getByRole('button', { name: /^(Reply|Resolve|Approve|Post)$/ }).count(), 0);
    assert.equal(await page.locator('textarea').count(), 0);
    await page.getByRole('button', { name: 'Open requirement req_external' }).first().click();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('External requirement');
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(root.statement);
    const saved = await readFile(shard, 'utf8');
    await writeFile(shard, 'invalid JSON\n');
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.getByRole('status')).toContainText('Catch-up failed');
    await expect(page.locator('[data-record-id]')).toHaveCount(0);
    await writeFile(shard, saved);
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.locator('[data-record-id]')).toHaveCount(206);
    const status = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' });
    assert.ok(status.includes('.provenance/'), 'Graph reads must include uncommitted graph files');
    assert.deepEqual(errors, []);
    await page.screenshot({ path: join(repository, 'review.png'), fullPage: false });
    console.log(`Validated 206 saved records and independent discussions in ${repository}`);
  } finally { await browser.close(); await running.close(); }
});

test('reads an existing product Requirement from the supplied working copy', { timeout: 30_000, skip: !process.env.PROVENANCE_REVIEW_REAL_REPO }, async () => {
  const running = await host(resolve(process.env.PROVENANCE_REVIEW_REAL_REPO!));
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(running.config.endpoint);
    await page.getByLabel('Local access token').fill(running.config.bearer);
    await page.getByRole('button', { name: 'Connect', exact: true }).click();
    await page.getByLabel('Requirement ID').fill('req_review_page_replaces_wiki');
    await page.getByRole('button', { name: 'Open / Refresh' }).click();
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('The review page replaces the generated Wiki as the Provenance web surface.');
    await expect(page.getByRole('status')).toContainText('Saved working-copy document');
    console.log('Validated req_review_page_replaces_wiki from the real working copy');
  } finally { await browser.close(); await running.close(); }
});
