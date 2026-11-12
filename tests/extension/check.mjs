#!/usr/bin/env node
/*
 * Créno — Firefox extension package validation (static, offline).
 *
 *  1. When tools/build-extension.sh does not exist yet: print SKIP and exit 0
 *     (the extension agent has not landed its work).
 *  2. Otherwise: run `bash tools/build-extension.sh --check`, build twice,
 *     assert the zip is byte-identical across the two builds, then inspect it
 *     with python3 -m zipfile/zipfile: manifest, permissions, forbidden files,
 *     size budget.
 *
 * Run:  node tests/extension/check.mjs
 * Exit: 0 pass or skip, 1 a check failed, 2 setup error.
 */

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(process.env.CRENO_REPO || path.join(import.meta.dirname, '..', '..'));
const BUILD = path.join(ROOT, 'tools', 'build-extension.sh');
const MANIFEST = path.join(ROOT, 'extension', 'manifest.json');
const ZIP = path.join(ROOT, 'dist', 'creno-extension.zip');
const SIZE_BUDGET = 250 * 1024; // 250 KB

const EXPECTED_PERMISSIONS = ['tabs', 'https://api.skello.io/*'];
const GECKO_ID = 'creno@loganbozier.fr';
const FORBIDDEN_FILES = ['sw.js', 'manifest.webmanifest', 'readme.md'];
const FORBIDDEN_PERMISSION_HOSTS = ['allorigins', 'corsproxy', 'codetabs', 'relay.test'];

const failures = [];
const fail = (label, detail) => failures.push({ label, detail });

function run(cmd, args) {
  return spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024 });
}

function showFailure(name, res) {
  const out = `${res.stdout || ''}${res.stderr || ''}`.trim().split('\n').slice(-12).join('\n');
  console.error(`[extension] ${name} failed (exit ${res.status ?? res.signal ?? '?'}).`);
  if (out) console.error(out);
}

function validateManifest(manifest, label) {
  if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest)) {
    fail(`${label}: manifest object`, 'not a JSON object');
    return;
  }
  if (manifest.manifest_version !== 2) fail(`${label}: manifest_version`, `expected 2, got ${JSON.stringify(manifest.manifest_version)}`);

  const bg = manifest.background;
  if (!bg || typeof bg !== 'object') fail(`${label}: background`, 'missing');
  else if (bg.persistent !== false) fail(`${label}: background.persistent`, `expected false, got ${JSON.stringify(bg.persistent)}`);

  const gecko = manifest.browser_specific_settings?.gecko?.id ?? manifest.applications?.gecko?.id;
  if (gecko !== GECKO_ID) fail(`${label}: gecko id`, `expected ${GECKO_ID}, got ${JSON.stringify(gecko)}`);

  const perms = manifest.permissions;
  if (!Array.isArray(perms)) { fail(`${label}: permissions`, 'missing or not an array'); return; }
  const sorted = [...perms].sort();
  const expected = [...EXPECTED_PERMISSIONS].sort();
  if (sorted.length !== expected.length || sorted.some((p, i) => p !== expected[i])) {
    fail(`${label}: permissions`, `expected exactly ${JSON.stringify(EXPECTED_PERMISSIONS)}, got ${JSON.stringify(perms)}`);
  }
  const badHost = perms.find(p => typeof p === 'string' && FORBIDDEN_PERMISSION_HOSTS.some(h => p.toLowerCase().includes(h)));
  if (badHost) fail(`${label}: relay host still in permissions`, badHost);
}

function zipEntries(zip) {
  const py = [
    'import json, sys, zipfile',
    'z = zipfile.ZipFile(sys.argv[1])',
    'entries = [{"name": i.filename, "size": i.file_size} for i in z.infolist()]',
    'manifest = None',
    'for e in z.infolist():',
    '    if e.filename.rstrip("/").split("/")[-1] == "manifest.json":',
    '        manifest = z.read(e.filename).decode("utf-8", "replace")',
    '        break',
    'print(json.dumps({"entries": entries, "bad": z.testzip(), "manifestText": manifest}))',
  ].join('\n');
  const res = run('python3', ['-c', py, zip]);
  if (res.status !== 0) throw new Error(`python3 zipfile failed: ${(res.stderr || '').trim()}`);
  return JSON.parse(res.stdout);
}

