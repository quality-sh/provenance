import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
export const categories = ['duplicate', 'placement', 'state', 'citationOnly'];
const text = r => r.statement ?? r.position;
const requirements = r => r.requirement_ids ?? [];
const resolutions = r => [...(r.resolution_ids ?? []), ...(r.spawned_by ? [r.spawned_by] : [])];

// Equality identifies review candidates; it cannot decide whether meaning is duplicated.
export function score(graph) {
  const records = [...graph.requirements, ...graph.rules, ...graph.resolutions];
  const findings = [];
  for (let i = 0; i < records.length; i++) {
    for (const b of records.slice(i + 1)) {
      const a = records[i];
      const linked = [a.refines, ...requirements(a), ...resolutions(a)].includes(b.id)
        || [b.refines, ...requirements(b), ...resolutions(b)].includes(a.id);
      if (linked && text(a) && text(a) === text(b)) findings.push({
        category: 'duplicate', recordIds: [a.id, b.id],
        evidenceLocator: `graph.json:${a.id},${b.id}`, reviewerAssessment: null,
      });
    }
  }
  return { findings, reviewRequired: categories };
}

export function compare(base, candidate) {
  const runners = [...new Set([...Object.keys(base), ...Object.keys(candidate)])];
  const gates = runners.flatMap(runner => categories.map((category, i) => {
    const before = base[runner]?.[i], after = candidate[runner]?.[i];
    const valid = Number.isInteger(before) && before >= 0 && Number.isInteger(after) && after >= 0;
    return { runner, category, before, after, passed: valid && (before === 0 ? after === 0 : after < before) };
  }));
  return { passed: runners.length > 0 && gates.every(g => g.passed), gates };
}

export function totals(assessment) {
  if (assessment.completedCategories?.length !== categories.length
      || categories.some(c => !assessment.completedCategories.includes(c))) throw new Error('Review is incomplete');
  const counts = categories.map(() => 0);
  for (const f of assessment.findings) {
    if (!categories.includes(f.category) || !f.recordIds?.length || !f.evidenceLocator
        || typeof f.reviewerAssessment?.isError !== 'boolean' || !f.reviewerAssessment.reason) throw new Error('Finding lacks evidence or assessment');
    if (f.reviewerAssessment.isError) counts[categories.indexOf(f.category)]++;
  }
  return counts;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [basePath, candidatePath, output] = process.argv.slice(2);
  const result = compare(JSON.parse(await readFile(basePath)), JSON.parse(await readFile(candidatePath)));
  await writeFile(output, JSON.stringify(result, null, 2));
  if (!result.passed) process.exitCode = 1;
}
