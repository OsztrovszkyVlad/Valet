#!/usr/bin/env node
// Writes apps/desktop/src-tauri/tauri.updater.conf.json for a release build,
// merged by `tauri build --config src-tauri/tauri.updater.conf.json`.
// Reads the public key from the VALET_UPDATER_PUBKEY environment variable
// (GitHub Actions variable, set by Vlad). Never prints the key.
// The file is gitignored; tauri.conf.json itself has no updater section.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repo } from './version-files.mjs';

// Stable: GitHub's "latest" release, which excludes drafts and pre-releases.
export const STABLE_ENDPOINT =
  'https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json';
// Beta: kept by the release workflow's updater-channel job on the long-lived
// pre-release `updater-channel`; points at the newest published version,
// pre-release or stable.
export const BETA_ENDPOINT =
  'https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json';

const pubkey = (process.env.VALET_UPDATER_PUBKEY ?? '').trim();
if (!pubkey) {
  console.error('::error title=updater config::VALET_UPDATER_PUBKEY is empty');
  process.exit(1);
}

const config = {
  bundle: { createUpdaterArtifacts: true },
  plugins: {
    updater: {
      pubkey,
      endpoints: [STABLE_ENDPOINT, BETA_ENDPOINT],
    },
  },
};
const out = join(repo, 'apps/desktop/src-tauri/tauri.updater.conf.json');
writeFileSync(out, `${JSON.stringify(config, null, 2)}\n`);
console.log(`wrote ${out} (pubkey: ${pubkey.length} characters, endpoints: stable, beta)`);