function sha256(file) {
  return createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function main() {
  if (!fs.existsSync(BUILD)) {
    console.log('SKIP extension package: tools/build-extension.sh not built yet (extension agent has not landed)');
    process.exit(0);
  }

  if (!fs.existsSync(MANIFEST)) {
    console.error('[extension] FAIL extension/manifest.json missing while tools/build-extension.sh exists');
    process.exit(1);
  }

  let manifest;
  try { manifest = JSON.parse(fs.readFileSync(MANIFEST, 'utf8')); }
  catch (e) { console.error(`[extension] FAIL extension/manifest.json is not valid JSON: ${e.message}`); process.exit(1); }
  validateManifest(manifest, 'extension/manifest.json');

  const check = run('bash', ['tools/build-extension.sh', '--check']);
  if (check.status !== 0) {
    showFailure('tools/build-extension.sh --check', check);
    fail('build --check', `exit ${check.status}`);
  }

  const build1 = run('bash', ['tools/build-extension.sh']);
  if (build1.status !== 0) {
    showFailure('tools/build-extension.sh', build1);
    fail('build #1', `exit ${build1.status}`);
  }
  if (!fs.existsSync(ZIP)) {
    console.error('[extension] FAIL dist/creno-extension.zip was not produced by the build');
    process.exit(1);
  }
  const size = fs.statSync(ZIP).size;
  const hash1 = sha256(ZIP);
  if (size === 0) fail('zip not empty', '0 byte');
  if (size > SIZE_BUDGET) fail('zip size budget', `${size} B > ${SIZE_BUDGET} B (250 KB)`);

  let parsed;
  try { parsed = zipEntries(ZIP); }
  catch (e) { console.error(`[extension] FAIL ${e.message}`); process.exit(1); }
  if (parsed.bad) fail('zip integrity', `corrupt entry: ${parsed.bad}`);

  const names = parsed.entries.map(e => e.name.replace(/\\/g, '/'));
  const base = n => n.split('/').filter(Boolean).pop() || '';
  const has = wanted => names.some(n => base(n) === wanted);

  if (!has('manifest.json')) fail('zip contents', 'manifest.json missing');
  if (!has('background.js')) fail('zip contents', 'background.js missing');
  for (const forbidden of FORBIDDEN_FILES) {
    const hit = names.find(n => base(n).toLowerCase() === forbidden);
    if (hit) fail('forbidden file in zip', hit);
  }

  if (parsed.manifestText == null) fail('zip manifest', 'no manifest.json readable inside the zip');
  else {
    try { validateManifest(JSON.parse(parsed.manifestText), 'zip manifest.json'); }
    catch (e) { fail('zip manifest.json: valid JSON', e.message); }
  }

  const build2 = run('bash', ['tools/build-extension.sh']);
  if (build2.status !== 0) {
    showFailure('tools/build-extension.sh (second run)', build2);
    fail('build #2', `exit ${build2.status}`);
  } else {
    const hash2 = sha256(ZIP);
    if (hash1 !== hash2) fail('reproducible zip', `sha256 differs across two builds: ${hash1} != ${hash2}`);
    else console.log(`[extension] sha256 stable across two builds: ${hash1}`);
  }

  if (failures.length) {
    console.log(`FAIL extension package (${failures.length} check(s) failed)`);
    for (const f of failures) console.log(`     ✗ ${f.label}${f.detail ? `: ${f.detail}` : ''}`);
    process.exit(1);
  }
  console.log(`PASS extension package: ${names.length} entries, ${size} B (budget ${SIZE_BUDGET} B), sha256 ${hash1.slice(0, 16)}…`);
  process.exit(0);
}

try { main(); }
catch (e) { console.error(`[extension] SETUP ERROR: ${e.message}`); process.exit(2); }
