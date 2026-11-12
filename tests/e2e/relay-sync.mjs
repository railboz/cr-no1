#!/usr/bin/env node
/*
 * Créno — private relay sync E2E harness (real browser, no network, no build).
 *
 * Serves the repository on 127.0.0.1 with python3 -m http.server, launches the
 * already-installed Brave binary through the Playwright module found in the npm
 * npx cache, and intercepts EVERY external request with page.route:
 *   - the direct Skello feed always fails (network error),
 *   - the private relay and the three public relays answer per scenario,
 *   - nothing ever leaves the machine.
 *
 * Run:  node tests/e2e/relay-sync.mjs
 * Env:  CRENO_REPO=/path/to/app          repository (or a temp copy) under test
 *       CRENO_BROWSER=/path/to/browser   browser executable (default: Brave)
 *       PLAYWRIGHT_MODULE=/path/to/playwright
 *
 * Exit: 0 all cases pass, 1 at least one case fails, 2 harness/setup error.
 */

import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

const require = createRequire(import.meta.url);
const REPO = path.resolve(process.env.CRENO_REPO || path.join(import.meta.dirname, '..', '..'));
const LS_KEY = 'creno-data-v1';

/* Canonical feed URL. The extension manifest allows exactly this host. */
const SK_ORIGIN = 'https://api.skello.io';
const SK_URL = SK_ORIGIN + '/users/qa/feeds/ics/test';
const RELAY_ORIGIN = 'https://relay.test';
const RELAY_ICS_URL = RELAY_ORIGIN + '/ics?url=' + encodeURIComponent(SK_URL);
const PUBLIC = {
  allorigins: 'https://api.allorigins.win/raw?url=' + encodeURIComponent(SK_URL),
  corsproxy: 'https://corsproxy.io/?url=' + encodeURIComponent(SK_URL),
  codetabs: 'https://api.codetabs.com/v1/proxy?quest=' + encodeURIComponent(SK_URL),
};
const ALLOWED_EXTERNAL = [SK_ORIGIN + '/', RELAY_ORIGIN + '/', 'https://api.allorigins.win/', 'https://corsproxy.io/', 'https://api.codetabs.com/'];

const ICS_BODY = [
  'BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Créno QA//EN', 'BEGIN:VEVENT',
  'UID:qa-relay-1@creno.test', 'DTSTAMP:20300101T000000Z',
  'DTSTART:20300107T080000', 'DTEND:20300107T120000',
  'SUMMARY:QA SHIFT', 'LOCATION:QA STORE', 'END:VEVENT', 'END:VCALENDAR', '',
].join('\r\n');
const HTML_BODY = '<!doctype html><html><body><h1>Connectez-vous à Skello</h1></body></html>';
const FORBIDDEN_JSON = '{"error":"Forbidden"}';

/* Today's schema (app.js:82 and app.js:113), used by case F only. */
const BASE_STATE_KEYS = ['version', 'profile', 'establishments', 'shifts', 'absences', 'availability', 'settings'];
const BASE_SETTINGS_KEYS = ['skelloUrl', 'lastIcsSync', 'allowProxy', 'mobileView'];

/* ------------------------------------------------------------------ */
/* Playwright / browser / server                                      */
/* ------------------------------------------------------------------ */

function resolvePlaywright() {
  const tried = [];
  const candidates = [];
  if (process.env.PLAYWRIGHT_MODULE) candidates.push(process.env.PLAYWRIGHT_MODULE);
  candidates.push('playwright');
  const npxRoot = path.join(process.env.HOME || '', '.npm', '_npx');
  if (fs.existsSync(npxRoot)) {
    for (const d of fs.readdirSync(npxRoot).sort()) {
      candidates.push(path.join(npxRoot, d, 'node_modules', 'playwright'));
    }
  }
  candidates.push(path.join(process.env.HOME || '', '.config', 'opencode', 'node_modules', 'playwright'));
  for (const c of candidates) {
    try { return { mod: require(c), from: c }; }
    catch (e) { tried.push(`${c} (${e.code || e.message})`); }
  }
  throw new Error(`Playwright module not found. Tried:\n  ${tried.join('\n  ')}`);
}

