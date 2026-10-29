#!/usr/bin/env node
// UI smoke test (PLAN 2 exit criterion, PLAN 4.6): onboarding -> preview ->
// organize -> undo -> history, driven through tauri-driver + WebDriver.
//
// Builds nothing. Needs a built app (with `tauri/custom-protocol`, so it
// serves the embedded dist/), `tauri-driver` and, on Linux, `WebKitWebDriver`
// on PATH and a display (e.g. `Xvfb :99` + DISPLAY=:99).
//
//   node tests/e2e/smoke.mjs --app target/debug/app [--screenshots dir]
//        [--driver tauri-driver] [--native-driver <WebKitWebDriver|msedgedriver>]
//        [--port 4444] [--keep-temp] [--allow-real-user-dirs]
//
// Isolation on Linux: tauri-driver (and so the app) runs with HOME and the
// XDG base dirs inside a fresh temp dir, and `user-dirs.dirs` points the
// Downloads/Desktop/Documents/Pictures folder tokens there. With
// `dbus-run-session` available it also gets a private session bus, so the
// single-instance plugin never reaches a Valet the user has open.
// On Windows the config dir and known folders cannot be redirected: the test
// refuses to run outside CI unless --allow-real-user-dirs is given.
//
// Exit code 0 on success, 1 on failure. On failure it prints a GitHub
// `::error::` annotation with the step and the last UI text, and saves
// `failure-<step>.png` into the screenshots dir.

import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Session, waitForServer } from './webdriver.mjs';

// ---------- arguments ----------

function parseArgs(argv) {
    const out = { driver: 'tauri-driver', nativeDriver: null, port: 4444, screenshots: null, keepTemp: false, allowReal: false };
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i];
        const next = () => {
            if (i + 1 >= argv.length) throw new Error(`${a} needs a value`);
            return argv[++i];
        };
        if (a === '--app') out.app = next();
        else if (a === '--screenshots') out.screenshots = next();
        else if (a === '--driver') out.driver = next();
        else if (a === '--native-driver') out.nativeDriver = next();
        else if (a === '--port') out.port = Number(next());
        else if (a === '--keep-temp') out.keepTemp = true;
        else if (a === '--allow-real-user-dirs') out.allowReal = true;
        else if (!a.startsWith('--') && out.app === undefined) out.app = a;
        else throw new Error(`unknown argument ${a}`);
    }
    if (!out.app) throw new Error('usage: smoke.mjs --app <path to app binary> [--screenshots <dir>]');
    out.app = path.resolve(out.app);
    if (!fs.existsSync(out.app)) throw new Error(`app binary not found: ${out.app}`);
    if (out.screenshots) out.screenshots = path.resolve(out.screenshots);
    return out;
}

// ---------- labels (apps/desktop/src/i18n/ro.json; the app starts in Romanian) ----------

const RO = {
    foldersTitle: 'Ce foldere să urmărească Valet?',
    presetsTitle: 'Ce reguli să folosească?',
    next: 'Continuă',
    downloads: 'Descărcări',
    desktop: 'Desktop',
    introStep: 'Pasul 3 din 3: previzualizare',
    executePrefix: 'Organizează fișierele',
    donePrefix: 'Operații făcute:',
    lastBatch: 'Ultimul lot',
    undoButton: 'Anulează lotul',
    undoPreviewPrefix: 'Fișiere de mutat înapoi:',
    undoConfirm: 'Confirmă anularea',
    undoResult: 'Rezultatul anulării',
    undonePrefix: 'Operații anulate:',
    close: 'Închide',
    tabHistory: 'Istoric',
    tabRules: 'Reguli',
    settings: 'Setări',
    kindApply: 'Organizare',
    kindUndo: 'Anulare',
    invoicesRule: 'Facturi și chitanțe',
};

