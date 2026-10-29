#!/usr/bin/env node
// Prints a TAURI_CONFIG value (merged into tauri.conf.json at build time) that
// adds `--remote-debugging-port=<port>` to every window's WebView2 arguments,
// keeping wry's default arguments. For the Windows e2e build only: msedgedriver
// needs the DevTools port and WebView2 ignores WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
// once the app sets AdditionalBrowserArguments.
//   node scripts/e2e-tauri-config.mjs [port, default 0]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const port = process.argv[2] ?? '0';
const here = path.dirname(fileURLToPath(import.meta.url));
const conf = JSON.parse(fs.readFileSync(path.join(here, '..', 'apps', 'desktop', 'src-tauri', 'tauri.conf.json'), 'utf8'));
// wry's defaults when additionalBrowserArgs is unset (wry src/webview2/mod.rs).
const defaults = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection';
const windows = (conf.app?.windows ?? []).map((w) => ({
    ...w,
    additionalBrowserArgs: `${w.additionalBrowserArgs ?? defaults} --remote-debugging-port=${port}`,
}));
if (windows.length === 0) throw new Error('tauri.conf.json has no app.windows');
process.stdout.write(JSON.stringify({ app: { windows } }));
