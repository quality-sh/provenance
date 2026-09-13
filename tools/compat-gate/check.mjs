#!/usr/bin/env node
// Compatibility gate (bead provenance-if20).
//
// Fails when a watched compatibility value changes without a fresh human
// authorization marker (AUTHORIZED-BUMP). Contract: docs/compatibility-gate.md
// and docs/api-contract-v2.md section 6. Agents never bump versions; this
// check is the enforcement arm of that Rule.
//
// The core is pure and unit-tested (check.test.mjs). The Git plumbing lives in
// main() and only this file runs in CI.

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

export const MARKER_PATH = 'AUTHORIZED-BUMP';

export const WATCHED = [
  {
    key: 'wire (SDK_PROTOCOL_VERSION)',
    path: 'crates/provenance-core/src/protocol.rs',
    pattern: /SDK_PROTOCOL_VERSION:\s*u32\s*=\s*(\d+)/,
  },
  {
    key: 'state (SUPPORTED_SCHEMA_VERSION)',
    path: 'crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs',
    pattern: /SUPPORTED_SCHEMA_VERSION:\s*SchemaVersion\s*=\s*SchemaVersion\((\d+)\)/,
  },
  {
    key: 'review_journal (REVIEW_SCHEMA_VERSION)',
    path: 'crates/provenance-core/src/review.rs',
    pattern: /REVIEW_SCHEMA_VERSION:\s*SchemaVersion\s*=\s*SchemaVersion\((\d+)\)/,
  },
  {
    key: 'read_derivation (READ_DERIVATION)',
    path: 'crates/provenance-store/src/operations/stamp.rs',
    pattern: /READ_DERIVATION:\s*u32\s*=\s*(\d+)/,
  },
  {
    key: 'state mirror (STATE_SCHEMA_VERSION, TypeScript)',
    path: 'packages/provenance/src/protocol.ts',
    pattern: /STATE_SCHEMA_VERSION\s*=\s*(\d+)/,
  },
  {
    key: 'workspace crate version',
    path: 'Cargo.toml',
    pattern: /^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
  },
  {
    key: 'packages/provenance version',
    path: 'packages/provenance/package.json',
    pattern: /"version"\s*:\s*"([^"]+)"/,
  },
  {
    key: 'packages/create-provenance version',
    path: 'packages/create-provenance/package.json',
    pattern: /"version"\s*:\s*"([^"]+)"/,
  },
];

export function extractWatchedValue(content, entry) {
  const match = String(content).match(entry.pattern);
  return match ? match[1] : null;
}

export function watchedValues(files) {
  const values = {};
  for (const entry of WATCHED) {
    const content = files[entry.path];
    values[entry.key] = content === undefined ? null : extractWatchedValue(content, entry);
  }
  return values;
}

export function parseMarker(content) {
  if (content === null || content === undefined) return { ok: false, errors: ['marker file is absent'] };
  const fields = {};
  const errors = [];
  for (const line of String(content).split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const match = trimmed.match(/^([a-z][a-z0-9_-]*):\s*(.+)$/);
    if (!match) { errors.push(`unparsable marker line: ${trimmed}`); continue; }
    fields[match[1]] = match[2].trim();
  }
  if (!fields['authorized-by'] || fields['authorized-by'].length < 3) errors.push('authorized-by is missing; name the authorizing human');
  const date = (fields.date ?? '').match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!date) errors.push('date is missing or is not YYYY-MM-DD');
  else {
    const [, year, month, day] = date.map(Number);
    const calendar = new Date(Date.UTC(year, month - 1, day));
    if (calendar.getUTCFullYear() !== year || calendar.getUTCMonth() !== month - 1 || calendar.getUTCDate() !== day) {
      errors.push('date is not a real calendar date');
    }
  }
  if (!/^[1-9][0-9]*$/.test(fields['pull-request'] ?? '')) errors.push('pull-request is missing or is not a pull-request number; bind the marker to the change it authorizes');
  if (!/^[0-9a-f]{7,40}$/.test(fields.commit ?? '')) errors.push('commit is missing or is not a commit SHA; bind the marker to a commit of the change it authorizes');
  if (!fields.reason || fields.reason.length < 4) errors.push('reason is missing; state why the change is authorized');
  return errors.length ? { ok: false, errors, fields } : { ok: true, errors: [], fields };
}

// A parsed marker only counts when it is bound to the change under evaluation.
// The binding is the detectable proxy for human authorship: a marker copied
// from an earlier change names a pull request that is not under review, a
// commit outside this change, and a date before the base commit existed.
export function bindingViolations(fields, context) {
  const violations = [];
  if (context.pullRequest !== null && context.pullRequest !== undefined && fields['pull-request'] !== String(context.pullRequest)) {
    violations.push(`authorization refused: marker pull-request ${fields['pull-request']} does not match pull request ${context.pullRequest} under review`);
  }
  const commit = fields.commit ?? '';
  if (Array.isArray(context.commits) && !context.commits.some(sha => sha.startsWith(commit))) {
    violations.push(`authorization refused: marker commit ${commit} is not a commit of this change (base..head); a marker from an earlier change authorizes nothing`);
  }
  if (context.baseDate && fields.date < context.baseDate) {
    violations.push(`authorization refused: marker date ${fields.date} predates the base commit date ${context.baseDate}`);
  }
  return violations;
}

