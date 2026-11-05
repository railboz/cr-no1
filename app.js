'use strict';

/* ============================================================
   Créno — app de planning 100 % locale
   Données : localStorage · Import/Export : JSON
   ============================================================ */

/* ---------- Utilitaires ---------- */
const $  = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

const pad2 = n => String(n).padStart(2, '0');
const toISO = d => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
const fromISO = s => { const [y, m, d] = s.split('-').map(Number); return new Date(y, m - 1, d); };
const addDays = (d, n) => { const x = new Date(d.getFullYear(), d.getMonth(), d.getDate()); x.setDate(x.getDate() + n); return x; };
const startOfWeek = d => { const x = new Date(d.getFullYear(), d.getMonth(), d.getDate()); x.setDate(x.getDate() - ((x.getDay() + 6) % 7)); return x; };
const todayISO = () => toISO(new Date());
const uid = () => (crypto.randomUUID ? crypto.randomUUID() : 'id-' + Date.now() + '-' + Math.random().toString(36).slice(2, 9));

const MONTHS = ['janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.'];
const DAYS_SHORT = ['lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.', 'dim.'];
const DAYS_LONG = ['lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi', 'dimanche'];

function isoWeek(d) {
  const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
  const dn = (t.getUTCDay() + 6) % 7;
  t.setUTCDate(t.getUTCDate() - dn + 3);
  const f = new Date(Date.UTC(t.getUTCFullYear(), 0, 4));
  const fd = (f.getUTCDay() + 6) % 7;
  f.setUTCDate(f.getUTCDate() - fd + 3);
  return 1 + Math.round((t - f) / 604800000);
}

const mins = t => { const [h, m] = t.split(':').map(Number); return h * 60 + (m || 0); };
const durMinutes = (s, e) => { let d = mins(e) - mins(s); if (d <= 0) d += 1440; return d; };
function fmtDur(m) {
  m = Math.round(m);
  const h = Math.floor(m / 60), r = m % 60;
  if (r === 0) return h + 'h';
  if (h === 0) return r + ' min';
  return h + 'h' + pad2(r);
}
const fmtEur = n => n.toLocaleString('fr-FR', { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 });
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
function shade(hex, f) { /* f < 0 => assombrit, f > 0 => éclaircit */
  const n = parseInt(hex.slice(1), 16);
  let r = n >> 16 & 255, g = n >> 8 & 255, b = n & 255;
  const t = f < 0 ? 0 : 255, p = Math.abs(f);
  r = Math.round((t - r) * p + r);
  g = Math.round((t - g) * p + g);
  b = Math.round((t - b) * p + b);
  return '#' + [r, g, b].map(v => v.toString(16).padStart(2, '0')).join('');
}
function fmtRange(mon) {
  const sun = addDays(mon, 6);
  return `${mon.getDate()} ${MONTHS[mon.getMonth()]} – ${sun.getDate()} ${MONTHS[sun.getMonth()]} ${sun.getFullYear()}`;
}

/* ---------- Constantes métier ---------- */
const LS_KEY = 'creno-data-v1';
const IS_EXTENSION = location.protocol.startsWith('moz-extension');
const PALETTE = ['#34d399', '#60a5fa', '#f472b6', '#fbbf24', '#a78bfa', '#f87171', '#2dd4bf', '#e879f9'];
const AB_TYPES = {
  'Congés':     { color: '#a78bfa', icon: '🌴' },
  'RTT':        { color: '#60a5fa', icon: '🕘' },
  'Maladie':    { color: '#f87171', icon: '🤒' },
  'Formation':  { color: '#fbbf24', icon: '🎓' },
  'Sans solde': { color: '#94a3b8', icon: '📄' },
  'Autre':      { color: '#fb923c', icon: '📌' },
};

/* ---------- État & persistance ---------- */
function blankState() {
  return {
    version: 1,
    profile: { firstName: '', lastName: '', email: '', phone: '', rate: 12.5, objective: 35, refSalary: 0 },
    establishments: [],
    shifts: [],
    absences: [],
    availability: [true, true, true, true, true, false, false],
    settings: { skelloUrl: '', lastIcsSync: 0, allowProxy: true, mobileView: 'day' },
  };
}
const RE_ISO = /^\d{4}-\d{2}-\d{2}$/;
const RE_TIME = /^([01]\d|2[0-3]):[0-5]\d$/;
const RE_COLOR = /^#[0-9a-f]{6}$/i;
const RE_ID = /^[\w.:-]{1,64}$/;
const safeId = v => (typeof v === 'string' && RE_ID.test(v)) ? v : uid();
function migrate(data) {
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('format');
  const s = Object.assign(blankState(), data);
  s.profile = Object.assign({ firstName: '', lastName: '', email: '', phone: '', rate: 12.5, objective: 35, refSalary: 0 }, data.profile || {});
  /* Assainissement : une donnée mal formée (import JSON, ancienne version) ne doit ni planter
     l'affichage ni injecter du HTML (id / couleur sont insérés dans des attributs). */
  const isObj = x => x && typeof x === 'object' && !Array.isArray(x);
  s.establishments = (Array.isArray(data.establishments) ? data.establishments : []).filter(isObj).map((e, i) => ({
    ...e,
    id: safeId(e.id),
    name: String(e.name ?? '').slice(0, 40) || 'ÉTABLISSEMENT',
    color: RE_COLOR.test(e.color) ? e.color : PALETTE[i % PALETTE.length],
  }));
  const estIds = new Set(s.establishments.map(e => e.id));
  s.shifts = (Array.isArray(data.shifts) ? data.shifts : []).filter(isObj)
    .filter(x => RE_ISO.test(x.date) && RE_TIME.test(x.start) && RE_TIME.test(x.end) && estIds.has(x.estId))
    .map(x => ({ ...x, id: safeId(x.id), label: String(x.label ?? ''), note: String(x.note ?? '') }));
  s.absences = (Array.isArray(data.absences) ? data.absences : []).filter(isObj)
    .filter(x => RE_ISO.test(x.start) && RE_ISO.test(x.end) && x.end >= x.start)
    .map(x => ({ ...x, id: safeId(x.id), type: AB_TYPES[x.type] ? x.type : 'Autre', note: String(x.note ?? ''), status: String(x.status || 'Confirmée') }));
  s.availability = (Array.isArray(data.availability) && data.availability.length === 7)
    ? data.availability.slice()
    : [true, true, true, true, true, false, false];
  s.settings = Object.assign({ skelloUrl: '', lastIcsSync: 0, allowProxy: true, mobileView: 'day' }, data.settings || {});
  return s;
}
function seedDemo() { /* jeu de démo inspiré du planning Skello, ancré sur la semaine courante */
  const s = blankState();
  const est = { id: uid(), name: 'CHANTONNAY DISTRIBUTION', color: PALETTE[0] };
  const mon = startOfWeek(new Date());
  const mk = (off, start, end, label) => ({ id: uid(), estId: est.id, date: toISO(addDays(mon, off)), start, end, label, note: '' });
  s.establishments = [est];
  s.shifts = [
    mk(0, '08:00', '11:00', 'DRIVE'), mk(0, '12:00', '18:00', 'DRIVE'),
    mk(1, '12:00', '18:00', 'DRIVE'),
    mk(2, '12:00', '18:00', 'DRIVE'),
    mk(4, '12:00', '18:45', 'DRIVE'),
    mk(5, '08:00', '11:00', 'DRIVE'), mk(5, '12:00', '18:00', 'DRIVE'),
  ];
  try { localStorage.setItem(LS_KEY, JSON.stringify(s)); } catch (e) { /* ignore */ }
  return s;
}
function load() {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return seedDemo();
    try {
      return migrate(JSON.parse(raw));
    } catch (e) {
      /* Ne jamais écraser des données illisibles : on les met de côté avant de repartir sur une démo */
      console.error('Créno : données locales illisibles, copie de sauvegarde conservée.', e);
      try { localStorage.setItem(LS_KEY + '-corrupt-' + Date.now(), raw); } catch (e2) { /* ignore */ }
      return seedDemo();
    }
  } catch (e) {
    console.error('Créno : stockage local inaccessible.', e);
    return blankState();
  }
}

