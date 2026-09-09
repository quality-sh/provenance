import { copyFileSync, mkdirSync } from 'node:fs';
const destination = process.argv[2] ?? 'dist';
const root = new URL('../', import.meta.url);
const output = new URL(`${destination}/generated/`, root);
mkdirSync(output, { recursive: true });
for (const name of ['validators.mjs', 'validators.d.mts']) {
  copyFileSync(new URL(`src/generated/${name}`, root), new URL(name, output));
}
