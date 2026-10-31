import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const bundledSkillNames = ['provenance-fork-tournament', 'provenance-grounded-writing', 'provenance-shaping', 'provenance-swarm-backtrace'];
export async function prepareSkills(root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')) {
  // Read the complete source set before replacing any package files.
  const sources = await Promise.all(bundledSkillNames.map(name => readFile(join(root, 'skills', name, 'SKILL.md'))));
  for (const [index, name] of bundledSkillNames.entries()) {
    const directory = join(root, 'crates/provenance-cli/skills', name);
    await mkdir(directory, { recursive: true });
    await writeFile(join(directory, 'SKILL.md'), sources[index]);
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await prepareSkills();