let state = load();
let weekStart = startOfWeek(new Date());
let currentView = 'planning';

let flashTimer = null;
function flashSaved() {
  const el = $('#saveState');
  if (!el) return;
  el.classList.add('flash');
  clearTimeout(flashTimer);
  flashTimer = setTimeout(() => el.classList.remove('flash'), 1200);
}
function save() {
  try { localStorage.setItem(LS_KEY, JSON.stringify(state)); }
  catch (e) { toast('Sauvegarde impossible (stockage plein ?)', 'error'); }
  flashSaved();
}

function getInitials() {
  return ((state.profile.firstName?.[0] || '') + (state.profile.lastName?.[0] || '')).toUpperCase() || '?';
}

/* ---------- Toasts & confirmation ---------- */
function toast(msg, type = 'ok') {
  const t = document.createElement('div');
  t.className = 'toast ' + type;
  t.textContent = msg;
  $('#toasts').appendChild(t);
  setTimeout(() => { t.classList.add('out'); setTimeout(() => t.remove(), 350); }, 2600);
}

let cfResolve = null;
function confirmDialog(title, msg, okLabel = 'Confirmer', danger = true) {
  return new Promise(res => {
    $('#cfTitle').textContent = title;
    $('#cfMsg').textContent = msg;
    const ok = $('#cfOk');
    ok.textContent = okLabel;
    ok.className = 'btn ' + (danger ? 'danger' : 'primary');
    settleConfirm(false);
    cfResolve = res;
    $('#confirmModal').showModal();
  });
}
function settleConfirm(v) { if (cfResolve) { const r = cfResolve; cfResolve = null; r(v); } }

/* ---------- Rendu global ---------- */
function switchView(v) {
  currentView = v;
  $$('#mainNav .nav-link').forEach(b => b.classList.toggle('active', b.dataset.view === v));
  $$('.view').forEach(sec => sec.classList.toggle('hidden', sec.id !== 'view-' + v));
  render();
}
function render() {
  renderAvatarArea();
  updateSyncBtn();
  if (currentView === 'planning') renderPlanning();
  else if (currentView === 'absences') renderAbsences();
  else if (currentView === 'stats') renderStats();
  else if (currentView === 'profile') renderProfile();
  else if (currentView === 'establishments') renderEstablishments();
}
function renderAvatarArea() {
  const ini = getInitials();
  $('#avatarBtn').textContent = ini;
  $('#menuAvatar').textContent = ini;
  const name = (state.profile.firstName + ' ' + state.profile.lastName).trim();
  $('#menuName').textContent = name || 'Mon profil';
  $('#menuEmail').textContent = state.profile.email || 'Aucun e-mail renseigné';
}

/* ---------- Planning ---------- */
function weekShifts() {
  const from = toISO(weekStart), to = toISO(addDays(weekStart, 6));
  return state.shifts.filter(s => s.date >= from && s.date <= to);
}
function minutesInWeek(mon, estId = null) {
  const from = toISO(mon), to = toISO(addDays(mon, 6));
  let t = 0;
  state.shifts.forEach(s => {
    if (s.date >= from && s.date <= to && (!estId || s.estId === estId)) t += durMinutes(s.start, s.end);
  });
  return t;
}
function renderPlanning() {
  $('#weekLabel').textContent = fmtRange(weekStart);
  $('#weekNum').textContent = 'Semaine ' + isoWeek(weekStart);
  $('#jumpDate').value = toISO(weekStart);
  renderEstPanel();
  renderGrid();
  renderSummary();
  renderMobileView();
  const wb = $('#mdWeekBtn');
  if (wb) wb.textContent = mdMode() === 'week' ? '🗓️ Jour' : '📅 Semaine';
}
function renderEstPanel() {
  const list = $('#estList');
  if (!state.establishments.length) {
    list.innerHTML = '<div class="est-empty">Aucun établissement.<br>Ajoutez-en un ci-dessous 👇</div>';
    return;
  }
  list.innerHTML = state.establishments.map(est => {
    const t = minutesInWeek(weekStart, est.id);
    const from = toISO(weekStart), to = toISO(addDays(weekStart, 6));
    const cnt = state.shifts.filter(s => s.estId === est.id && s.date >= from && s.date <= to).length;
    return `<div class="est-card" data-est="${est.id}" title="Modifier cet établissement">
      <div class="est-name"><span class="est-dot" style="background:${est.color}"></span>${esc(est.name)}</div>
      <div class="est-hours">${t ? fmtDur(t) : '0h'} · ${cnt} shift${cnt > 1 ? 's' : ''} cette semaine</div>
    </div>`;
  }).join('');
}
function renderGrid() {
  const grid = $('#weekGrid');
  const days = [...Array(7)].map((_, i) => addDays(weekStart, i));
  const today = todayISO();
  let html = '<div class="grid-head corner">Établissements</div>';
  days.forEach(d => {
    const iso = toISO(d), isT = iso === today;
    html += `<div class="grid-head day-head ${isT ? 'today' : ''}">
      <span class="dh-name">${DAYS_SHORT[(d.getDay() + 6) % 7]}</span>
      <span class="dh-num ${isT ? 'badge' : ''}">${d.getDate()}</span>
      <span class="dh-month">${MONTHS[d.getMonth()]}</span>
    </div>`;
  });
  if (!state.establishments.length) {
    html += '<div class="grid-empty">Ajoutez un établissement dans le panneau de gauche pour commencer votre planning ✨</div>';
  } else {
    state.establishments.forEach(est => {
      const t = minutesInWeek(weekStart, est.id);
      html += `<div class="est-cell sticky-left" data-est="${est.id}" title="Modifier cet établissement">
        <div class="est-name"><span class="est-dot" style="background:${est.color}"></span><span>${esc(est.name)}</span></div>
        <div class="est-hours">${t ? fmtDur(t) : '—'}</div>
      </div>`;
      days.forEach(d => {
        const iso = toISO(d);
        const dayShifts = state.shifts.filter(s => s.estId === est.id && s.date === iso).sort((a, b) => mins(a.start) - mins(b.start));
        const dayAbs = state.absences.filter(a => a.start <= iso && iso <= a.end);
        html += `<div class="cell ${iso === today ? 'today' : ''}" data-est="${est.id}" data-date="${iso}">`;
        dayAbs.forEach(a => {
          const meta = AB_TYPES[a.type] || AB_TYPES['Congés'];
          html += `<div class="absence" data-absence="1" style="border-color:${meta.color}55;color:${meta.color};background:${meta.color}14" title="${esc(a.note || a.type)}">${meta.icon} ${esc(a.type.toUpperCase())}</div>`;
        });
        dayShifts.forEach(s => {
          html += `<button type="button" class="shift" data-id="${s.id}" style="background:linear-gradient(135deg, ${est.color}, ${shade(est.color, -0.3)})" title="${esc(s.note || '')}">
            <span class="shift-time">${esc(s.start)} – ${esc(s.end)}</span>
            <span class="shift-dur">${fmtDur(durMinutes(s.start, s.end))}</span>
            <span class="shift-label">${esc(s.label || 'SHIFT')}</span>
          </button>`;
        });
        html += '</div>';
      });
    });
  }
  grid.innerHTML = html;
}
function renderSummary() {
  const ws = weekShifts();
  const total = ws.reduce((t, s) => t + durMinutes(s.start, s.end), 0);
  const obj = Math.max(1, Number(state.profile.objective) || 35);
  const pct = Math.min(100, Math.round(total / (obj * 60) * 100));
  const rate = Number(state.profile.rate) || 0;
  const pay = total / 60 * rate;
  $('#summaryBar').innerHTML = `
    <div class="sum-item"><span class="sum-num">${ws.length}</span><span>shift${ws.length > 1 ? 's' : ''}</span></div>
    <div class="sum-item"><span class="sum-num">${fmtDur(total)}</span><span>travaillées</span></div>
    <div class="progress-wrap"><div class="progress" style="width:${pct}%"></div></div>
    <div class="progress-label">Objectif ${obj} h · ${pct} %</div>
    <div class="sum-item"><span class="sum-num sum-money">${fmtEur(pay)}</span><span>estimés · ${rate.toLocaleString('fr-FR')} €/h</span></div>
    ${ws.length === 0 ? '<div class="progress-label">💡 Cliquez sur une case pour créer un shift</div>' : ''}`;
}
function changeWeek(n) { weekStart = addDays(weekStart, n * 7); renderPlanning(); }

