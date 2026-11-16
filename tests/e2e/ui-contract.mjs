#!/usr/bin/env node
/*
 * Créno — UI contract E2E harness (real browser, offline, deterministic).
 *
 * Freezes the user-visible contract the visual redesign must not break: week
 * navigation, salary estimate, statistics, Skello panel states, persistence, and
 * the delegation/template invariants app.js relies on (views, dialogs, grid,
 * --c/--v variables, toast roles, localStorage schema).
 *
 * Serves the repository on 127.0.0.1 with `python3 -m http.server` (ephemeral
 * port), launches the installed Brave binary through the Playwright module found
 * in the npm npx cache, and intercepts EVERY external request with page.route:
 * nothing leaves the machine. `--host-resolver-rules=MAP * ~NOTFOUND` is the
 * second safety net, exactly like tests/e2e/relay-sync.mjs.
 *
 * Determinism: `Date` is pinned to a fixed instant (frozen clock), while real
 * timers are kept so Playwright polling still works. The localStorage fixture is
 * derived from the frozen week's Monday (never from the real date).
 *
 * Contract cases:
 *  1 week navigation ‹ ›            next/prev move by whole ISO weeks
 *  2 "Aujourd'hui"                  back to the current week, jumpDate = Monday
 *  3 jumpDate                       jumps to the week containing the date
 *  4 weekly salary estimate         39h45 × 12,5 €/h = "497 €" in #summaryBar
 *  5 weekly objective               "Objectif 35 h · 100 %" (progress capped)
 *  6 statistics                     497 € / 39h45 / 8 shifts / "31 % de 1 600 €"
 *  7 Skello sync button             hidden without URL, visible with one
 *  8 Skello panel initial state     "Jamais synchronisé" + sync button disabled
 *  9 invalid private relay          #icsRelayError + aria-invalid="true"
 * 10 relay URL ending in /ics       normalised with a visible notice, no error
 * 11 persistence                    profile rate survives a reload (596 €)
 * 12 mobile day/week toggle         7 day blocks at 390 px, label flips
 * 13 five views                     .nav-link[data-view] ×4 + avatar menu (establishments)
 * 14 dialog bodies                  every <dialog> has exactly one .dialog-body
 * 15 grid composition               1 corner + 7 day heads, 2 × (est-cell + 7 cells)
 * 16 template variables             .shift/.absence carry --c, .progress carries --v
 * 17 toast roles                    role="status" on success, role="alert" on error
 * 18 localStorage schema            settings = 4 baseline keys + relayUrl, nothing else
 *
 * Run:  node tests/e2e/ui-contract.mjs
 * Env:  CRENO_REPO=/path/to/app          repository (or a temp copy) under test
 *       CRENO_BROWSER=/path/to/browser   browser executable (default: Brave)
 *       PLAYWRIGHT_MODULE=/path/to/playwright
 *
 * Exit: 0 all 18 cases pass, 1 at least one case fails, 2 harness/setup error.
 */

import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

const require = createRequire(import.meta.url);
const REPO = path.resolve(process.env.CRENO_REPO || path.join(import.meta.dirname, '..', '..'));
const LS_KEY = 'creno-data-v1';

/* Frozen instant: Wednesday 2026-03-11 09:00 Europe/Paris (ISO week 11).
   Changing it would change the month/ISO-week expectations below: the fixture week
   must stay inside a single month for cases 4-6. */
const FIXED_ISO = '2026-03-11T09:00:00+01:00';
const FIXED_MS = Date.parse(FIXED_ISO);
const SK_URL = 'https://api.skello.io/users/qa/feeds/ics/test';
const RELAY_ORIGIN = 'https://relay.test';

const MONTHS = ['janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.'];
const BASE_SETTINGS_KEYS = ['skelloUrl', 'lastIcsSync', 'allowProxy', 'mobileView'];

/* ------------------------------------------------------------------ */
/* Calendar helpers (ISO strings only, UTC arithmetic: host TZ proof) */
/* ------------------------------------------------------------------ */

/* Calendar date of the frozen instant in Europe/Paris, e.g. "2026-03-11". */
const FIXED_DATE = new Intl.DateTimeFormat('sv-SE', { timeZone: 'Europe/Paris' }).format(new Date(FIXED_MS));

