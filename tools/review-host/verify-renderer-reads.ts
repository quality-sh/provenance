// Implementation aid: the pinned renderer ships its own SDK build, so this flow checks the
// host's read responses against the response validators that the renderer bundles.
import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { withFixtureHost } from './fixture-host.ts';
import { refusedFields, type Validator } from './renderer-reads-report.ts';

const [binaryArg, assetsArg, sdkArg] = process.argv.slice(2);
if (!binaryArg || !assetsArg || !sdkArg) throw new Error('Usage: node verify-renderer-reads.ts BINARY COMPOSED_ASSETS SDK_PACKAGE');
const assets = resolve(assetsArg);
const review = await readFile(join(assets, 'review.js'), 'utf8');
const { HttpClient } = await import(pathToFileURL(join(resolve(sdkArg), 'dist/client.js')).href);

// The renderer client loads the validators of each operation from a chunk with a hashed name.
// The chunk is found from the client method of the operation, so a new renderer build needs no edit.
async function rendererValidator(operation: string, name: string): Promise<Validator> {
  const start = review.indexOf(`async ${operation}(`);
  const chunk = start < 0 ? null : /import\(`(\.\/assets\/[^`]+\.js)`\)/.exec(review.slice(start, start + 400));
  if (!chunk) throw new Error(`The renderer client has no ${operation} read`);
  const validator = (await import(pathToFileURL(join(assets, chunk[1])).href))[name];
  if (typeof validator !== 'function') throw new Error(`The renderer ${operation} chunk has no ${name} validator`);
  return validator;
}

// Each record type that a document can hold gets a real body: Requirements, a Rule, a
// Resolution, a Topic, and a Question.
const requirements: [string, string[]][] = [
  ['req_root', []], ['req_child', ['req_root']], ['req_joint', ['req_root', 'req_child']],
];
const statement = 'The record is readable.';
const { failures, summary } = await withFixtureHost(binaryArg, repository => {
  for (const [id, refines] of requirements) {
    repository.create('requirements', {
      actor: 'review-test', id, statement, status: 'discovery',
      depends_on: [], supersedes: [], ...(refines.length ? { refines } : {}),
    });
  }
  repository.create('rules', { id: 'rule_readable', requirement_ids: ['req_child'], statement, status: 'draft' });
  repository.create('resolutions', {
    id: 'res_readable', requirement_ids: ['req_child'], title: 'Readable record', position: statement, rationale: statement,
  });
  repository.create('topics', { id: 'topic_readable', requirement_id: 'req_child', title: 'Readable record' });
  repository.create('questions', {
    id: 'q_readable', topic_id: 'topic_readable', question: 'Is the record readable?', resolution_method: 'research',
  });
}, async config => {
  let body: unknown;
  const recording: typeof fetch = async (input, init) => {
    const response = await fetch(input, init);
    body = await response.clone().json().catch(() => undefined);
    return response;
  };
  const client = await HttpClient.connectWithBearer(
    config.endpoint, config.bearer, recording, { repository: 'fixture', scope: 'default' },
  );
  const failures: string[] = [];
  const counts: Record<string, number> = {};
  // Reads every page of one read and checks each response body with the renderer validator.
  async function check(operation: string, name: string, label: string, read: (cursor?: string) => Promise<unknown>) {
    const validator = await rendererValidator(operation, name);
    let cursor: string | undefined;
    let page = 0;
    do {
      page += 1;
      await read(cursor);
      for (const field of refusedFields(validator, body)) failures.push(`${operation} ${label} page ${page}: ${field}`);
      counts[operation] = (counts[operation] ?? 0) + 1;
      cursor = (body as { meta?: { next_cursor?: string | null } }).meta?.next_cursor ?? undefined;
    } while (cursor);
  }
  for (const [id] of requirements) {
    await check('getRequirementDocument', 'GetRequirementDocumentSuccess', id,
      cursor => client.getRequirementDocument({ id, limit: 1, ...(cursor ? { cursor } : {}) }));
    await check('getRequirement', 'GetRequirementBaseSuccess', id, () => client.getRequirement({ id }));
    await check('listRequirementHistory', 'ListRequirementHistorySuccess', id,
      cursor => client.listRequirementHistory({ id, limit: 1, ...(cursor ? { cursor } : {}) }));
  }
  await check('listRequirements', 'ListRequirementsBaseSuccess', 'list',
    cursor => client.listRequirements({ limit: 1, ...(cursor ? { cursor } : {}) }));
  await check('listRequirements', 'ListRequirementsSearchSuccess', 'search',
    cursor => client.listRequirements({ query: 'search', text: 'readable', limit: 1, ...(cursor ? { cursor } : {}) }));
  // The page shows a refusal when a document root is gone, so its failure body is checked too.
  await check('getRequirementDocument', 'GetRequirementDocumentFailure', 'req_missing',
    () => client.getRequirementDocument({ id: 'req_missing' }).then(
      () => { throw new Error('The host read a document for a missing root'); },
      () => undefined,
    ));
  return { failures, summary: Object.entries(counts).map(([operation, count]) => `${operation} ${count}`).join(', ') };
});
if (failures.length) {
  for (const failure of failures) console.error(failure);
  throw new Error(`The renderer validators refused ${failures.length} host response fields (${summary} pages)`);
}
console.log(`The renderer validators accepted every host response (${summary} pages): ${binaryArg}`);