/* ---------- Vue mobile jour-par-jour + liste semaine (bouton Semaine) ---------- */
let mobileDayIdx = (new Date().getDay() + 6) % 7;
const mobileTodayIdx = () => (new Date().getDay() + 6) % 7;
const mdMode = () => (state.settings.mobileView === 'week' ? 'week' : 'day');

/* Animation de glissement (swipe jour précédent/suivant, semaine en vue Semaine) */
let mdSlideDir = 0; /* 0 = aucune, 1 = entrée par la droite (suivant), -1 = par la gauche (précédent) */
function mdClearAnim(wrap) { wrap.classList.remove('md-anim-next', 'md-anim-prev', 'md-bump-next', 'md-bump-prev'); }
function mdPlayAnim(wrap) {
  if (!mdSlideDir) return;
  mdClearAnim(wrap);
  void wrap.offsetWidth; /* reflow : relance l'animation même sur deux swipes rapprochés */
  wrap.classList.add(mdSlideDir > 0 ? 'md-anim-next' : 'md-anim-prev');
  mdSlideDir = 0;
}
function mdBump(wrap, dir) {
  mdClearAnim(wrap);
  void wrap.offsetWidth;
  wrap.classList.add(dir > 0 ? 'md-bump-next' : 'md-bump-prev');
}

function renderMobileView() {
  const wrap = $('#mobileDayView');
  if (!wrap) return;
  const today = todayISO();

  if (mdMode() === 'week') {
    /* ---- Liste verticale des 7 jours ---- */
    let daysHtml = '';
    for (let i = 0; i < 7; i++) {
      const d = addDays(weekStart, i), iso = toISO(d);
      const past = iso < today, isToday = iso === today;

      const dayShifts = [];
      state.establishments.forEach(est => {
        state.shifts.filter(s => s.estId === est.id && s.date === iso).forEach(s => dayShifts.push({ s, est }));
      });
      dayShifts.sort((a, b) => mins(a.s.start) - mins(b.s.start));
      const abs = state.absences.filter(a => a.start <= iso && iso <= a.end);

      let cards = abs.map(a => {
        const meta = AB_TYPES[a.type] || AB_TYPES['Congés'];
        return `<div class="md-absence ${past ? 'past' : ''}" data-absence="1" style="border-color:${meta.color}55;color:${meta.color};background:${meta.color}14">${meta.icon} ${esc(a.type.toUpperCase())}</div>`;
      }).join('');
      cards += dayShifts.map(({ s, est }) => `
        <button type="button" class="md-shift ${past ? 'past' : ''}" data-id="${s.id}" style="--c:${est.color}" title="${esc(s.note || '')}">
          <span class="md-bar"></span>
          <span class="md-row"><span class="md-time">${esc(s.start)} – ${esc(s.end)}</span><span class="md-estname">${esc(est.name)}</span><span class="md-dur">${fmtDur(durMinutes(s.start, s.end))}</span></span>
          <span class="md-label">${esc(s.label || 'SHIFT')}</span>
        </button>`).join('');
      if (!cards) cards = `<button type="button" class="md-nochip" data-date="${iso}">ⓘ Aucun shift prévu</button>`;

      daysHtml += `<div class="md-dayblock">
        <div class="md-dayhead ${isToday ? 'today' : ''}">
          <span class="md-hline"></span>
          <span class="md-dhead">${DAYS_SHORT[i]} <b class="md-dnum">${d.getDate()}</b> ${MONTHS[d.getMonth()]}</span>
          <span class="md-hline"></span>
        </div>
        <div class="md-cards">${cards}</div>
      </div>`;
    }

    const sync = state.settings.skelloUrl ? '<button type="button" class="md-sync" title="Synchroniser Skello">⟳ Sync</button>' : '';
    wrap.innerHTML = `
      <div class="md-date">${sync}<b>Semaine ${isoWeek(weekStart)}</b> · ${esc(fmtRange(weekStart))}</div>
      <div class="md-days">${daysHtml}</div>`;
    mdPlayAnim(wrap);
    return;
  }

  /* ---- Vue jour (par défaut) ---- */
  const day = addDays(weekStart, mobileDayIdx);
  const iso = toISO(day);

  let body = '';
  if (!state.establishments.length) {
    body = '<div class="md-empty" id="mdNoEst">🏢 Aucun établissement<br><span class="small">Touchez ici pour en ajouter un (menu → Établissements)</span></div>';
  } else {
    state.establishments.forEach(est => {
      const shifts = state.shifts.filter(s => s.estId === est.id && s.date === iso).sort((a, b) => mins(a.start) - mins(b.start));
      const abs = state.absences.filter(a => a.start <= iso && iso <= a.end);
      if (!shifts.length && !abs.length) return;
      const total = shifts.reduce((t, s) => t + durMinutes(s.start, s.end), 0);
      body += `<div class="md-est">
        <div class="md-est-head" data-est="${est.id}">
          <span class="est-dot" style="background:${est.color}"></span>
          <span>${esc(est.name)}</span>
          ${total ? `<span class="md-est-total">${fmtDur(total)}</span>` : ''}
        </div>
        ${abs.map(a => {
          const meta = AB_TYPES[a.type] || AB_TYPES['Congés'];
          return `<div class="md-absence" data-absence="1" style="border-color:${meta.color}55;color:${meta.color};background:${meta.color}14">${meta.icon} ${esc(a.type.toUpperCase())}</div>`;
        }).join('')}
        ${shifts.map(s => `
          <button type="button" class="shift" data-id="${s.id}" style="background:linear-gradient(135deg, ${est.color}, ${shade(est.color, -0.3)})" title="${esc(s.note || '')}">
            <span class="shift-time">${esc(s.start)} – ${esc(s.end)}</span>
            <span class="shift-dur">${fmtDur(durMinutes(s.start, s.end))}</span>
            <span class="shift-label">${esc(s.label || 'SHIFT')}</span>
          </button>`).join('')}
      </div>`;
    });
    if (!body) {
      body = `<div class="md-empty">😌 Rien de prévu le ${DAYS_LONG[mobileDayIdx]} ${day.getDate()} ${MONTHS[day.getMonth()]}<br><span class="small">Touchez ici pour ajouter un shift</span></div>`;
    }
  }

  /* ⟳ Sync compact dans la ligne de date (uniquement si une URL Skello est enregistrée) */
  const sync = state.settings.skelloUrl ? '<button type="button" class="md-sync" title="Synchroniser Skello">⟳ Sync</button>' : '';

  wrap.innerHTML = `
    <div class="md-date">${sync}<b>${DAYS_LONG[mobileDayIdx]}</b> ${day.getDate()} ${MONTHS[day.getMonth()]} ${day.getFullYear()}${iso === today ? ' · <span class="md-today">aujourd&#39;hui</span>' : ''}</div>
    <div class="md-list">${body}</div>
    <button type="button" class="btn primary md-add" id="mdAdd">＋ Ajouter un shift</button>`;
  mdPlayAnim(wrap);
}
/* ---------- Modal shift ---------- */
let editingShiftId = null;
function fillEstSelect(sel) {
  sel.innerHTML = state.establishments.map(e => `<option value="${e.id}">${esc(e.name)}</option>`).join('');
}
function openShiftModal(id = null, defaults = {}) {
  if (!state.establishments.length) {
    toast("Créez d'abord un établissement", 'error');
    $('#estAddName')?.focus();
    return;
  }
  editingShiftId = id;
  fillEstSelect($('#shEst'));
  const s = id ? state.shifts.find(x => x.id === id) : null;
  if (s) {
    $('#shEst').value = s.estId;
    $('#shDate').value = s.date;
    $('#shStart').value = s.start;
    $('#shEnd').value = s.end;
    $('#shLabel').value = s.label || '';
    $('#shNote').value = s.note || '';
    $('#shDelete').classList.remove('hidden');
    $('#shTitle').textContent = 'Modifier le shift';
  } else {
    $('#shEst').value = defaults.estId || state.establishments[0].id;
    $('#shDate').value = defaults.date || todayISO();
    $('#shStart').value = defaults.start || '09:00';
    $('#shEnd').value = defaults.end || '17:00';
    $('#shLabel').value = defaults.label || '';
    $('#shNote').value = '';
    $('#shDelete').classList.add('hidden');
    $('#shTitle').textContent = 'Nouveau shift';
  }
  updateShiftMeta();
  $('#shiftModal').showModal();
}
function updateShiftMeta() {
  const s = $('#shStart').value, e = $('#shEnd').value, d = $('#shDate').value;
  $('#shDur').textContent = (s && e) ? 'Durée : ' + fmtDur(durMinutes(s, e)) : '';
  const warn = $('#shWarn');
  if (d && s && e) {
    const wd = (fromISO(d).getDay() + 6) % 7;
    if (state.availability[wd] === false) {
      warn.textContent = `⚠️ Vous êtes habituellement indisponible le ${DAYS_LONG[wd]}.`;
      warn.classList.remove('hidden');
    } else warn.classList.add('hidden');
  } else warn.classList.add('hidden');
}

