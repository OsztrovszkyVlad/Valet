#!/usr/bin/env node
// Fails unless every file that carries the app version has the same value.
//   node scripts/check-version.mjs            all files agree
//   node scripts/check-version.mjs v1.2.3     ... and the tag is v<version>
// Prints GitHub Actions ::error:: lines on failure and, when GITHUB_OUTPUT is
// set, writes `version=` and `prerelease=` (true when the version has a '-').
import { appendFileSync } from 'node:fs';
import { VERSION_RE, files, readVersion } from './version-files.mjs';

const tag = process.argv[2];
let ok = true;
const fail = (msg) => {
  console.log(`::error title=version::${msg}`);
  ok = false;
};

const found = files.map((file) => {
  try {
    return [file.path, readVersion(file)];
  } catch (e) {
    fail(e.message);
    return [file.path, undefined];
  }
});
for (const [path, v] of found) console.log(`${path}: ${v}`);

const version = found[0][1];
if (version !== undefined && !VERSION_RE.test(version)) {
  fail(`${found[0][0]}: "${version}" is not x.y.z or x.y.z-pre`);
}
for (const [path, v] of found.slice(1)) {
  if (v !== undefined && v !== version) fail(`${path} has ${v}, ${found[0][0]} has ${version}`);
}
if (tag !== undefined && tag !== '' && tag !== `v${version}`) {
  fail(`tag ${tag} does not match the version ${version} (expected v${version})`);
}

if (!ok) process.exit(1);
const prerelease = version.includes('-');
console.log(`version ${version}${prerelease ? ' (pre-release)' : ''}${tag ? `, tag ${tag}` : ''}: OK`);
if (process.env.GITHUB_OUTPUT) {
  appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\nprerelease=${prerelease}\n`);
}
