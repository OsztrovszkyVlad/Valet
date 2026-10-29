#!/usr/bin/env node
// Dev check of the static site (site/): renders each page with Playwright Chromium at
// 360x800 and 1280x800, in the light and the dark color scheme, and fails on
//   - horizontal overflow (document.documentElement.scrollWidth > innerWidth),
//   - any request to a URL that is not file:// (the site must load nothing remote),
//   - text below WCAG AA contrast (4.5:1) for body text, links, headings and buttons
//     against their effective background.
// Missing local files (e.g. a placeholder image not added yet) are listed as warnings.
//
//   node scripts/check-site.mjs [--shots <dir>] [page.html ...]
//
// Default pages: site/index.html and site/en/index.html. Needs the `playwright` package
// with its Chromium (resolved through NODE_PATH). Not part of CI.

import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
let chromium;
try {
  ({ chromium } = require('playwright'));
} catch {
  console.error('check-site: the playwright package is not installed (npm i -g playwright; npx playwright install chromium)');
  process.exit(2);
}

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
let shots = null;
const pages = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--shots') shots = path.resolve(args[++i]);
  else pages.push(path.resolve(args[i]));
}
if (pages.length === 0) pages.push(path.join(repo, 'site/index.html'), path.join(repo, 'site/en/index.html'));
if (shots) fs.mkdirSync(shots, { recursive: true });

const VIEWPORTS = [
  { width: 360, height: 800 },
  { width: 1280, height: 800 },
];
const SCHEMES = ['light', 'dark'];
const MIN_RATIO = 4.5;