/* ---------- Modal absence ---------- */
let editingAbsenceId = null;
function openAbsenceModal(id = null) {
  editingAbsenceId = id;
  const sel = $('#abType');
  sel.innerHTML = Object.keys(AB_TYPES).map(t => `<option value="${t}">${AB_TYPES[t].icon} ${t}</option>`).join('');
  const a = id ? state.absences.find(x => x.id === id) : null;
  if (a) {
    sel.value = a.type;
    $('#abStart').value = a.start;
    $('#abEnd').value = a.end;
    $('#abNote').value = a.note || '';
    $('#abDelete').classList.remove('hidden');
    $('#abTitle').textContent = "Modifier l'absence";
  } else {
    sel.selectedIndex = 0;
    const t = todayISO();
    $('#abStart').value = t;
    $('#abEnd').value = t;
    $('#abNote').value = '';
    $('#abDelete').classList.add('hidden');
    $('#abTitle').textContent = 'Demander une absence';
  }
  $('#absenceModal').showModal();
}

/* ---------- Modal établissement ---------- */
let editingEstId = null, estModalColor = null;
function openEstModal(id) {
  const est = state.establishments.find(e => e.id === id);
  if (!est) return;
  editingEstId = id;
  estModalColor = est.color;
  $('#estName').value = est.name;
  renderEstColors();
  $('#estDelete').classList.remove('hidden');
  $('#estModal').showModal();
}
function renderEstColors() {
  $('#estColors').innerHTML = PALETTE.map(c =>
    `<button type="button" class="color-dot ${c === estModalColor ? 'sel' : ''}" data-color="${c}" style="background:${c}" title="${c}"></button>`
  ).join('');
}

/* ---------- Établissements (vue dédiée, ouverte depuis le menu) ---------- */
function renderEstablishments() {
  const list = $('#estViewList');
  if (!state.establishments.length) {
    list.innerHTML = '<div class="est-empty">Aucun établissement.<br>Ajoutez-en un ci-dessus 👆</div>';
    return;
  }
  list.innerHTML = state.establishments.map(est => {
    const mine = state.shifts.filter(s => s.estId === est.id);
    const total = mine.reduce((t, s) => t + durMinutes(s.start, s.end), 0);
    return `<div class="est-card" data-est="${est.id}" title="Modifier cet établissement">
      <div class="est-name"><span class="est-dot" style="background:${est.color}"></span>${esc(est.name)}</div>
      <div class="est-hours">${fmtDur(total)} · ${mine.length} shift${mine.length > 1 ? 's' : ''} au total</div>
    </div>`;
  }).join('');
}

/* ---------- Absences (vue) ---------- */
function renderAbsences() {
  const list = $('#absenceList'), empty = $('#absenceEmpty');
  if (!state.absences.length) {
    empty.classList.remove('hidden');
    list.classList.add('hidden');
    list.innerHTML = '';
    return;
  }
  empty.classList.add('hidden');
  list.classList.remove('hidden');
  list.innerHTML = state.absences.slice().sort((a, b) => a.start.localeCompare(b.start)).map(a => {
    const meta = AB_TYPES[a.type] || AB_TYPES['Congés'];
    const d1 = fromISO(a.start), d2 = fromISO(a.end);
    const dates = a.start === a.end
      ? `le ${d1.getDate()} ${MONTHS[d1.getMonth()]} ${d1.getFullYear()}`
      : `du ${d1.getDate()} ${MONTHS[d1.getMonth()]} au ${d2.getDate()} ${MONTHS[d2.getMonth()]} ${d2.getFullYear()}`;
    return `<div class="absence-item" style="--ac:${meta.color}">
      <div class="a-icon">${meta.icon}</div>
      <div>
        <div class="a-type">${esc(a.type)}</div>
        <div class="a-dates">${dates}${a.note ? ` · <em>${esc(a.note)}</em>` : ''}</div>
      </div>
      <span class="badge-status">✓ ${esc(a.status || 'Confirmée')}</span>
      <div class="a-actions">
        <button type="button" class="icon-btn" data-edit="${a.id}" title="Modifier">✎</button>
        <button type="button" class="icon-btn" data-del="${a.id}" title="Supprimer">🗑️</button>
      </div>
    </div>`;
  }).join('');
}

/* ---------- Statistiques ---------- */
function renderStats() {
  const now = new Date();
  const curWeek = startOfWeek(now);
  const wk = minutesInWeek(curWeek);
  let mMin = 0, mCount = 0;
  const mPrefix = `${now.getFullYear()}-${pad2(now.getMonth() + 1)}`;
  state.shifts.forEach(s => {
    if (s.date.startsWith(mPrefix)) { mMin += durMinutes(s.start, s.end); mCount++; }
  });
  const rate = Number(state.profile.rate) || 0;
  const obj = Number(state.profile.objective) || 35;
  $('#statWeek').textContent = fmtDur(wk);
  $('#statWeekSub').textContent = `Objectif : ${obj} h`;
  $('#statMonth').textContent = fmtDur(mMin);
  $('#statMonthSub').textContent = MONTHS[now.getMonth()].replace('.', '') + ' ' + now.getFullYear();
  $('#statShifts').textContent = mCount;
  const pay = mMin / 60 * rate;
  $('#statPay').textContent = fmtEur(pay);
  $('#statPaySub').textContent = `${mCount} shifts · ${fmtDur(mMin)} en ${MONTHS[now.getMonth()]} · ${rate.toLocaleString('fr-FR')} €/h`;
  const ref = Number(state.profile.refSalary) || 0;
  if (ref > 0) {
    $('#statPayMini').classList.remove('hidden');
    $('#statPayBar').style.width = Math.min(100, Math.round(pay / ref * 100)) + '%';
    $('#statPaySub').textContent += ` · ${Math.round(pay / ref * 100)} % de ${fmtEur(ref)}`;
  } else {
    $('#statPayMini').classList.add('hidden');
  }

  /* 8 dernières semaines */
  const start = addDays(curWeek, -7 * 7);
  const weeks = [];
  for (let i = 0; i < 8; i++) {
    const mon = addDays(start, i * 7);
    weeks.push({ mon, min: minutesInWeek(mon) });
  }
  const max = Math.max(...weeks.map(w => w.min), 60);
  $('#barsWeeks').innerHTML = weeks.map(w => {
    const pct = Math.round(w.min / max * 100);
    const cur = toISO(w.mon) === toISO(curWeek);
    return `<div class="bar-col">
      <div class="bar-zone"><div class="bar ${cur ? 'cur' : ''}" style="height:${Math.max(3, pct)}%"><span class="bar-tip">${w.min ? fmtDur(w.min) : ''}</span></div></div>
      <span class="bar-x">S${isoWeek(w.mon)}</span>
    </div>`;
  }).join('');

  /* 30 derniers jours */
  const from = toISO(addDays(now, -29)), upto = toISO(now);
  const recent = state.shifts.filter(s => s.date >= from && s.date <= upto);

  const byEst = state.establishments.map(est => ({
    name: est.name, color: est.color,
    min: recent.filter(s => s.estId === est.id).reduce((t, s) => t + durMinutes(s.start, s.end), 0),
  })).filter(x => x.min > 0).sort((a, b) => b.min - a.min);
  const maxEst = Math.max(...byEst.map(x => x.min), 1);
  $('#hbarsEst').innerHTML = byEst.length
    ? byEst.map(x => `<div class="hbar-row">
        <div class="hbar-key"><span class="est-dot" style="background:${x.color}"></span>${esc(x.name)}</div>
        <div class="hbar-track"><div class="hbar" style="width:${Math.round(x.min / maxEst * 100)}%;background:${x.color}"></div></div>
        <div class="hbar-val">${fmtDur(x.min)}</div>
      </div>`).join('')
    : '<div class="muted">Pas encore de données sur les 30 derniers jours.</div>';

  const byLabel = {};
  recent.forEach(s => {
    const k = (s.label || 'SHIFT').toUpperCase();
    byLabel[k] = (byLabel[k] || 0) + durMinutes(s.start, s.end);
  });
  const top = Object.entries(byLabel).sort((a, b) => b[1] - a[1]).slice(0, 5);
  const maxL = Math.max(...top.map(x => x[1]), 1);
  $('#hbarsLabels').innerHTML = top.length
    ? top.map(([k, v]) => `<div class="hbar-row">
        <div class="hbar-key">${esc(k)}</div>
        <div class="hbar-track"><div class="hbar" style="width:${Math.round(v / maxL * 100)}%;background:var(--blue)"></div></div>
        <div class="hbar-val">${fmtDur(v)}</div>
      </div>`).join('')
    : '<div class="muted">Pas encore de données.</div>';
}