const parseISO = iso => {
  const [y, m, d] = iso.split('-').map(Number);
  return { y, m, d, date: new Date(Date.UTC(y, m - 1, d)) };
};
const addDaysISO = (iso, n) => {
  const { date } = parseISO(iso);
  date.setUTCDate(date.getUTCDate() + n);
  return date.toISOString().slice(0, 10);
};
const mondayOf = iso => addDaysISO(iso, -((parseISO(iso).date.getUTCDay() + 6) % 7));
/* Same display as app.js fmtRange(): "9 mars – 15 mars 2026". */
const fmtRange = (from, to) => {
  const a = parseISO(from), b = parseISO(to);
  return `${a.d} ${MONTHS[a.m - 1]} – ${b.d} ${MONTHS[b.m - 1]} ${b.y}`;
};
/* ISO-8601 week number (the definition used by the app and by ISO). */
function isoWeek(iso) {
  const t = parseISO(iso).date;
  const dn = (t.getUTCDay() + 6) % 7;
  t.setUTCDate(t.getUTCDate() - dn + 3);
  const f = new Date(Date.UTC(t.getUTCFullYear(), 0, 4));
  const fd = (f.getUTCDay() + 6) % 7;
  f.setUTCDate(f.getUTCDate() - fd + 3);
  return 1 + Math.round((t - f) / 604800000);
}

const MON = mondayOf(FIXED_DATE);
const WEEK = [...Array(7)].map((_, i) => addDaysISO(MON, i));
const TODAY_ISO = FIXED_DATE;                       /* Wednesday 2026-03-11 */
const NEXT_MON = addDaysISO(MON, 7);
/* Case 1 clicks next once then prev twice: net −1 week, not −2. */
const BACK1_MON = addDaysISO(MON, -7);
const JUMP_DATE = addDaysISO(MON, 42);              /* 2026-04-22, Wednesday */
const JUMP_MON = mondayOf(JUMP_DATE);

/* ------------------------------------------------------------------ */
/* Deterministic fixture (dates anchored on the frozen week)           */
/* ------------------------------------------------------------------ */

function fixture(settings = {}) {
  return {
    version: 1,
    profile: {
      firstName: 'Camille', lastName: 'Martin', email: 'camille.martin@example.test',
      phone: '', rate: 12.5, objective: 35, refSalary: 1600,
    },
    establishments: [
      { id: 'est-demo', name: 'CHANTONNAY DISTRIBUTION', color: '#34d399' },
      { id: 'est-cafe', name: 'BOULANGERIE CENTRE', color: '#60a5fa' },
    ],
    /* 8 shifts = 39h45 = 497 € at 12,5 €/h (3 + 6 + 6 + 6 + 6:45 + 3 + 6 + 3). */
    shifts: [
      { id: 'sh-01', estId: 'est-demo', date: WEEK[0], start: '08:00', end: '11:00', label: 'DRIVE', note: '' },
      { id: 'sh-02', estId: 'est-demo', date: WEEK[0], start: '12:00', end: '18:00', label: 'DRIVE', note: '' },
      { id: 'sh-03', estId: 'est-demo', date: WEEK[1], start: '12:00', end: '18:00', label: 'DRIVE', note: '' },
      { id: 'sh-04', estId: 'est-demo', date: WEEK[2], start: '12:00', end: '18:00', label: 'DRIVE', note: 'Inventaire soir' },
      { id: 'sh-05', estId: 'est-demo', date: WEEK[4], start: '12:00', end: '18:45', label: 'DRIVE', note: '' },
      { id: 'sh-06', estId: 'est-demo', date: WEEK[5], start: '08:00', end: '11:00', label: 'DRIVE', note: '' },
      { id: 'sh-07', estId: 'est-demo', date: WEEK[5], start: '12:00', end: '18:00', label: 'DRIVE', note: '' },
      { id: 'sh-08', estId: 'est-cafe', date: WEEK[1], start: '06:00', end: '09:00', label: 'CAISSE', note: '' },
    ],
    absences: [
      { id: 'ab-01', type: 'Congés', start: WEEK[3], end: WEEK[3], note: '', status: 'Confirmée' },
    ],
    availability: [true, true, true, true, true, false, false],
    settings: { skelloUrl: '', lastIcsSync: 0, allowProxy: true, mobileView: 'day', relayUrl: '', ...settings },
  };
}

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
  /* /usr/bin/chromium-browser is a snap shim here ("requires the chromium snap"). */
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
/* Page factory                                                       */
/* ------------------------------------------------------------------ */

