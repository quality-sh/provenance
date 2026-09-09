import { rmSync } from 'node:fs';
const directory = process.argv[2] ?? 'dist';
if (!['dist', '.test-dist'].includes(directory)) throw new Error('Unknown build directory');
rmSync(new URL(`../${directory}/`, import.meta.url), { recursive: true, force: true });
