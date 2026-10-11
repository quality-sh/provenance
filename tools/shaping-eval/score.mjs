import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
export const dimensions = ['position', 'locator', 'reason', 'alternative'];
const fields = ['statement', 'description', 'position', 'rationale', 'context'];
export function authored(seed, graph) {
  const records = g => ['requirements', 'rules', 'resolutions', 'questions', 'sources'].flatMap(k => g[k] ?? []);
  const originals = new Map(records(seed).map(r => [r.id, r]));
  return records(graph).filter(r => fields.some(f => typeof r[f] === 'string' && r[f])
    && fields.some(f => r[f] !== originals.get(r.id)?.[f]));
}
// Phrase matches suggest review candidates. They do not decide the human label.
export function score(graph) {
  const records = authored({}, graph);
  return { findings: records.filter(r => fields.some(f => /not yet accepted|has not decided|proposed by|recommends|awaiting review/i.test(r[f] ?? ''))).map(r => ({ recordId: r.id, reviewRequired: true })) };
}
export function assess(records, labels) {
  const result = { total: records.length, contaminated: 0, rationales: 0, grounding: [0, 0, 0, 0] };
  for (const record of records) {
    const matches = labels.filter(l => l.id === record.id);
    if (matches.length !== 1 || typeof matches[0].contaminated !== 'boolean') throw new Error(`Missing or duplicate label: ${record.id}`);
    const label = matches[0];
    result.contaminated += Number(label.contaminated);
    if (record.rationale) {
      result.rationales++;
      for (const [i, d] of dimensions.entries()) {
        if (typeof label.grounding?.[d] !== 'boolean') throw new Error(`Missing grounding ${d}: ${record.id}`);
        result.grounding[i] += Number(label.grounding[d]);
      }
    }
  }
  return result;
}
export function compare(base, candidate, required = ['codex', 'claude']) {
  const runners = [...new Set([...required, ...Object.keys(base), ...Object.keys(candidate)])];
  const gates = runners.map(runner => {
    const before = base[runner], after = candidate[runner];
    const valid = before?.total > 0 && after?.total > 0 && before.rationales > 0 && after.rationales > 0;
    const contamination = Boolean(valid && after.contaminated / after.total < before.contaminated / before.total);
    const grounding = dimensions.map((d, i) => ({ dimension: d, passed: Boolean(valid && after.grounding[i] / after.rationales >= before.grounding[i] / before.rationales) }));
    return { runner, before, after, contamination, grounding, passed: contamination && grounding.every(g => g.passed) };
  });
  return { gates, passed: gates.length > 0 && gates.every(g => g.passed) };
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [input, labels, output] = process.argv.slice(2);
  await writeFile(output, JSON.stringify(assess(JSON.parse(await readFile(input)), JSON.parse(await readFile(labels))), null, 2));
}