const external = [];   /* every request that is not the local app server */

async function openPage(browser, base, { width, height }, settings) {
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: 1,
    locale: 'fr-FR',
    timezoneId: 'Europe/Paris',
    serviceWorkers: 'block',
    reducedMotion: 'reduce',
  });

  await context.addInitScript(({ key, data, fixedMs }) => {
    /* Frozen clock reading; timers stay real so Playwright's own polling works. */
    const RealDate = Date;
    class FixedDate extends RealDate {
      constructor(...args) { if (args.length === 0) super(fixedMs); else super(...args); }
      static now() { return fixedMs; }
    }
    window.Date = FixedDate;
    /* Seed once per context: a reload must keep what the app itself persisted. */
    try {
      if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(data));
    } catch { /* about:blank */ }
    /* The app auto-syncs once after 1.2 s; the contract cases must be the only caller. */
    try { Object.defineProperty(Navigator.prototype, 'onLine', { configurable: true, get: () => false }); } catch { /* ignore */ }
  }, { key: LS_KEY, data: fixture(settings), fixedMs: FIXED_MS });

  const page = await context.newPage();
  page.setDefaultTimeout(15_000);

  await page.route('**/*', route => {
    const url = route.request().url();
    if (url.startsWith(base + '/')) return route.continue();
    if (!/^https?:/i.test(url)) return route.continue();
    external.push(url);
    return route.fulfill({ status: 204, body: '', headers: { 'access-control-allow-origin': '*' } });
  });

  const load = async () => {
    await page.goto(base + '/index.html', { waitUntil: 'load' });
    await page.waitForFunction(() => typeof state !== 'undefined' && !!document.querySelector('#summaryBar'));
    await page.waitForFunction(() => (document.querySelector('#summaryBar')?.textContent || '').trim().length > 0);
  };
  await load();

  return { context, page, load };
}

/* ------------------------------------------------------------------ */
/* Assertions                                                         */
/* ------------------------------------------------------------------ */

const norm = s => String(s ?? '').replace(/\s+/g, ' ').trim();
const txt = (page, sel) => page.evaluate(s => {
  const el = document.querySelector(s);
  return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null;
}, sel);
const shown = (page, sel) => page.evaluate(s => {
  const el = document.querySelector(s);
  return !!el && !el.classList.contains('hidden') && getComputedStyle(el).display !== 'none';
}, sel);