/** XPath string literal (labels contain no double quotes). */
const lit = (s) => `"${s}"`;
/** data-testid attributes (apps/desktop/src, commit bd34a55); text XPath where the UI has none. */
const tid = (id) => `//*[@data-testid=${lit(id)}]`;
const X = {
    onboardingNext: tid('onboarding-next'),
    onboardingFinish: tid('onboarding-finish'),
    preset: (id) => tid(`preset-${id}`),
    presetAll: `//input[starts-with(@data-testid,'preset-')]`,
    previewExecute: tid('preview-execute'),
    navTab: (view) => tid(`nav-tab-${view}`),
    navSettings: tid('nav-settings'),
    heading: (text) => `//h2[normalize-space()=${lit(text)}]`,
    folderCheckbox: (label) =>
        `//label[contains(@class,'checkbox-label')][.//span[normalize-space()=${lit(label)}]]//input[@type='checkbox']`,
    presetList: `//ul[contains(@class,'preset-list')]/li`,
    introNote: `//*[@role='note']//strong[normalize-space()=${lit(RO.introStep)}]`,
    previewRows: `//div[contains(@class,'dry-run-results')]//table/tbody/tr`,
    success: `//div[contains(@class,'success-message')][starts-with(normalize-space(), ${lit(RO.donePrefix)})]`,
    lastBatch: `//section[@aria-label=${lit(RO.lastBatch)}]`,
    lastBatchUndo: `//section[@aria-label=${lit(RO.lastBatch)}]//button[@data-testid='last-batch-undo']`,
    undoPreviewTitle: `//section[@aria-label=${lit(RO.lastBatch)}]//h4[starts-with(normalize-space(), ${lit(RO.undoPreviewPrefix)})]`,
    undoConfirm: `//section[@aria-label=${lit(RO.lastBatch)}]//button[@data-testid='undo-confirm']`,
    undoResult: `//section[@aria-label=${lit(RO.lastBatch)}]//div[contains(@class,'undo-result')]`,
    undoResultClose: `//section[@aria-label=${lit(RO.lastBatch)}]//div[contains(@class,'undo-result')]//button[normalize-space()=${lit(RO.close)}]`,
    historyRows: `//tr[contains(@class,'history-row')]`,
};

// ---------- small helpers ----------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function assert(cond, message) {
    if (!cond) throw new Error(`assertion failed: ${message}`);
}

/** Sorted [relative path, sha256] of every regular file under `root`. */
function snapshot(root) {
    const out = [];
    const walk = (dir) => {
        for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
            const full = path.join(dir, ent.name);
            if (ent.isDirectory()) walk(full);
            else if (ent.isFile()) {
                const rel = path.relative(root, full).split(path.sep).join('/');
                out.push([rel, createHash('sha256').update(fs.readFileSync(full)).digest('hex')]);
            } else {
                out.push([path.relative(root, full).split(path.sep).join('/'), 'non-regular']);
            }
        }
    };
    walk(root);
    return out.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
}

function listDirs(root) {
    const out = [];
    const walk = (dir) => {
        for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
            if (ent.isDirectory()) {
                const full = path.join(dir, ent.name);
                out.push(path.relative(root, full).split(path.sep).join('/'));
                walk(full);
            }
        }
    };
    walk(root);
    return out.sort();
}

function annotationEscape(s) {
    return s.replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
}

// ---------- environment ----------

/** Temp HOME with XDG dirs and user-dirs.dirs (Linux), or the real folders (Windows). */
function prepareEnvironment(opts) {
    if (process.platform === 'win32') {
        if (!process.env.CI && !opts.allowReal) {
            throw new Error('on Windows the app uses the real Downloads and config dir; run only on CI or pass --allow-real-user-dirs');
        }
        const profile = process.env.USERPROFILE;
        const dirs = {
            downloads: path.join(profile, 'Downloads'),
            desktop: path.join(profile, 'Desktop'),
            documents: path.join(profile, 'Documents'),
            pictures: path.join(profile, 'Pictures'),
        };
        for (const d of Object.values(dirs)) fs.mkdirSync(d, { recursive: true });
        return { tmp: null, dirs, env: { ...process.env } };
    }

    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-e2e-'));
    const home = path.join(tmp, 'home');
    const config = path.join(home, '.config');
    const dirs = {
        downloads: path.join(home, 'Downloads'),
        desktop: path.join(home, 'Desktop'),
        documents: path.join(home, 'Documents'),
        pictures: path.join(home, 'Pictures'),
    };
    for (const d of [config, path.join(home, '.local', 'share'), path.join(home, '.cache'), path.join(home, '.local', 'state'), ...Object.values(dirs)]) {
        fs.mkdirSync(d, { recursive: true });
    }
    fs.writeFileSync(
        path.join(config, 'user-dirs.dirs'),
        [
            `XDG_DOWNLOAD_DIR="${dirs.downloads}"`,
            `XDG_DESKTOP_DIR="${dirs.desktop}"`,
            `XDG_DOCUMENTS_DIR="${dirs.documents}"`,
            `XDG_PICTURES_DIR="${dirs.pictures}"`,
            '',
        ].join('\n'),
    );
    const env = {
        ...process.env,
        HOME: home,
        XDG_CONFIG_HOME: config,
        XDG_DATA_HOME: path.join(home, '.local', 'share'),
        XDG_CACHE_HOME: path.join(home, '.cache'),
        XDG_STATE_HOME: path.join(home, '.local', 'state'),
        // OS locale of a user in Romania (GTK/WebKit, process locale). UI dates
        // follow i18n.language (src/i18n/format.ts), not this.
        LANG: 'ro_RO.UTF-8',
        LC_ALL: 'ro_RO.UTF-8',
        // A software-rendered WebKit under Xvfb; avoids GPU/DMABUF failures.
        WEBKIT_DISABLE_DMABUF_RENDERER: '1',
        WEBKIT_DISABLE_COMPOSITING_MODE: '1',
        LIBGL_ALWAYS_SOFTWARE: '1',
    };
    delete env.XDG_RUNTIME_DIR;
    delete env.DBUS_SESSION_BUS_ADDRESS;
    return { tmp, dirs, env };
}

