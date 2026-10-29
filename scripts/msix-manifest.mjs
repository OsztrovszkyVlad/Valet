#!/usr/bin/env node
// Checks and values for the Microsoft Store (MSIX) build, and the filled-in
// AppxManifest.xml. Used by scripts/msix-pack.ps1 and .github/workflows/msix.yml.
//
//   node scripts/msix-manifest.mjs [--out <AppxManifest.xml>] [--exe <Valet.exe>]
//                                  [--allow-prerelease] [--require-identity]
//
// - Identity from the environment (GitHub Actions variables, set by Vlad from
//   Partner Center > Product identity): VALET_MSIX_IDENTITY_NAME,
//   VALET_MSIX_PUBLISHER (CN=...), VALET_MSIX_PUBLISHER_DISPLAY_NAME.
//   Missing values fall back to fake ones (Valet.Dev, CN=ValetDev) with a
//   warning; --require-identity turns that into an error.
// - Version: tauri.conf.json x.y.z -> MSIX x.y.z.0 (the Store requires the
//   fourth part to be 0). A pre-release version (x.y.z-...) is refused unless
//   --allow-prerelease (dry runs), which maps it to x.y.z.0 as well, so a
//   pre-release package can never be told apart from the stable one and must
//   not be submitted.
// - No updater in the Store build: fails if tauri.conf.json has
//   plugins.updater or bundle.createUpdaterArtifacts, if the generated
//   tauri.updater.conf.json exists, if TAURI_CONFIG is set, and (--exe) if
//   the built executable contains an updater public key.
// - Fails if tauri.conf.json bundles resources or external binaries (the
//   package would only contain Valet.exe).
// - --out writes packaging/msix/AppxManifest.xml with the placeholders filled.
// Writes version=, msix_version=, prerelease=, identity_name=, publisher=,
// identity_defaults= to $GITHUB_OUTPUT when set. Never reads secrets.
import { appendFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { VERSION_RE, repo } from './version-files.mjs';

export const DEFAULT_IDENTITY = {
  name: 'Valet.Dev',
  publisher: 'CN=ValetDev',
  publisherDisplayName: 'Valet dev build (not for the Store)',
};

// Base64 of "untrusted comment: minisign public key" without the last
// character, which depends on what follows. Every Tauri updater pubkey
// (the content of the .pub file from `tauri signer generate`) starts with it.
const UPDATER_PUBKEY_MARKER = 'dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk';

const CONF = 'apps/desktop/src-tauri/tauri.conf.json';
const UPDATER_CONF = 'apps/desktop/src-tauri/tauri.updater.conf.json';
const TEMPLATE = 'packaging/msix/AppxManifest.xml';

const annotate = (s) => s.replace(/%/g, '%25').replace(/\r/g, '').replace(/\n/g, ' ');

/** x.y.z or x.y.z-pre -> { msixVersion: 'x.y.z.0', prerelease }. Throws. */
export function msixVersion(version) {
  if (!VERSION_RE.test(version)) throw new Error(`"${version}" is not x.y.z or x.y.z-pre`);
  const [core] = version.split('-');
  const parts = core.split('.').map(Number);
  for (const p of parts) {
    if (p > 65535) throw new Error(`"${version}": MSIX version parts must be at most 65535`);
  }
  return { msixVersion: `${parts.join('.')}.0`, prerelease: version.includes('-') };
}

/** Identity from env, with the fake defaults filled in. */
export function identityFrom(env) {
  const pick = (key, def) => {
    const v = (env[key] ?? '').trim();
    return v ? { value: v, isDefault: false } : { value: def, isDefault: true };
  };
  const name = pick('VALET_MSIX_IDENTITY_NAME', DEFAULT_IDENTITY.name);
  const publisher = pick('VALET_MSIX_PUBLISHER', DEFAULT_IDENTITY.publisher);
  const display = pick('VALET_MSIX_PUBLISHER_DISPLAY_NAME', DEFAULT_IDENTITY.publisherDisplayName);
  const defaults = [];
  if (name.isDefault) defaults.push('VALET_MSIX_IDENTITY_NAME');
  if (publisher.isDefault) defaults.push('VALET_MSIX_PUBLISHER');
  if (display.isDefault) defaults.push('VALET_MSIX_PUBLISHER_DISPLAY_NAME');
  return {
    name: name.value,
    publisher: publisher.value,
    publisherDisplayName: display.value,
    defaults,
  };
}

/** Errors for identity values the manifest schema would reject. */
export function identityErrors(id) {
  const errors = [];
  if (!/^[A-Za-z0-9.-]{3,50}$/.test(id.name)) {
    errors.push(`identity name "${id.name}" must be 3-50 characters of A-Z, a-z, 0-9, '.', '-'`);
  }
  if (!/^CN=\S/.test(id.publisher) || id.publisher.length > 8192) {
    errors.push(`publisher "${id.publisher}" must start with CN= (copy it from Partner Center)`);
  }
  if (id.publisherDisplayName.length < 1 || id.publisherDisplayName.length > 256) {
    errors.push('publisher display name must be 1-256 characters');
  }
  return errors;
}

/** Errors if the build config would put the updater or extra files in the package. */
export function configErrors(conf, env, updaterConfExists) {
  const errors = [];
  if (conf.plugins && Object.hasOwn(conf.plugins, 'updater')) {
    errors.push(`${CONF} has plugins.updater; the Store build must not contain the updater`);
  }
  if (conf.bundle?.createUpdaterArtifacts) {
    errors.push(`${CONF} has bundle.createUpdaterArtifacts`);
  }
  if (updaterConfExists) {
    errors.push(`${UPDATER_CONF} exists (written by the release workflow); the Store build must not merge it`);
  }
  if ((env.TAURI_CONFIG ?? '').trim() !== '') {
    errors.push('TAURI_CONFIG is set; the Store build uses tauri.conf.json only');
  }
  const resources = conf.bundle?.resources;
  if (resources && (Array.isArray(resources) ? resources.length : Object.keys(resources).length)) {
    errors.push(`${CONF} has bundle.resources; scripts/msix-pack.ps1 packs Valet.exe only`);
  }
  if (Array.isArray(conf.bundle?.externalBin) && conf.bundle.externalBin.length) {
    errors.push(`${CONF} has bundle.externalBin; scripts/msix-pack.ps1 packs Valet.exe only`);
  }
  return errors;
}

const xmlEscape = (s) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');

/** The template with {{IDENTITY_NAME}}, {{PUBLISHER}}, {{PUBLISHER_DISPLAY_NAME}}, {{VERSION}} filled. */
export function fillTemplate(template, id, version) {
  const values = {
    IDENTITY_NAME: id.name,
    PUBLISHER: id.publisher,
    PUBLISHER_DISPLAY_NAME: id.publisherDisplayName,
    VERSION: version,
  };
  const out = template.replace(/\{\{([A-Z_]+)\}\}/g, (m, key) => {
    if (!Object.hasOwn(values, key)) throw new Error(`unknown placeholder ${m} in ${TEMPLATE}`);
    return xmlEscape(values[key]);
  });
  if (out.includes('{{')) throw new Error(`unfilled placeholder left in ${TEMPLATE}`);
  return out;
}

function parseArgs(argv) {
  const opts = { out: null, exe: null, allowPrerelease: false, requireIdentity: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--out') opts.out = argv[++i];
    else if (a === '--exe') opts.exe = argv[++i];
    else if (a === '--allow-prerelease') opts.allowPrerelease = true;
    else if (a === '--require-identity') opts.requireIdentity = true;
    else throw new Error(`unknown argument ${a}`);
    if ((a === '--out' || a === '--exe') && !argv[i]) throw new Error(`${a} needs a path`);
  }
  return opts;
}

function main() {
  const errors = [];
  const fail = (msg) => {
    console.log(`::error title=msix::${annotate(msg)}`);
    errors.push(msg);
  };

  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (e) {
    fail(e.message);
    process.exit(1);
  }

  const conf = JSON.parse(readFileSync(join(repo, CONF), 'utf8'));
  for (const e of configErrors(conf, process.env, existsSync(join(repo, UPDATER_CONF)))) fail(e);

  if (opts.exe) {
    const bytes = readFileSync(resolve(opts.exe));
    if (bytes.includes(Buffer.from(UPDATER_PUBKEY_MARKER, 'latin1'))) {
      fail(`${opts.exe} contains an updater public key; it was built with the updater config`);
    } else {
      console.log(`no updater public key in ${opts.exe} (${bytes.length} bytes scanned)`);
    }
  }

  const version = conf.version;
  let mv = null;
  try {
    mv = msixVersion(version);
  } catch (e) {
    fail(`${CONF}: ${e.message}`);
  }
  if (mv?.prerelease && !opts.allowPrerelease) {
    fail(
      `version ${version} is a pre-release; Store packages are built only for stable versions ` +
        '(dry runs pass --allow-prerelease)',
    );
  }

  const id = identityFrom(process.env);
  for (const e of identityErrors(id)) fail(e);
  if (id.defaults.length) {
    const msg =
      `NOT FOR THE STORE: fake identity used for ${id.defaults.join(', ')} ` +
      `(Name=${id.name}, Publisher=${id.publisher}). Set these GitHub Actions variables from ` +
      'Partner Center > Product identity (packaging/msix/README.md).';
    if (opts.requireIdentity) {
      fail(msg);
    } else {
      console.log('='.repeat(78));
      console.log(msg);
      console.log('='.repeat(78));
      console.log(`::warning title=msix identity::${annotate(msg)}`);
    }
  }

  if (errors.length) process.exit(1);

  if (opts.out) {
    const template = readFileSync(join(repo, TEMPLATE), 'utf8');
    writeFileSync(resolve(opts.out), fillTemplate(template, id, mv.msixVersion));
    console.log(`wrote ${resolve(opts.out)}`);
  }
  console.log(
    `msix: app version ${version}${mv.prerelease ? ' (pre-release)' : ''} -> package version ${mv.msixVersion}; ` +
      `identity ${id.name}, publisher ${id.publisher}${id.defaults.length ? ' (fake defaults)' : ''}; no updater config`,
  );
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(
      process.env.GITHUB_OUTPUT,
      `version=${version}\nmsix_version=${mv.msixVersion}\nprerelease=${mv.prerelease}\n` +
        `identity_name=${id.name}\npublisher=${id.publisher}\nidentity_defaults=${id.defaults.length > 0}\n`,
    );
  }
}

// Run only when started as a script (the functions above can be imported).
const samePath = (a, b) => (process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b);
if (process.argv[1] && samePath(resolve(process.argv[1]), fileURLToPath(import.meta.url))) main();
