// Writes catalog.json: the catalog answers of one provenance binary that catalog.test.mjs reads.
// Usage: node tools/api-latency/fixtures/capture.mjs <provenance binary> <repository>
//
// catalog.json is a sample of the answers of one binary, not the contract of the
// current commit. The file is the byte-for-byte output of this tool: do not edit it
// by hand. A contract change does not need a new capture. Capture again only when the
// tests of the tool need a new form of the answer.
//
// The listing keeps three routes of the answer to `api --format json`: a read with a
// query form, a write, and a POST that only checks. Its bounds say three routes.
// Each route also has the answer to the filter of its method and path.
// The tool prints the SHA-256 digest and the byte length of the file that it writes,
// so that a reader can compare the committed file with a capture.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { routeFilter } from '../catalog.mjs';

const [bin, repo] = process.argv.slice(2);
const FILTERS = ['GET /sources', 'POST /sources', 'POST /statement-checks'];
const api = args => JSON.parse(execFileSync(bin, ['api', '--format', 'json', ...args], { cwd: repo, maxBuffer: 1 << 28, encoding: 'utf8' }));

const all = api([]);
const routes = all.routes.filter(route => FILTERS.includes(routeFilter(route)));
const listing = { bounds: { ...all.bounds, matching: routes.length, returned: routes.length }, routes };
const answers = FILTERS.map(filter => `    ${JSON.stringify(filter)}: ${JSON.stringify(api(['--filter', filter]))}`);
const text = `{\n  "listing": ${JSON.stringify(listing)},\n  "answers": {\n${answers.join(',\n')}\n  }\n}\n`;
writeFileSync(new URL('catalog.json', import.meta.url), text);
console.log(`sha256 ${createHash('sha256').update(text).digest('hex')} bytes ${Buffer.byteLength(text)}`);
