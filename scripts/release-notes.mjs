#!/usr/bin/env node
// Prints the GitHub release body for a version: the `## [x.y.z]` section of
// CHANGELOG.md, a link to the full changelog and the SmartScreen note.
//   node scripts/release-notes.mjs <version> [--allow-missing]
// Exits 1 if CHANGELOG.md has no section for the version, unless
// --allow-missing (dry runs) is given.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repo } from './version-files.mjs';

const [version, flag] = process.argv.slice(2);
if (!version) {
  console.error('usage: node scripts/release-notes.mjs <version> [--allow-missing]');
  process.exit(2);
}

const lines = readFileSync(join(repo, 'CHANGELOG.md'), 'utf8').split(/\r?\n/);
const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
let section = '';
if (start >= 0) {
  let end = lines.findIndex((l, i) => i > start && l.startsWith('## '));
  if (end < 0) end = lines.length;
  section = lines
    .slice(start + 1, end)
    .filter((l) => !/^\[[^\]]+\]:\s/.test(l)) // link references at the end of the file
    .join('\n')
    .trim();
} else if (flag !== '--allow-missing') {
  console.error(`::error title=changelog::CHANGELOG.md has no "## [${version}]" section`);
  process.exit(1);
}

const tag = `v${version}`;
const out = [
  section || `(CHANGELOG.md has no section for ${version} yet.)`,
  '',
  `Full changelog: https://github.com/OsztrovszkyVlad/Valet/blob/${tag}/CHANGELOG.md`,
  '',
  'The installer is not code-signed yet, so Windows SmartScreen shows "Windows protected your PC". ' +
    'Choose "More info", then "Run anyway". It installs for the current user only and needs no administrator rights.',
];
console.log(out.join('\n'));
