#!/usr/bin/env node
// Fills the winget manifest templates in packaging/winget/templates for one
// release and writes them in the microsoft/winget-pkgs folder layout:
//   node scripts/winget-manifest.mjs <version> <installer-url> <sha256> [--beta]
//   -> packaging/winget/out/manifests/o/OsztrovszkyVlad/Valet/<version>/
// --beta uses the identifier OsztrovszkyVlad.Valet.Beta (winget-pkgs keeps
// pre-releases under their own identifier so `winget upgrade` does not move
// stable users to a beta).
// The SHA-256 is printed by the release workflow ("installer" notice) or by
// `Get-FileHash <file> -Algorithm SHA256` on Windows.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { VERSION_RE, repo } from './version-files.mjs';

const args = process.argv.slice(2);
const beta = args.includes('--beta');
const [version, url, sha256] = args.filter((a) => a !== '--beta');

const usage = 'usage: node scripts/winget-manifest.mjs <version> <installer-url> <sha256> [--beta]';
const die = (msg) => {
  console.error(msg);
  process.exit(2);
};
if (!version || !url || !sha256) die(usage);
if (!VERSION_RE.test(version)) die(`not a version: ${version}\n${usage}`);
if (!/^https:\/\/\S+\.exe$/.test(url)) die(`installer URL must be https and end in .exe: ${url}`);
if (!/^[0-9a-fA-F]{64}$/.test(sha256)) die(`not a SHA-256 (64 hex characters): ${sha256}`);
if (beta !== version.includes('-')) {
  console.warn(
    beta
      ? `warning: --beta with a stable version ${version}`
      : `warning: ${version} is a pre-release; winget-pkgs expects pre-releases under the .Beta identifier (--beta)`,
  );
}

const baseId = 'OsztrovszkyVlad.Valet';
const id = beta ? `${baseId}.Beta` : baseId;
const values = {
  ID: id,
  NAME: beta ? 'Valet Beta' : 'Valet',
  VERSION: version,
  INSTALLER_URL: url,
  SHA256: sha256.toUpperCase(),
  RELEASE_DATE: new Date().toISOString().slice(0, 10),
  RELEASE_NOTES_URL: `https://github.com/OsztrovszkyVlad/Valet/releases/tag/v${version}`,
};

const templates = join(repo, 'packaging/winget/templates');
const out = join(repo, 'packaging/winget/out/manifests/o/OsztrovszkyVlad', ...id.split('.').slice(1), version);
mkdirSync(out, { recursive: true });

for (const name of readdirSync(templates).filter((n) => n.endsWith('.yaml')).sort()) {
  const text = readFileSync(join(templates, name), 'utf8')
    .replace(/^# Template filled by .*\r?\n/m, '')
    .replace(/\{\{([A-Z0-9_]+)\}\}/g, (m, key) => {
      if (!(key in values)) die(`${name}: unknown placeholder ${m}`);
      return values[key];
    });
  const target = join(out, name.replace(baseId, id));
  writeFileSync(target, text);
  console.log(target);
}
console.log(`\nValidate on Windows: winget validate --manifest "${out}"`);
