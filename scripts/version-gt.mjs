#!/usr/bin/env node
// Exits 0 when version <a> is higher than <b> by SemVer 2.0 precedence,
// 1 otherwise (2 on bad input). Used by the release workflow to keep
// latest-beta.json from moving to a lower version.
//   node scripts/version-gt.mjs 1.0.0 1.0.0-beta.2   -> exit 0
import { VERSION_RE } from './version-files.mjs';

export function compare(a, b) {
  const split = (v) => {
    const [main, ...rest] = v.split('-');
    return [main.split('.').map(Number), rest.length ? rest.join('-').split('.') : []];
  };
  const [am, ap] = split(a);
  const [bm, bp] = split(b);
  for (let i = 0; i < 3; i++) if (am[i] !== bm[i]) return am[i] > bm[i] ? 1 : -1;
  if (!ap.length || !bp.length) return ap.length === bp.length ? 0 : ap.length ? -1 : 1;
  for (let i = 0; i < Math.min(ap.length, bp.length); i++) {
    const x = ap[i];
    const y = bp[i];
    if (x === y) continue;
    const xn = /^\d+$/.test(x);
    const yn = /^\d+$/.test(y);
    if (xn && yn) return Number(x) > Number(y) ? 1 : -1;
    if (xn !== yn) return xn ? -1 : 1;
    return x > y ? 1 : -1;
  }
  return ap.length === bp.length ? 0 : ap.length > bp.length ? 1 : -1;
}

if (process.argv[1]?.endsWith('version-gt.mjs')) {
  const [a, b] = process.argv.slice(2);
  if (!VERSION_RE.test(a ?? '') || !VERSION_RE.test(b ?? '')) {
    console.error('usage: node scripts/version-gt.mjs <a> <b>');
    process.exit(2);
  }
  process.exit(compare(a, b) > 0 ? 0 : 1);
}