function browserCandidates() {
  const list = [];
  if (process.env.CRENO_BROWSER) list.push(process.env.CRENO_BROWSER);
  list.push('/opt/brave.com/brave/brave');
  list.push('/usr/bin/brave-browser');
  list.push('/usr/bin/google-chrome-stable');
  list.push('/usr/bin/google-chrome');
  list.push('/usr/bin/chromium');
  /* /usr/bin/chromium-browser is a snap shim here ("requires the chromium snap").
     It is deliberately not tried: preferring it would produce a misleading setup error. */
  list.push('/usr/bin/firefox');
  return [...new Set(list)].filter(p => { try { return fs.statSync(p).isFile(); } catch { return false; } });
}

async function launchBrowser(chromium) {
  const errors = [];
  for (const executablePath of browserCandidates()) {
    try {
      const browser = await chromium.launch({
        headless: true,
        executablePath,
        args: [
          '--no-sandbox',
          '--disable-dev-shm-usage',
          // Hard offline guarantee for anything route interception would miss.
          '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1',
          '--disable-background-networking',
          '--disable-component-update',
          '--disable-sync',
          '--no-pings',
        ],
      });
      return { browser, executablePath };
    } catch (e) {
      errors.push(`${executablePath}: ${String(e.message).split('\n')[0]}`);
    }
  }
  throw new Error(`No usable browser found.\n  ${errors.join('\n  ')}`);
}

function freePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.on('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

async function startServer(root) {
  const port = await freePort();
  const child = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const base = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${base}/index.html`, { signal: AbortSignal.timeout(1000) });
      if (res.ok) return { base, child };
    } catch { /* not up yet */ }
    await new Promise(r => setTimeout(r, 100));
  }
  child.kill('SIGTERM');
  throw new Error(`python3 -m http.server did not answer on ${base}`);
}

/* ------------------------------------------------------------------ */
/* Fixtures                                                           */
/* ------------------------------------------------------------------ */

function seedData({ relayUrl, allowProxy = true, legacySettings = false }) {
  const settings = {
    skelloUrl: SK_URL,
    /* Recent enough that the 12 h auto-sync (app.js:1366) stays quiet; old enough
       that an import must update it to a strictly later timestamp. */
    lastIcsSync: Date.now() - 60_000,
    allowProxy,
  };
  if (!legacySettings) settings.mobileView = 'day';
  if (relayUrl) settings.relayUrl = relayUrl;
  return {
    version: 1,
    profile: { firstName: 'QA', lastName: 'Harness', email: 'qa@example.test', phone: '', rate: 12.5, objective: 35, refSalary: 0 },
    establishments: [{ id: 'est-qa', name: 'QA STORE', color: '#34d399' }],
    shifts: [],
    absences: [],
    availability: [true, true, true, true, true, false, false],
    settings,
  };
}

/* ------------------------------------------------------------------ */
/* One scenario                                                       */
/* ------------------------------------------------------------------ */

async function runCase(browser, base, spec) {
  const failures = [];
  const requests = [];
  const unexpected = [];
  const seed = spec.seed;
  const baselineLastSync = seed.settings.lastIcsSync;

  const context = await browser.newContext({
    serviceWorkers: 'block', locale: 'fr-FR', timezoneId: 'Europe/Paris',
    viewport: { width: 1280, height: 900 },
  });

  await context.addInitScript(({ key, data }) => {
    try { localStorage.setItem(key, JSON.stringify(data)); } catch { /* about:blank */ }
    /* The app auto-syncs once after 1.2 s. Tests must be the only caller. */
    try { Object.defineProperty(Navigator.prototype, 'onLine', { configurable: true, get: () => false }); } catch { /* ignore */ }
  }, { key: LS_KEY, data: seed });

  const page = await context.newPage();
  page.setDefaultTimeout(20_000);

  await page.route('**/*', async route => {
    const url = route.request().url();
    if (url.startsWith(base + '/')) return route.continue();
    if (!url.startsWith('http')) return route.continue();

    const hostOk = ALLOWED_EXTERNAL.some(p => url.startsWith(p));
    if (!hostOk) {
      unexpected.push(url);
      return route.fulfill({ status: 204, body: '', headers: { 'access-control-allow-origin': '*' } });
    }
    requests.push(url);

    const fulfill = (status, body, type = 'text/calendar') => route.fulfill({
      status, body, contentType: type,
      headers: { 'access-control-allow-origin': '*', 'access-control-expose-headers': '*' },
    });

    if (url.startsWith(SK_ORIGIN)) {
      if (spec.direct === 'ics') return fulfill(200, ICS_BODY);
      return route.abort('failed'); // direct blocked: relays must take over
    }
    if (url.startsWith(RELAY_ORIGIN)) {
      if (spec.relay === 'ics') return fulfill(200, ICS_BODY);
      if (spec.relay === 'html') return fulfill(200, HTML_BODY, 'text/html');
      if (spec.relay === '403') return fulfill(403, FORBIDDEN_JSON, 'application/json');
      return fulfill(500, 'not configured in this scenario', 'text/plain');
    }
    const pub = url.startsWith('https://api.allorigins.win/') || url.startsWith('https://corsproxy.io/') || url.startsWith('https://api.codetabs.com/');
    if (pub) {
      if (spec.publicMode === 'codetabs-ics' && url.startsWith('https://api.codetabs.com/')) return fulfill(200, ICS_BODY);
      if (spec.publicMode === 'allorigins-ics' && url.startsWith('https://api.allorigins.win/')) return fulfill(200, ICS_BODY);
      if (spec.publicMode === 'html') return fulfill(200, HTML_BODY, 'text/html');
      return fulfill(503, 'relay unavailable', 'text/plain');
    }
    unexpected.push(url);
    return fulfill(204, '', 'text/plain');
  });

  try {
    await page.goto(base + '/index.html', { waitUntil: 'load' });
    await page.waitForFunction(() => typeof syncSkello === 'function' && typeof state !== 'undefined');

    await page.evaluate(async () => {
      openIcsModal();
      document.getElementById('icsSyncNowBtn').click();
      const t0 = Date.now();
      while (icsSyncing && Date.now() - t0 < 30000) await new Promise(r => setTimeout(r, 25));
      await new Promise(r => setTimeout(r, 100)); // let the final toast render
    });

    const snap = await page.evaluate(() => ({
      shifts: state.shifts.length,
      absences: state.absences.length,
      lastIcsSync: state.settings.lastIcsSync,
      hasRelay: Object.prototype.hasOwnProperty.call(state.settings, 'relayUrl'),
      toasts: [...document.querySelectorAll('#toasts .toast')].map(t => ({ text: (t.textContent || '').trim(), cls: t.className })),
      stored: JSON.parse(localStorage.getItem('creno-data-v1') || 'null'),
    }));

    const fail = (label, detail) => failures.push({ label, detail });
    const relayMissing = !requests.some(u => u.startsWith(RELAY_ORIGIN + '/'));
    const errorToast = snap.toasts.filter(t => /\berror\b/.test(t.cls) && t.text);

    /* --- graceful signal while agent/app-relay is not merged --- */
    if (spec.relayDependent && relayMissing) {
      fail('relay route not present (agent/app-relay not merged)',
        `relayUrl configured as ${RELAY_ORIGIN}; observed: ${requests.map(short).join(' -> ') || '(none)'}`);
    } else {
      spec.assert({ requests, unexpected, snap, fail, baselineLastSync, errorToast });
    }

    for (const u of unexpected) fail('unexpected external request (network must stay offline)', u);
    return { failures, requests, missingRelay: spec.relayDependent && relayMissing };
  } finally {
    await context.close();
  }
}

function short(url) {
  try {
    const u = new URL(url);
    if (u.host === 'api.skello.io') return 'direct';
    if (u.host === 'relay.test') return 'private-relay';
    if (u.host === 'api.allorigins.win') return 'allorigins';
    if (u.host === 'corsproxy.io') return 'corsproxy';
    if (u.host === 'api.codetabs.com') return 'codetabs';
    return u.host;
  } catch { return url; }
}

/* ------------------------------------------------------------------ */
/* Assertions                                                         */
/* ------------------------------------------------------------------ */

function expectUrls(fail, requests, expected) {
  const ok = requests.length === expected.length && requests.every((u, i) => u === expected[i]);
  if (!ok) {
    fail('request order',
      `expected [${expected.map(short).join(', ')}] but got [${requests.map(short).join(', ')}]`);
  }
}

function expectImported(fail, snap, label) {
  if (snap.shifts !== 1) fail(`${label}: shifts imported`, `expected 1 shift, got ${snap.shifts}`);
  if (!(snap.lastIcsSync > 0)) fail(`${label}: lastIcsSync set`, `lastIcsSync=${snap.lastIcsSync}`);
}

function caseF(snap, fail) {
  if (!snap.stored) { fail('localStorage written', 'no cre-no-data-v1 entry'); return; }
  const stateKeys = Object.keys(snap.stored);
  const settingsKeys = Object.keys(snap.stored.settings || {});
  const missingState = BASE_STATE_KEYS.filter(k => !stateKeys.includes(k));
  const missingSettings = BASE_SETTINGS_KEYS.filter(k => !settingsKeys.includes(k));
  const extraState = stateKeys.filter(k => !BASE_STATE_KEYS.includes(k));
  const extraSettings = settingsKeys.filter(k => !BASE_SETTINGS_KEYS.includes(k));
  if (missingState.length) fail('state schema: baseline keys removed/renamed', missingState.join(', '));
  if (missingSettings.length) fail('settings schema: baseline keys removed/renamed', missingSettings.join(', '));
  if (extraState.length) fail('state schema: unexpected top-level keys', extraState.join(', '));
  const allowed = extraSettings.filter(k => k !== 'relayUrl');
  if (allowed.length) fail('settings schema: unexpected keys beyond relayUrl', allowed.join(', '));
  if (Object.prototype.hasOwnProperty.call(snap.stored.settings || {}, 'relayUrl')) {
    if (typeof snap.stored.settings.relayUrl !== 'string') fail('settings.relayUrl type', typeof snap.stored.settings.relayUrl);
  }
}

/* ------------------------------------------------------------------ */
/* Main                                                               */
/* ------------------------------------------------------------------ */

const CASES = [
  {
    id: 'A', relayDependent: true,
    title: 'private relay configured + allowProxy: direct fails, private relay succeeds, public relays untouched',
    seed: seedData({ relayUrl: RELAY_ORIGIN }), relay: 'ics', publicMode: 'fail',
    assert({ requests, snap, fail, baselineLastSync }) {
      expectUrls(fail, requests, [SK_URL, RELAY_ICS_URL]);
      expectImported(fail, snap, 'relay import');
      if (!(snap.lastIcsSync > baselineLastSync)) fail('lastIcsSync advanced after import', `${snap.lastIcsSync} <= ${baselineLastSync}`);
      if (requests.some(u => PUBLIC.allorigins === u || PUBLIC.corsproxy === u || PUBLIC.codetabs === u)) fail('public relays not called', requests.map(short).join(' -> '));
    },
  },
  {
    id: 'B', relayDependent: false,
    title: 'no private relay + allowProxy: direct, allorigins, corsproxy, codetabs',
    seed: seedData({ allowProxy: true }), relay: null, publicMode: 'codetabs-ics',
    assert({ requests, snap, fail }) {
      expectUrls(fail, requests, [SK_URL, PUBLIC.allorigins, PUBLIC.corsproxy, PUBLIC.codetabs]);
      expectImported(fail, snap, 'public relay fallback');
    },
  },
  {
    id: 'C', relayDependent: false,
    title: 'no private relay + allowProxy=false: only direct, visible error, no other request',
    seed: seedData({ allowProxy: false }), relay: null, publicMode: 'fail',
    assert({ requests, snap, fail, baselineLastSync, errorToast }) {
      expectUrls(fail, requests, [SK_URL]);
      if (snap.shifts !== 0) fail('no import expected', `shifts=${snap.shifts}`);
      if (snap.lastIcsSync !== baselineLastSync) fail('lastIcsSync unchanged', `${snap.lastIcsSync} != ${baselineLastSync}`);
      if (!errorToast.length) fail('user-visible error toast', snap.toasts.map(t => `${t.cls}: ${t.text}`).join(' | ') || '(no toast)');
    },
  },
  {
    id: 'D1', relayDependent: true,
    title: 'private relay returns 403 JSON + allowProxy: falls through to public relays',
    seed: seedData({ relayUrl: RELAY_ORIGIN }), relay: '403', publicMode: 'allorigins-ics',
    assert({ requests, snap, fail }) {
      expectUrls(fail, requests, [SK_URL, RELAY_ICS_URL, PUBLIC.allorigins]);
      expectImported(fail, snap, '403 fallback import');
    },
  },
  {
    id: 'D2', relayDependent: true,
    title: 'private relay returns 403 JSON + allowProxy=false: clean error, no public relay',
    seed: seedData({ relayUrl: RELAY_ORIGIN, allowProxy: false }), relay: '403', publicMode: 'fail',
    assert({ requests, snap, fail, baselineLastSync, errorToast }) {
      expectUrls(fail, requests, [SK_URL, RELAY_ICS_URL]);
      if (snap.shifts !== 0) fail('no import expected', `shifts=${snap.shifts}`);
      if (snap.lastIcsSync !== baselineLastSync) fail('lastIcsSync unchanged', `${snap.lastIcsSync} != ${baselineLastSync}`);
      if (!errorToast.length) fail('user-visible error toast', snap.toasts.map(t => `${t.cls}: ${t.text}`).join(' | ') || '(no toast)');
    },
  },
  {
    id: 'E', relayDependent: true,
    title: 'private relay returns 200 HTML without VCALENDAR: rejected, no import',
    seed: seedData({ relayUrl: RELAY_ORIGIN }), relay: 'html', publicMode: 'html',
    assert({ requests, snap, fail, baselineLastSync, errorToast }) {
      const prefix = [SK_URL, RELAY_ICS_URL];
      const prefixed = requests.length >= prefix.length && prefix.every((u, i) => requests[i] === u);
      if (!prefixed) fail('request order', `expected prefix [${prefix.map(short).join(', ')}] but got [${requests.map(short).join(', ')}]`);
      if (snap.shifts !== 0) fail('HTML body must not import shifts', `shifts=${snap.shifts}`);
      if (snap.lastIcsSync !== baselineLastSync) fail('lastIcsSync must not change', `${snap.lastIcsSync} != ${baselineLastSync}`);
      if (!errorToast.length) fail('user-visible error toast', snap.toasts.map(t => `${t.cls}: ${t.text}`).join(' | ') || '(no toast)');
    },
  },
  {
    id: 'F', relayDependent: false,
    title: 'localStorage schema unchanged after import; relayUrl is the only allowed addition',
    seed: seedData({ allowProxy: false, legacySettings: true }), relay: null, direct: 'ics', publicMode: 'fail',
    assert({ requests, snap, fail }) {
      expectUrls(fail, requests, [SK_URL]);
      if (snap.shifts !== 1) fail('direct import used for schema check', `expected 1 shift, got ${snap.shifts}`);
      caseF(snap, fail);
    },
  },
];

async function main() {
  if (!fs.existsSync(path.join(REPO, 'app.js')) || !fs.existsSync(path.join(REPO, 'index.html'))) {
    throw new Error(`not a Créno app root: ${REPO}`);
  }
  const { mod: playwright, from } = resolvePlaywright();
  const { browser, executablePath } = await launchBrowser(playwright.chromium);
  const server = await startServer(REPO);
  console.log(`[e2e] repo: ${REPO}`);
  console.log(`[e2e] playwright: ${from}`);
  console.log(`[e2e] browser: ${executablePath}`);

  let failed = 0;
  let missingRelay = 0;
  try {
    for (const spec of CASES) {
      const { failures, requests, missingRelay: missing } = await runCase(browser, server.base, spec);
      if (missing) missingRelay++;
      if (failures.length) {
        failed++;
        console.log(`FAIL case ${spec.id} — ${spec.title}`);
        for (const f of failures) console.log(`     ✗ ${f.label}${f.detail ? `: ${f.detail}` : ''}`);
        console.log(`     requests: ${requests.map(short).join(' -> ') || '(none)'}`);
      } else {
        console.log(`PASS case ${spec.id} — ${spec.title}`);
      }
    }
  } finally {
    await browser.close();
    server.child.kill('SIGTERM');
  }

  const passed = CASES.length - failed;
  console.log(`[e2e] ${passed}/${CASES.length} cases passed${missingRelay ? ` (${missingRelay} blocked by missing relay code)` : ''}`);
  process.exit(failed ? 1 : 0);
}

main().catch(err => {
  console.error(`[e2e] SETUP ERROR: ${err.message}`);
  process.exit(2);
});
