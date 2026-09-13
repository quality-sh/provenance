import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

async function inventory(root, prefix = '') {
  let entries;
  try { entries = await readdir(join(root, prefix), { withFileTypes: true }); }
  catch (error) { if (error.code === 'ENOENT') return new Map(); throw error; }
  const files = new Map();
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
    const name = join(prefix, entry.name);
    if (entry.isDirectory()) {
      for (const [path, content] of await inventory(root, name)) files.set(path, content);
    } else {
      if (!entry.isFile()) throw new Error(`Generated path is not a regular file: ${name}`);
      files.set(name, await readFile(join(root, name), 'utf8'));
    }
  }
  return files;
}

export async function compareTrees(expectedRoot, actualRoot) {
  const [expected, actual] = await Promise.all([inventory(expectedRoot), inventory(actualRoot)]);
  const errors = [];
  for (const name of new Set([...expected.keys(), ...actual.keys()])) {
    if (!expected.has(name)) errors.push(`stale: ${name}`);
    else if (!actual.has(name)) errors.push(`missing: ${name}`);
    else if (expected.get(name) !== actual.get(name)) errors.push(`changed: ${name}`);
    for (const files of [expected, actual]) {
      const content = files.get(name);
      if (name.endsWith('.rs') && content !== undefined && content.split('\n').length - Number(content.endsWith('\n')) > 500) {
        errors.push(`Rust file exceeds 500 lines: ${name}`);
      }
    }
  }
  return errors.sort();
}
