// The files that carry the app version, and how to read and write it in each.
// Used by set-version.mjs and check-version.mjs. Edits are targeted
// replacements so the formatting of each file stays as it is.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const repo = join(dirname(fileURLToPath(import.meta.url)), '..');

// SemVer 2.0 without build metadata (Windows file versions and the updater
// compare major.minor.patch and the pre-release part only).
export const VERSION_RE =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/;

const NL = String.raw`\r?\n`;

export const files = [
  {
    path: 'apps/desktop/src-tauri/tauri.conf.json',
    // The top-level "version" key (two-space indent).
    re: /^( {2}"version": ")([^"]*)(")/m,
  },
  {
    path: 'apps/desktop/package.json',
    re: /^( {2}"version": ")([^"]*)(")/m,
  },
  {
    path: 'apps/desktop/src-tauri/Cargo.toml',
    // `version = "..."` inside [package], before the next table header.
    re: new RegExp(String.raw`(^\[package\]${NL}(?:(?!\[)[^\r\n]*${NL})*?version = ")([^"]*)(")`, 'm'),
  },
  {
    path: 'Cargo.lock',
    // The lock entry of the desktop crate (package name "app").
    re: new RegExp(String.raw`(^\[\[package\]\]${NL}name = "app"${NL}version = ")([^"]*)(")`, 'm'),
  },
];

export function readVersion(file) {
  const text = readFileSync(join(repo, file.path), 'utf8');
  const m = text.match(file.re);
  if (!m) throw new Error(`${file.path}: version field not found`);
  return m[2];
}

export function writeVersion(file, version) {
  const full = join(repo, file.path);
  const text = readFileSync(full, 'utf8');
  if (!file.re.test(text)) throw new Error(`${file.path}: version field not found`);
  const next = text.replace(file.re, (_, a, _old, c) => `${a}${version}${c}`);
  if (next !== text) writeFileSync(full, next);
}