/* ---------- Profil ---------- */
function renderProfile() {
  const p = state.profile;
  $('#pFirst').value = p.firstName;
  $('#pLast').value = p.lastName;
  $('#pEmail').value = p.email;
  $('#pPhone').value = p.phone;
  $('#pRate').value = p.rate;
  $('#pObjective').value = p.objective;
  $('#pRefSalary').value = p.refSalary || '';
  renderProfilePreview();
  renderChips();
}
function renderProfilePreview() {
  const ini = getInitials();
  $('#profileAvatar').textContent = ini;
  $('#avatarBtn').textContent = ini;
  $('#menuAvatar').textContent = ini;
  const name = (state.profile.firstName + ' ' + state.profile.lastName).trim();
  $('#profileName').textContent = name || 'Votre profil';
  $('#menuName').textContent = name || 'Mon profil';
  $('#profileSub').textContent = state.profile.email || "Renseignez vos informations pour personnaliser l'app";
  $('#menuEmail').textContent = state.profile.email || 'Aucun e-mail renseigné';
}
function renderChips() {
  $('#availChips').innerHTML = DAYS_LONG.map((d, i) =>
    `<button type="button" class="chip ${state.availability[i] !== false ? 'on' : ''}" data-i="${i}">${d.charAt(0).toUpperCase() + d.slice(1)}</button>`
  ).join('');
}

/* ---------- Import Skello (ICS) ---------- */
function unfoldICS(text) {
  return String(text).replace(/\r\n/g, '\n').replace(/\r/g, '\n').replace(/\n[ \t]/g, '');
}
function unescapeICS(v) {
  return String(v ?? '').replace(/\\n/gi, ' ').replace(/\\,/g, ',').replace(/\\;/g, ';').replace(/\\\\/g, '\\').trim();
}
function parseICS(text) {
  const events = [];
  let cur = null;
  for (const raw of unfoldICS(text).split('\n')) {
    const line = raw.trim();
    if (/^BEGIN:VEVENT$/i.test(line)) { cur = {}; continue; }
    if (/^END:VEVENT$/i.test(line)) { if (cur) events.push(cur); cur = null; continue; }
    if (!cur) continue;
    const ci = line.indexOf(':');
    if (ci <= 0) continue;
    const segs = line.slice(0, ci).split(';');
    const key = segs[0].toUpperCase();
    if (!['DTSTART', 'DTEND', 'SUMMARY', 'LOCATION', 'DESCRIPTION'].includes(key)) continue;
    const params = {};
    segs.slice(1).forEach(p => { const eq = p.indexOf('='); if (eq > -1) params[p.slice(0, eq).toUpperCase()] = p.slice(eq + 1); });
    cur[key] = { value: line.slice(ci + 1), params };
  }
  return events;
}
function parseICalDate(prop) {
  if (!prop) return null;
  const str = String(prop.value).trim();
  if (/^\d{8}$/.test(str) || prop.params.VALUE === 'DATE') {
    const m = str.match(/^(\d{4})(\d{2})(\d{2})/);
    if (!m) return null;
    return { allDay: true, date: new Date(+m[1], +m[2] - 1, +m[3]) };
  }
  const m = str.match(/^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})?(Z)?$/i);
  if (!m) return null;
  const [, y, mo, d, h, mi, s, z] = m;
  const date = z
    ? new Date(Date.UTC(+y, +mo - 1, +d, +h, +mi, +(s || 0)))
    : new Date(+y, +mo - 1, +d, +h, +mi, +(s || 0)); // TZID / heure flottante => heure locale
  return { allDay: false, date };
}
function guessAbsType(text) {
  const t = (text || '').toUpperCase();
  if (/RTT/.test(t)) return 'RTT';
  if (/MALAD|ARR[ÊE]T|SICK/.test(t)) return 'Maladie';
  if (/FORMATION|TRAINING/.test(t)) return 'Formation';
  if (/SOLDE|UNPAID/.test(t)) return 'Sans solde';
  if (/AUTRE|MISC/.test(t)) return 'Autre';
  return 'Congés';
}
async function applyICSImport(text, opts = {}) {
  if (!/BEGIN:VCALENDAR|BEGIN:VEVENT/i.test(text)) {
    toast('Ce contenu ne ressemble pas à un calendrier ICS', 'error');
    return;
  }
  const events = parseICS(text);
  if (!events.length) { toast('Aucun événement trouvé dans ce calendrier', 'error'); return; }

  const estByName = new Map(state.establishments.map(e => [e.name.toUpperCase(), e]));
  const newEst = [], newShifts = [], newAbs = [];
  let ignored = 0;

  for (const ev of events) {
    const s = parseICalDate(ev.DTSTART);
    if (!s) { ignored++; continue; }
    const summary = unescapeICS(ev.SUMMARY?.value || '');
    if (s.allDay) {
      /* Journée entière => absence (DTEND est exclusif dans le format ICS) */
      const eProp = parseICalDate(ev.DTEND);
      let end = s.date;
      if (eProp && eProp.allDay) end = addDays(eProp.date, -1);
      if (end < s.date) end = s.date;
      newAbs.push({
        type: guessAbsType(summary),
        start: toISO(s.date),
        end: toISO(end),
        note: summary,
        status: 'Confirmée',
      });
      continue;
    }
    /* Créneau horaire => shift */
    const eProp = parseICalDate(ev.DTEND);
    const startDT = s.date;
    const endDT = eProp && eProp.date >= startDT ? eProp.date : new Date(startDT.getTime() + 60 * 60000);
    const loc = unescapeICS(ev.LOCATION?.value || '');
    let est = loc ? estByName.get(loc.toUpperCase()) : null;
    if (loc && !est) {
      est = { id: uid(), name: loc.toUpperCase().slice(0, 40), color: PALETTE[(state.establishments.length + newEst.length) % PALETTE.length] };
      newEst.push(est);
      estByName.set(est.name.toUpperCase(), est);
    }
    newShifts.push({
      estId: est ? est.id : null,
      date: toISO(startDT),
      start: pad2(startDT.getHours()) + ':' + pad2(startDT.getMinutes()),
      end: pad2(endDT.getHours()) + ':' + pad2(endDT.getMinutes()),
      label: (summary || 'SHIFT').toUpperCase().slice(0, 40),
      note: unescapeICS(ev.DESCRIPTION?.value || ''),
    });
  }

  if (opts.confirm !== false) {
    const ok = await confirmDialog(
      'Importer ce calendrier ?',
      `${newShifts.length} shift(s), ${newAbs.length} absence(s) et ${newEst.length} nouvel(aux) établissement(s) détecté(s)` +
      (ignored ? ` · ${ignored} événement(s) illisible(s)` : '') +
      '. Les doublons seront ignorés.',
      'Importer', false
    );
    if (!ok) return null;
  }

  state.establishments.push(...newEst);
  if (!state.establishments.length) {
    state.establishments.push({ id: uid(), name: 'SKELLO', color: PALETTE[0] });
  }
  const existing = new Set(state.shifts.map(s => `${s.estId}|${s.date}|${s.start}|${s.end}`));
  const existingAbs = new Set(state.absences.map(a => `${a.type}|${a.start}|${a.end}`));
  let addedS = 0, addedA = 0, dup = 0, minDate = null;

  for (const sh of newShifts) {
    if (!sh.estId) sh.estId = state.establishments[0].id;
    const k = `${sh.estId}|${sh.date}|${sh.start}|${sh.end}`;
    if (existing.has(k)) { dup++; continue; }
    existing.add(k);
    state.shifts.push({ id: uid(), ...sh });
    addedS++;
    if (!minDate || sh.date < minDate) minDate = sh.date;
  }
  for (const ab of newAbs) {
    const k = `${ab.type}|${ab.start}|${ab.end}`;
    if (existingAbs.has(k)) { dup++; continue; }
    existingAbs.add(k);
    state.absences.push({ id: uid(), ...ab });
    addedA++;
    if (!minDate || ab.start < minDate) minDate = ab.start;
  }

  const stats = { addedS, addedA, dup, ignored, est: newEst.length };
  save();
  const manual = opts.confirm !== false; /* import manuel : on affiche les nouveautés ; synchro : on reste où on est */
  if (manual && (addedS || addedA) && minDate) weekStart = startOfWeek(fromISO(minDate));
  if ($('#icsModal').open) $('#icsModal').close();
  if (manual && (addedS || addedA)) switchView('planning');
  else render();
  if (opts.toast !== false) {
    toast(`Import Skello : ${addedS} shift(s), ${addedA} absence(s)${dup ? ` · ${dup} doublon(s) ignoré(s)` : ''} ✓`);
  }
  return stats;
}
function openIcsModal() {
  $('#icsUrl').value = state.settings.skelloUrl || '';
  $('#icsAllowProxy').checked = state.settings.allowProxy !== false;
  if (IS_EXTENSION) $('#extSyncNote').classList.remove('hidden');
  renderSyncInfo();
  $('#icsModal').showModal();
}
function renderSyncInfo() {
  const t = state.settings.lastIcsSync;
  $('#icsLastSync').textContent = t
    ? 'Dernière synchro : ' + new Date(t).toLocaleString('fr-FR', { dateStyle: 'short', timeStyle: 'short' })
    : 'Jamais synchronisé';
  $('#icsSyncNowBtn').disabled = !(state.settings.skelloUrl || '').trim();
}
function updateSyncBtn() {
  $('#syncIcsBtn').classList.toggle('hidden', !(state.settings.skelloUrl || '').trim());
}
function setSyncBtn(mode) {
  const b = $('#syncIcsBtn');
  if (b) b.textContent = mode === 'busy' ? '⟳ Synchro…' : '⟳ Sync Skello';
}