/** Fixture in Downloads: mtimes two hours back, so none counts as "being written". */
function writeFixture(downloads) {
    const files = {
        'factura_2026.pdf': '%PDF-1.4\nfactura 2026\n',
        'poza.jpg': Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0, 16, 0x4a, 0x46, 0x49, 0x46, 0, 1, 0xff, 0xd9]),
        'arhiva.zip': Buffer.from([0x50, 0x4b, 0x05, 0x06, ...new Array(18).fill(0)]),
        'setup.exe': Buffer.concat([Buffer.from('MZ'), Buffer.alloc(126, 0x90)]),
        'bonus.pdf': '%PDF-1.4\nbonus, not an invoice\n',
    };
    const mtime = new Date(Date.now() - 2 * 3600 * 1000);
    for (const [name, data] of Object.entries(files)) {
        const p = path.join(downloads, name);
        if (fs.existsSync(p)) throw new Error(`fixture file already exists, refusing to overwrite: ${p}`);
        fs.writeFileSync(p, data);
        fs.utimesSync(p, mtime, mtime);
    }
    return { names: Object.keys(files), year: String(mtime.getFullYear()) };
}

function startDriver(opts, env) {
    let cmd = opts.driver;
    let args = ['--port', String(opts.port)];
    if (opts.nativeDriver) args.push('--native-driver', opts.nativeDriver);
    if (process.platform === 'linux' && spawnSync('dbus-run-session', ['--version']).status === 0) {
        args = ['--', cmd, ...args];
        cmd = 'dbus-run-session';
    }
    const log = [];
    // Own process group on Unix, so teardown also stops what dbus-run-session started.
    const child = spawn(cmd, args, { env, stdio: ['ignore', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
    const keep = (chunk) => {
        log.push(chunk.toString());
        if (log.length > 400) log.splice(0, log.length - 400);
    };
    child.stdout.on('data', keep);
    child.stderr.on('data', keep);
    child.on('error', (err) => log.push(`spawn error: ${err.message}\n`));
    return { child, log };
}

function stopDriver(child, signal = 'SIGTERM') {
    try {
        if (process.platform === 'win32') child.kill();
        else process.kill(-child.pid, signal);
    } catch {
        // already gone
    }
}

// ---------- the test ----------

async function main() {
    const opts = parseArgs(process.argv.slice(2));
    if (opts.screenshots) fs.mkdirSync(opts.screenshots, { recursive: true });
    const t0 = Date.now();
    const timings = [];
    let currentStep = 'setup';
    let session = null;
    let driver = null;
    let envInfo = null;

    // `css`: element to scroll to the top of the view; default: the top of the page.
    const shot = async (name, css = null) => {
        if (!opts.screenshots || !session) return;
        // Clicks scroll the page; captures for the user guide start at the top.
        await session.execute(
            "document.activeElement?.blur(); window.scrollTo(0, 0); for (const e of document.querySelectorAll('*')) if (e.scrollTop) e.scrollTop = 0;" +
                'const el = arguments[0] && document.querySelector(arguments[0]); if (el) el.scrollIntoView({ block: "start" });',
            [css],
        );
        await sleep(100);
        const b64 = await session.screenshot();
        const file = path.join(opts.screenshots, `${name}.png`);
        fs.writeFileSync(file, Buffer.from(b64, 'base64'));
        console.log(`  screenshot ${file} (${fs.statSync(file).size} bytes)`);
    };

    const step = async (name, fn) => {
        currentStep = name;
        const s = Date.now();
        console.log(`- ${name}`);
        const r = await fn();
        const ms = Date.now() - s;
        timings.push([name, ms]);
        console.log(`  ok (${ms} ms)`);
        return r;
    };

    try {
        envInfo = prepareEnvironment(opts);
        const { dirs, env } = envInfo;
        if (envInfo.tmp) console.log(`temp HOME: ${env.HOME}`);
        const fixture = writeFixture(dirs.downloads);
        const before = snapshot(dirs.downloads);
        const documentsBefore = snapshot(dirs.documents);

        const base = `http://127.0.0.1:${opts.port}`;
        await step('start tauri-driver and the app', async () => {
            driver = startDriver(opts, env);
            await waitForServer(base);
            session = await Session.create(base, opts.app);
            try {
                await session.setWindowSize(1100, 750);
            } catch (err) {
                console.log(`  window resize not applied: ${err.message}`);
            }
        });

        await step('onboarding step 1: folders', async () => {
            await session.waitFor(X.heading(RO.foldersTitle), { timeout: 30000, what: 'onboarding step 1 heading' });
            const downloads = await session.xpath(X.folderCheckbox(RO.downloads));
            assert(await downloads.selected(), 'Descărcări is checked by default');
            const desktop = await session.xpath(X.folderCheckbox(RO.desktop));
            if (!fs.existsSync(dirs.desktop) && (await desktop.selected())) await desktop.click();
            await sleep(300); // let the window settle at the new size before the capture
            await shot('onboarding-1-folders');
            const next = await session.xpath(X.onboardingNext);
            assert((await next.text()).trim() === RO.next, 'step 1 button reads Continuă');
            await next.click();
        });

        await step('onboarding step 2: presets (defaults)', async () => {
            await session.waitFor(X.heading(RO.presetsTitle), { what: 'onboarding step 2 heading' });
            await session.waitFor(X.presetList, { what: 'preset list' });
            const items = await session.xpathAll(X.presetList);
            assert(items.length === 8, `8 presets listed, got ${items.length}`);
            const boxes = await session.xpathAll(X.presetAll);
            assert(boxes.length === 8, `8 preset checkboxes (data-testid preset-*), got ${boxes.length}`);
            for (const id of ['downloads-by-type', 'invoices']) {
                assert(await (await session.xpath(X.preset(id))).selected(), `preset ${id} is checked by default`);
            }
            await shot('onboarding-2-presets');
            const next = await session.waitFor(`${X.onboardingFinish}[not(@disabled)]`, { what: 'enabled onboarding-finish' });
            await next.click();
        });

        const planned = await step('preview lists the planned moves', async () => {
            await session.waitFor(X.introNote, { timeout: 30000, what: 'preview intro (step 3 of 3)' });
            await session.waitFor(X.previewRows, { what: 'preview rows' });
            const rows = await session.xpathAll(X.previewRows);
            assert(rows.length > 0, 'preview has operations');
            const ops = [];
            for (const row of rows) {
                const cells = await row.xpathAll('./td');
                // textContent, not the rendered text: WebKitWebDriver returns '' for
                // cells clipped by `text-overflow: ellipsis` or below the viewport.
                const text = await Promise.all(cells.map((c) => c.property('textContent')));
                ops.push({ file: text[0].trim(), op: text[1].trim(), from: text[2].trim(), to: text[3].trim(), rule: text[4].trim() });
            }
            console.log(`  ${ops.length} operations:`);
            for (const o of ops) console.log(`    ${o.file}: ${o.op} -> ${o.to} [${o.rule}]`);
            const byFile = Object.fromEntries(ops.map((o) => [o.file, o]));
            for (const name of fixture.names) assert(byFile[name], `${name} is in the preview`);
            const under = (p, dir) => p === dir || p.startsWith(dir + path.sep);
            const inv = byFile['factura_2026.pdf'];
            const invDir = path.join(dirs.documents, 'Facturi', fixture.year);
            assert(under(inv.to, invDir), `factura_2026.pdf goes to ${invDir}, got ${inv.to}`);
            assert(inv.rule === RO.invoicesRule, `factura_2026.pdf rule is ${RO.invoicesRule}, got ${inv.rule}`);
            const imgDir = path.join(dirs.downloads, 'Imagini');
            assert(under(byFile['poza.jpg'].to, imgDir), `poza.jpg goes to ${imgDir}, got ${byFile['poza.jpg'].to}`);
            assert(under(byFile['arhiva.zip'].to, path.join(dirs.downloads, 'Arhive')), `arhiva.zip goes to Downloads/Arhive, got ${byFile['arhiva.zip'].to}`);
            assert(under(byFile['setup.exe'].to, path.join(dirs.downloads, 'Instalatoare')), `setup.exe goes to Downloads/Instalatoare, got ${byFile['setup.exe'].to}`);
            assert(under(byFile['bonus.pdf'].to, path.join(dirs.downloads, 'Documente')), `bonus.pdf goes to Downloads/Documente (not an invoice), got ${byFile['bonus.pdf'].to}`);
            const execute = await session.xpath(X.previewExecute);
            const label = await execute.text();
            assert(label.startsWith(RO.executePrefix), `preview-execute reads "${RO.executePrefix} (N)": "${label}"`);
            assert(label.includes(`(${ops.length})`), `Organize button shows the count ${ops.length}: "${label}"`);
            await shot('preview', '.dry-run-controls');
            const skippedRows = await session.xpathAll(`//div[contains(@class,'skipped-files')]`);
            if (skippedRows.length > 0) {
                await (await session.xpath(`//div[contains(@class,'skipped-files')]//button`)).click();
                const rowsS = await session.xpathAll(`//div[contains(@class,'skipped-files')]//tbody/tr`);
                for (const r of rowsS) console.log(`    skipped: ${(await r.property('textContent')).trim()}`);
            }
            return ops;
        });

        // Destination file path of an operation (`destination_path` may be the folder or the file).
        const destFile = (o) => (path.basename(o.to) === o.file ? o.to : path.join(o.to, o.file));

        await step('organize moves the files', async () => {
            await (await session.xpath(X.previewExecute)).click();
            const ok = await session.waitFor(X.success, { timeout: 30000, what: 'success message "Operații făcute"' });
            const msg = await ok.text();
            assert(msg.includes(`${planned.length}`), `success message counts ${planned.length}: "${msg}"`);
            for (const o of planned) {
                const src = path.join(dirs.downloads, o.file);
                assert(!fs.existsSync(src), `${src} left the source`);
                assert(fs.existsSync(destFile(o)), `${destFile(o)} exists`);
            }
            const undo = await session.waitFor(X.lastBatchUndo, { what: 'last-batch box with "Anulează lotul"' });
            assert((await undo.text()).trim() === RO.undoButton, `last-batch-undo reads ${RO.undoButton}`);
            await shot('after-organize');
        });

        await step('undo the batch (preview, confirm)', async () => {
            await (await session.xpath(X.lastBatchUndo)).click();
            const title = await session.waitFor(X.undoPreviewTitle, { what: 'undo preview title' });
            const t = await title.text();
            assert(t.endsWith(`${planned.length}`), `undo preview lists ${planned.length} files: "${t}"`);
            await shot('undo-confirm');
            const confirm = await session.xpath(X.undoConfirm);
            assert((await confirm.text()).trim() === RO.undoConfirm, `undo-confirm reads ${RO.undoConfirm}`);
            await confirm.click();
            const result = await session.waitFor(X.undoResult, { timeout: 30000, what: 'undo result' });
            const text = await result.text();
            assert(text.includes(`${RO.undonePrefix} ${planned.length}`), `undo result says ${planned.length} undone: "${text}"`);
            const after = snapshot(dirs.downloads);
            assert(
                JSON.stringify(after) === JSON.stringify(before),
                `Downloads byte-identical after undo\nbefore: ${JSON.stringify(before)}\nafter:  ${JSON.stringify(after)}`,
            );
            const documentsAfter = snapshot(dirs.documents);
            assert(
                JSON.stringify(documentsAfter) === JSON.stringify(documentsBefore),
                `Documents has the same files after undo: ${JSON.stringify(documentsAfter)}`,
            );
            console.log(`  Downloads: ${after.length} files, identical (sha256) to the initial snapshot`);
            console.log(`  folders left after undo: Downloads [${listDirs(dirs.downloads).join(', ')}], Documents [${listDirs(dirs.documents).join(', ')}]`);
            await (await session.xpath(X.undoResultClose)).click();
            await session.waitGone(X.lastBatch, { what: 'last-batch box' });
        });

        await step('history shows the apply and the undo batches', async () => {
            await (await session.xpath(X.navTab('history'))).click();
            await session.waitFor(X.heading(RO.tabHistory), { what: 'History heading' });
            await session.waitFor(X.historyRows, { what: 'history rows' });
            const rows = await session.xpathAll(X.historyRows);
            const kinds = [];
            for (const row of rows) kinds.push((await (await row.xpathAll('./td[1]'))[0].property('textContent')).trim());
            console.log(`  batches: ${kinds.join(', ')}`);
            assert(kinds.includes(RO.kindApply), `an "${RO.kindApply}" batch is listed`);
            assert(kinds.includes(RO.kindUndo), `an "${RO.kindUndo}" batch is listed`);
            // Open the apply batch so the capture shows its operations.
            // Click the cell: WebKitWebDriver reports a <tr> as not interactable.
            const apply = rows[kinds.indexOf(RO.kindApply)];
            await (await apply.xpathAll('./td[1]'))[0].click();
            await session.waitFor(`//div[contains(@class,'history-detail')]//table/tbody/tr`, { what: 'history detail rows' });
            await shot('history');
        });

        await step('rules screen', async () => {
            await (await session.xpath(X.navTab('rules'))).click();
            await session.waitFor(X.heading(RO.tabRules), { what: 'Rules heading' });
            await session.waitFor(`//*[normalize-space()=${lit(RO.invoicesRule)}]`, { what: 'installed preset rule in the list' });
            await shot('rules');
        });

        await step('settings screen', async () => {
            await (await session.xpath(X.navSettings)).click();
            await session.waitFor(X.heading(RO.settings), { what: 'Settings heading' });
            await sleep(300);
            await shot('settings');
        });

        currentStep = 'teardown';
        console.log('\nsmoke test passed');
        for (const [name, ms] of timings) console.log(`  ${String(ms).padStart(6)} ms  ${name}`);
        console.log(`  ${String(Date.now() - t0).padStart(6)} ms  total`);
        if (process.env.GITHUB_ACTIONS) {
            const lines = timings.map(([name, ms]) => `${ms} ms  ${name}`).concat(`${Date.now() - t0} ms  total`);
            console.log(`::notice title=e2e smoke passed (${process.platform})::${annotationEscape(lines.join('\n'))}`);
        }
        return 0;
    } catch (err) {
        let ui = '';
        try {
            if (session) ui = (await session.bodyText()).trim();
        } catch (e) {
            ui = `(UI text unavailable: ${e.message})`;
        }
        try {
            await shot(`failure-${currentStep.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}`);
        } catch {
            // ignore
        }
        const tail = ui.length > 800 ? '...' + ui.slice(-800) : ui;
        console.error(`\nsmoke test FAILED at step "${currentStep}": ${err.stack || err.message}`);
        console.error(`last UI text:\n${tail}`);
        if (driver) console.error(`tauri-driver / app output (tail):\n${driver.log.join('').slice(-3000)}`);
        console.log(
            `::error title=e2e smoke failed at: ${annotationEscape(currentStep)}::${annotationEscape(
                `${err.message}\nLast UI text: ${tail.replace(/\s+/g, ' ')}`,
            )}`,
        );
        if (driver) {
            const out = driver.log.join('').trim();
            console.log(`::notice title=e2e driver and app output (tail)::${annotationEscape(out ? out.slice(-4000) : '(no output)')}`);
        }
        return 1;
    } finally {
        if (session) {
            try {
                await session.delete();
            } catch {
                // the app may already be gone
            }
        }
        if (driver) stopDriver(driver.child);
        if (driver) await sleep(500);
        if (driver) stopDriver(driver.child, 'SIGKILL');
        if (envInfo?.tmp) {
            if (opts.keepTemp) console.log(`kept ${envInfo.tmp}`);
            else fs.rmSync(envInfo.tmp, { recursive: true, force: true });
        }
    }
}

process.exitCode = await main();
