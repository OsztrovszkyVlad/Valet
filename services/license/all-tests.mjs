// Entry point for `node --test services/license`. Since Node 21 a directory given to
// `node --test` is loaded as a module (package.json "main") instead of being searched
// for test files, so this file imports every test/*.test.mjs. Inside this directory,
// `node --test` (npm test) finds the test files directly and ignores this file.
import { readdirSync } from 'node:fs';

const dir = new URL('./test/', import.meta.url);
for (const name of readdirSync(dir).filter((f) => f.endsWith('.test.mjs')).sort()) {
  await import(new URL(name, dir));
}