/* Récupère l'ICS depuis l'URL enregistrée : direct, puis relais publics si bloqué */
let icsSyncing = false;
async function syncSkello(opts = {}) {
  const url = (state.settings.skelloUrl || '').trim();
  if (!url) { openIcsModal(); return; }
  if (icsSyncing) return;
  icsSyncing = true;
  $('#icsSyncNowBtn').disabled = true;
  setSyncBtn('busy');
  if (!opts.silent) toast('Synchronisation avec Skello…');

  const routes = [{ name: 'direct', url }];
  if (state.settings.allowProxy !== false) {
    const enc = encodeURIComponent(url);
    routes.push({ name: 'relais allorigins', url: 'https://api.allorigins.win/raw?url=' + enc });
    routes.push({ name: 'relais corsproxy', url: 'https://corsproxy.io/?url=' + enc });
    routes.push({ name: 'relais codetabs', url: 'https://api.codetabs.com/v1/proxy?quest=' + enc });
  }

  let txt = null, via = null;
  for (const r of routes) {
    try {
      const res = await fetch(r.url, { redirect: 'follow', signal: AbortSignal.timeout(15000) });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const body = await res.text();
      if (!/BEGIN:VCALENDAR|BEGIN:VEVENT/i.test(body)) throw new Error('format');
      txt = body;
      via = r.name;
      break;
    } catch (e) { /* route suivante */ }
  }
  icsSyncing = false;
  $('#icsSyncNowBtn').disabled = false;

  if (!txt) {
    setSyncBtn('idle');
    toast('Récupération impossible (hors-ligne, CORS ou URL invalide). Le fichier .ics reste la méthode 100 % fiable.', 'error');
    return;
  }

  let stats = null;
  try { stats = await applyICSImport(txt, { confirm: false, toast: !opts.silent }); }
  catch (e) { console.error('Créno : échec import ICS', e); toast('Import du calendrier impossible', 'error'); }
  finally { setSyncBtn('idle'); }
  if (!stats) return;
  state.settings.lastIcsSync = Date.now();
  save();
  renderSyncInfo();
  if (opts.silent && (stats.addedS || stats.addedA)) {
    toast(`Skello : +${stats.addedS} shift(s), +${stats.addedA} absence(s) · via ${via}`);
  }
}

