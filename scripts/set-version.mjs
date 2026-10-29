#!/usr/bin/env node
// Sets the app version in every file that carries it:
//   node scripts/set-version.mjs <x.y.z[-pre]>
// Files: apps/desktop/src-tauri/tauri.conf.json, apps/desktop/package.json,
// apps/desktop/src-tauri/Cargo.toml and the `app` entry of Cargo.lock.
// Then run `node scripts/check-version.mjs` and commit the four files.
import { VERSION_RE, files, readVersion, writeVersion } from './version-files.mjs';

const version = process.argv[2];
if (!version || !VERSION_RE.test(version)) {
  console.error('usage: node scripts/set-version.mjs <x.y.z[-pre]>   (e.g. 1.0.0-beta.1)');
  process.exit(2);
}

for (const file of files) {
  const before = readVersion(file);
  writeVersion(file, version);
  console.log(`${file.path}: ${before} -> ${readVersion(file)}`);
}