const CASES = [
  {
    n: 1,
    title: 'week navigation ‹ › (next/prev by whole ISO weeks)',
    async run({ page }) {
      await page.click('#nextWeek');
      const n1 = norm(await txt(page, '#weekNum'));
      const l1 = norm(await txt(page, '#weekLabel'));
      await page.click('#prevWeek');
      await page.click('#prevWeek');
      const n2 = norm(await txt(page, '#weekNum'));
      const l2 = norm(await txt(page, '#weekLabel'));
      const exp1 = `Semaine ${isoWeek(NEXT_MON)}`, exp2 = `Semaine ${isoWeek(BACK1_MON)}`;
      return {
        ok: n1 === exp1 && l1 === fmtRange(NEXT_MON, addDaysISO(NEXT_MON, 6))
          && n2 === exp2 && l2 === fmtRange(BACK1_MON, addDaysISO(BACK1_MON, 6)),
        expected: `${exp1} / ${fmtRange(NEXT_MON, addDaysISO(NEXT_MON, 6))} then ${exp2} / ${fmtRange(BACK1_MON, addDaysISO(BACK1_MON, 6))}`,
        observed: `${n1} / ${l1} then ${n2} / ${l2}`,
      };
    },
  },
  {
    n: 2,
    title: '"Aujourd\'hui" returns to the current week',
    async run({ page }) {
      await page.click('#todayBtn');
      const n = norm(await txt(page, '#weekNum'));
      const l = norm(await txt(page, '#weekLabel'));
      const jump = await page.inputValue('#jumpDate');
      return {
        ok: n === `Semaine ${isoWeek(MON)}` && l === fmtRange(MON, WEEK[6]) && jump === MON,
        expected: `Semaine ${isoWeek(MON)} / ${fmtRange(MON, WEEK[6])} / jumpDate ${MON}`,
        observed: `${n} / ${l} / ${jump}`,
      };
    },
  },
  {
    n: 3,
    title: 'jumpDate opens the week containing the picked date',
    async run({ page }) {
      await page.fill('#jumpDate', JUMP_DATE);
      await page.dispatchEvent('#jumpDate', 'change');
      const n = norm(await txt(page, '#weekNum'));
      const l = norm(await txt(page, '#weekLabel'));
      return {
        ok: n === `Semaine ${isoWeek(JUMP_MON)}` && l === fmtRange(JUMP_MON, addDaysISO(JUMP_MON, 6)),
        expected: `Semaine ${isoWeek(JUMP_MON)} / ${fmtRange(JUMP_MON, addDaysISO(JUMP_MON, 6))}`,
        observed: `${n} / ${l}`,
      };
    },
  },
  {
    n: 4,
    title: 'weekly salary estimate "497 €" (39h45 × 12,5 €/h)',
    async run({ page }) {
      await page.click('#todayBtn');
      const money = norm(await txt(page, '#summaryBar .sum-money'));
      const bar = norm(await txt(page, '#summaryBar'));
      return {
        ok: money === '497 €' && bar.includes('estimés · 12,5 €/h'),
        expected: '"497 €" + "estimés · 12,5 €/h"',
        observed: `${money} | ${bar}`,
      };
    },
  },
  {
    n: 5,
    title: 'weekly objective "Objectif 35 h · 100 %" (capped)',
    async run({ page }) {
      const bar = norm(await txt(page, '#summaryBar'));
      return {
        ok: bar.includes('Objectif 35 h · 100 %') && bar.includes('39h45'),
        expected: '"Objectif 35 h · 100 %" + "39h45"',
        observed: bar,
      };
    },
  },
  {
    n: 6,
    title: 'statistics 497 € / 39h45 / 8 shifts / "31 % de 1 600 €"',
    async run({ page }) {
      await page.click('#mainNav .nav-link[data-view="stats"]');
      await page.waitForFunction(() => !document.querySelector('#view-stats').classList.contains('hidden'));
      const pay = norm(await txt(page, '#statPay'));
      const month = norm(await txt(page, '#statMonth'));
      const shifts = norm(await txt(page, '#statShifts'));
      const sub = norm(await txt(page, '#statPaySub'));
      const week = norm(await txt(page, '#statWeek'));
      return {
        ok: pay === '497 €' && month === '39h45' && shifts === '8' && week === '39h45'
          && sub.includes('31 % de 1 600 €'),
        expected: '497 € / 39h45 (mois) / 39h45 (semaine) / 8 / "31 % de 1 600 €"',
        observed: `${pay} / ${month} / ${week} / ${shifts} / ${sub}`,
      };
    },
  },
  {
    n: 7,
    title: 'Skello sync button hidden without URL, visible with one',
    async run({ page, browser, base }) {
      const hiddenBefore = await page.evaluate(() => document.querySelector('#syncIcsBtn').classList.contains('hidden'));
      const other = await openPage(browser, base, { width: 1440, height: 900 }, { skelloUrl: SK_URL });
      try {
        const hiddenAfter = await other.page.evaluate(() => document.querySelector('#syncIcsBtn').classList.contains('hidden'));
        const label = norm(await txt(other.page, '#syncIcsBtn'));
        return {
          ok: hiddenBefore === true && hiddenAfter === false && label === '⟳ Sync Skello',
          expected: 'hidden without URL, visible + "⟳ Sync Skello" with URL',
          observed: `before hidden=${hiddenBefore}, after hidden=${hiddenAfter}, label "${label}"`,
        };
      } finally {
        await other.context.close();
      }
    },
  },
  {
    n: 8,
    title: 'Skello panel initial state "Jamais synchronisé" + disabled sync',
    async run({ page }) {
      await page.evaluate(() => openIcsModal());
      await page.waitForFunction(() => document.querySelector('#icsModal').open);
      const last = norm(await txt(page, '#icsLastSync'));
      const disabled = await page.evaluate(() => document.querySelector('#icsSyncNowBtn').disabled);
      const relayHelp = await shown(page, '#icsRelayHelp');
      await page.evaluate(() => document.querySelector('#icsModal').close());
      return {
        ok: last === 'Jamais synchronisé' && disabled === true && relayHelp === true,
        expected: '"Jamais synchronisé", disabled=true, relay help visible',
        observed: `"${last}", disabled=${disabled}, relayHelp=${relayHelp}`,
      };
    },
  },
  {
    n: 9,
    title: 'invalid private relay: aria-invalid + explicit error',
    async run({ page }) {
      await page.evaluate(() => openIcsModal());
      await page.waitForFunction(() => document.querySelector('#icsModal').open);
      await page.fill('#icsRelayUrl', 'http://relais.local');
      const errorShown = await shown(page, '#icsRelayError');
      const msg = norm(await txt(page, '#icsRelayError'));
      const aria = await page.getAttribute('#icsRelayUrl', 'aria-invalid');
      await page.evaluate(() => document.querySelector('#icsModal').close());
      return {
        ok: errorShown && aria === 'true' && msg.includes('Aucun relais public ne sera utilisé'),
        expected: 'error visible, aria-invalid="true", "Aucun relais public ne sera utilisé"',
        observed: `visible=${errorShown}, aria-invalid=${aria}, "${msg.slice(0, 120)}"`,
      };
    },
  },
  {
    n: 10,
    title: 'relay URL ending in /ics is normalised (notice, no error)',
    async run({ page }) {
      await page.evaluate(() => openIcsModal());
      await page.waitForFunction(() => document.querySelector('#icsModal').open);
      await page.fill('#icsRelayUrl', RELAY_ORIGIN + '/ics');
      const noticeShown = await shown(page, '#icsRelayNotice');
      const notice = norm(await txt(page, '#icsRelayNotice'));
      const errorShown = await shown(page, '#icsRelayError');
      await page.evaluate(() => document.querySelector('#icsModal').close());
      return {
        ok: noticeShown && notice.includes('URL normalisée') && !errorShown,
        expected: 'notice "URL normalisée…" visible, error hidden',
        observed: `notice=${noticeShown} "${notice.slice(0, 120)}", error=${errorShown}`,
      };
    },
  },
  {
    n: 11,
    title: 'profile rate persists across reload (15 €/h → "596 €")',
    async run({ page, load }) {
      await page.click('#mainNav .nav-link[data-view="profile"]');
      await page.waitForFunction(() => !document.querySelector('#view-profile').classList.contains('hidden'));
      await page.fill('#pRate', '15');
      await page.click('#profileSave');
      await page.evaluate(() => new Promise(r => setTimeout(r, 50)));
      const storedBefore = await page.evaluate(() => JSON.parse(localStorage.getItem('creno-data-v1') || '{}')?.profile?.rate);
      await load();
      const stored = await page.evaluate(() => JSON.parse(localStorage.getItem('creno-data-v1') || '{}')?.profile?.rate);
      const money = norm(await txt(page, '#summaryBar .sum-money'));
      return {
        ok: storedBefore === 15 && stored === 15 && money === '596 €',
        expected: 'profile.rate=15 before and after reload, summary "596 €"',
        observed: `stored=${storedBefore}, reloaded=${stored}, summary=${money}`,
      };
    },
  },
  {
    n: 12,
    title: 'mobile day/week toggle at 390 px (7 day blocks, label flips)',
    async run({ browser, base }) {
      const mobile = await openPage(browser, base, { width: 390, height: 844 }, { skelloUrl: '' });
      const page = mobile.page;
      try {
        const before = norm(await txt(page, '#mdWeekBtn'));
        await page.click('#mdWeekBtn');
        await page.waitForFunction(() => document.querySelectorAll('#mobileDayView .md-dayblock').length === 7);
        const blocks = await page.evaluate(() => document.querySelectorAll('#mobileDayView .md-dayblock').length);
        const labelWeek = norm(await txt(page, '#mdWeekBtn'));
        const mobileView = await page.evaluate(() => state.settings.mobileView);
        await page.click('#mdWeekBtn');
        await page.waitForFunction(() => document.querySelectorAll('#mobileDayView .md-dayblock').length === 0);
        const labelDay = norm(await txt(page, '#mdWeekBtn'));
        return {
          ok: before === '📅 Semaine' && blocks === 7 && labelWeek === '🗓️ Jour'
            && mobileView === 'week' && labelDay === '📅 Semaine',
          expected: '"📅 Semaine" → 7 blocks + "🗓️ Jour" → back to "📅 Semaine"',
          observed: `"${before}" → ${blocks} blocks + "${labelWeek}" (mobileView=${mobileView}) → "${labelDay}"`,
        };
      } finally {
        await mobile.context.close();
      }
    },
  },
  {
    n: 13,
    title: 'five views reachable: 4 nav links + avatar menu for establishments',
    async run({ page }) {
      const navViews = ['planning', 'absences', 'stats', 'profile'];
      let ok = true;
      const seen = [];
      for (const v of navViews) {
        await page.click(`#mainNav .nav-link[data-view="${v}"]`);
        const st = await page.evaluate(() => ({
          visible: [...document.querySelectorAll('.view')].filter(x => !x.classList.contains('hidden')).map(x => x.id),
          active: document.querySelector('#mainNav .nav-link.active')?.dataset.view,
          current: document.querySelectorAll('#mainNav .nav-link[aria-current="page"]').length,
          currentView: document.querySelector('#mainNav .nav-link[aria-current="page"]')?.dataset.view,
        }));
        const good = st.visible.length === 1 && st.visible[0] === 'view-' + v
          && st.active === v && st.current === 1 && st.currentView === v;
        seen.push(`${v}:${good ? 'ok' : `visible=${st.visible} active=${st.active} aria-current=${st.current}`}`);
        ok = ok && good;
      }
      /* #view-establishments has no nav entry by design: it is opened from the avatar menu. */
      await page.click('#avatarBtn');
      const menuOpen = await shown(page, '#avatarMenu');
      await page.click('#avatarMenu [data-action="goto-establishments"]');
      const estShown = await page.evaluate(() => !document.querySelector('#view-establishments').classList.contains('hidden'));
      const hasNavEntry = await page.evaluate(() => !!document.querySelector('#mainNav .nav-link[data-view="establishments"]'));
      const views = await page.evaluate(() => document.querySelectorAll('.view').length);
      ok = ok && menuOpen && estShown && !hasNavEntry && views === 5;
      return {
        ok,
        expected: 'each nav link shows its view with aria-current, avatar menu shows #view-establishments, no nav entry, 5 .view',
        observed: `${seen.join(' ')} | menu=${menuOpen} establishments=${estShown} navEntry=${hasNavEntry} views=${views}`,
      };
    },
  },
  {
    n: 14,
    title: 'every <dialog> has exactly one .dialog-body (dictaphone host)',
    async run({ page }) {
      const d = await page.evaluate(() => {
        const dialogs = [...document.querySelectorAll('dialog')];
        return {
          count: dialogs.length,
          ids: dialogs.map(x => x.id).sort(),
          bodies: dialogs.map(x => ({ id: x.id, n: x.querySelectorAll('.dialog-body').length })),
          dicta: !!document.querySelector('#dictaBtn') && !!document.querySelector('#dictaPill') && !!document.querySelector('#dictaLive'),
        };
      });
      const expectedIds = ['absenceModal', 'confirmModal', 'estModal', 'icsModal', 'shiftModal'];
      const oneEach = d.bodies.every(b => b.n === 1);
      return {
        ok: d.count === 5 && JSON.stringify(d.ids) === JSON.stringify(expectedIds) && oneEach && d.dicta,
        expected: `5 dialogs ${expectedIds.join(',')}, one .dialog-body each, dicta controls present`,
        observed: `${d.count} dialogs [${d.ids.join(',')}] bodies=[${d.bodies.map(b => b.id + ':' + b.n).join(' ')}] dicta=${d.dicta}`,
      };
    },
  },
  {
    n: 15,
    title: 'grid composition: 1 corner + 7 day heads, per establishment 1 est-cell + 7 cells',
    async run({ page }) {
      const g = await page.evaluate(() => {
        const days = [...document.querySelectorAll('#weekGrid .grid-head.day-head')].map(h => h.querySelector('.dh-name')?.textContent?.trim());
        const cells = [...document.querySelectorAll('#weekGrid .cell')].map(c => ({ est: c.dataset.est, date: c.dataset.date }));
        return {
          corner: document.querySelectorAll('#weekGrid .grid-head.corner').length,
          heads: days.length,
          days,
          estCells: document.querySelectorAll('#weekGrid .est-cell').length,
          estCellIds: [...document.querySelectorAll('#weekGrid .est-cell')].map(c => c.dataset.est),
          cells,
        };
      });
      const estIds = [...new Set(g.cells.map(c => c.est))];
      const perEst = estIds.map(id => g.cells.filter(c => c.est === id).length);
      const allDates = g.cells.every(c => WEEK.includes(c.date));
      const ok = g.corner === 1 && g.heads === 7
        && JSON.stringify(g.days) === JSON.stringify(['lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.', 'dim.'])
        && g.estCells === 2 && g.estCellIds.join(',') === 'est-demo,est-cafe'
        && g.cells.length === 14 && perEst.every(n => n === 7) && allDates;
      return {
        ok,
        expected: 'corner=1, day-head=7 (lun.→dim.), est-cell=2, cells=14 (7 per establishment, valid dates)',
        observed: `corner=${g.corner}, heads=${g.heads} [${g.days.join(',')}], est-cells=${g.estCells} [${g.estCellIds.join(',')}], cells=${g.cells.length} perEst=[${perEst.join(',')}] datesOk=${allDates}`,
      };
    },
  },
  {
    n: 16,
    title: 'templates expose --c (shift/absence) and --v (progress)',
    async run({ page }) {
      await page.evaluate(() => switchView('planning'));
      const v = await page.evaluate(() => {
        const val = (el, name) => getComputedStyle(el).getPropertyValue(name).trim();
        const shifts = [...document.querySelectorAll('#weekGrid .shift')].map(s => ({
          id: s.dataset.id, c: val(s, '--c'),
          parts: ['shift-time', 'shift-dur', 'shift-label'].every(k => !!s.querySelector('.' + k)),
        }));
        const absences = [...document.querySelectorAll('#weekGrid .absence')].map(a => val(a, '--c'));
        const bars = [...document.querySelectorAll('#summaryBar .progress')].map(p => val(p, '--v'));
        return { shifts, absences, bars };
      });
      const colors = [...new Set(v.shifts.map(s => s.c))].sort();
      const ok = v.shifts.length === 8
        && v.shifts.every(s => /^#[0-9a-f]{6}$/i.test(s.c) && s.parts)
        && JSON.stringify(colors) === JSON.stringify(['#34d399', '#60a5fa'])
        && v.absences.length === 2 && v.absences.every(c => c === '#a78bfa')
        && v.bars.length === 1 && v.bars[0] === '100%';
      return {
        ok,
        expected: '8 shifts with --c ∈ {#34d399,#60a5fa} + time/dur/label spans, absences --c=#a78bfa, .progress --v=100%',
        observed: `${v.shifts.length} shifts colors=[${colors.join(',')}] partsOk=${v.shifts.every(s => s.parts)} | absences=[${v.absences.join(',')}] | progress=[${v.bars.join(',')}]`,
      };
    },
  },
  {
    n: 17,
    title: 'toast roles: role="status" on success, role="alert" on error',
    async run({ page }) {
      const toasts = await page.evaluate(() => {
        document.querySelectorAll('#toasts .toast').forEach(t => t.remove());
        toast('contrat ok');
        toast('contrat erreur', 'error');
        return [...document.querySelectorAll('#toasts .toast')].map(t => ({
          cls: t.className, role: t.getAttribute('role'), text: t.textContent,
        }));
      });
      await page.evaluate(() => document.querySelectorAll('#toasts .toast').forEach(t => t.remove()));
      const ok = toasts.length === 2
        && toasts[0].cls === 'toast ok' && toasts[0].role === 'status'
        && toasts[1].cls === 'toast error' && toasts[1].role === 'alert';
      return {
        ok,
        expected: 'toast ok → role=status, toast error → role=alert',
        observed: toasts.map(t => `${t.cls}:${t.role}`).join(', '),
      };
    },
  },
  {
    n: 18,
    title: 'localStorage settings keeps 4 baseline keys + relayUrl only',
    async run({ page }) {
      await page.evaluate(() => openIcsModal());
      await page.waitForFunction(() => document.querySelector('#icsModal').open);
      await page.fill('#icsUrl', SK_URL);
      await page.fill('#icsRelayUrl', RELAY_ORIGIN);
      await page.click('#icsSaveUrlBtn');
      await page.evaluate(() => document.querySelector('#icsModal').close());
      const s = await page.evaluate(() => {
        const stored = JSON.parse(localStorage.getItem('creno-data-v1') || 'null');
        return stored ? { keys: Object.keys(stored.settings || {}).sort(), relayUrl: stored.settings.relayUrl, skelloUrl: stored.settings.skelloUrl } : null;
      });
      const expectedKeys = [...BASE_SETTINGS_KEYS, 'relayUrl'].sort();
      const ok = !!s && JSON.stringify(s.keys) === JSON.stringify(expectedKeys)
        && s.relayUrl === RELAY_ORIGIN && s.skelloUrl === SK_URL;
      return {
        ok,
        expected: `settings keys = [${expectedKeys.join(',')}], relayUrl=${RELAY_ORIGIN}`,
        observed: s ? `keys=[${s.keys.join(',')}] relayUrl=${s.relayUrl} skelloUrl=${s.skelloUrl}` : 'no cre-no-data-v1 entry',
      };
    },
  },
];

/* ------------------------------------------------------------------ */
/* Runner                                                             */
/* ------------------------------------------------------------------ */

async function main() {
  if (!fs.existsSync(path.join(REPO, 'app.js')) || !fs.existsSync(path.join(REPO, 'index.html'))) {
    throw new Error(`not a Créno app root: ${REPO}`);
  }
  const { mod: playwright, from } = resolvePlaywright();
  const { browser, executablePath } = await launchBrowser(playwright.chromium);
  const server = await startServer(REPO);
  console.log(`[ui] repo: ${REPO}`);
  console.log(`[ui] playwright: ${from}`);
  console.log(`[ui] browser: ${executablePath}`);
  console.log(`[ui] frozen clock: ${FIXED_ISO} (week ${isoWeek(MON)}, ${fmtRange(MON, WEEK[6])})`);

  let failed = 0;
  let main1 = null;
  try {
    main1 = await openPage(browser, server.base, { width: 1440, height: 900 }, { skelloUrl: '' });
    for (const spec of CASES) {
      let r;
      try {
        r = await spec.run({ ...main1, browser, base: server.base });
      } catch (e) {
        r = { ok: false, expected: 'no throw', observed: String(e.message).split('\n')[0] };
      }
      if (!r.ok) {
        failed++;
        console.log(`FAIL ${spec.n}/${CASES.length} — ${spec.title}`);
        console.log(`     ✗ attendu ${r.expected}, obtenu ${r.observed}`);
      } else {
        console.log(`PASS ${spec.n}/${CASES.length} — ${spec.title}`);
      }
      /* Keep cases independent: no dialog or toast may leak into the next one. */
      await main1.page.evaluate(() => {
        document.querySelectorAll('dialog[open]').forEach(d => d.close());
        document.querySelectorAll('#toasts .toast').forEach(t => t.remove());
      });
    }
  } finally {
    if (main1) await main1.context.close();
    await browser.close();
    server.child.kill('SIGTERM');
  }

  if (external.length) {
    failed++;
    console.log('FAIL unexpected external request (network must stay offline)');
    for (const u of external.slice(0, 10)) console.log(`     ✗ ${u}`);
  }

  const passed = CASES.length - failed;
  console.log(`[ui] ${passed}/${CASES.length} cases passed`);
  process.exit(failed ? 1 : 0);
}

main().catch(err => {
  console.error(`[ui] SETUP ERROR: ${err.message}`);
  process.exit(2);
});