/* ---------- Import / Export / Réinitialisation ---------- */
function exportJSON() {
  const blob = new Blob([JSON.stringify(state, null, 2)], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `creno-export-${todayISO()}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
  toast('Export téléchargé ✓');
}
function importJSON(file) {
  file.text().then(txt => {
    let data;
    try { data = JSON.parse(txt); } catch { throw new Error('parse'); }
    if (!data || !Array.isArray(data.establishments) || !Array.isArray(data.shifts)) throw new Error('format');
    return confirmDialog(
      'Importer ces données ?',
      `${data.establishments.length} établissement(s), ${data.shifts.length} shift(s) et ${data.absences?.length || 0} absence(s) remplaceront vos données actuelles.`,
      'Importer', false
    ).then(ok => {
      if (!ok) return;
      state = migrate(data);
      save();
      weekStart = startOfWeek(new Date());
      switchView('planning');
      toast('Données importées ✓');
    });
  }).catch(err => {
    toast(err.message === 'parse' ? 'Fichier JSON illisible' : 'Format de fichier non reconnu', 'error');
  });
}
async function resetData() {
  const ok = await confirmDialog(
    'Réinitialiser toutes les données ?',
    'Votre planning, vos établissements et vos absences seront définitivement effacés de ce navigateur.',
    'Tout effacer'
  );
  if (!ok) return;
  state = blankState();
  save();
  weekStart = startOfWeek(new Date());
  switchView('planning');
  toast('Données réinitialisées');
}

/* ---------- Initialisation ---------- */
/* PWA : enregistrement du service worker (installation + hors-ligne).
   Ignoré dans l'extension Firefox et en file:// */
if ('serviceWorker' in navigator && location.protocol.startsWith('http')) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('sw.js').catch(() => { /* hors-ligne ou non supporté */ });
  });
}

function init() {
  /* Navigation */
  $$('#mainNav .nav-link').forEach(b => b.addEventListener('click', () => switchView(b.dataset.view)));

  /* Menu avatar */
  $('#avatarBtn').addEventListener('click', e => { e.stopPropagation(); $('#avatarMenu').classList.toggle('hidden'); });
  document.addEventListener('click', e => {
    if (!e.target.closest('.avatar-wrap')) $('#avatarMenu').classList.add('hidden');
  });
  $$('#avatarMenu [data-action]').forEach(b => b.addEventListener('click', () => {
    $('#avatarMenu').classList.add('hidden');
    const a = b.dataset.action;
    if (a === 'goto-profile') switchView('profile');
    else if (a === 'goto-establishments') switchView('establishments');
    else if (a === 'goto-absences') switchView('absences');
    else if (a === 'export') exportJSON();
    else if (a === 'import-ics') openIcsModal();
    else if (a === 'import') $('#importFile').click();
    else if (a === 'reset') resetData();
  }));

  /* Navigation semaines */
  $('#prevWeek').addEventListener('click', () => changeWeek(-1));
  $('#nextWeek').addEventListener('click', () => changeWeek(1));
  $('#todayBtn').addEventListener('click', () => { weekStart = startOfWeek(new Date()); mobileDayIdx = mobileTodayIdx(); renderPlanning(); });
  $('#jumpDate').addEventListener('change', () => {
    const v = $('#jumpDate').value;
    if (v) { const d = fromISO(v); weekStart = startOfWeek(d); mobileDayIdx = (d.getDay() + 6) % 7; renderPlanning(); }
  });
  $('#addShiftBtn').addEventListener('click', () => openShiftModal(null, { date: toISO(weekStart) }));

  /* Grille : clic sur shift / absence / cellule vide / établissement */
  $('#weekGrid').addEventListener('click', e => {
    const sh = e.target.closest('.shift');
    if (sh) { openShiftModal(sh.dataset.id); return; }
    if (e.target.closest('.absence')) { switchView('absences'); return; }
    const ec = e.target.closest('.est-cell');
    if (ec) { openEstModal(ec.dataset.est); return; }
    const cell = e.target.closest('.cell');
    if (cell) openShiftModal(null, { estId: cell.dataset.est, date: cell.dataset.date });
  });

  /* Vue mobile : clics + swipe (jour en vue Jour, semaine en vue Semaine) */
  const mdView = $('#mobileDayView');
  mdView.addEventListener('click', e => {
    if (e.target.closest('.md-sync')) { $('#syncIcsBtn')?.click(); return; }
    const wsh = e.target.closest('.md-shift');
    if (wsh) { openShiftModal(wsh.dataset.id); return; }
    const chip = e.target.closest('.md-nochip');
    if (chip) { openShiftModal(null, { date: chip.dataset.date }); return; }
    const sh = e.target.closest('.shift');
    if (sh) { openShiftModal(sh.dataset.id); return; }
    if (e.target.closest('.md-absence')) { switchView('absences'); return; }
    const head = e.target.closest('.md-est-head');
    if (head) { openEstModal(head.dataset.est); return; }
    if (e.target.closest('#mdNoEst')) { switchView('establishments'); return; }
    if (e.target.closest('#mdAdd') || e.target.closest('.md-empty')) {
      openShiftModal(null, { date: toISO(addDays(weekStart, mobileDayIdx)) });
    }
  });
  let mdTouch = null;
  mdView.addEventListener('touchstart', e => {
    if (e.touches.length === 1) mdTouch = { x: e.touches[0].clientX, y: e.touches[0].clientY };
  }, { passive: true });
  mdView.addEventListener('touchend', e => {
    if (!mdTouch) return;
    const dx = e.changedTouches[0].clientX - mdTouch.x;
    const dy = e.changedTouches[0].clientY - mdTouch.y;
    mdTouch = null;
    if (Math.abs(dx) > 56 && Math.abs(dx) > Math.abs(dy) * 1.4) {
      const dir = dx < 0 ? 1 : -1;
      if (mdMode() === 'week') { mdSlideDir = dir; changeWeek(dir); return; }
      const next = Math.min(6, Math.max(0, mobileDayIdx + dir));
      if (next !== mobileDayIdx) { mdSlideDir = dir; mobileDayIdx = next; renderMobileView(); }
      else mdBump(mdView, dir); /* bout de semaine : petit rebond plutôt qu'un swipe mort */
    }
  }, { passive: true });
  mdView.addEventListener('animationend', () => mdClearAnim(mdView));

  /* Bouton Semaine/Jour (mobile) : bascule entre la vue jour et la liste des 7 jours */
  $('#mdWeekBtn').addEventListener('click', () => {
    state.settings.mobileView = mdMode() === 'week' ? 'day' : 'week';
    if (mdMode() === 'day') {
      /* Retour en vue jour : toujours se replacer sur aujourd'hui,
         même si on consultait une autre semaine en vue Semaine */
      weekStart = startOfWeek(new Date());
      mobileDayIdx = mobileTodayIdx();
    }
    save(); renderPlanning();
  });

  /* Vue établissements : liste + ajout */
  $('#estViewList').addEventListener('click', e => {
    const card = e.target.closest('.est-card');
    if (card) openEstModal(card.dataset.est);
  });
  $('#estViewAddForm').addEventListener('submit', e => {
    e.preventDefault();
    const name = $('#estViewName').value.trim();
    if (!name) return;
    state.establishments.push({
      id: uid(),
      name: name.toUpperCase(),
      color: PALETTE[state.establishments.length % PALETTE.length],
    });
    $('#estViewName').value = '';
    save(); render();
    toast('Établissement ajouté ✓');
  });

  /* Panneau établissements (desktop) */
  $('#estList').addEventListener('click', e => {
    const card = e.target.closest('.est-card');
    if (card) openEstModal(card.dataset.est);
  });
  $('#estAddForm').addEventListener('submit', e => {
    e.preventDefault();
    const name = $('#estAddName').value.trim();
    if (!name) return;
    state.establishments.push({
      id: uid(),
      name: name.toUpperCase(),
      color: PALETTE[state.establishments.length % PALETTE.length],
    });
    $('#estAddName').value = '';
    save(); render();
    toast('Établissement ajouté ✓');
  });

  /* Modal shift */
  $('#shForm').addEventListener('submit', e => {
    e.preventDefault();
    const estId = $('#shEst').value, date = $('#shDate').value;
    const start = $('#shStart').value, end = $('#shEnd').value;
    if (!estId) { toast('Sélectionnez un établissement', 'error'); return; }
    if (!date || !start || !end) { toast('Champs incomplets', 'error'); return; }
    if (start === end) { toast('Le début et la fin sont identiques', 'error'); return; }
    const data = { estId, date, start, end, label: $('#shLabel').value.trim(), note: $('#shNote').value.trim() };
    if (editingShiftId) {
      const i = state.shifts.findIndex(x => x.id === editingShiftId);
      if (i > -1) state.shifts[i] = { ...state.shifts[i], ...data };
      else state.shifts.push({ id: uid(), ...data });
      toast('Shift modifié ✓');
    } else {
      state.shifts.push({ id: uid(), ...data });
      toast('Shift ajouté ✓');
    }
    save();
    $('#shiftModal').close();
    renderPlanning();
  });
  $('#shDelete').addEventListener('click', async () => {
    const s = state.shifts.find(x => x.id === editingShiftId);
    if (!s) return;
    const d = fromISO(s.date);
    const ok = await confirmDialog('Supprimer ce shift ?', `${fmtDur(durMinutes(s.start, s.end))} le ${d.getDate()} ${MONTHS[d.getMonth()]}`, 'Supprimer');
    if (!ok) return;
    state.shifts = state.shifts.filter(x => x.id !== s.id);
    save();
    $('#shiftModal').close();
    renderPlanning();
    toast('Shift supprimé');
  });
  $('#shCancel').addEventListener('click', () => $('#shiftModal').close());
  ['shStart', 'shEnd', 'shDate'].forEach(id => $('#' + id).addEventListener('input', updateShiftMeta));

  /* Modal absence */
  $('#addAbsenceBtn').addEventListener('click', () => openAbsenceModal());
  $('#addAbsenceBtn2').addEventListener('click', () => openAbsenceModal());
  $('#abForm').addEventListener('submit', e => {
    e.preventDefault();
    const type = $('#abType').value, start = $('#abStart').value, end = $('#abEnd').value;
    if (!start || !end || end < start) { toast('Dates invalides', 'error'); return; }
    const data = { type, start, end, note: $('#abNote').value.trim(), status: 'Confirmée' };
    if (editingAbsenceId) {
      const i = state.absences.findIndex(x => x.id === editingAbsenceId);
      if (i > -1) state.absences[i] = { ...state.absences[i], ...data };
      toast('Absence modifiée ✓');
    } else {
      state.absences.push({ id: uid(), ...data });
      toast('Absence enregistrée ✓');
    }
    save();
    $('#absenceModal').close();
    render();
  });
  $('#abDelete').addEventListener('click', async () => {
    const ok = await confirmDialog('Supprimer cette absence ?', '', 'Supprimer');
    if (!ok) return;
    state.absences = state.absences.filter(x => x.id !== editingAbsenceId);
    save();
    $('#absenceModal').close();
    render();
    toast('Absence supprimée');
  });
  $('#abCancel').addEventListener('click', () => $('#absenceModal').close());
  $('#absenceList').addEventListener('click', e => {
    const ed = e.target.closest('[data-edit]');
    if (ed) { openAbsenceModal(ed.dataset.edit); return; }
    const dl = e.target.closest('[data-del]');
    if (dl) {
      confirmDialog('Supprimer cette absence ?', '', 'Supprimer').then(ok => {
        if (!ok) return;
        state.absences = state.absences.filter(x => x.id !== dl.dataset.del);
        save();
        renderAbsences();
        toast('Absence supprimée');
      });
    }
  });

  /* Modal établissement */
  $('#estColors').addEventListener('click', e => {
    const d = e.target.closest('.color-dot');
    if (!d) return;
    estModalColor = d.dataset.color;
    renderEstColors();
  });
  $('#estForm').addEventListener('submit', e => {
    e.preventDefault();
    const est = state.establishments.find(x => x.id === editingEstId);
    if (!est) return;
    est.name = $('#estName').value.trim().toUpperCase() || est.name;
    est.color = estModalColor;
    save();
    $('#estModal').close();
    render();
    toast('Établissement mis à jour ✓');
  });
  $('#estDelete').addEventListener('click', async () => {
    const est = state.establishments.find(x => x.id === editingEstId);
    if (!est) return;
    const n = state.shifts.filter(s => s.estId === est.id).length;
    const ok = await confirmDialog(
      'Supprimer cet établissement ?',
      n ? `${n} shift${n > 1 ? 's' : ''} associé${n > 1 ? 's' : ''} seront également supprimés.` : 'Aucun shift associé.',
      'Supprimer'
    );
    if (!ok) return;
    state.establishments = state.establishments.filter(x => x.id !== est.id);
    state.shifts = state.shifts.filter(s => s.estId !== est.id);
    save();
    $('#estModal').close();
    render();
    toast('Établissement supprimé');
  });
  $('#estCancel').addEventListener('click', () => $('#estModal').close());

  /* Confirmation */
  $('#cfOk').addEventListener('click', () => { settleConfirm(true); $('#confirmModal').close(); });
  $('#cfCancel').addEventListener('click', () => { settleConfirm(false); $('#confirmModal').close(); });
  $('#confirmModal').addEventListener('close', () => settleConfirm(false));

  /* Profil */
  const PROFILE_FIELDS = { pFirst: 'firstName', pLast: 'lastName', pEmail: 'email', pPhone: 'phone', pRate: 'rate', pObjective: 'objective', pRefSalary: 'refSalary' };
  let profileTimer = null;
  Object.entries(PROFILE_FIELDS).forEach(([id, key]) => {
    $('#' + id).addEventListener('input', () => {
      let v = $('#' + id).value;
      /* Accepte la virgule française (12,50 => 12.5) pour les champs numériques */
      if (key === 'rate' || key === 'objective' || key === 'refSalary') v = parseFloat(String(v).replace(',', '.')) || 0;
      state.profile[key] = v;
      renderProfilePreview();
      clearTimeout(profileTimer);
      profileTimer = setTimeout(save, 600);
    });
  });
  const flushProfile = () => { if (profileTimer) { clearTimeout(profileTimer); profileTimer = null; save(); } };
  window.addEventListener('pagehide', flushProfile);
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden') flushProfile(); });
  $('#profileSave').addEventListener('click', () => { clearTimeout(profileTimer); profileTimer = null; save(); toast('Profil enregistré ✓'); });
  $('#resetBtn').addEventListener('click', resetData);
  $('#availChips').addEventListener('click', e => {
    const c = e.target.closest('.chip');
    if (!c) return;
    const i = Number(c.dataset.i);
    state.availability[i] = state.availability[i] !== false ? false : true;
    save(); renderChips();
    toast(state.availability[i] ? 'Disponible le ' + DAYS_LONG[i] : 'Indisponible le ' + DAYS_LONG[i]);
  });

  /* Import : input fichier + glisser-déposer (JSON ou ICS) */
  $('#importFile').addEventListener('change', e => {
    const f = e.target.files[0];
    if (f) importJSON(f);
    e.target.value = '';
  });
  $('#icsSaveUrlBtn').addEventListener('click', () => {
    const u = $('#icsUrl').value.trim();
    if (!/^https?:\/\//i.test(u)) { toast('URL invalide (elle doit commencer par https://…)', 'error'); return; }
    state.settings.skelloUrl = u;
    save();
    renderSyncInfo();
    updateSyncBtn();
    toast('URL Skello enregistrée ✓ — bouton « ⟳ Sync Skello » activé');
  });
  $('#icsSyncNowBtn').addEventListener('click', () => syncSkello());
  $('#syncIcsBtn').addEventListener('click', () => syncSkello());
  $('#icsAllowProxy').addEventListener('change', e => {
    state.settings.allowProxy = e.target.checked;
    save();
    toast(e.target.checked ? 'Relais publics autorisés' : 'Relais publics désactivés');
  });
  $('#icsFileBtn').addEventListener('click', () => $('#icsFile').click());
  $('#icsFile').addEventListener('change', async e => {
    const f = e.target.files[0];
    if (f) await applyICSImport(await f.text());
    e.target.value = '';
  });
  $('#icsParseBtn').addEventListener('click', async () => {
    const txt = $('#icsPaste').value;
    if (!txt.trim()) { toast('Collez le contenu du calendrier', 'error'); return; }
    await applyICSImport(txt);
  });
  $('#icsCancel').addEventListener('click', () => $('#icsModal').close());
  let dragDepth = 0;
  const isFileDrag = e => e.dataTransfer && [...(e.dataTransfer.types || [])].includes('Files');
  window.addEventListener('dragenter', e => {
    if (!isFileDrag(e)) return;
    e.preventDefault();
    dragDepth++;
    $('#dropOverlay').classList.remove('hidden');
  });
  window.addEventListener('dragover', e => e.preventDefault());
  window.addEventListener('dragleave', () => {
    dragDepth--;
    if (dragDepth <= 0) { dragDepth = 0; $('#dropOverlay').classList.add('hidden'); }
  });
  window.addEventListener('drop', async e => {
    e.preventDefault();
    dragDepth = 0;
    $('#dropOverlay').classList.add('hidden');
    const f = e.dataTransfer?.files?.[0];
    if (!f) return;
    if (/\.ics$/i.test(f.name) || f.type === 'text/calendar') await applyICSImport(await f.text());
    else importJSON(f);
  });

  /* Raccourcis clavier */
  document.addEventListener('keydown', e => {
    if (document.querySelector('dialog[open]')) return;
    if (/INPUT|TEXTAREA|SELECT/.test(e.target.tagName)) return;
    if (currentView !== 'planning') return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === 'ArrowLeft') changeWeek(-1);
    else if (e.key === 'ArrowRight') changeWeek(1);
    else if (e.key.toLowerCase() === 'n') { e.preventDefault(); openShiftModal(null, { date: todayISO() }); }
  });

  /* Synchro Skello automatique au démarrage (1 fois toutes les 12 h) */
  if ((state.settings.skelloUrl || '').trim() && Date.now() - (state.settings.lastIcsSync || 0) > 12 * 3600 * 1000) {
    setTimeout(() => syncSkello({ silent: true }), 800);
  }

  render();
}

init();