// Runs in the page: every element with its own visible text, its color and the
// effective background (ancestors' background colors composited, white canvas).
function collectContrast() {
  const parse = (c) => {
    const m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const over = (top, bottom) => {
    const a = top.a + bottom.a * (1 - top.a);
    if (a === 0) return { r: 0, g: 0, b: 0, a: 0 };
    const mix = (k) => (top[k] * top.a + bottom[k] * bottom.a * (1 - top.a)) / a;
    return { r: mix('r'), g: mix('g'), b: mix('b'), a };
  };
  const canvas = (() => {
    // The canvas takes the root's (or body's) background; else white.
    for (const el of [document.documentElement, document.body]) {
      const c = parse(getComputedStyle(el).backgroundColor);
      if (c && c.a > 0) return over(c, { r: 255, g: 255, b: 255, a: 1 });
    }
    return { r: 255, g: 255, b: 255, a: 1 };
  })();
  const background = (el) => {
    const layers = [];
    for (let e = el; e; e = e.parentElement) {
      const c = parse(getComputedStyle(e).backgroundColor);
      if (c && c.a > 0) layers.push(c);
      if (c && c.a === 1) break;
    }
    let bg = canvas;
    for (let i = layers.length - 1; i >= 0; i--) bg = over(layers[i], bg);
    return bg;
  };
  const lum = ({ r, g, b }) => {
    const f = (v) => {
      v /= 255;
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  const ratio = (a, b) => {
    const [x, y] = [lum(a), lum(b)].sort((m, n) => n - m);
    return (x + 0.05) / (y + 0.05);
  };
  const label = (el) => {
    const cls = el.classList.length ? '.' + [...el.classList].join('.') : '';
    return `${el.tagName.toLowerCase()}${cls}`;
  };
  const out = [];
  for (const el of document.body.querySelectorAll('*')) {
    const own = [...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent.trim() !== '');
    if (!own) continue;
    const st = getComputedStyle(el);
    if (st.visibility === 'hidden' || st.display === 'none') continue;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) continue;
    // Off-screen until focused (skip link): checked in its focused state elsewhere.
    if (rect.right < 0 || rect.left > document.documentElement.scrollWidth) continue;
    const fg0 = parse(st.color);
    const bg = background(el);
    const fg = over(fg0, bg);
    out.push({
      el: label(el),
      text: el.textContent.trim().replace(/\s+/g, ' ').slice(0, 50),
      color: st.color,
      bg: `rgb(${Math.round(bg.r)}, ${Math.round(bg.g)}, ${Math.round(bg.b)})`,
      ratio: Math.round(ratio(fg, bg) * 100) / 100,
      button: el.closest('.button, button') !== null,
    });
  }
  return out;
}

const failures = [];
const warnings = [];
const summary = [];
const browser = await chromium.launch();
try {
  for (const page of pages) {
    const rel = path.relative(repo, page);
    const url = pathToFileURL(page).href;
    const darkColors = {};
    for (const scheme of SCHEMES) {
      for (const vp of VIEWPORTS) {
        const ctx = await browser.newContext({ viewport: vp, colorScheme: scheme });
        const tab = await ctx.newPage();
        const remote = [];
        const missing = [];
        await tab.route('**/*', (route) => {
          const u = route.request().url();
          if (!u.startsWith('file:')) {
            remote.push(u);
            return route.abort();
          }
          return route.continue();
        });
        tab.on('requestfailed', (req) => {
          const u = req.url();
          if (u.startsWith('file:')) missing.push(path.relative(repo, fileURLToPath(u)));
        });
        await tab.goto(url, { waitUntil: 'load' });
        const where = `${rel} ${vp.width}x${vp.height} ${scheme}`;
        const { scrollWidth, innerWidth, wide } = await tab.evaluate(() => {
          const iw = window.innerWidth;
          const w = [];
          for (const el of document.body.querySelectorAll('*')) {
            const r = el.getBoundingClientRect();
            if (r.right > iw + 0.5 && r.width > 0) w.push(`${el.tagName.toLowerCase()}${el.className ? '.' + el.className : ''} right=${Math.round(r.right)}`);
          }
          return { scrollWidth: document.documentElement.scrollWidth, innerWidth: iw, wide: w.slice(0, 5) };
        });
        if (scrollWidth > innerWidth) {
          failures.push(`${where}: horizontal overflow, scrollWidth ${scrollWidth} > innerWidth ${innerWidth} (${wide.join('; ')})`);
        }
        for (const u of remote) failures.push(`${where}: request to a non-file URL: ${u}`);
        for (const m of missing) warnings.push(`${where}: missing local file ${m}`);

        const items = await tab.evaluate(collectContrast);
        // The skip link, focused.
        const skip = await tab.$('.skip');
        if (skip) {
          await tab.keyboard.press('Tab');
          const focused = await tab.evaluate(collectContrast);
          items.push(...focused.filter((i) => i.el.startsWith('a.skip')));
        }
        const low = items.filter((i) => i.ratio < MIN_RATIO);
        for (const i of low) {
          failures.push(`${where}: contrast ${i.ratio}:1 < ${MIN_RATIO}:1 for ${i.el} "${i.text}" (${i.color} on ${i.bg})`);
        }
        const body = items.filter((i) => !i.button);
        const buttons = items.filter((i) => i.button);
        const min = (list) => (list.length ? Math.min(...list.map((i) => i.ratio)) : null);
        summary.push(
          `${where}: scrollWidth ${scrollWidth}/${innerWidth}, ${items.length} text elements, ` +
            `min contrast text ${min(body)}:1, buttons ${min(buttons)}:1 (${buttons.length}), remote requests ${remote.length}`,
        );
        if (vp.width === 1280) darkColors[scheme] = items.map((i) => `${i.color}|${i.bg}`).join(',');
        if (shots) {
          const name = `${rel.replace(/[\\/]/g, '_').replace(/\.html$/, '')}-${vp.width}-${scheme}.png`;
          await tab.goto(url, { waitUntil: 'load' });
          await tab.screenshot({ path: path.join(shots, name), fullPage: true });
        }
        await ctx.close();
      }
    }
    if (darkColors.light === darkColors.dark) {
      summary.push(`${rel}: no dark color scheme (light and dark render the same colors)`);
    }
  }
} finally {
  await browser.close();
}

for (const s of summary) console.log(s);
for (const w of warnings) console.log(`warning: ${w}`);
if (failures.length) {
  for (const f of failures) console.error(`FAIL: ${f}`);
  console.error(`check-site: ${failures.length} failure(s)`);
  process.exit(1);
}
console.log(`check-site: ok (${pages.length} pages x ${VIEWPORTS.length} viewports x ${SCHEMES.length} schemes)`);