export function evaluate({ base, head, baseMarker, headMarker, context = null }) {
  const changes = [];
  for (const entry of WATCHED) {
    const from = base[entry.key];
    const to = head[entry.key];
    if (from !== to) changes.push({ key: entry.key, from, to });
  }
  if (changes.length === 0) return { authorized: true, changes, violations: [], base, head };

  const violations = [];
  const marker = parseMarker(headMarker);
  if (!marker.ok) violations.push(...marker.errors.map(error => `authorization refused: ${error}`));
  const fresh = baseMarker !== null && baseMarker === headMarker;
  if (fresh) violations.push('authorization refused: AUTHORIZED-BUMP is unchanged from the base; a human must write a new authorization for this change');
  if (marker.ok && context) violations.push(...bindingViolations(marker.fields, context));
  return { authorized: violations.length === 0, changes, violations, base, head };
}

function git(root, args) {
  const result = spawnSync('git', ['-C', root, ...args], { encoding: 'utf8', maxBuffer: 4 * 1024 * 1024 });
  if (result.error) throw result.error;
  return { ok: result.status === 0, stdout: result.stdout, stderr: result.stderr };
}

function revExists(root, rev) {
  return git(root, ['cat-file', '-e', `${rev}^{commit}`]).ok;
}

function readBlob(root, rev, path) {
  const result = git(root, ['show', `${rev}:${path}`]);
  return result.ok ? result.stdout : null;
}

function readFiles(root, rev) {
  const files = {};
  for (const entry of WATCHED) files[entry.path] = readBlob(root, rev, entry.path) ?? undefined;
  return watchedValues(files);
}

function changeCommits(root, base, head) {
  const result = git(root, ['rev-list', `${base}..${head}`]);
  return result.ok ? result.stdout.split('\n').filter(Boolean) : [];
}

function baseCommitDate(root, base) {
  const result = git(root, ['show', '-s', '--format=%cI', base]);
  return result.ok ? result.stdout.trim().slice(0, 10) : null;
}

const ZERO_SHA = /^0+$/;

function defaultBase(root) {
  const mergeBase = git(root, ['merge-base', 'HEAD', 'origin/main']);
  return mergeBase.ok ? mergeBase.stdout.trim() : null;
}

async function main() {
  const root = process.env.COMPAT_GATE_ROOT ?? process.cwd();
  const args = process.argv.slice(2);
  const flag = name => {
    const index = args.indexOf(name);
    return index >= 0 ? args[index + 1] : undefined;
  };
  let base = flag('--base');
  const head = flag('--head') ?? 'HEAD';
  const pullRequest = flag('--pull-request') ?? process.env.COMPAT_PULL_REQUEST ?? null;
  if (base === undefined && process.env.COMPAT_BASE) base = process.env.COMPAT_BASE;
  if (base === undefined) base = defaultBase(root);
  if (base === undefined || base === null || ZERO_SHA.test(base)) {
    console.log('Compatibility gate: no base revision; nothing to compare. Passing vacuously.');
    return;
  }
  if (!revExists(root, base)) {
    console.error(`Compatibility gate: base revision ${base} does not exist in this checkout.`);
    process.exitCode = 1;
    return;
  }
  if (!revExists(root, head)) {
    console.error(`Compatibility gate: head revision ${head} does not exist in this checkout.`);
    process.exitCode = 1;
    return;
  }

  // The full binding (pull request, change commit, marker date) needs the
  // pull-request context. Pushes to main carry none, so they get the format
  // and freshness rules only; the pull-request run is the authoritative one.
  const context = pullRequest !== null
    ? { pullRequest, commits: changeCommits(root, base, head), baseDate: baseCommitDate(root, base) }
    : null;
  const result = evaluate({
    base: readFiles(root, base),
    head: readFiles(root, head),
    baseMarker: readBlob(root, base, MARKER_PATH),
    headMarker: readBlob(root, head, MARKER_PATH),
    context,
  });

  for (const entry of WATCHED) {
    const change = result.changes.find(c => c.key === entry.key);
    const from = change ? change.from : result.base[entry.key];
    const to = change ? change.to : result.head[entry.key];
    console.log(`${change ? 'changed' : 'same   '}  ${entry.key}: ${from} -> ${to}`);
  }
  if (result.authorized) {
    console.log(result.changes.length
      ? 'Compatibility gate: compatibility change carries a fresh AUTHORIZED-BUMP authorization.'
      : 'Compatibility gate: no compatibility change.');
    return;
  }
  console.error('Compatibility gate failed. Compatibility values changed without a valid human authorization.');
  for (const violation of result.violations) console.error(`  - ${violation}`);
  console.error(`Write ${MARKER_PATH} (authorized-by, date, pull-request, commit, reason) by hand in the same change, or revert the change.`);
  process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
