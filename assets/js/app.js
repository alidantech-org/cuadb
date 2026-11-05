// Browser-only behavior: popovers, display preferences, drawer, splitter, row inspector,
// icons and keyboard shortcuts. Data, paging, sorting and every mutation are server-side
// (HTMX fragments). No inline script or style is used, so the CSP stays strict.
/** @typedef {any} Any */

const $ = (/** @type {string} */ selector) => document.querySelector(selector);
const $$ = (/** @type {string} */ selector) => Array.from(document.querySelectorAll(selector));
/** @type {Any} */
const win = window;
/** @type {Any} */
const prefs = win.cuadbPrefs;
const S = prefs.S;
/** Set once the rich-cell helpers below are defined (they are `const`s declared later). */
var gridReady = false;
const TP = prefs.TP;
let restoreRequestPath = '';
/** @type {number | null} */
let pendingGridScrollLeft = null;

/** Ask HTMX for a fragment. */
function fetchInto(/** @type {string} */ url, /** @type {string} */ target, swap = 'outerHTML') {
  win.htmx?.ajax('GET', url, { target, swap });
}

function setHash(/** @type {string} */ value) {
  const url = new URL(location.href);
  url.hash = value;
  history.replaceState(history.state, '', url);
}
function pushHash(/** @type {string} */ value) {
  const url = new URL(location.href);
  url.hash = value;
  history.pushState({}, '', url);
}
function encodeHash(/** @type {Record<string, unknown>} */ value) {
  return encodeURIComponent(JSON.stringify(value));
}
function decodeHash(/** @type {string} */ prefix) {
  if (!location.hash.startsWith(`#${prefix}=`)) return null;
  try {
    return JSON.parse(decodeURIComponent(location.hash.slice(prefix.length + 2)));
  } catch {
    return null;
  }
}

function routeForRequest(/** @type {string} */ path) {
  const url = new URL(path, location.origin);
  const p = url.searchParams;
  const profile = p.get('profile');
  const extra = new URLSearchParams(p);
  if (profile) extra.set('profile', profile);
  let route = url.pathname;
  if (route === '/catalog/data' || route === '/catalog/structure' || route === '/catalog/indexes') {
    const schema = p.get('schema') ?? '';
    const relation = p.get('relation') ?? '';
    for (const key of ['schema', 'relation']) extra.delete(key);
    const view = route.endsWith('/structure')
      ? '/structure'
      : route.endsWith('/indexes')
        ? '/indexes'
        : '';
    route = `/table/${encodeURIComponent(schema)}/${encodeURIComponent(relation)}${view}`;
  } else if (route === '/catalog/overview') route = '/overview';
  else if (route === '/catalog/object')
    route = `/object/${encodeURIComponent(p.get('schema') ?? '')}/${encodeURIComponent(p.get('name') ?? '')}`;
  else if (route === '/admin') route = '/workspace/admin';
  else if (route === '/admin/users') route = '/workspace/admin/users';
  else if (route === '/admin/connections') route = '/workspace/admin/connections';
  else if (route === '/admin/grants') route = '/workspace/admin/permissions';
  else if (route === '/admin/audit') route = '/workspace/admin/audit';
  else if (route === '/admin/security') route = '/workspace/admin/security';
  else if (route === '/admin/connections/form') {
    route = '/workspace/admin/connections';
    return `${route}#connection=${encodeURIComponent(p.get('id') ?? 'add')}`;
  } else if (route === '/sql') route = '/workspace/sql';
  else if (
    !route.startsWith('/table/') &&
    !route.startsWith('/workspace/') &&
    route !== '/overview' &&
    !route.startsWith('/object/')
  )
    return null;
  const query = extra.toString();
  return `${route}${query ? `?${query}` : ''}`;
}

function requestForLocation() {
  const parts = location.pathname.split('/').filter(Boolean).map(decodeURIComponent);
  const params = new URLSearchParams(location.search);
  params.delete('open');
  params.delete('search');
  params.delete('collapse');
  params.delete('database');
  const profile = params.get('profile');
  if (parts[0] === 'table' && parts.length >= 3) {
    const [_, schema, relation, view] = parts;
    params.set('schema', schema ?? '');
    params.set('relation', relation ?? '');
    if (profile) params.set('profile', profile);
    return `/catalog/${view === 'structure' ? 'structure' : view === 'indexes' ? 'indexes' : 'data'}?${params}`;
  }
  if (parts[0] === 'overview')
    return `/catalog/overview${profile ? `?profile=${encodeURIComponent(profile)}` : ''}`;
  if (parts[0] === 'object' && parts.length >= 3) {
    params.set('schema', parts[1] ?? '');
    params.set('name', parts[2] ?? '');
    return `/catalog/object?${params}`;
  }
  if (parts[0] === 'workspace' && parts[1] === 'admin' && parts[2] === 'connections')
    return '/admin/connections';
  if (parts[0] === 'workspace' && parts[1] === 'admin') return '/admin';
  if (parts[0] === 'workspace' && parts[1] === 'sql')
    return `/sql${profile ? `?profile=${encodeURIComponent(profile)}` : ''}`;
  if (location.pathname !== '/') return null;
  return null;
}

/* ------------------------------------------------------------ toast + clipboard */
/** @type {ReturnType<typeof setTimeout> | undefined} */
let toastTimer;
function toast(/** @type {string} */ message) {
  const el = document.getElementById('toast');
  if (!el) return;
  el.textContent = message;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 1800);
}
win.cuadbToast = toast;
function noticeText(/** @type {string} */ html) {
  return new DOMParser().parseFromString(html, 'text/html').body.textContent?.trim() ?? '';
}

async function copyText(/** @type {string} */ text, /** @type {string} */ done) {
  try {
    await navigator.clipboard.writeText(text);
    toast(done);
  } catch {
    toast('Clipboard access is unavailable');
  }
}

/* ------------------------------------------------------------ CSRF for HTMX */
/** Set a request parameter whether htmx hands us an object or a FormData-like proxy. */
function setParam(
  /** @type {Any} */ params,
  /** @type {string} */ name,
  /** @type {string} */ value,
) {
  if (typeof params.set === 'function') params.set(name, value);
  else params[name] = value;
}
document.addEventListener('htmx:configRequest', (event) => {
  const detail = /** @type {CustomEvent} */ (event).detail;
  const token = document.querySelector('meta[name="csrf-token"]')?.getAttribute('content');
  if (token && detail.verb !== 'get') detail.headers['x-csrf-token'] = token;
  const path = String(detail.path ?? '');
  if (
    detail.verb === 'get' &&
    path.startsWith('/catalog/data') &&
    detail.elt instanceof Element &&
    detail.elt.matches('#tree a.row')
  ) {
    detail.path = recallTable(path);
    if (Number(S.psize) !== 100 && !/[?&]size=/.test(detail.path)) {
      detail.path += `${detail.path.includes('?') ? '&' : '?'}size=${encodeURIComponent(String(S.psize))}`;
    }
  }
  // The connection list must mark the connection that is actually active, not the default.
  if (
    path.startsWith('/connection/profiles') &&
    win.cuadbProfile &&
    !(typeof detail.parameters.get === 'function'
      ? detail.parameters.get('profile')
      : detail.parameters.profile)
  ) {
    setParam(detail.parameters, 'profile', String(win.cuadbProfile).split('~')[0] ?? '');
  }
  if (path.startsWith('/catalog/relations') && S.mode === 'advanced') {
    setParam(detail.parameters, 'group', '1');
  }
  if (/^\/(sql\/(execute|cancel)|tx\/begin)/.test(path) && win.cuadbProfile) {
    setParam(detail.parameters, 'profile', win.cuadbProfile);
  }
  if (path.startsWith('/sql/execute')) {
    win.cuadbSid = crypto.randomUUID().replaceAll('-', '');
    setParam(detail.parameters, 'statement_id', win.cuadbSid);
    if (win.cuadbTx) setParam(detail.parameters, 'tx', win.cuadbTx);
  }
  if (path.startsWith('/sql/cancel') && win.cuadbSid)
    setParam(detail.parameters, 'statement_id', win.cuadbSid);
});

// Server error fragments (4xx/5xx with our error markup) are shown in place.
document.addEventListener('htmx:beforeSwap', (event) => {
  const detail = /** @type {CustomEvent} */ (event).detail;
  const text = detail.xhr.responseText;
  if (detail.target?.id === 'catalog-panel' && detail.xhr.status >= 400) {
    const message = new DOMParser()
      .parseFromString(detail.xhr.responseText, 'text/html')
      .querySelector('.cuadb-error p')?.textContent;
    toast(message || 'That request failed. The table was left as it was.');
    detail.shouldSwap = false;
    detail.isError = false;
    return;
  }
  if (detail.target?.id === 'tree' && detail.xhr.status === 403) {
    // A member with no grants at all: say so in the sidebar instead of an error card.
    detail.serverResponse =
      '<div class="note" role="status">No databases assigned yet. Ask an administrator for access.</div>';
    detail.shouldSwap = true;
    detail.isError = false;
    return;
  }
  if (
    detail.xhr.status >= 400 &&
    (text.includes('class="cuadb-error"') || text.includes('class="notice'))
  ) {
    detail.shouldSwap = true;
    detail.isError = false;
    detail.swapOverride = 'innerHTML';
  }
});

/* ------------------------------------------------------------ icons (decorative) */
/** @type {Record<string, [number, string]>} */
const IC = {
  table: [200, '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18M9 4v16"/>'],
  user: [215, '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 4-6 8-6s8 2 8 6"/>'],
  cart: [
    28,
    '<circle cx="9" cy="20" r="1.5"/><circle cx="18" cy="20" r="1.5"/><path d="M2 3h3l3 12h10l3-8H6"/>',
  ],
  box: [35, '<path d="M3 7l9-4 9 4v10l-9 4-9-4zM3 7l9 4 9-4M12 11v10"/>'],
  bolt: [48, '<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>'],
  key: [265, '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M16 7l3 3"/>'],
  card: [150, '<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20"/>'],
  mail: [340, '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>'],
  file: [190, '<path d="M6 2h8l5 5v15H6zM14 2v5h5"/>'],
  sliders: [
    160,
    '<path d="M4 6h8M16 6h4M4 12h2M10 12h10M4 18h10M18 18h2"/><circle cx="14" cy="6" r="2"/><circle cx="8" cy="12" r="2"/><circle cx="16" cy="18" r="2"/>',
  ],
  tag: [310, '<path d="M3 12V4h8l10 10-8 8z"/><circle cx="7.5" cy="8.5" r="1"/>'],
  chat: [175, '<path d="M4 4h16v12H9l-5 4z"/>'],
  shield: [0, '<path d="M12 3l8 3v6c0 5-4 8-8 9-4-1-8-4-8-9V6z"/>'],
  pin: [
    12,
    '<path d="M12 22s7-6 7-12a7 7 0 10-14 0c0 6 7 12 7 12z"/><circle cx="12" cy="10" r="2.5"/>',
  ],
  view: [
    255,
    '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  ],
  overview: [
    180,
    '<rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/>',
  ],
  folder: [45, '<path d="M3 6h7l2 2h9v11H3z"/>'],
  fn: [285, '<path d="M15 4c-3 0-3 2-4 6l-2 10c-.5 2-1.5 3-3 3M7 10h8"/>'],
  proc: [
    95,
    '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2"/>',
  ],
  seq: [20, '<path d="M5 9h14M5 15h14M10 4L8 20M16 4l-2 16"/>'],
  enum: [330, '<path d="M4 6h16M4 12h10M4 18h14"/>'],
  folderopen: [45, '<path d="M3 19V5h7l2 2h8v3"/><path d="M3 19l3-9h17l-3 9z"/>'],
};
/** @type {Array<[RegExp, string]>} */
const NAME_ICONS = [
  [/user|account|member|customer|profile/, 'user'],
  [/order|purchase|cart|checkout/, 'cart'],
  [/product|item|sku|inventory|catalog/, 'box'],
  [/event|log|audit|activity|track/, 'bolt'],
  [/session|auth|token|login|credential/, 'key'],
  [/pay|invoice|billing|charge|price/, 'card'],
  [/mail|message|notif/, 'mail'],
  [/file|doc|media|upload|asset|image/, 'file'],
  [/setting|config|pref/, 'sliders'],
  [/tag|categor|label/, 'tag'],
  [/comment|review|chat/, 'chat'],
  [/role|permission|grant/, 'shield'],
  [/address|location|geo|place/, 'pin'],
];
const SVG_NS = 'http://www.w3.org/2000/svg';

function iconName(/** @type {HTMLElement} */ slot) {
  const kind = slot.dataset.ic;
  if (kind === 'folder' || kind === 'overview') return kind;
  if (kind === 'object') {
    return (
      { function: 'fn', procedure: 'proc', sequence: 'seq', type: 'enum', trigger: 'bolt' }[
        slot.dataset.kind ?? ''
      ] ?? 'fn'
    );
  }
  if (slot.dataset.kind && /view/i.test(slot.dataset.kind)) return 'view';
  const name = (slot.dataset.name ?? '').toLowerCase();
  return NAME_ICONS.find((pair) => pair[0].test(name))?.[1] ?? 'table';
}

function decorateIcons(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('.ic-slot:not([data-done])').forEach((node) => {
    const slot = /** @type {HTMLElement} */ (node);
    slot.dataset.done = '1';
    if (S.icons === 'off') return;
    const [hue, body] = IC[iconName(slot)] ?? /** @type {[number, string]} */ (IC.table);
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('class', 'ic');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', '14');
    svg.setAttribute('height', '14');
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '1.8');
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');
    svg.setAttribute('aria-hidden', 'true');
    svg.style.setProperty('--h', String(hue));
    svg.innerHTML = body;
    slot.replaceWith(svg);
  });
}

function setDrawer(/** @type {boolean} */ open) {
  document.getElementById('side')?.classList.toggle('show', open);
  document.documentElement.classList.toggle('drawer', open);
  document.getElementById('menu')?.setAttribute('aria-expanded', String(open));
}

/* ------------------------------------------------------------ modal */
/** @type {Element | null} */
let modalOpener = null;
function openModal() {
  const modal = document.getElementById('modal');
  if (!modal) return;
  if (!modal.classList.contains('show')) modalOpener = document.activeElement;
  modal.classList.add('show');
  /** @type {HTMLElement | null} */ (
    modal.querySelector('input:not([type=hidden]),select,textarea')
  )?.focus();
}
function closeModal() {
  const modal = document.getElementById('modal');
  if (!modal) return;
  modal.classList.remove('show');
  modal.replaceChildren();
  if (location.hash.startsWith('#connection=')) setHash('');
  if (modalOpener instanceof HTMLElement && document.contains(modalOpener)) modalOpener.focus();
  modalOpener = null;
}

/** Bar widths come from data attributes so no inline style is needed. */
function decorateBars(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('.bar-fill[data-pct]').forEach((node) => {
    const el = /** @type {HTMLElement} */ (node);
    el.style.setProperty('--p', el.dataset.pct ?? '0');
  });
}

/** Advanced mode: load each expanded schema's functions, sequences, types and triggers. */
function loadObjects() {
  if (S.mode !== 'advanced') return;
  document.querySelectorAll('#tree .objs[data-objs]:not([data-loaded])').forEach((node) => {
    const el = /** @type {HTMLElement} */ (node);
    el.dataset.loaded = '1';
    win.htmx?.ajax('GET', el.dataset.objs ?? '', { target: el, swap: 'innerHTML' });
  });
}

/** The footer's database button shows the connected database and what the user may do there. */
/** @type {{db: string, access: string, engine: string, state: string, label: string}} */
let currentDb = { db: '', access: 'read', engine: '', state: '', label: '' };
function syncDatabase(/** @type {HTMLElement} */ option) {
  currentDb = {
    db: option.dataset.db ?? '',
    access: option.dataset.access ?? 'read',
    engine: option.dataset.engine ?? '',
    state: option.dataset.state ?? '',
    label: option.dataset.label ?? '',
  };
  const name = document.getElementById('dname');
  if (name) name.textContent = currentDb.db || 'database';
}
function renderDatabasePop() {
  const pop = document.getElementById('dpop');
  if (!pop) return;
  pop.replaceChildren();
  const list = document.createElement('div');
  list.id = 'dblist';
  list.className = 'dblist';
  const loading = document.createElement('div');
  loading.className = 'note';
  loading.textContent = 'Loading…';
  list.append(loading);
  pop.append(list);
  const profile = String(win.cuadbProfile || '');
  const query = profile ? `?profile=${encodeURIComponent(profile)}` : '';
  fetch(`/connection/databases${query}`)
    .then(async (response) => {
      if (!response.ok) throw new Error('unsupported');
      list.innerHTML = await response.text();
    })
    .catch(() => {
      list.replaceChildren();
      const note = document.createElement('div');
      note.className = 'note';
      note.textContent = `${currentDb.db || 'This database'} is the only one this connection can open.`;
      list.append(note);
    });
}

/** Switch to another database of the same server through a server-side derived connection. */
async function useDatabase(/** @type {HTMLElement} */ option) {
  const database = option.dataset.useDb ?? '';
  const base = option.dataset.base ?? '';
  const token = document.querySelector('meta[name="csrf-token"]')?.getAttribute('content') ?? '';
  const body = new URLSearchParams({ database });
  if (win.cuadbProfile) body.set('profile', String(win.cuadbProfile).split('~')[0] ?? '');
  else if (base) body.set('profile', base);
  try {
    const response = await fetch('/connection/use', {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': token },
      body,
    });
    if (!response.ok) throw new Error('refused');
    const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
    const derived = doc.getElementById('use-result')?.getAttribute('data-derived');
    if (!derived) throw new Error('none');
    win.cuadbProfile = derived;
    currentDb = { ...currentDb, db: database };
    const url = new URL('/', location.origin);
    url.searchParams.set('profile', derived);
    url.searchParams.set('database', database);
    history.pushState({}, '', url);
    const name = document.getElementById('dname');
    if (name) name.textContent = database;
    closePops();
    closeInspector();
    setMainHint(`Connected to ${database}. Choose a table from the sidebar.`);
    fetchInto(`/catalog/schemas?profile=${encodeURIComponent(derived)}`, '#tree', 'innerHTML');
  } catch {
    toast('That database could not be opened.');
  }
}

function setMainHint(/** @type {string} */ message) {
  const main = document.getElementById('mainbody');
  if (!main) return;
  main.replaceChildren();
  const hint = document.createElement('section');
  hint.className = 'es';
  const h = document.createElement('h2');
  h.textContent = 'Pick a table to explore';
  const p = document.createElement('p');
  p.textContent = message;
  hint.append(h, p);
  main.append(hint);
}

/* ------------------------------------------------------------ popovers */
const POPS = ['cpop', 'dpop', 'opop', 'colpop', 'exppop', 'fpop'];
function closePops(/** @type {string} */ except = '') {
  POPS.forEach((id) => {
    if (id === except) return;
    const el = document.getElementById(id);
    if (!el) return;
    el.classList.remove('show');
    document
      .querySelectorAll(`[aria-controls="${id}"]`)
      .forEach((btn) => btn.setAttribute('aria-expanded', 'false'));
  });
  if (!except && location.hash.startsWith('#popover/')) setHash('');
}
function togglePop(/** @type {string} */ id) {
  const el = document.getElementById(id);
  if (!el) return;
  closePops(id);
  const open = el.classList.toggle('show');
  document
    .querySelectorAll(`[aria-controls="${id}"]`)
    .forEach((btn) => btn.setAttribute('aria-expanded', String(open)));
  if (open && id === 'opop') syncSettings();
  if (open)
    pushHash(
      `popover/${{ cpop: 'connections', dpop: 'database', opop: 'sidebar', colpop: 'columns', exppop: 'export' }[id] ?? id}`,
    );
  else if (location.hash.startsWith('#popover/')) setHash('');
}

/* ------------------------------------------------------------ settings dialog */
/** @type {Element | null} */
let settingsOpener = null;
function openSettings(/** @type {string} */ pane = '') {
  const dialog = document.getElementById('spop');
  if (!dialog) return;
  closePops();
  settingsOpener = document.activeElement;
  dialog.classList.add('show');
  document.getElementById('sbtn')?.setAttribute('aria-expanded', 'true');
  showSettingsPane(pane || String(win.cuadbPane || 'general'));
  pushHash(`settings/${encodeURIComponent(String(win.cuadbPane || 'general'))}`);
  syncSettings();
  /** @type {HTMLElement | null} */ (dialog.querySelector('.snav.on'))?.focus();
}
function closeSettings() {
  const dialog = document.getElementById('spop');
  if (!dialog?.classList.contains('show')) return;
  dialog.classList.remove('show');
  if (location.hash.startsWith('#settings/')) setHash('');
  document.getElementById('sbtn')?.setAttribute('aria-expanded', 'false');
  if (settingsOpener instanceof HTMLElement && document.contains(settingsOpener))
    settingsOpener.focus();
}
function showSettingsPane(/** @type {string} */ pane) {
  win.cuadbPane = pane;
  document.querySelectorAll('#spop [data-sp]').forEach((node) => {
    const on = node.getAttribute('data-sp') === pane;
    node.classList.toggle('on', on);
    node.setAttribute('aria-selected', String(on));
  });
  document.querySelectorAll('#spop [data-sp-pane]').forEach((node) => {
    /** @type {HTMLElement} */ (node).hidden = node.getAttribute('data-sp-pane') !== pane;
  });
}

/* ------------------------------------------------------------ settings */
function syncSettings() {
  for (const id of ['spop', 'opop']) syncSettingsIn(document.getElementById(id));
}
function syncSettingsIn(/** @type {HTMLElement | null} */ pop) {
  if (!pop) return;
  pop.querySelectorAll('select[data-ss]').forEach((node) => {
    const select = /** @type {HTMLSelectElement} */ (node);
    select.value = String(S[select.dataset.ss ?? ''] ?? '');
  });
  pop.querySelectorAll('[data-st]').forEach((node) => {
    const btn = /** @type {HTMLElement} */ (node);
    btn.classList.toggle('on', String(S[btn.dataset.st ?? '']) === btn.dataset.val);
  });
  pop.querySelectorAll('[data-tg]').forEach((node) => {
    const input = /** @type {HTMLInputElement} */ (node);
    input.checked = !!S[input.dataset.tg ?? ''];
  });
  pop.querySelectorAll('[data-ac]').forEach((node) => {
    const btn = /** @type {HTMLElement} */ (node);
    btn.classList.toggle('on', S.accent === (btn.dataset.ac ?? ''));
  });
  const color = /** @type {HTMLInputElement | null} */ (pop.querySelector('[data-cc]'));
  if (color) color.value = S.accent || '#0e7c86';
  const reset = /** @type {HTMLButtonElement | null} */ (document.getElementById('rst'));
  if (reset) reset.disabled = !currentTable();
}
function currentTable() {
  return document.getElementById('catalog-panel')?.dataset.table ?? '';
}
let lastMode = S.mode;
function applyPrefs() {
  prefs.apply();
  formatNumbers(document);
  formatDates(document);
  decorateGrid(document);
  applyTreeView();
  loadSchemaSummaries();
  $$('#tree .sub').forEach((sub) => loadRelationStats(sub));
  if (S.mode !== lastMode) {
    lastMode = S.mode;
    // Grouping and metadata objects depend on the mode: rebuild the tree.
    if (document.getElementById('tree')?.querySelector('.sch')) {
      const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
      fetchInto(`/catalog/schemas${profile}`, '#tree', 'innerHTML');
    }
  }
  prefs.save();
  syncSettings();
  const selb = document.getElementById('selb');
  if (selb) selb.hidden = S.mode === 'explore';
  document.querySelectorAll('.row .c').forEach((node) => {
    /* the count column is a server value; the toggle only shows or hides it */
    node.classList.toggle('hidden-count', S.side === 'size');
  });
}

/* ------------------------------------------------------------ row sheet */
/** The row details live in a slide-over sheet; any click outside it closes it. */
function sheetOpen() {
  return document.getElementById('insp')?.classList.contains('show') ?? false;
}
function openInspector(/** @type {HTMLTableRowElement} */ row) {
  const panel = document.getElementById('insp');
  const table = row.closest('table');
  if (!panel || !table) return;
  $$('tr.sel').forEach((el) => el.classList.remove('sel'));
  row.classList.add('sel');
  const heads = /** @type {HTMLElement[]} */ (
    Array.from(table.querySelectorAll('thead th[data-col]'))
  );
  const cells = Array.from(row.querySelectorAll('td:not(.rn)'));
  panel.replaceChildren();
  const title = document.createElement('h3');
  const label = sheetTitle(
    `Row ${row.dataset.row ?? ''}`,
    document.getElementById('catalog-panel')?.dataset.table ?? '',
    'table',
  );
  const close = document.createElement('button');
  close.type = 'button';
  close.className = 'btn ico';
  close.setAttribute('data-close-insp', '');
  close.setAttribute('aria-label', 'Close row details');
  close.textContent = '×';
  const actions = document.createElement('span');
  actions.className = 'ia';
  if (document.getElementById('catalog-panel')?.dataset.edit === '1') {
    for (const [attr, text, danger] of [
      ['data-row-edit', 'Edit', false],
      ['data-row-delete', 'Delete', true],
    ]) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = danger ? 'btn dg' : 'btn';
      button.setAttribute(String(attr), '');
      button.textContent = String(text);
      actions.append(button);
    }
  }
  actions.append(close);
  title.append(label, actions);
  panel.append(title);
  win.cuadbRow = row;
  heads.forEach((head, index) => {
    const cell = /** @type {HTMLElement | undefined} */ (cells[index]);
    const name = head.getAttribute('data-col') ?? '';
    const native = head.getAttribute('data-type') ?? '';
    const kind = head.dataset.kind ?? 'text';
    const isNull = Boolean(cell?.querySelector('[data-null]'));
    const detail = cell?.dataset.detail ?? '';
    const fkLink = cell?.querySelector('a.fk');
    const tools = [];
    const copy = document.createElement('button');
    copy.type = 'button';
    copy.className = 'cpy';
    copy.setAttribute('data-copy-value', String(index));
    copy.setAttribute('aria-label', `Copy ${name}`);
    copy.textContent = 'Copy';
    tools.push(copy);
    if (cell?.querySelector('.truncation') && gridColumns().some((c) => c.pk)) {
      const full = document.createElement('button');
      full.type = 'button';
      full.className = 'cpy';
      full.setAttribute('data-load-full', String(index));
      full.textContent = 'Load full';
      tools.push(full);
    }
    const value = sheetValue(
      kind,
      detail,
      isNull,
      native,
      fkLink instanceof HTMLAnchorElement ? fkLink : null,
    );
    panel.append(
      sheetField({
        name,
        type: head.querySelector('.tyb')?.textContent || simpleType(native, kind === 'enum'),
        native,
        kind,
        pk: head.dataset.pk === '1',
        fk: head.dataset.fk === '1',
        value,
        tools,
      }),
    );
  });
  panel.classList.add('show');
  const key = row.dataset.rowKey;
  const grid = document.getElementById('catalog-panel');
  if (key && grid)
    pushHash(
      `row=${encodeHash({ schema: grid.dataset.schema, relation: grid.dataset.relation, key: JSON.parse(key), profile: grid.dataset.prof ?? '' })}`,
    );
  const backdrop = document.getElementById('sheet-bk');
  if (backdrop) backdrop.hidden = false;
  panel.scrollTop = 0;
}
function closeInspector(preserveHash = false) {
  win.cuadbEditing = false;
  document.getElementById('insp')?.classList.remove('show');
  if (!preserveHash && (location.hash.startsWith('#row=') || location.hash.startsWith('#fk=')))
    setHash('');
  const backdrop = document.getElementById('sheet-bk');
  if (backdrop) backdrop.hidden = true;
  $$('tr.sel').forEach((el) => el.classList.remove('sel'));
}

async function openForeignRow(/** @type {Any} */ item, updateHash = true) {
  if (
    !item ||
    typeof item.schema !== 'string' ||
    typeof item.relation !== 'string' ||
    typeof item.key !== 'string' ||
    typeof item.value !== 'string'
  )
    return;
  if (updateHash) pushHash(`fk=${encodeHash(item)}`);
  const params = new URLSearchParams({
    schema: item.schema,
    relation: item.relation,
    key: item.key,
    value: item.value,
  });
  if (item.profile) params.set('profile', item.profile);
  try {
    const response = await fetch(`/catalog/foreign?${params}`, {
      headers: { accept: 'application/json' },
    });
    if (!response.ok) throw new Error('unavailable');
    const data = await response.json();
    const panel = document.getElementById('insp');
    if (!panel) return;
    panel.replaceChildren();
    const title = document.createElement('h3');
    const heading = sheetTitle(`${data.schema}.${data.relation}`, 'Referenced row', 'link');
    const close = document.createElement('button');
    close.type = 'button';
    close.className = 'btn ico';
    close.dataset.closeInsp = '';
    close.setAttribute('aria-label', 'Close row details');
    close.textContent = '×';
    const actions = document.createElement('span');
    actions.className = 'ia';
    actions.append(close);
    title.append(heading, actions);
    panel.append(title);
    for (const field of data.values) {
      const native = String(field.type ?? '');
      const numeric = /int|numeric|decimal|float|real|double|serial|money/i.test(native);
      const kind = columnKind(String(field.name), native.toLowerCase(), false, false, numeric);
      const text = field.value === null ? '' : String(field.value) + (field.truncated ? '…' : '');
      const tools = [];
      if (field.value !== null) {
        const copy = document.createElement('button');
        copy.type = 'button';
        copy.className = 'cpy';
        copy.textContent = 'Copy';
        copy.addEventListener('click', () => void copyText(String(field.value), 'Value copied'));
        tools.push(copy);
      }
      panel.append(
        sheetField({
          name: String(field.name),
          type: simpleType(native, false),
          native,
          kind,
          pk: false,
          fk: false,
          value: sheetValue(kind, text, field.value === null, native, null),
          tools,
        }),
      );
    }
    if (data.truncated) {
      const note = document.createElement('div');
      note.className = 'note';
      note.textContent = 'Some fields were omitted to keep the row preview bounded.';
      panel.append(note);
    }
    panel.classList.add('show');
    const backdrop = document.getElementById('sheet-bk');
    if (backdrop) backdrop.hidden = false;
    panel.scrollTop = 0;
    win.cuadbRow = null;
  } catch {
    toast('The referenced row could not be opened.');
  }
}

/** Fetch the whole text of a truncated cell (by primary key) and show it in the sheet. */
async function loadFullValue(/** @type {HTMLButtonElement} */ button) {
  const grid = document.getElementById('catalog-panel');
  const row = /** @type {HTMLTableRowElement | undefined} */ (win.cuadbRow);
  const heads = Array.from(
    document.querySelectorAll('#catalog-panel table.grid thead th[data-col]'),
  );
  const head = /** @type {HTMLElement | undefined} */ (heads[Number(button.dataset.loadFull)]);
  if (!grid || !row || !head) return;
  const params = new URLSearchParams({
    schema: grid.dataset.schema ?? '',
    relation: grid.dataset.relation ?? '',
    column: head.dataset.col ?? '',
  });
  if (grid.dataset.prof) params.set('profile', grid.dataset.prof);
  const held = rowValues(row);
  for (const col of gridColumns().filter((c) => c.pk)) {
    const value = held[col.name]?.value;
    if (value === null || value === undefined) return toast('This row has no usable key.');
    params.set(`key.${col.name}`, value);
  }
  button.disabled = true;
  button.textContent = 'Loading…';
  try {
    const response = await fetch(`/catalog/cell?${params.toString()}`);
    if (!response.ok) throw new Error('failed');
    const pre = button.closest('.kv')?.querySelector('pre');
    if (pre) pre.textContent = await response.text();
    button.remove();
  } catch {
    button.disabled = false;
    button.textContent = 'Load full';
    toast('The full value is not available for this column.');
  }
}

/* ------------------------------------------------------------ row selection */
/** The number column doubles as a checkbox: the header's # becomes check-all. */
function gridRows() {
  return /** @type {HTMLTableRowElement[]} */ (
    Array.from(document.querySelectorAll('#catalog-panel table.grid tbody tr[data-row]'))
  );
}
function updateSelection() {
  const table = document.querySelector('#catalog-panel table.grid');
  const rows = gridRows();
  const checked = rows.filter((tr) => tr.classList.contains('ck'));
  table?.classList.toggle('hs', checked.length > 0);
  const all = /** @type {HTMLInputElement | null} */ (document.querySelector('.rck-all'));
  if (all) {
    all.checked = rows.length > 0 && checked.length === rows.length;
    all.indeterminate = checked.length > 0 && checked.length < rows.length;
  }
  const info = document.getElementById('selinfo');
  if (!info) return;
  info.replaceChildren();
  info.hidden = checked.length === 0;
  if (!checked.length) return;
  const text = document.createElement('span');
  text.textContent = `${checked.length} selected`;
  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'btn';
  copy.setAttribute('data-copy-selected', '');
  copy.textContent = 'Copy CSV';
  const clear = document.createElement('button');
  clear.type = 'button';
  clear.className = 'btn';
  clear.setAttribute('data-clear-selected', '');
  clear.textContent = 'Clear';
  info.append(text, copy, clear);
}
function setRowChecked(/** @type {HTMLTableRowElement} */ tr, /** @type {boolean} */ on) {
  tr.classList.toggle('ck', on);
  const box = /** @type {HTMLInputElement | null} */ (tr.querySelector('.rck'));
  if (box) box.checked = on;
}
function selectedCsv() {
  const quote = (/** @type {string} */ text) =>
    /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
  const table = document.querySelector('#catalog-panel table.grid');
  if (!table) return { csv: '', count: 0 };
  const heads = Array.from(table.querySelectorAll('thead th[data-col]')).map(
    (th) => th.getAttribute('data-col') ?? '',
  );
  const lines = [heads.map(quote).join(',')];
  gridRows()
    .filter((tr) => tr.classList.contains('ck'))
    .forEach((tr) => {
      lines.push(
        Array.from(tr.querySelectorAll('td:not(.rn)'))
          .map((td) =>
            td.querySelector('[data-null]')
              ? ''
              : quote(/** @type {HTMLElement} */ (td).dataset.detail ?? ''),
          )
          .join(','),
      );
    });
  return { csv: lines.join('\n'), count: lines.length - 1 };
}

/* ------------------------------------------------------------ row editor */
/** The grid's columns as the editor needs them. */
function gridColumns() {
  return Array.from(document.querySelectorAll('#catalog-panel table.grid thead th[data-col]')).map(
    (node) => {
      const th = /** @type {HTMLElement} */ (node);
      return {
        name: th.dataset.col ?? '',
        type: (th.dataset.type ?? '').toLowerCase(),
        pk: th.dataset.pk === '1',
        nullable: th.dataset.nullable === '1',
        required: th.dataset.required === '1',
        editable: th.dataset.editable === '1',
      };
    },
  );
}

/** Values of a grid row, keyed by column. `null` marks SQL NULL. */
function rowValues(/** @type {HTMLTableRowElement} */ row) {
  /** @type {Record<string, {value: string | null, big: boolean}>} */
  const out = {};
  const cols = gridColumns();
  Array.from(row.querySelectorAll('td:not(.rn)')).forEach((node, index) => {
    const cell = /** @type {HTMLElement} */ (node);
    const col = cols[index];
    if (!col) return;
    out[col.name] = {
      value: cell.querySelector('[data-null]') ? null : (cell.dataset.detail ?? ''),
      big: !!cell.querySelector('.truncation'),
    };
  });
  return out;
}

/** Show the editor in the sheet: the form comes from the server, values are filled here. */
function openEditor(/** @type {HTMLTableRowElement | null} */ row) {
  const panel = document.getElementById('insp');
  const grid = document.getElementById('catalog-panel');
  if (!panel || !grid) return;
  const params = new URLSearchParams({
    schema: grid.dataset.schema ?? '',
    relation: grid.dataset.relation ?? '',
    mode: row ? 'update' : 'insert',
  });
  if (grid.dataset.prof) params.set('profile', grid.dataset.prof);
  win.cuadbRow = row ?? win.cuadbRow;
  const show = () => {
    panel.classList.add('show');
    const backdrop = document.getElementById('sheet-bk');
    if (backdrop) backdrop.hidden = false;
    win.cuadbEditing = true;
  };
  show();
  if (!row && location.hash !== '#row=new') pushHash('row=new');
  const request = win.htmx?.ajax('GET', `/data/editor?${params.toString()}`, {
    target: '#insp',
    swap: 'innerHTML',
  });
  Promise.resolve(request).then(() => {
    const form = document.getElementById('row-form');
    if (!(form instanceof HTMLFormElement)) return;
    if (row) {
      const values = rowValues(row);
      for (const [name, held] of Object.entries(values)) {
        const control =
          /** @type {HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement | null} */ (
            form.querySelector(`[name="${CSS.escape(name)}"]`)
          );
        if (!control) continue;
        const nullBox = /** @type {HTMLInputElement | null} */ (
          form.querySelector(`[data-null-for="${CSS.escape(name)}"]`)
        );
        if (held.value === null) {
          if (control.dataset.ro) control.value = 'NULL';
          if (nullBox) {
            nullBox.checked = true;
            control.disabled = true;
          }
        } else if (control instanceof HTMLSelectElement) {
          control.value =
            ['true', 't', '1'].includes(held.value.toLowerCase()) &&
            control.querySelector('option[value="true"]')
              ? 'true'
              : held.value;
        } else if (held.big) {
          control.value = '';
          control.readOnly = true;
          control.dataset.ro = '1';
          control.placeholder = 'too large to edit here';
        } else {
          control.value = held.value;
        }
      }
    }
    /** @type {HTMLElement | null} */ (
      form.querySelector('input:not([readonly]):not(:disabled),select,textarea')
    )?.focus();
  });
}

function editorMessage(/** @type {string} */ text) {
  const el = document.getElementById('eerr');
  if (el) el.textContent = text;
}

/** Send the editor's changes; the server validates, authorizes and audits everything. */
function submitEditor(/** @type {HTMLFormElement} */ form) {
  const panel = document.getElementById('catalog-panel');
  if (!panel) return;
  const insert = form.dataset.mode === 'insert';
  const unchecked = Array.from(form.querySelectorAll('.lk')).filter((box) => {
    const input = box.querySelector('input');
    const nullBox = input?.closest('.fl')?.querySelector('[data-null-for]');
    return (
      input instanceof HTMLInputElement &&
      input.value.trim() &&
      box.getAttribute('data-state') !== 'ok' &&
      !(nullBox instanceof HTMLInputElement && nullBox.checked)
    );
  });
  if (unchecked.length && !form.dataset.verified) {
    editorMessage('Checking references…');
    void Promise.all(
      unchecked.map((box) =>
        lookupFk(/** @type {HTMLInputElement} */ (box.querySelector('input')), true),
      ),
    ).then(() => {
      form.dataset.verified = '1';
      submitEditor(form);
      delete form.dataset.verified;
    });
    return;
  }
  const badLookup = Array.from(form.querySelectorAll('.lk[data-state="bad"]')).find((box) => {
    const input = box.querySelector('input');
    const nullBox = input?.closest('.fl')?.querySelector('[data-null-for]');
    return input?.value && !(nullBox instanceof HTMLInputElement && nullBox.checked);
  });
  if (badLookup) {
    const name = badLookup.querySelector('input')?.name ?? 'A field';
    return editorMessage(
      `${name}: no row in the referenced table has this value. Search and pick one.`,
    );
  }
  const cols = gridColumns();
  const row = /** @type {HTMLTableRowElement | undefined} */ (win.cuadbRow);
  const old = row && !insert ? rowValues(row) : {};
  /** @type {Record<string, string>} */
  const values = { schema: panel.dataset.schema ?? '', relation: panel.dataset.relation ?? '' };
  if (panel.dataset.prof) values.profile = panel.dataset.prof;
  let changed = 0;
  for (const col of cols) {
    const control =
      /** @type {HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement | null} */ (
        form.querySelector(`[name="${CSS.escape(col.name)}"]`)
      );
    const nullBox = /** @type {HTMLInputElement | null} */ (
      form.querySelector(`[data-null-for="${CSS.escape(col.name)}"]`)
    );
    if (!insert && col.pk) {
      const keyValue = old[col.name]?.value;
      if (keyValue === null || keyValue === undefined)
        return editorMessage('This row has no usable key.');
      values[`key.${col.name}`] = keyValue;
      continue;
    }
    if (!control || control.dataset.ro) continue;
    const before = old[col.name]?.value ?? null;
    const wantsNull = !!nullBox?.checked;
    const now = control.value;
    if (insert) {
      if (wantsNull) values[`null.${col.name}`] = '1';
      else if (now !== '') values[`col.${col.name}`] = now;
      else if (col.required && !col.pk) return editorMessage(`${col.name} is required.`);
      changed += 1;
      continue;
    }
    if (wantsNull ? before === null : before === now) continue;
    changed += 1;
    if (wantsNull) values[`null.${col.name}`] = '1';
    else values[`col.${col.name}`] = now;
    if (before === null) values[`oldnull.${col.name}`] = '1';
    else values[`old.${col.name}`] = before;
  }
  if (!changed) return editorMessage('No changes to save.');
  editorMessage('Saving…');
  win.htmx?.ajax('POST', insert ? '/data/insert' : '/data/update', {
    target: '#eerr',
    swap: 'innerHTML',
    values,
  });
}

function askDelete() {
  const panel = document.getElementById('insp');
  const row = /** @type {HTMLTableRowElement | undefined} */ (win.cuadbRow);
  if (!panel || !row) return;
  panel.querySelector('.ef')?.remove();
  const foot = document.createElement('div');
  foot.className = 'ef del';
  const message = document.createElement('div');
  message.id = 'eerr';
  message.setAttribute('role', 'status');
  message.textContent = 'Delete this row? This cannot be undone. It is recorded in the audit log.';
  const cancel = document.createElement('button');
  cancel.type = 'button';
  cancel.className = 'btn';
  cancel.setAttribute('data-close-insp', '');
  cancel.textContent = 'Cancel';
  const confirm = document.createElement('button');
  confirm.type = 'button';
  confirm.className = 'btn dg on';
  confirm.setAttribute('data-row-delete-confirm', '');
  confirm.textContent = 'Delete row';
  foot.append(message, cancel, confirm);
  panel.append(foot);
}

function deleteRow() {
  const panel = document.getElementById('catalog-panel');
  const row = /** @type {HTMLTableRowElement | undefined} */ (win.cuadbRow);
  if (!panel || !row) return;
  const old = rowValues(row);
  /** @type {Record<string, string>} */
  const values = {
    schema: panel.dataset.schema ?? '',
    relation: panel.dataset.relation ?? '',
    confirm: `${panel.dataset.schema}.${panel.dataset.relation}`,
  };
  if (panel.dataset.prof) values.profile = panel.dataset.prof;
  for (const col of gridColumns().filter((c) => c.pk)) {
    const value = old[col.name]?.value;
    if (value === null || value === undefined) return editorMessage('This row has no usable key.');
    values[`key.${col.name}`] = value;
  }
  win.htmx?.ajax('POST', '/data/delete', { target: '#eerr', swap: 'innerHTML', values });
}

/* ------------------------------------------------------------ grid niceties */
/** Thousands separators on numeric cells (never on key columns), restored when turned off. */
function formatNumbers(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('table.grid').forEach((table) => {
    const heads = Array.from(table.querySelectorAll('thead th'));
    table.querySelectorAll('tbody tr[data-row]').forEach((tr) => {
      Array.from(tr.children).forEach((node, index) => {
        const cell = /** @type {HTMLElement} */ (node);
        if (!cell.classList.contains('numeric') && !cell.classList.contains('num')) return;
        if (heads[index]?.hasAttribute('data-pk') || cell.querySelector('a,[data-null]')) return;
        const raw = cell.dataset.detail ?? '';
        const value = Number(raw);
        if (!raw || !Number.isFinite(value) || /e/i.test(raw)) return;
        const text = S.sep ? value.toLocaleString() : raw;
        if (cell.firstChild?.nodeType === Node.TEXT_NODE && cell.childNodes.length === 1) {
          cell.firstChild.textContent = text;
        }
      });
    });
  });
}

/**
 * Every table with a header gets fixed layout: header and body cells share one column
 * width (a colgroup), and each header has a drag handle to resize its column.
 */
function enhanceTables(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('table:not([data-fx])').forEach((node) => {
    const table = /** @type {HTMLTableElement} */ (node);
    const headRow = table.tHead?.rows[0];
    if (!headRow || (table.closest('#modal') === null && table.offsetParent === null)) return;
    table.dataset.fx = '1';
    if (gridReady) decorateGrid(table.parentElement ?? document);
    const ths = Array.from(headRow.cells);
    // Natural widths come from the browser's own auto layout, measured once.
    const natural = ths.map(
      (th) =>
        Math.ceil(th.getBoundingClientRect().width) +
        (th.dataset.col ? (th.querySelector('.hico') ? 28 : 64) : 0),
    );
    const panel = table.closest('#catalog-panel');
    const prefKey =
      panel instanceof HTMLElement
        ? `${panel.dataset.schema ?? ''}.${panel.dataset.relation ?? ''}`
        : '';
    const savedWidths = S.remember && prefKey ? TP[prefKey]?.w : null;
    const group = document.createElement('colgroup');
    const cols = ths.map((th, index) => {
      const col = document.createElement('col');
      const fixed = th.classList.contains('rn');
      const stored = Array.isArray(savedWidths) ? Number(savedWidths[index]) : 0;
      const width = fixed
        ? 46
        : stored >= 48 && stored <= 900
          ? stored
          : Math.max(64, Math.min(420, natural[index] ?? 120));
      col.style.width = `${width}px`;
      group.append(col);
      return col;
    });
    table.prepend(group);
    const total = () => cols.reduce((sum, col) => sum + parseFloat(col.style.width || '0'), 0);
    const fit = () => {
      table.style.width = `${total()}px`;
    };
    table.classList.add('fx');
    fit();
    ths.forEach((th, index) => {
      if (th.classList.contains('rn') || th.classList.contains('acts-h')) return;
      const handle = document.createElement('span');
      handle.className = 'rzh';
      handle.setAttribute('role', 'separator');
      handle.setAttribute('aria-orientation', 'vertical');
      handle.setAttribute('aria-label', `Resize column ${th.textContent?.trim() ?? ''}`);
      th.append(handle);
      const col = cols[index];
      if (!col) return;
      let startX = 0;
      let startW = 0;
      let dragging = false;
      const set = (/** @type {number} */ width) => {
        col.style.width = `${Math.max(48, Math.min(900, Math.round(width)))}px`;
        fit();
      };
      handle.addEventListener('pointerdown', (event) => {
        event.preventDefault();
        event.stopPropagation();
        startX = event.clientX;
        startW = parseFloat(col.style.width || '0');
        dragging = true;
        handle.classList.add('d');
        try {
          handle.setPointerCapture(event.pointerId);
        } catch {
          /* the drag still tracks while the pointer stays over the handle */
        }
      });
      handle.addEventListener('pointermove', (event) => {
        if (dragging) set(startW + event.clientX - startX);
      });
      const stop = () => {
        dragging = false;
        handle.classList.remove('d');
        if (prefKey) {
          TP[prefKey] ??= {};
          TP[prefKey].w = cols.map((col) => Math.round(parseFloat(col.style.width || '0')));
          prefs.save();
        }
      };
      handle.addEventListener('pointerup', stop);
      handle.addEventListener('pointercancel', stop);
      handle.addEventListener('click', (event) => event.stopPropagation());
      handle.addEventListener('dblclick', (event) => {
        // Double-click: fit the column to its widest visible content.
        event.stopPropagation();
        let widest = 0;
        table.querySelectorAll('tbody tr').forEach((tr) => {
          const cell = /** @type {HTMLElement | undefined} */ (tr.children[index]);
          if (cell) widest = Math.max(widest, cell.scrollWidth + 20);
        });
        set(Math.max(widest, th.scrollWidth + 12));
        stop();
      });
    });
  });
}

/** Remember sort and chosen columns per table (only while the preference is on). */
function tableKey(/** @type {URLSearchParams} */ params) {
  const schema = params.get('schema');
  const relation = params.get('relation');
  return schema && relation ? `${schema}.${relation}` : '';
}
function tableDisplayKey(/** @type {HTMLElement} */ panel) {
  return `${panel.dataset.schema ?? ''}.${panel.dataset.relation ?? ''}`;
}
function restoreTableDisplay(/** @type {HTMLElement | null} */ panel) {
  if (!S.remember || !panel) return;
  const saved = TP[tableDisplayKey(panel)];
  if (saved?.dens && ['compact', 'comfortable', 'dense', 'roomy'].includes(saved.dens)) {
    S.dens = saved.dens;
    prefs.apply();
    prefs.save();
  }
}
function rememberTable(/** @type {string} */ path) {
  if (!S.remember) return;
  const query = path.split('?')[1] ?? '';
  const params = new URLSearchParams(query);
  const key = tableKey(params);
  if (!key) return;
  const sort = params.get('sort');
  const cols = params.has('pick') ? params.getAll('column') : null;
  const prior = TP[key] ?? {};
  const widths = Array.from(document.querySelectorAll('.grid colgroup col')).map((col) =>
    Math.round(parseFloat(/** @type {HTMLElement} */ (col).style.width || '0')),
  );
  if (!sort && !cols && !prior.w) delete TP[key];
  else
    TP[key] = {
      ...prior,
      s: sort ? { c: sort, d: params.get('direction') === 'desc' ? -1 : 1 } : null,
      h: cols,
      w: prior.w ?? widths,
      dens: S.dens,
    };
  prefs.save();
}
function recallTable(/** @type {string} */ path) {
  if (!S.remember) return path;
  const [base, query = ''] = path.split('?');
  const params = new URLSearchParams(query);
  const key = tableKey(params);
  const saved = key ? TP[key] : null;
  if (!saved || params.has('sort') || params.has('pick') || params.has('filter')) return path;
  if (saved.s) {
    params.set('sort', saved.s.c);
    params.set('direction', saved.s.d < 0 ? 'desc' : 'asc');
  }
  if (Array.isArray(saved.h)) {
    params.set('pick', '1');
    saved.h.forEach((/** @type {string} */ name) => params.append('column', name));
  }
  return `${base}?${params.toString()}`;
}

/** Keyboard use of the grid: arrows move between rows, Ctrl/Cmd+C copies the focused row. */
document.addEventListener('keydown', (event) => {
  const row = event.target;
  if (!(row instanceof HTMLTableRowElement) || !row.dataset.row) return;
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    const sibling = event.key === 'ArrowDown' ? row.nextElementSibling : row.previousElementSibling;
    if (sibling instanceof HTMLElement) sibling.focus();
    return;
  }
  if (
    (event.ctrlKey || event.metaKey) &&
    event.key.toLowerCase() === 'c' &&
    !window.getSelection()?.toString()
  ) {
    event.preventDefault();
    const values = Object.values(rowValues(row)).map((v) => (v.value === null ? '' : v.value));
    void copyText(values.join('\t'), 'Row copied');
  }
});

/* ------------------------------------------------------------ export selection + column previews */
/** @type {Set<string>} */
const picked = new Set();
/** @type {Map<string, {out: string[], in: string[]}>} */
const linkCache = new Map();
let selectMode = false;

async function linksOf(/** @type {string} */ qual) {
  const hit = linkCache.get(qual);
  if (hit) return hit;
  const holder = document.querySelector(`.relw[data-qual="${CSS.escape(qual)}"]`);
  const schema = holder instanceof HTMLElement ? (holder.dataset.schema ?? '') : '';
  const relation = holder instanceof HTMLElement ? (holder.dataset.rel ?? '') : '';
  const params = new URLSearchParams({ schema, relation });
  if (win.cuadbProfile) params.set('profile', win.cuadbProfile);
  const result = { out: /** @type {string[]} */ ([]), in: /** @type {string[]} */ ([]) };
  try {
    const html = await (await fetch(`/catalog/links?${params.toString()}`)).text();
    const doc = new DOMParser().parseFromString(html, 'text/html');
    doc.querySelectorAll('li[data-link]').forEach((li) => {
      const name = li.getAttribute('data-link') ?? '';
      (li.getAttribute('data-dir') === 'in' ? result.in : result.out).push(name);
    });
  } catch {
    /* highlighting is a hint; export still works without it */
  }
  linkCache.set(qual, result);
  return result;
}

async function refreshPicks() {
  const linked = new Set();
  for (const qual of picked) {
    const found = await linksOf(qual);
    [...found.out, ...found.in].forEach((name) => linked.add(name));
  }
  $$('#tree .relw').forEach((node) => {
    const holder = /** @type {HTMLElement} */ (node);
    const qual = holder.dataset.qual ?? '';
    const on = picked.has(qual);
    const box = /** @type {HTMLInputElement | null} */ (holder.querySelector('.sel-ck'));
    if (box) box.checked = on;
    holder.querySelector('a.row')?.classList.toggle('lk', linked.has(qual) && !on);
    holder.querySelector('a.row')?.classList.toggle('picked', on);
  });
  renderSelbar();
}

function renderSelbar() {
  const bar = document.getElementById('selbar');
  if (!bar) return;
  bar.style.display = selectMode ? 'block' : 'none';
  bar.replaceChildren();
  if (!selectMode) return;
  const top = document.createElement('div');
  top.className = 'sr';
  const count = document.createElement('b');
  count.textContent = `${picked.size} selected`;
  const buttons = document.createElement('span');
  buttons.className = 'ia';
  const done = document.createElement('button');
  done.type = 'button';
  done.className = 'btn';
  done.setAttribute('data-sel-done', '');
  done.textContent = 'Done';
  const go = document.createElement('button');
  go.type = 'button';
  go.className = 'btn on';
  go.setAttribute('data-sel-export', '');
  go.textContent = 'Export ZIP';
  go.disabled = picked.size === 0;
  buttons.append(done, go);
  top.append(count, buttons);
  const row = document.createElement('label');
  row.className = 'sr';
  const text = document.createElement('span');
  text.textContent = 'Include linked tables';
  const toggle = document.createElement('input');
  toggle.type = 'checkbox';
  toggle.className = 'tg';
  toggle.id = 'sel-linked';
  toggle.checked = win.cuadbIncludeLinked !== false;
  toggle.setAttribute('data-sel-linked', '');
  row.append(text, toggle);
  bar.append(top, row);
}

function setSelectMode(/** @type {boolean} */ on) {
  selectMode = on;
  document.getElementById('tree')?.classList.toggle('selmode', on);
  document.getElementById('selb')?.setAttribute('aria-pressed', String(on));
  if (!on) picked.clear();
  void refreshPicks();
}

function downloadZip() {
  const [first, ...rest] = Array.from(picked);
  if (!first) return;
  const split = (/** @type {string} */ qual) => {
    const holder = document.querySelector(`.relw[data-qual="${CSS.escape(qual)}"]`);
    return holder instanceof HTMLElement
      ? [holder.dataset.schema ?? '', holder.dataset.rel ?? '']
      : ['', ''];
  };
  const [schema, relation] = split(first);
  const params = new URLSearchParams({
    schema: schema ?? '',
    relation: relation ?? '',
    format: 'zip',
    scope: 'related',
    depth: win.cuadbIncludeLinked === false ? '0' : '1',
  });
  if (rest.length) params.set('with', rest.map((q) => split(q).join('\u001f')).join('\u001e'));
  if (win.cuadbProfile) params.set('profile', win.cuadbProfile);
  const a = document.createElement('a');
  a.href = `/catalog/export?${params.toString()}`;
  a.download = '';
  document.body.append(a);
  a.click();
  a.remove();
  toast(`Exporting ${picked.size} table${picked.size === 1 ? '' : 's'} as ZIP`);
}

/* ------------------------------------------------------------ sidebar view options */
function sortValue(/** @type {HTMLElement} */ el, /** @type {string} */ key) {
  const raw = Number(el.dataset[key === 'bytes' ? 'bytes' : key]);
  return Number.isFinite(raw) ? raw : -1;
}
function sortChildren(
  /** @type {Element} */ parent,
  /** @type {string} */ selector,
  /** @type {string} */ key,
  /** @type {string} */ dir,
  /** @type {(el: HTMLElement) => string} */ nameOf,
) {
  const items = /** @type {HTMLElement[]} */ (
    Array.from(parent.children).filter((el) => el instanceof HTMLElement && el.matches(selector))
  );
  const sign = dir === 'desc' ? -1 : 1;
  items.sort((a, b) => {
    if (key === 'name')
      return (
        sign * nameOf(a).localeCompare(nameOf(b), undefined, { numeric: true, sensitivity: 'base' })
      );
    const x = sortValue(a, key);
    const y = sortValue(b, key);
    if (x < 0 && y >= 0) return 1;
    if (y < 0 && x >= 0) return -1;
    return sign * (x - y) || nameOf(a).localeCompare(nameOf(b));
  });
  items.forEach((el) => parent.append(el));
}
const relName = (/** @type {HTMLElement} */ el) => el.dataset.relationName ?? '';

/** Apply grouping, sorting and empty-schema hiding to whatever the tree currently holds. */
function applyTreeView() {
  const tree = document.getElementById('tree');
  if (!tree) return;
  const holder = tree.querySelector('.schema-tree');
  if (holder) {
    sortChildren(
      holder,
      '.sch',
      S.ssort === 'name' ? 'name' : S.ssort,
      S.ssort === 'name' ? 'asc' : 'desc',
      (el) => el.dataset.schemaName ?? '',
    );
    holder.querySelectorAll('.sch').forEach((node) => {
      const sch = /** @type {HTMLElement} */ (node);
      sch.classList.toggle('empty-hide', !!S.hideEmpty && sch.dataset.tables === '0');
    });
  }
  const flat = !S.tgroup;
  tree.classList.toggle('flat', flat);
  const loaded = $$('#tree .sch').filter((el) => el.querySelector('.relw, .grp'));
  let box = document.getElementById('flatbox');
  if (flat) {
    // Flat mode: every schema's tables in one list, prefixed with their schema.
    $$('#tree .sch').forEach((sch) => {
      const button = sch.querySelector('[data-s]');
      if (
        !sch.querySelector('.relw, .grp') &&
        !sch.classList.contains('o') &&
        button instanceof HTMLElement
      )
        button.click();
    });
    if (!box) {
      box = document.createElement('div');
      box.id = 'flatbox';
      box.className = 'flatbox';
      holder?.after(box);
    }
    loaded.forEach((sch) => {
      sch.querySelectorAll('.relw').forEach((node) => {
        const rel = /** @type {HTMLElement} */ (node);
        if (!rel.querySelector('.sq')) {
          const q = document.createElement('span');
          q.className = 'sq';
          q.textContent = `${rel.dataset.schema ?? ''}.`;
          rel.querySelector('a.row .n')?.before(q);
        }
        box?.append(rel);
      });
      const more = sch.querySelector('.rel-more');
      if (more) box?.append(more);
    });
    if (box) {
      const flatbox = box;
      sortChildren(flatbox, '.relw', S.tsort, S.tdir, relName);
      flatbox.querySelectorAll('.rel-more').forEach((button) => flatbox.append(button));
    }
  } else {
    if (box) {
      box.remove();
      // Tables were moved out of their schemas: reload the tree to put them back.
      const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
      fetchInto(`/catalog/schemas${profile}`, '#tree', 'innerHTML');
      return;
    }
    tree.querySelectorAll('.sub').forEach((sub) => {
      if (sub.querySelector(':scope > .relw')) sortChildren(sub, '.relw', S.tsort, S.tdir, relName);
    });
  }
}

/** Expand, collapse or reload the whole tree. */
function treeAction(/** @type {string} */ action) {
  if (action === 'reload') {
    const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
    document.getElementById('flatbox')?.remove();
    fetchInto(`/catalog/schemas${profile}`, '#tree', 'innerHTML');
    return;
  }
  $$('#tree .sch').forEach((sch) => {
    const open = sch.classList.contains('o');
    if (action === 'expand' && !open)
      /** @type {HTMLElement | null} */ (sch.querySelector('[data-s]'))?.click();
    if (action === 'collapse' && open) openSchema(sch, false);
  });
  $$('#tree .grp').forEach((g) => g.classList.toggle('o', action === 'expand'));
  $$('#tree .relw').forEach((r) => {
    if (action === 'collapse') r.classList.remove('o');
  });
  syncSidebarUrl();
}

/** Show timestamps in the chosen mode (display only; copies keep the stored value). */
function formatDates(/** @type {ParentNode} */ root = document) {
  const mode = String(S.dates || 'raw');
  root.querySelectorAll('td.ts-value').forEach((node) => {
    const cell = /** @type {HTMLElement} */ (node);
    const span = cell.querySelector('.ts');
    if (!span || cell.querySelector('[data-null]')) return;
    const raw = cell.dataset.detail ?? '';
    if (cell.dataset.shown === undefined) cell.dataset.shown = span.textContent ?? '';
    const stored = cell.dataset.shown;
    const hasDate = /\d{4}-\d\d-\d\d/.test(raw);
    const when = new Date(raw.replace(' ', 'T').replace(/([+-]\d\d)$/, '$1:00'));
    if (mode === 'raw' || !hasDate || Number.isNaN(when.getTime())) {
      span.textContent = stored;
      span.removeAttribute('title');
      return;
    }
    span.setAttribute('title', raw);
    if (mode === 'local') {
      span.textContent = when.toLocaleString(undefined, {
        dateStyle: 'medium',
        timeStyle: 'medium',
      });
    } else if (mode === 'date') {
      span.textContent = when.toLocaleDateString(undefined, { dateStyle: 'medium' });
    } else if (mode === 'relative') {
      const seconds = (when.getTime() - Date.now()) / 1000;
      const units = /** @type {Array<[Intl.RelativeTimeFormatUnit, number]>} */ ([
        ['year', 31536000],
        ['month', 2592000],
        ['week', 604800],
        ['day', 86400],
        ['hour', 3600],
        ['minute', 60],
      ]);
      const pick = units.find(([, size]) => Math.abs(seconds) >= size);
      const fmt = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
      span.textContent = pick
        ? fmt.format(Math.round(seconds / pick[1]), pick[0])
        : fmt.format(Math.round(seconds), 'second');
    }
  });
}

/** Remember the last table for 'reopen on load' (names only). */
function rememberLast(/** @type {string} */ path) {
  try {
    const params = new URLSearchParams(path.split('?')[1] ?? '');
    if (
      params.get('schema') &&
      params.get('relation') &&
      !(params.get('profile') ?? '').includes('~')
    ) {
      localStorage.setItem(
        'cuadb:last',
        JSON.stringify({
          s: params.get('schema'),
          r: params.get('relation'),
          p: params.get('profile') ?? '',
        }),
      );
    }
  } catch {
    /* optional */
  }
}
function restoreLast() {
  if (
    !S.restore ||
    document.getElementById('catalog-panel') ||
    location.pathname !== '/' ||
    location.search
  )
    return;
  try {
    const last = JSON.parse(localStorage.getItem('cuadb:last') || 'null');
    if (!last?.s || !last?.r) return;
    const params = new URLSearchParams({ schema: last.s, relation: last.r });
    if (last.p) params.set('profile', last.p);
    fetchInto(`/catalog/data?${params.toString()}`, '#mainbody', 'innerHTML');
  } catch {
    /* optional */
  }
}

/* ------------------------------------------------------------ sidebar tree */
function openSchema(/** @type {Element} */ holder, /** @type {boolean} */ open) {
  holder.classList.toggle('o', open);
  holder.setAttribute('aria-expanded', String(open));
}
function autoOpenSchema() {
  // Schemas open only by user choice or explicit URL state.
}
function filterTree() {
  const input = /** @type {HTMLInputElement | null} */ (document.getElementById('f'));
  const needle = (input?.value ?? '').trim().toLocaleLowerCase();
  $$('#tree .sch').forEach((schema) => {
    const schemaName = (schema.getAttribute('data-schema-name') ?? '').toLocaleLowerCase();
    const schemaHit = needle !== '' && fuzzyMatch(needle, schemaName);
    let any = false;
    schema.querySelectorAll('[data-relation-name]').forEach((relation) => {
      const name = (relation.getAttribute('data-relation-name') ?? '').toLocaleLowerCase();
      const qualified = `${schemaName}.${name}`;
      const hit =
        needle === '' || schemaHit || fuzzyMatch(needle, name) || fuzzyMatch(needle, qualified);
      relation.toggleAttribute('hidden', !hit);
      any ||= hit;
    });
    schema.toggleAttribute('hidden', needle !== '' && !schemaHit && !any);
    schema.classList.toggle('search-results', needle !== '');
  });
  const flat = document.getElementById('flatbox');
  if (flat) {
    flat.querySelectorAll('[data-relation-name]').forEach((relation) => {
      const name = (relation.getAttribute('data-relation-name') ?? '').toLocaleLowerCase();
      const qual = (relation.getAttribute('data-qual') ?? '').toLocaleLowerCase();
      relation.toggleAttribute(
        'hidden',
        needle !== '' && !fuzzyMatch(needle, name) && !fuzzyMatch(needle, qual),
      );
    });
  }
  const none = document.getElementById('tree-none');
  if (none) {
    const anyVisible =
      $$('#tree .sch').some((el) => !el.hasAttribute('hidden')) ||
      Array.from(flat?.querySelectorAll('[data-relation-name]') ?? []).some(
        (el) => !el.hasAttribute('hidden'),
      );
    none.hidden = needle === '' || anyVisible;
  }
}
/** Search every visible schema on the server so paging and unloaded relations are included. */
/** @type {ReturnType<typeof setTimeout> | undefined} */
let treeSearchTimer;
function searchServerTree() {
  clearTimeout(treeSearchTimer);
  treeSearchTimer = setTimeout(() => {
    const needle =
      /** @type {HTMLInputElement | null} */ (document.getElementById('f'))?.value.trim() ?? '';
    // Flat mode moves rows out of their schema containers. Clear those old copies
    // before server results are moved into the same flat list, or matches duplicate.
    if (!S.tgroup) {
      document
        .querySelectorAll('#flatbox .relw, #flatbox [data-load-relations]')
        .forEach((row) => row.remove());
    }
    for (const schema of $$('#tree .sch')) {
      const button = schema.querySelector('[data-s]');
      const sub = schema.querySelector('.sub');
      const href = button?.getAttribute('hx-get');
      const relationsHref = button?.getAttribute('data-relations');
      const name = schema.getAttribute('data-schema-name') ?? '';
      if (!(sub instanceof HTMLElement) || (!href && !relationsHref)) continue;
      if (!needle && !schema.classList.contains('o')) continue;
      const url = new URL(relationsHref ?? href ?? '', location.origin);
      url.searchParams.set('offset', '0');
      url.searchParams.set('limit', '200');
      if (needle) url.searchParams.set('q', needle);
      else url.searchParams.delete('q');
      if (S.mode === 'advanced') url.searchParams.set('group', '1');
      loadSchemaRelations(schema, `${url.pathname}${url.search}`, true);
    }
  }, 120);
}
function loadSchemaRelations(
  /** @type {Element} */ schema,
  /** @type {string} */ requestUrl,
  /** @type {boolean} */ force = false,
) {
  const sch = schema instanceof HTMLElement ? schema : null;
  const button = sch?.querySelector('[data-s]');
  const sub = sch?.querySelector('.sub');
  if (!(button instanceof HTMLElement) || !(sub instanceof HTMLElement)) return;
  if (!force && (sub.dataset.loaded === '1' || sub.dataset.loading === '1')) return;
  if (sub.dataset.requestUrl !== requestUrl) {
    sub.dataset.statsLoaded = '0';
    sub.dataset.statsLoading = '0';
  }
  sub.dataset.loading = '1';
  if (!sub.querySelector('[data-relation-name]')) {
    const loading = document.createElement('div');
    loading.className = 'note';
    loading.textContent = 'Loading table names…';
    sub.replaceChildren(loading);
  }
  const url = new URL(requestUrl, location.origin);
  if (S.mode === 'advanced') url.searchParams.set('group', '1');
  const finalUrl = `${url.pathname}${url.search}`;
  sub.dataset.requestUrl = finalUrl;
  const target = `#tree .sch[data-schema-name="${CSS.escape(sch?.dataset.schemaName ?? '')}"] .sub`;
  win.htmx?.ajax('GET', finalUrl, { target, swap: 'innerHTML', source: button });
}
async function loadSchemaSummaries() {
  const root = document.querySelector('#tree .schema-tree');
  if (!(root instanceof HTMLElement) || root.dataset.details === 'true' || !S.tsums) return;
  const params = new URLSearchParams({ details: 'true' });
  if (win.cuadbProfile) params.set('profile', win.cuadbProfile);
  const profile = win.cuadbProfile ?? '';
  try {
    const response = await fetch(`/catalog/schemas?${params}`, {
      headers: { accept: 'text/html' },
    });
    if (!response.ok || profile !== (win.cuadbProfile ?? '')) return;
    const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
    doc.querySelectorAll('.sch[data-schema-name]').forEach((loaded) => {
      const name = loaded.getAttribute('data-schema-name') ?? '';
      const current = document.querySelector(`#tree .sch[data-schema-name="${CSS.escape(name)}"]`);
      if (!(current instanceof HTMLElement)) return;
      current.dataset.tables = loaded.getAttribute('data-tables') ?? '-1';
      current.dataset.bytes = loaded.getAttribute('data-bytes') ?? '-1';
      const source = loaded.querySelector('.sch-sum');
      const target = current.querySelector('.sch-sum');
      if (source && target instanceof HTMLElement) {
        target.textContent = source.textContent ?? '';
        target.hidden = !target.textContent;
      }
    });
    root.dataset.details = 'true';
    applyTreeView();
  } catch {
    /* The schema names are already available; summaries can be retried on refresh. */
  }
}
async function loadRelationStats(/** @type {Element} */ subElement) {
  const sub = subElement instanceof HTMLElement ? subElement : null;
  if (!(sub instanceof HTMLElement) || (!S.tcounts && !S.tsums)) return;
  const requestUrl = sub.dataset.requestUrl ?? '';
  if (!requestUrl || sub.dataset.statsLoading === '1' || sub.dataset.statsLoaded === '1') return;
  sub.dataset.statsLoading = '1';
  const url = new URL(requestUrl, location.origin);
  url.searchParams.set('details', 'true');
  try {
    const response = await fetch(`${url.pathname}${url.search}`, {
      headers: { accept: 'text/html' },
    });
    if (!response.ok || sub.dataset.requestUrl !== requestUrl) return;
    const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
    const details = new Map(
      Array.from(doc.querySelectorAll('.relw[data-rel]')).map((row) => [
        row.getAttribute('data-rel') ?? '',
        row,
      ]),
    );
    const schemaName = new URL(requestUrl, location.origin).searchParams.get('schema') ?? '';
    document
      .querySelectorAll(`#tree .relw[data-schema="${CSS.escape(schemaName)}"][data-rel]`)
      .forEach((row) => {
        const source = details.get(row.getAttribute('data-rel') ?? '');
        if (!(source instanceof HTMLElement) || !(row instanceof HTMLElement)) return;
        row.dataset.rows = source.dataset.rows ?? '-1';
        row.dataset.bytes = source.dataset.bytes ?? '-1';
        const sourceLink = source.querySelector('a.row');
        const targetLink = row.querySelector('a.row');
        if (sourceLink instanceof HTMLElement && targetLink instanceof HTMLElement) {
          targetLink.title = sourceLink.title;
          const count = sourceLink.querySelector('[data-count]');
          let targetCount = targetLink.querySelector('[data-count]');
          if (count && !targetCount) {
            targetCount = document.createElement('span');
            targetCount.className = 'c';
            targetCount.setAttribute('data-count', '');
            targetLink.append(targetCount);
          }
          if (targetCount instanceof HTMLElement) {
            targetCount.textContent = count?.textContent ?? '';
            targetCount.hidden = !S.tcounts || !count;
            targetCount.classList.toggle('hidden-count', S.side === 'size');
          }
        }
      });
    sub.dataset.statsLoaded = '1';
  } catch {
    /* Estimates are optional; relation names remain usable. */
  } finally {
    sub.dataset.statsLoading = '0';
  }
}
function fuzzyMatch(/** @type {string} */ query, /** @type {string} */ value) {
  if (!query) return true;
  if (value.includes(query)) return true;
  const distance = (/** @type {string} */ a, /** @type {string} */ b) => {
    let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
    for (let i = 1; i <= a.length; i++) {
      const row = [i];
      for (let j = 1; j <= b.length; j++)
        row[j] = Math.min(
          (row[j - 1] ?? Infinity) + 1,
          (prev[j] ?? Infinity) + 1,
          (prev[j - 1] ?? Infinity) + (a[i - 1] === b[j - 1] ? 0 : 1),
        );
      prev = row;
    }
    return prev[b.length] ?? Infinity;
  };
  const tolerance = Math.max(1, Math.floor(query.length / 4));
  return value.split(/[._\s-]+/).some((part) => distance(query, part) <= tolerance);
}
function syncSidebarUrl() {
  const url = new URL(location.href);
  const opened = $$('#tree .sch.o')
    .map((node) => node.getAttribute('data-schema-name') ?? '')
    .filter(Boolean);
  const search =
    /** @type {HTMLInputElement | null} */ (document.getElementById('f'))?.value.trim() ?? '';
  url.searchParams.delete('open');
  let openQuery = '';
  if (opened.length) {
    openQuery = opened.map((schema) => encodeURIComponent(schema)).join(',');
    url.searchParams.set('open', opened.join(','));
    url.searchParams.delete('collapse');
  } else {
    url.searchParams.set('collapse', 'all');
  }
  if (search) url.searchParams.set('search', search);
  else url.searchParams.delete('search');
  const href = openQuery
    ? url.toString().replace(/([?&])open=[^&#]*/, `$1open=${openQuery}`)
    : url.toString();
  history.replaceState(history.state, '', href);
}
function restoreSidebarState() {
  const params = new URLSearchParams(location.search);
  const search = /** @type {HTMLInputElement | null} */ (document.getElementById('f'));
  if (search && params.has('search')) search.value = params.get('search') ?? '';
  let opened = new Set(
    params
      .getAll('open')
      .flatMap((value) => value.split(','))
      .filter(Boolean),
  );
  // Read legacy links once so bookmarks made by older versions still restore.
  if (
    opened.size === 1 &&
    !params.has('collapse') &&
    (Array.from(opened)[0] ?? '').startsWith('[')
  ) {
    try {
      const saved = JSON.parse(Array.from(opened)[0] ?? '[]');
      if (Array.isArray(saved)) opened = new Set(saved.filter((name) => typeof name === 'string'));
    } catch {
      /* malformed URL state is ignored */
    }
  }
  const active = location.pathname.match(/^\/table\/([^/]+)\/([^/]+)/);
  if (active?.[1] && !params.has('collapse') && !params.has('open'))
    opened.add(decodeURIComponent(active[1]));
  for (const schema of $$('#tree .sch')) {
    const name = schema.getAttribute('data-schema-name') ?? '';
    if (!opened.has(name)) continue;
    const button = schema.querySelector('[data-s]');
    openSchema(schema, true);
    const sub = schema.querySelector('.sub');
    const href = button?.getAttribute('data-relations') ?? button?.getAttribute('hx-get');
    if (sub && href && !sub.querySelector('[data-relation-name]'))
      loadSchemaRelations(schema, href);
  }
  if (search?.value) filterTree();
  selectActiveTable();
}
function selectActiveTable() {
  const active = location.pathname.match(/^\/table\/([^/]+)\/([^/]+)/);
  if (!active) {
    markActiveRow(null);
    return;
  }
  const schema = decodeURIComponent(active[1] ?? '');
  const relation = decodeURIComponent(active[2] ?? '');
  const link = document.querySelector(
    `#tree .relw[data-schema="${CSS.escape(schema)}"][data-rel="${CSS.escape(relation)}"] a.row`,
  );
  if (link instanceof Element) markActiveRow(link);
  else markActiveRow(null);
}
function markActiveRow(/** @type {Element | null} */ link) {
  $$('#tree .row.act').forEach((el) => {
    el.classList.remove('act');
    el.removeAttribute('aria-current');
  });
  link?.classList.add('act');
  link?.setAttribute('aria-current', 'page');
}

/* ------------------------------------------------------------ column picker (server side) */
function columnsRequest(/** @type {boolean | null} */ all) {
  const form = /** @type {HTMLFormElement | null} */ (document.querySelector('.row-search'));
  const params = new URLSearchParams(form ? /** @type {Any} */ (new FormData(form)) : undefined);
  params.delete('column');
  params.delete('page');
  params.delete('cursor');
  params.set('pick', '1');
  const boxes = $$('[data-column-choice]');
  boxes.forEach((node, index) => {
    const box = /** @type {HTMLInputElement} */ (node);
    if (all === true) box.checked = index < 24;
    if (all === false) box.checked = false;
    if (box.checked) params.append('column', box.dataset.columnChoice ?? '');
  });
  fetchInto(`/catalog/data?${params.toString()}`, '#catalog-panel');
}

/* ------------------------------------------------------------ clicks */
document.addEventListener('click', async (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const foreignLink = target.closest('a.fk[href^="#fk="]');
  if (foreignLink instanceof HTMLAnchorElement) {
    event.preventDefault();
    try {
      const fragment = new URL(foreignLink.href).hash.slice(4);
      void openForeignRow(JSON.parse(decodeURIComponent(fragment)));
    } catch {
      toast('The referenced row link is invalid.');
    }
    return;
  }
  // The sheet closes on any click outside it. That click only closes: it never opens another
  // row, so a double-click on a row opens the sheet and the second click closes it.
  if (sheetOpen() && !target.closest('#insp, #insp-rs, [data-row-add], .pop')) {
    closeInspector();
    if (target.closest('tr[data-row], #sheet-bk, .gw')) return;
  }
  const numberCell = target.closest('td.rn, th.rn');
  if (numberCell) {
    if (numberCell.tagName === 'TH') {
      const rows = gridRows();
      const on = !rows.every((tr) => tr.classList.contains('ck'));
      rows.forEach((tr) => setRowChecked(tr, on));
    } else {
      const tr = numberCell.closest('tr');
      if (tr instanceof HTMLTableRowElement && tr.dataset.row)
        setRowChecked(tr, !tr.classList.contains('ck'));
    }
    updateSelection();
    return;
  }
  if (target.closest('[data-copy-selected]')) {
    const { csv, count } = selectedCsv();
    void copyText(csv, `Copied ${count} selected rows as CSV`);
    return;
  }
  if (target.closest('[data-clear-selected]')) {
    gridRows().forEach((tr) => setRowChecked(tr, false));
    updateSelection();
    return;
  }
  const fullBtn = target.closest('[data-load-full]');
  if (fullBtn instanceof HTMLButtonElement) {
    void loadFullValue(fullBtn);
    return;
  }
  const copyBtn = target.closest('[data-copy-value]');
  if (copyBtn instanceof HTMLElement) {
    const index = Number(copyBtn.dataset.copyValue);
    const cell = win.cuadbRow?.querySelectorAll('td:not(.rn)')[index];
    void copyText(
      cell?.querySelector('[data-null]') ? '' : (cell?.dataset.detail ?? ''),
      'Value copied',
    );
    return;
  }
  const inPop = target.closest('.pop');
  if (!inPop && !target.closest('[data-pop],#cbtn,#dbtn,#sbtn,#obtn')) closePops();

  if (target.closest('[data-modal-close]') || target.id === 'modal') {
    closeModal();
    return;
  }
  if (target.closest('[data-toggle-sidebar]')) {
    setDrawer(!(document.getElementById('side')?.classList.contains('show') ?? false));
    return;
  }
  if (innerWidth < 761 && !target.closest('#side, #menu')) setDrawer(false);
  if (target.closest('#obtn')) {
    togglePop('opop');
    return;
  }
  const treeBtn = target.closest('[data-tree]');
  if (treeBtn instanceof HTMLElement) {
    treeAction(treeBtn.dataset.tree ?? '');
    closePops();
    return;
  }
  const dbOption = target.closest('[data-use-db]');
  if (dbOption instanceof HTMLElement) {
    void useDatabase(dbOption);
    return;
  }
  if (target.closest('#dbtn')) {
    renderDatabasePop();
    togglePop('dpop');
    return;
  }
  if (target.closest('#cbtn')) {
    togglePop('cpop');
    return;
  }
  if (target.closest('#sbtn')) {
    openSettings();
    return;
  }
  if (target.closest('[data-sdlg-close]') || target.id === 'spop') {
    closeSettings();
    return;
  }
  const paneBtn = target.closest('[data-sp]');
  if (paneBtn instanceof HTMLElement) {
    showSettingsPane(paneBtn.dataset.sp ?? 'general');
    setHash(`settings/${encodeURIComponent(paneBtn.dataset.sp ?? 'general')}`);
    return;
  }
  const popBtn = target.closest('[data-pop]');
  if (popBtn instanceof HTMLElement) {
    togglePop(popBtn.dataset.pop ?? '');
    return;
  }

  const profile = target.closest('[data-profile]');
  if (profile instanceof HTMLElement) {
    event.preventDefault();
    event.stopImmediatePropagation();
    const label = profile.dataset.label ?? 'Connection';
    const name = document.getElementById('cname');
    if (name) name.textContent = label;
    // Optimistic: show the choice at once, then confirm it with a real check below.
    document.getElementById('cdot')?.classList.remove('off');
    document.getElementById('cdot')?.classList.add('pend');
    const shownDb = document.getElementById('dname');
    if (shownDb) shownDb.textContent = profile.dataset.db || 'database';
    const previousProfile = String(win.cuadbProfile || '').split('~')[0];
    const nextProfile = profile.dataset.profile ?? '';
    const token = document.querySelector('meta[name="csrf-token"]')?.getAttribute('content') ?? '';
    try {
      const reconnect = await fetch('/connection/reconnect', {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': token },
        body: new URLSearchParams({ id: nextProfile }),
      });
      if (!reconnect.ok) throw new Error('reconnect failed');
    } catch {
      toast('This connection could not be refreshed.');
      return;
    }
    win.cuadbProfile = nextProfile;
    const url = new URL('/', location.origin);
    if (win.cuadbProfile) url.searchParams.set('profile', win.cuadbProfile);
    history.pushState({}, '', url);
    syncDatabase(profile);
    closePops();
    document.getElementById('f')?.focus();
    closeInspector();
    const main = document.getElementById('mainbody');
    if (main) {
      main.replaceChildren();
      const hint = document.createElement('section');
      hint.className = 'es';
      const h = document.createElement('h2');
      h.textContent = 'Pick a table to explore';
      const p = document.createElement('p');
      p.textContent = `Connected to ${label}. Choose a table from the sidebar.`;
      hint.append(h, p);
      main.append(hint);
    }
    fetchInto(`/catalog/schemas?profile=${encodeURIComponent(nextProfile)}`, '#tree', 'innerHTML');
    void checkConnection();
    if (previousProfile && previousProfile !== nextProfile) {
      void fetch('/connection/disconnect', {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': token },
        body: new URLSearchParams({ profile: previousProfile }),
      });
    }
    return;
  }

  const groupBtn = target.closest('#tree [data-g]');
  if (groupBtn) {
    const holder = groupBtn.closest('.grp');
    const open = holder?.classList.toggle('o') ?? false;
    groupBtn.setAttribute('aria-expanded', String(open));
    return;
  }
  const schemaBtn = target.closest('#tree [data-s]');
  if (schemaBtn) {
    const holder = schemaBtn.closest('.sch');
    if (holder) {
      const opening = !holder.classList.contains('o');
      openSchema(holder, opening);
      if (opening) {
        const href = schemaBtn.getAttribute('data-relations') ?? '';
        if (href) loadSchemaRelations(holder, href);
      }
    }
    syncSidebarUrl();
    return;
  }
  const more = target.closest('#tree [data-load-relations]');
  if (more instanceof HTMLButtonElement) {
    const schema = more.dataset.schema ?? '';
    const href = more.dataset.href ?? '';
    const holder = document.querySelector(`#tree .sch[data-schema-name="${CSS.escape(schema)}"]`);
    const sub = holder?.querySelector('.sub');
    if (sub instanceof HTMLElement && href) {
      more.disabled = true;
      more.textContent = 'Loading table names…';
      sub.dataset.requestUrl = href;
      sub.dataset.statsLoaded = '0';
      sub.dataset.statsLoading = '0';
      fetchInto(href, `#tree .sch[data-schema-name="${CSS.escape(schema)}"] .sub`, 'beforeend');
      more.remove();
    }
    return;
  }
  const rowLink = target.closest('#tree a.row');
  if (rowLink) {
    markActiveRow(rowLink);
    syncSidebarUrl();
    if (innerWidth < 761) setDrawer(false);
    return;
  }

  const viewBtn = target.closest('[data-view]');
  if (viewBtn instanceof HTMLElement && viewBtn.dataset.view) {
    const view = viewBtn.dataset.view;
    closeSettings();
    closePops();
    closeInspector();
    markActiveRow(null);
    if (innerWidth < 761) setDrawer(false);
    const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
    if (view === 'connections') {
      fetchInto('/admin/connections', '#mainbody', 'innerHTML');
      history.pushState({}, '', '/workspace/admin/connections');
      return;
    }
    fetchInto(view === 'admin' ? '/admin' : `/sql${profile}`, '#mainbody', 'innerHTML');
    return;
  }
  const admToggle = target.closest('[data-adm-toggle]');
  if (admToggle instanceof HTMLElement) {
    const el = document.getElementById(admToggle.dataset.admToggle ?? '');
    if (el) el.hidden = !el.hidden;
    el?.querySelector('input,select')?.dispatchEvent(new Event('focus'));
    return;
  }

  /* settings */
  const st = target.closest('[data-st]');
  if (st instanceof HTMLElement && st.dataset.st) {
    S[st.dataset.st] = st.dataset.val ?? '';
    applyPrefs();
    return;
  }
  const accent = target.closest('[data-ac]');
  if (accent instanceof HTMLElement) {
    S.accent = accent.dataset.ac ?? '';
    applyPrefs();
    return;
  }
  if (target.closest('#rforget')) {
    Object.keys(TP).forEach((key) => delete TP[key]);
    prefs.save();
    toast('Forgot every remembered table');
    return;
  }
  if (target.closest('#sexport')) {
    const blob = new Blob([JSON.stringify({ settings: S, tables: TP }, null, 2)], {
      type: 'application/json',
    });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'cuadb-settings.json';
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 800);
    return;
  }
  if (target.closest('#rall')) {
    Object.assign(S, prefs.DEF);
    Object.keys(TP).forEach((key) => delete TP[key]);
    applyPrefs();
    return;
  }
  if (target.closest('#rst')) {
    const table = currentTable();
    if (table) {
      delete TP[table];
      prefs.save();
      toast(`Reset ${table}`);
    }
    return;
  }

  /* grid */
  if (target.closest('[data-density]')) {
    S.dens = S.dens === 'compact' ? 'comfortable' : 'compact';
    const panel = document.getElementById('catalog-panel');
    if (S.remember && panel instanceof HTMLElement) {
      const key = tableDisplayKey(panel);
      TP[key] = { ...TP[key], dens: S.dens };
    }
    applyPrefs();
    return;
  }
  if (target.closest('[data-row-add]')) {
    openEditor(null);
    return;
  }
  if (target.closest('[data-row-edit]')) {
    if (win.cuadbRow) openEditor(win.cuadbRow);
    return;
  }
  if (target.closest('[data-row-delete]')) {
    askDelete();
    return;
  }
  if (target.closest('[data-row-delete-confirm]')) {
    deleteRow();
    return;
  }
  if (target.closest('[data-close-insp]')) {
    closeInspector();
    return;
  }
  if (target.closest('[data-columns-all]')) return columnsRequest(true);
  if (target.closest('[data-columns-none]')) return columnsRequest(false);
  if (target.closest('[data-copy-csv]')) {
    const table = document.querySelector('#catalog-panel table.grid');
    if (table) {
      const quote = (/** @type {string} */ text) =>
        /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
      const heads = Array.from(table.querySelectorAll('thead th[data-col]')).map(
        (th) => th.getAttribute('data-col') ?? '',
      );
      const lines = [heads.map(quote).join(',')];
      table.querySelectorAll('tbody tr[data-row]').forEach((tr) => {
        lines.push(
          Array.from(tr.querySelectorAll('td:not(.rn)'))
            .map((td) =>
              td.querySelector('[data-null]')
                ? ''
                : quote(/** @type {HTMLElement} */ (td).dataset.detail ?? ''),
            )
            .join(','),
        );
      });
      void copyText(lines.join('\n'), `Copied ${lines.length - 1} rows as CSV`);
    }
    closePops();
    return;
  }
  const row = target.closest('tr[data-row]');
  if (row instanceof HTMLTableRowElement && !target.closest('a,button')) {
    openInspector(row);
  }
});

document.addEventListener('submit', (event) => {
  const form = event.target;
  if (form instanceof HTMLFormElement && form.getAttribute('id') === 'row-form') {
    event.preventDefault();
    submitEditor(form);
  }
});

document.addEventListener('change', (event) => {
  const picked = event.target;
  if (picked instanceof HTMLSelectElement && picked.dataset.ss) {
    S[picked.dataset.ss] = picked.value;
    applyPrefs();
    return;
  }
  if (picked instanceof HTMLInputElement && picked.id === 'simport' && picked.files?.[0]) {
    void picked.files[0].text().then((text) => {
      try {
        const data = JSON.parse(text);
        const known = Object.keys(prefs.DEF);
        for (const [key, value] of Object.entries(data.settings ?? {})) {
          if (known.includes(key) && typeof value === typeof prefs.DEF[key]) S[key] = value;
        }
        applyPrefs();
        toast('Settings imported');
      } catch {
        toast('That file is not a CuaDB settings file');
      }
    });
    return;
  }
  const field = event.target;
  if (field instanceof HTMLSelectElement && field.matches('[data-size-select]')) {
    fetchInto(field.value, '#catalog-panel');
    return;
  }
  if (field instanceof HTMLSelectElement && field.matches('[data-engine-select]')) {
    field.closest('form')?.setAttribute('data-engine', field.value);
    return;
  }
  const el = event.target;
  if (!(el instanceof HTMLInputElement)) return;
  if (el.dataset.nullFor !== undefined) {
    const control = el.closest('.fl')?.querySelector('input:not([type=checkbox]),select,textarea');
    if (control instanceof HTMLElement)
      /** @type {HTMLInputElement} */ (control).disabled = el.checked;
    return;
  }
  if (el.dataset.tg) {
    S[el.dataset.tg] = el.checked;
    applyPrefs();
  } else if (el.dataset.cc !== undefined) {
    S.accent = el.value;
    applyPrefs();
  } else if (el.matches('[data-column-choice]')) {
    columnsRequest(null);
  }
});

document.addEventListener('input', (event) => {
  const el = event.target;
  if (el instanceof HTMLInputElement && el.matches('[data-conn-filter]')) {
    const needle = el.value.trim().toLocaleLowerCase();
    document.querySelectorAll('[data-conn-row]').forEach((row) => {
      if (row instanceof HTMLElement)
        row.hidden = !(row.dataset.search ?? '').toLocaleLowerCase().includes(needle);
    });
    return;
  }
  if (el instanceof HTMLInputElement && el.id === 'f') {
    filterTree();
    searchServerTree();
    syncSidebarUrl();
  }
});

/** Visible, focusable rows of the sidebar tree, in order. */
function treeItems() {
  return /** @type {HTMLElement[]} */ (
    Array.from(document.querySelectorAll('#tree .sh, #tree a.row')).filter(
      (el) => el instanceof HTMLElement && el.offsetParent !== null,
    )
  );
}
document.addEventListener('keydown', (event) => {
  const here = event.target;
  if (
    (event.key === 'ArrowDown' || event.key === 'ArrowUp') &&
    here instanceof HTMLElement &&
    (here.id === 'f' || here.closest('#tree'))
  ) {
    const items = treeItems();
    if (here.id === 'f') {
      if (event.key === 'ArrowDown' && items[0]) {
        event.preventDefault();
        items[0].focus();
      }
      return;
    }
    const current = here.closest('#tree .sh, #tree a.row');
    const index = items.findIndex((el) => el === current);
    const item = items[event.key === 'ArrowDown' ? index + 1 : index - 1];
    event.preventDefault();
    if (item) item.focus();
    else document.getElementById('f')?.focus();
    return;
  }
});
document.addEventListener('keydown', (event) => {
  if (
    (event.metaKey || event.ctrlKey) &&
    event.key === 'Enter' &&
    event.target instanceof HTMLTextAreaElement &&
    event.target.id === 'sql'
  ) {
    event.preventDefault();
    document.getElementById('sqlform') instanceof HTMLFormElement &&
      /** @type {HTMLFormElement} */ (document.getElementById('sqlform')).requestSubmit();
    return;
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    setDrawer(true);
    const input = /** @type {HTMLInputElement | null} */ (document.getElementById('f'));
    input?.focus();
    input?.select();
    return;
  }
  if (event.key === 'Tab' && document.getElementById('spop')?.classList.contains('show')) {
    const nodes = Array.from(
      document.querySelectorAll(
        '#spop button, #spop select, #spop input:not([type=hidden]), #spop label.btn',
      ),
    ).filter(
      (el) => el instanceof HTMLElement && el.offsetParent !== null && !el.hasAttribute('disabled'),
    );
    const firstEl = /** @type {HTMLElement | undefined} */ (nodes[0]);
    const lastEl = /** @type {HTMLElement | undefined} */ (nodes[nodes.length - 1]);
    if (firstEl && lastEl) {
      if (event.shiftKey && document.activeElement === firstEl) {
        event.preventDefault();
        lastEl.focus();
      } else if (!event.shiftKey && document.activeElement === lastEl) {
        event.preventDefault();
        firstEl.focus();
      }
    }
    return;
  }
  if (event.key === 'Tab' && document.getElementById('modal')?.classList.contains('show')) {
    // Keep keyboard focus inside the open dialog.
    const focusable = Array.from(
      document.querySelectorAll(
        '#modal input:not([type=hidden]):not(:disabled), #modal select:not(:disabled), #modal textarea:not(:disabled), #modal button:not(:disabled)',
      ),
    ).filter((el) => el instanceof HTMLElement && el.offsetParent !== null);
    const first = /** @type {HTMLElement | undefined} */ (focusable[0]);
    const last = /** @type {HTMLElement | undefined} */ (focusable[focusable.length - 1]);
    if (first && last) {
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      } else if (!document.getElementById('modal')?.contains(document.activeElement)) {
        event.preventDefault();
        first.focus();
      }
    }
    return;
  }
  if (event.key === 'Escape') {
    if (document.getElementById('spop')?.classList.contains('show')) {
      closeSettings();
      return;
    }
    closeModal();
    closePops();
    setDrawer(false);
    closeInspector();
    return;
  }
  const target = event.target;
  if (target instanceof HTMLElement && (event.key === 'Enter' || event.key === ' ')) {
    if (target.id === 'connection-indicator') {
      event.preventDefault();
      win.htmx?.trigger(target, 'click');
    } else if (target instanceof HTMLTableRowElement && target.dataset.row) {
      event.preventDefault();
      openInspector(target);
    }
  }
});

/* ------------------------------------------------------------ swaps */
document.addEventListener('htmx:afterSwap', (event) => {
  const el = /** @type {Any} */ (event).detail?.elt ?? event.target;
  const root = el instanceof Element ? el : document;
  decorateIcons(root.ownerDocument === document ? document : root);
  if (event.target instanceof Element) {
    if (event.target.id === 'cpop') {
      const selected = event.target.querySelector('[aria-selected="true"]');
      const name = document.getElementById('cname');
      if (selected instanceof HTMLElement && name)
        name.textContent = selected.dataset.label ?? 'Connection';
      if (selected instanceof HTMLElement) syncDatabase(selected);
      const remembered = new URLSearchParams(location.search).get('database');
      if (remembered) {
        currentDb = { ...currentDb, db: remembered };
        const databaseName = document.getElementById('dname');
        if (databaseName) databaseName.textContent = remembered;
      }
      if (document.getElementById('dpop')?.classList.contains('show')) renderDatabasePop();
      const dot = document.getElementById('cdot');
      if (selected instanceof HTMLElement && dot) {
        const ready = selected.dataset.ready === 'true';
        dot.classList.toggle('off', !ready);
        document
          .getElementById('cbtn')
          ?.setAttribute('title', ready ? 'Connected' : 'Not connected: open the list to retry');
      }
    }
    if (event.target.id === 'tree') {
      applyTreeView();
      restoreSidebarState();
      restoreLast();
      loadSchemaSummaries();
    }
    if (event.target.id === 'mainbody' || event.target.id === 'catalog-panel') {
      const panel =
        event.target.id === 'catalog-panel'
          ? event.target
          : document.getElementById('catalog-panel');
      restoreTableDisplay(panel instanceof HTMLElement ? panel : null);
      const route = `/table/${encodeURIComponent(document.getElementById('catalog-panel')?.getAttribute('data-schema') ?? '')}/${encodeURIComponent(document.getElementById('catalog-panel')?.getAttribute('data-relation') ?? '')}`;
      const path = location.pathname;
      const match = path.match(/^\/table\/([^/]+)\/([^/]+)(?:\/(structure|indexes))?$/);
      if (match) {
        const selected = document.querySelector(
          `#tree .relw[data-schema="${CSS.escape(decodeURIComponent(match[1] ?? ''))}"][data-rel="${CSS.escape(decodeURIComponent(match[2] ?? ''))}"] a.row`,
        );
        if (selected instanceof Element) markActiveRow(selected);
      }
      void route;
      if (location.hash.startsWith('#fk=')) {
        const foreign = decodeHash('fk');
        if (foreign) void openForeignRow(foreign, false);
      }
      if (location.hash === '#row=new') setTimeout(() => openEditor(null), 0);
      else if (location.hash.startsWith('#row=')) {
        const row = decodeHash('row');
        if (row) setTimeout(() => restoreRowSheet(row), 0);
      }
      if (event.target.id === 'mainbody' && partsAdminTab()) setTimeout(restoreAdminTab, 0);
      if (pendingGridScrollLeft !== null) {
        const left = pendingGridScrollLeft;
        pendingGridScrollLeft = null;
        requestAnimationFrame(() => {
          const grid = document.querySelector('.gw');
          if (grid instanceof HTMLElement) grid.scrollLeft = left;
        });
      }
    }
    if (event.target.matches('.sub')) {
      const subTarget = /** @type {HTMLElement} */ (event.target);
      subTarget.dataset.loading = '0';
      subTarget.dataset.loaded = '1';
      const more = Array.from(subTarget.querySelectorAll('[data-load-relations]'));
      more.slice(0, -1).forEach((button) => button.remove());
      filterTree();
      loadObjects();
      applyTreeView();
      selectActiveTable();
      loadRelationStats(subTarget);
    }
    decorateBars(event.target);
    // A new page replaces the sheet's row; swaps inside the sheet itself (the editor form,
    // its messages) and elsewhere must leave it open.
    if (event.target.id === 'mainbody' || event.target.id === 'catalog-panel') closeInspector(true);
  }
});

/** Show server timestamps in the viewer's locale. */
function formatTimes(/** @type {ParentNode} */ root) {
  root.querySelectorAll('time[data-ts]').forEach((node) => {
    const el = /** @type {HTMLElement} */ (node);
    const seconds = Number(el.dataset.ts);
    if (!Number.isFinite(seconds) || seconds <= 0) return;
    const when = new Date(seconds * 1000);
    el.setAttribute('datetime', when.toISOString());
    el.textContent = when.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  });
}
function syncAdminTabs() {
  const tab = document.querySelector('#admin-body [data-tab]')?.getAttribute('data-tab');
  document.querySelectorAll('#adm-seg [data-tab]').forEach((btn) => {
    btn.classList.toggle('on', btn.getAttribute('data-tab') === tab);
  });
}
document.addEventListener('htmx:afterSwap', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  if (target.id === 'admin-body') {
    formatTimes(target);
    syncAdminTabs();
  }
  if (target.id === 'modal') {
    openModal();
    const form = target.querySelector('#conn-form');
    if (form) {
      const id = form.querySelector('input[name=id]')?.getAttribute('value');
      const next = `connection=${id ? encodeURIComponent(id) : 'add'}`;
      if (location.hash !== `#${next}`) pushHash(next);
    }
  }
  enhanceTables(document);
  formatNumbers(document);
  updateSelection();
  if (target.id === 'sql-tx') {
    const open = target.querySelector('[data-tx]');
    win.cuadbTx = open instanceof HTMLElement ? (open.dataset.tx ?? '') : '';
  }
});
document.addEventListener('htmx:afterRequest', (event) => {
  const detail = /** @type {Any} */ (event).detail;
  const path = String(detail.pathInfo?.requestPath ?? '');
  const restoring = !!restoreRequestPath && path === restoreRequestPath;
  if (restoring) restoreRequestPath = '';
  const finalRoute = routeForRequest(String(detail.pathInfo?.finalRequestPath ?? path));
  if (
    !restoring &&
    finalRoute &&
    Number(detail.xhr?.status ?? 0) >= 200 &&
    Number(detail.xhr?.status ?? 0) < 400
  ) {
    const url = new URL(finalRoute, location.origin);
    const openSchemas = $$('#tree .sch.o')
      .map((node) => node.getAttribute('data-schema-name') ?? '')
      .filter(Boolean);
    const search = /** @type {HTMLInputElement | null} */ (
      document.getElementById('f')
    )?.value.trim();
    url.searchParams.delete('open');
    openSchemas.forEach((name) => url.searchParams.append('open', name));
    if (openSchemas.length) url.searchParams.delete('collapse');
    else url.searchParams.set('collapse', 'all');
    if (search) url.searchParams.set('search', search);
    else url.searchParams.delete('search');
    if (currentDb.db) url.searchParams.set('database', currentDb.db);
    // Initial deep-link restoration reissues the current view through HTMX. Keep its
    // fragment so row/settings sheets remain addressable after the fragment loads.
    if (url.pathname === location.pathname) url.hash = location.hash;
    history.pushState({}, '', url);
    selectActiveTable();
  }
  const cancel = document.getElementById('sql-cancel');
  if (path.startsWith('/catalog/data')) {
    const finalPath = String(detail.pathInfo?.finalRequestPath ?? path);
    const status = Number(detail.xhr?.status ?? 0);
    if (status >= 200 && status < 400) {
      rememberTable(finalPath);
      rememberLast(finalPath);
    } else if (status >= 400) {
      // A remembered sort or column list may no longer fit the table: forget it and reopen plainly.
      const params = new URLSearchParams(finalPath.split('?')[1] ?? '');
      const key = tableKey(params);
      if (key && TP[key]) {
        delete TP[key];
        prefs.save();
        if (detail.elt instanceof Element && detail.elt.matches('#tree a.row')) {
          const plain = new URLSearchParams();
          for (const name of ['schema', 'relation', 'profile']) {
            const value = params.get(name);
            if (value) plain.set(name, value);
          }
          fetchInto(`/catalog/data?${plain.toString()}`, '#mainbody', 'innerHTML');
        }
      }
    }
  }
  if (path.startsWith('/catalog/data'))
    win.cuadbDataUrl = String(detail.pathInfo?.finalRequestPath ?? path);
  if (path.startsWith('/sql/execute')) {
    if (cancel) cancel.hidden = true;
    if (win.cuadbTx)
      fetchInto(`/tx/status?tx=${encodeURIComponent(win.cuadbTx)}`, '#sql-tx', 'innerHTML');
  }
  if (path.startsWith('/tx/commit') || path.startsWith('/tx/rollback')) {
    win.cuadbTx = '';
  }
});

function restoreLocation() {
  if (!location.hash.startsWith('#row=') && !location.hash.startsWith('#fk=')) closeInspector(true);
  const rememberedDatabase = new URLSearchParams(location.search).get('database');
  if (rememberedDatabase) {
    currentDb = { ...currentDb, db: rememberedDatabase };
    const dbName = document.getElementById('dname');
    if (dbName) dbName.textContent = rememberedDatabase;
  }
  const profile = new URLSearchParams(location.search).get('profile');
  if (profile) {
    // A removed or unknown connection in the URL would 404 every request: drop it and reload.
    void fetch('/connection/profiles')
      .then((r) => (r.ok ? r.text() : ''))
      .then((html) => {
        const base = profile.split('~')[0] ?? '';
        if (html && !html.includes(`data-profile="${base}"`)) {
          const url = new URL(location.href);
          url.searchParams.delete('profile');
          location.replace(url);
        }
      })
      .catch(() => {});
    win.cuadbProfile = profile;
    const tree = document.getElementById('tree');
    tree?.setAttribute('hx-get', `/catalog/schemas?profile=${encodeURIComponent(profile)}`);
    document
      .getElementById('cpop')
      ?.setAttribute('hx-get', `/connection/profiles?profile=${encodeURIComponent(profile)}`);
  }
  const request = requestForLocation();
  if (request) {
    restoreRequestPath = request;
    fetchInto(request, '#mainbody', 'innerHTML');
  }
  if (!location.hash.startsWith('#row=') && !location.hash.startsWith('#fk=')) closeInspector(true);
  const settings = location.hash.match(/^#settings\/(.*)$/);
  if (settings) openSettings(decodeURIComponent(settings[1] ?? ''));
  const pop = location.hash.match(/^#popover\/(.*)$/);
  if (pop) {
    const id = {
      connections: 'cpop',
      database: 'dpop',
      sidebar: 'opop',
      columns: 'colpop',
      export: 'exppop',
    }[decodeURIComponent(pop[1] ?? '')];
    if (id === 'dpop') renderDatabasePop();
    const node = document.getElementById(id ?? '');
    if (node) node.classList.add('show');
  }
  const foreign = decodeHash('fk');
  if (foreign) void openForeignRow(foreign, false);
  const row = decodeHash('row');
  if (row) restoreRowSheet(row);
  if (location.hash.startsWith('#connection=')) {
    const id = decodeURIComponent(location.hash.slice('#connection='.length));
    fetchInto(
      `/admin/connections/form${id === 'add' ? '' : `?id=${encodeURIComponent(id)}`}`,
      '#modal',
      'innerHTML',
    );
  }
  if (document.querySelector('#tree .sch')) restoreSidebarState();
}
function restoreRowSheet(/** @type {Any} */ row) {
  const expected = JSON.stringify(row.key ?? []);
  const match = $$('#catalog-panel tr[data-row-key]').find(
    (tr) => tr.getAttribute('data-row-key') === expected,
  );
  if (match instanceof HTMLTableRowElement) openInspector(match);
}
function partsAdminTab() {
  return location.pathname.startsWith('/workspace/admin/');
}
function restoreAdminTab() {
  const tab = decodeURIComponent(location.pathname.split('/').pop() ?? '');
  const key = tab === 'permissions' ? 'permissions' : tab;
  const button = document.querySelector(`#adm-seg [data-tab="${CSS.escape(key)}"]`);
  if (button instanceof HTMLElement) button.click();
  if (location.hash.startsWith('#connection=')) {
    const id = decodeURIComponent(location.hash.slice('#connection='.length));
    fetchInto(
      `/admin/connections/form${id === 'add' ? '' : `?id=${encodeURIComponent(id)}`}`,
      '#modal',
      'innerHTML',
    );
  }
}
window.addEventListener('popstate', restoreLocation);
window.addEventListener('hashchange', () => {
  if (location.hash.startsWith('#settings/')) {
    const match = location.hash.match(/^#settings\/(.*)$/);
    if (match) openSettings(decodeURIComponent(match[1] ?? ''));
  } else if (location.hash.startsWith('#fk=')) {
    const foreign = decodeHash('fk');
    if (foreign) void openForeignRow(foreign, false);
  } else if (location.hash.startsWith('#popover/')) {
    restoreLocation();
  } else if (location.hash.startsWith('#row=')) {
    const row = decodeHash('row');
    if (row) {
      const expected = JSON.stringify(row.key ?? []);
      const match = $$('#catalog-panel tr[data-row-key]').find(
        (tr) => tr.getAttribute('data-row-key') === expected,
      );
      if (match instanceof HTMLTableRowElement) openInspector(match);
    }
  } else {
    closeSettings();
    closeInspector(true);
  }
});
restoreLocation();
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    void navigator.serviceWorker.register('/sw.js').catch(() => {});
  });
}
document.addEventListener('htmx:beforeRequest', (event) => {
  const detailBefore = /** @type {Any} */ (event).detail;
  const requestPath = String(detailBefore.pathInfo?.requestPath ?? '');
  if (requestPath.startsWith('/catalog/data') && document.querySelector('.gw')) {
    const next = new URLSearchParams(requestPath.split('?')[1] ?? '');
    const panel = document.getElementById('catalog-panel');
    if (
      panel?.dataset.schema === next.get('schema') &&
      panel?.dataset.relation === next.get('relation')
    )
      pendingGridScrollLeft = document.querySelector('.gw')?.scrollLeft ?? 0;
  }
  if (
    win.cuadbEditing &&
    S.confirmDiscard &&
    detailBefore.requestConfig?.verb === 'get' &&
    String(detailBefore.pathInfo?.requestPath ?? '').startsWith('/catalog/') &&
    !window.confirm('Discard the unsaved row changes?')
  ) {
    event.preventDefault();
    return;
  }
  const path = String(/** @type {Any} */ (event).detail.pathInfo?.requestPath ?? '');
  if (path.startsWith('/sql/execute'))
    document.getElementById('sql-cancel')?.removeAttribute('hidden');
});
// A successful admin change: say so, then refresh the list it altered.
document.addEventListener('cuadb:changed', () => {
  // htmx fires response-header events before it swaps the body in; wait for the swap.
  setTimeout(afterChange, 80);
});
document.addEventListener('cuadb:connection-added', (event) => {
  const id = /** @type {Any} */ (event).detail?.id;
  if (!id) return;
  // The list refresh from cuadb:changed runs on a timer; refresh again, then switch.
  setTimeout(async () => {
    await refreshConnections();
    const button = document.querySelector(`#cpop [data-profile="${CSS.escape(id)}"]`);
    if (button instanceof HTMLElement) button.click();
  }, 250);
});
function refreshConnections() {
  const list = document.getElementById('cpop');
  if (!list) return;
  return Promise.resolve(
    win.htmx?.ajax('GET', '/connection/profiles', { target: '#cpop', swap: 'innerHTML' }),
  ).then(() => {
    const base = String(win.cuadbProfile || '').split('~')[0];
    const stillThere =
      !base || !!document.querySelector(`#cpop [data-profile="${CSS.escape(base)}"]`);
    if (!stillThere) {
      win.cuadbProfile = '';
      closeInspector();
      setMainHint('That connection was removed. Choose a table from the sidebar.');
      fetchInto('/catalog/schemas', '#tree', 'innerHTML');
    }
  });
}
function afterChange() {
  const cpopMsg = document.getElementById('cpop-msg')?.textContent?.trim();
  if (cpopMsg) {
    toast(cpopMsg);
    refreshConnections();
    return;
  }
  if (win.cuadbEditing && document.getElementById('eerr')) {
    const text = document.getElementById('eerr')?.textContent?.trim();
    win.cuadbEditing = false;
    closeInspector();
    toast(text && text !== 'Saving…' ? text : 'Saved');
    if (win.cuadbDataUrl) fetchInto(win.cuadbDataUrl, '#catalog-panel');
    return;
  }
  const inModal = document.getElementById('modal')?.classList.contains('show');
  const message = (
    inModal ? document.querySelector('#modal-msg') : document.querySelector('#adm-msg')
  )?.textContent?.trim();
  if (inModal) {
    closeModal();
    refreshConnections();
  }
  if (message) toast(message);
  const src = document.querySelector('#admin-body [data-src]')?.getAttribute('data-src');
  if (src) fetchInto(src, '#admin-body', 'innerHTML');
  else {
    const page = document.querySelector('#mainbody > .conn[data-src]');
    if (page)
      fetchInto(page.getAttribute('data-src') ?? '/admin/connections', '#mainbody', 'innerHTML');
  }
}

document.addEventListener('htmx:afterSettle', (event) => {
  if (event.target instanceof Element && event.target.id === 'tree') autoOpenSchema();
});

/* ------------------------------------------------------------ resizable sidebar */
const splitter = document.getElementById('rs');
const sidebar = document.getElementById('side');
if (splitter && sidebar) {
  const setWidth = (/** @type {number} */ width) => {
    const clamped = Math.max(180, Math.min(520, Math.round(width)));
    document.documentElement.style.setProperty('--sw', `${clamped}px`);
    splitter.setAttribute('aria-valuenow', String(clamped));
    try {
      localStorage.setItem('cuadb:sw', String(clamped));
    } catch {}
  };
  try {
    const saved = Number(localStorage.getItem('cuadb:sw'));
    if (saved >= 180 && saved <= 520) setWidth(saved);
  } catch {}
  let dragging = false;
  splitter.addEventListener('pointerdown', (event) => {
    if (innerWidth < 761) return;
    dragging = true;
    splitter.setPointerCapture(event.pointerId);
    splitter.classList.add('d');
  });
  splitter.addEventListener('pointermove', (event) => {
    if (dragging) setWidth(event.clientX);
  });
  const stop = () => {
    dragging = false;
    splitter.classList.remove('d');
  };
  splitter.addEventListener('pointerup', stop);
  splitter.addEventListener('pointercancel', stop);
  splitter.addEventListener('dblclick', () => setWidth(264));
  splitter.addEventListener('keydown', (event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    setWidth(sidebar.getBoundingClientRect().width + (event.key === 'ArrowRight' ? 16 : -16));
  });
}

/* ------------------------------------------------------------ start */
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (S.theme === 'system') prefs.apply();
});
applyPrefs();
decorateIcons();

enhanceTables(document);

// Select mode: clicking a table in the sidebar toggles it instead of opening it.
document.addEventListener(
  'click',
  (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const holder = target.closest('#tree .relw');
    if (!selectMode || !(holder instanceof HTMLElement) || target.closest('.cv')) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const qual = holder.dataset.qual ?? '';
    if (picked.has(qual)) picked.delete(qual);
    else picked.add(qual);
    void refreshPicks();
  },
  true,
);
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  if (target.closest('#selb')) return setSelectMode(!selectMode);
  if (target.closest('[data-sel-done]')) return setSelectMode(false);
  if (target.closest('[data-sel-export]')) return downloadZip();
  const chevron = target.closest('#tree .cv');
  if (chevron instanceof HTMLElement) {
    const holder = chevron.closest('.relw');
    const slot = holder?.querySelector('.cols-slot');
    if (!holder || !(slot instanceof HTMLElement)) return;
    const open = holder.classList.toggle('o');
    chevron.setAttribute('aria-expanded', String(open));
    if (open && !slot.dataset.loaded) {
      slot.dataset.loaded = '1';
      win.htmx?.ajax('GET', chevron.dataset.cols ?? '', { target: slot, swap: 'innerHTML' });
    }
  }
});
document.addEventListener('change', (event) => {
  const el = event.target;
  if (el instanceof HTMLInputElement && el.dataset.selLinked !== undefined) {
    win.cuadbIncludeLinked = el.checked;
  }
});

/* ------------------------------------------------------------ object actions (⋯ menu + dialog) */
/** @type {HTMLElement | null} */
let objMenu = null;
function closeObjMenu() {
  objMenu?.remove();
  objMenu = null;
}
function objActions(/** @type {string} */ scope, /** @type {string} */ engine) {
  const pg = !/sqlite/i.test(engine);
  const local = document.querySelector('.app')?.getAttribute('data-local') === 'true';
  /** @type {Array<[string, string, boolean?]>} */
  const items =
    scope === 'table'
      ? [
          ['rename', 'Rename table'],
          ['add-column', 'Add column'],
          ['rename-column', 'Rename column'],
          ['drop-column', 'Drop column', true],
          ...(pg ? [/** @type {[string, string]} */ (['comment', 'Edit comment'])] : []),
          ['truncate', 'Empty table', true],
          ['drop', 'Drop table', true],
        ]
      : [
          ['create-table', 'Create table'],
          ...(pg
            ? [
                /** @type {[string, string]} */ (['rename', 'Rename schema']),
                /** @type {[string, string]} */ (['comment', 'Edit comment']),
                /** @type {[string, string, boolean]} */ (['drop', 'Drop schema', true]),
              ]
            : []),
        ];
  if (local) items.push(['sql', 'Run SQL…']);
  return items;
}
function openObjMenu(/** @type {HTMLElement} */ button) {
  closeObjMenu();
  const scope = button.dataset.scope ?? 'table';
  const engine =
    /** @type {HTMLElement | null} */ (document.querySelector('#cpop [aria-selected="true"]'))
      ?.dataset.engine ?? '';
  const menu = document.createElement('div');
  menu.className = 'obj-menu';
  menu.setAttribute('role', 'menu');
  const head = document.createElement('div');
  head.className = 'obj-menu-h';
  head.textContent =
    scope === 'table'
      ? `${button.dataset.schema}.${button.dataset.rel}`
      : `Schema ${button.dataset.schema}`;
  menu.append(head);
  for (const [id, label, danger] of objActions(scope, engine)) {
    const item = document.createElement('button');
    item.type = 'button';
    item.setAttribute('role', 'menuitem');
    item.className = `obj-item${danger ? ' dg' : ''}`;
    item.textContent = label;
    item.addEventListener('click', () => {
      closeObjMenu();
      void openObjDialog(button, id);
    });
    menu.append(item);
  }
  document.body.append(menu);
  const rect = button.getBoundingClientRect();
  const left = Math.min(rect.right + 6, innerWidth - menu.offsetWidth - 8);
  const top = Math.min(rect.top, innerHeight - menu.offsetHeight - 8);
  menu.style.left = `${Math.max(8, left)}px`;
  menu.style.top = `${Math.max(8, top)}px`;
  objMenu = menu;
  /** @type {HTMLElement | null} */ (menu.querySelector('.obj-item'))?.focus();
}
async function openObjDialog(/** @type {HTMLElement} */ button, /** @type {string} */ tab) {
  const params = new URLSearchParams({
    scope: button.dataset.scope ?? 'table',
    schema: button.dataset.schema ?? '',
    relation: button.dataset.rel ?? '',
    database: button.dataset.database ?? '',
    profile: button.dataset.profile ?? String(win.cuadbProfile || ''),
  });
  const response = await fetch(`/object/actions?${params}`);
  const modal = document.getElementById('modal');
  if (!response.ok || !modal) {
    toast('Actions are not available for this connection.');
    return;
  }
  modal.innerHTML = await response.text();
  win.htmx?.process(modal);
  openModal();
  const tabBtn = document.querySelector(`[data-oa-tab="${CSS.escape(tab)}"]`);
  activateObjTab(
    tabBtn ? tab : (document.querySelector('[data-oa-tab]')?.getAttribute('data-oa-tab') ?? ''),
  );
}
function activateObjTab(/** @type {string} */ id) {
  const form = document.getElementById('oa-form');
  if (!(form instanceof HTMLFormElement) || !id) return;
  document.querySelectorAll('[data-oa-tab]').forEach((node) => {
    const on = node.getAttribute('data-oa-tab') === id;
    node.classList.toggle('on', on);
    node.setAttribute('aria-selected', String(on));
  });
  document.querySelectorAll('[data-oa-pane]').forEach((node) => {
    const on = node.getAttribute('data-oa-pane') === id;
    /** @type {HTMLElement} */ (node).hidden = !on;
    node.querySelectorAll('input,select,textarea').forEach((field) => {
      /** @type {HTMLInputElement} */ (field).disabled = !on;
    });
  });
  const action = /** @type {HTMLInputElement | null} */ (form.elements.namedItem('action'));
  if (action) action.value = id;
  const confirm = /** @type {HTMLInputElement | null} */ (document.getElementById('oa-confirm'));
  if (confirm) confirm.value = '';
  const out = document.getElementById('oa-out');
  if (out) out.replaceChildren();
  void refreshObjPreview();
}
function objFormBody() {
  const form = document.getElementById('oa-form');
  if (!(form instanceof HTMLFormElement)) return new URLSearchParams();
  const body = new URLSearchParams();
  new FormData(form).forEach((value, key) => body.append(key, String(value)));
  return body;
}
function csrfToken() {
  return document.querySelector('meta[name="csrf-token"]')?.getAttribute('content') ?? '';
}
/** @type {{ destructive: boolean, confirm: string }} */
let objPlan = { destructive: false, confirm: '' };
async function refreshObjPreview() {
  const pre = document.getElementById('oa-sql');
  if (!pre) return;
  const warn = document.getElementById('oa-warn');
  const row = document.getElementById('oa-confirm-row');
  const run = document.getElementById('oa-run');
  try {
    const response = await fetch('/object/actions/preview', {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': csrfToken() },
      body: objFormBody(),
    });
    const data = await response.json();
    if (!data.ok) {
      pre.textContent = '—';
      objPlan = { destructive: false, confirm: '' };
      if (warn) {
        warn.hidden = false;
        warn.className = 'oa-need';
        warn.textContent = data.error ?? 'This action cannot run yet.';
      }
      if (row) row.hidden = true;
      if (run) run.setAttribute('disabled', '');
      return;
    }
    pre.textContent = data.sql;
    objPlan = { destructive: Boolean(data.destructive), confirm: data.confirm ?? '' };
    if (warn) {
      warn.className = 'notice denied oa-warn';
      warn.hidden = !data.warning;
      warn.textContent = data.warning ? `⚠ ${data.warning}` : '';
    }
    if (row) row.hidden = !data.destructive;
    const label = document.getElementById('oa-confirm-label');
    if (label) label.textContent = `Type ${data.confirm} to confirm`;
    if (run) {
      run.removeAttribute('disabled');
      run.classList.toggle('dg', Boolean(data.destructive));
      run.textContent = data.destructive ? 'Run (destructive)' : 'Run';
    }
  } catch {
    pre.textContent = 'The preview could not be loaded.';
  }
}
async function runObjAction() {
  const out = document.getElementById('oa-out');
  const body = objFormBody();
  if (objPlan.destructive && (body.get('confirm') ?? '').trim() !== objPlan.confirm) {
    if (out) out.textContent = `Type ${objPlan.confirm} to confirm.`;
    document.getElementById('oa-confirm')?.focus();
    return;
  }
  const sql = body.get('sql') ?? '';
  const readish =
    body.get('action') === 'sql' &&
    /^\s*(select|with|explain|show|values|table|insert|update|delete)\b/i.test(sql);
  const run = document.getElementById('oa-run');
  run?.setAttribute('disabled', '');
  try {
    if (readish) {
      const response = await fetch('/sql/execute', {
        method: 'POST',
        headers: {
          'content-type': 'application/x-www-form-urlencoded',
          'x-csrf-token': csrfToken(),
        },
        body: new URLSearchParams({ sql, profile: body.get('profile') ?? '' }),
      });
      if (out) out.innerHTML = await response.text();
      decorateIcons(document);
      if (/^\s*(insert|update|delete)/i.test(sql)) refreshTreeAfterObjectChange();
      return;
    }
    const response = await fetch('/object/actions/run', {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': csrfToken() },
      body,
    });
    const data = await response.json().catch(() => ({ ok: false, error: 'Unexpected response' }));
    if (!data.ok) {
      if (out) {
        out.replaceChildren();
        const note = document.createElement('div');
        note.className = 'notice denied';
        note.textContent = data.error ?? 'The statement failed.';
        out.append(note);
      }
      return;
    }
    toast('Done');
    closeModal();
    refreshTreeAfterObjectChange();
  } finally {
    run?.removeAttribute('disabled');
  }
}
function refreshTreeAfterObjectChange() {
  const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
  fetchInto(`/catalog/schemas${profile}`, '#tree', 'innerHTML');
  setMainHint('The schema changed. Pick a table from the sidebar.');
}
document.addEventListener(
  'click',
  (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const more = target.closest('[data-obj-menu]');
    if (more instanceof HTMLElement) {
      event.preventDefault();
      event.stopImmediatePropagation();
      const same =
        objMenu &&
        objMenu.dataset.owner === `${more.dataset.schema ?? ''}${more.dataset.rel ?? ''}`;
      closeObjMenu();
      if (!same) {
        openObjMenu(more);
        if (objMenu) objMenu.dataset.owner = `${more.dataset.schema}${more.dataset.rel ?? ''}`;
      }
      return;
    }
    const dbMore = target.closest('[data-db-more]');
    if (dbMore instanceof HTMLElement) {
      event.preventDefault();
      event.stopImmediatePropagation();
      closePops();
      void openObjDialog(dbMore, dbMore.dataset.tab ?? 'drop-database');
      return;
    }
    const direct = target.closest('[data-create-database], [data-new-schema]');
    if (direct instanceof HTMLElement) {
      event.preventDefault();
      event.stopImmediatePropagation();
      closePops();
      const schema = direct.hasAttribute('data-new-schema');
      const opener = document.createElement('button');
      opener.dataset.scope = 'database';
      opener.dataset.database = '';
      opener.dataset.profile = direct.dataset.profile ?? String(win.cuadbProfile || '');
      void openObjDialog(opener, schema ? 'create-schema' : 'create-database');
      return;
    }
    if (objMenu && !target.closest('.obj-menu')) closeObjMenu();
    const tab = target.closest('[data-oa-tab]');
    if (tab instanceof HTMLElement) {
      activateObjTab(tab.dataset.oaTab ?? '');
      return;
    }
    if (target.closest('[data-oa-run]')) {
      void runObjAction();
      return;
    }
    if (target.closest('[data-oa-copy]')) {
      void copyText(document.getElementById('oa-sql')?.textContent ?? '', 'SQL copied');
    }
  },
  true,
);
document.addEventListener('input', (event) => {
  if (event.target instanceof Element && event.target.closest('#oa-form')) {
    clearTimeout(win.cuadbOaTimer);
    win.cuadbOaTimer = setTimeout(() => void refreshObjPreview(), 180);
  }
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && objMenu) closeObjMenu();
});

/* ------------------------------------------------------------ SQL editor helpers */
const SQL_KEYWORDS = (
  'select from where and or not in is null like ilike between exists distinct group by order having limit offset ' +
  'join inner left right full outer cross on using as union all intersect except case when then else end ' +
  'insert into values update set delete create table alter drop add column rename to index view unique primary key ' +
  'foreign references default constraint check begin commit rollback explain analyze with returning asc desc truncate cascade if'
).split(' ');
const SQL_FUNCTIONS =
  'count sum avg min max coalesce nullif lower upper length trim substr substring concat now date round cast abs'.split(
    ' ',
  );
/** @type {{ tables: Array<{ s: string, n: string, c: Array<{ n: string, t: string }> }> } | null} */
let sqlHints = null;
let sqlHintsFor = '';
/** @type {string[]} */
let sqlSuggestions = [];
let sqlSelected = 0;

function escapeHtml(/** @type {string} */ text) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
function highlightSql(/** @type {string} */ text) {
  const token =
    /(--[^\n]*|\/\*[\s\S]*?(?:\*\/|$))|('(?:[^']|'')*'?)|("(?:[^"]|"")*"?|`[^`]*`?)|(\b\d+(?:\.\d+)?\b)|(\b[A-Za-z_][A-Za-z0-9_]*\b)/g;
  let out = '';
  let last = 0;
  for (const match of text.matchAll(token)) {
    const index = match.index ?? 0;
    out += escapeHtml(text.slice(last, index));
    const [all, comment, str, quoted, num, word] = match;
    const body = escapeHtml(all);
    if (comment) out += `<span class="c">${body}</span>`;
    else if (str) out += `<span class="s">${body}</span>`;
    else if (quoted) out += `<span class="q">${body}</span>`;
    else if (num) out += `<span class="n">${body}</span>`;
    else if (word && SQL_KEYWORDS.includes(word.toLowerCase()))
      out += `<span class="k">${body}</span>`;
    else if (word && SQL_FUNCTIONS.includes(word.toLowerCase()) && text[index + all.length] === '(')
      out += `<span class="f">${body}</span>`;
    else out += body;
    last = index + all.length;
  }
  return out + escapeHtml(text.slice(last)) + '\n';
}
function syncSqlHighlight() {
  const area = document.getElementById('sql');
  const layer = document.getElementById('sqlhl');
  if (!(area instanceof HTMLTextAreaElement) || !layer) return;
  layer.innerHTML = highlightSql(area.value);
  layer.scrollTop = area.scrollTop;
  layer.scrollLeft = area.scrollLeft;
}
async function loadSqlHints() {
  const profile = String(win.cuadbProfile || '');
  if (sqlHints && sqlHintsFor === profile) return;
  sqlHintsFor = profile;
  try {
    const response = await fetch(
      `/sql/hints${profile ? `?profile=${encodeURIComponent(profile)}` : ''}`,
    );
    sqlHints = response.ok ? await response.json() : { tables: [] };
  } catch {
    sqlHints = { tables: [] };
  }
  fillSqlBuilder();
}
function sqlQuote(/** @type {string} */ name) {
  return /^[a-z_][a-z0-9_]*$/.test(name) ? name : `"${name.replace(/"/g, '""')}"`;
}
function sqlCandidates(/** @type {string} */ word, /** @type {string} */ text) {
  const lower = word.toLowerCase();
  const tables = sqlHints?.tables ?? [];
  /** @type {string[]} */
  let pool = [];
  const dot = word.lastIndexOf('.');
  if (dot > 0) {
    const owner = word.slice(0, dot).replace(/"/g, '').toLowerCase();
    for (const t of tables) {
      if (t.s.toLowerCase() === owner) pool.push(`${word.slice(0, dot)}.${sqlQuote(t.n)}`);
      if (t.n.toLowerCase() === owner)
        for (const c of t.c) pool.push(`${word.slice(0, dot)}.${sqlQuote(c.n)}`);
    }
    // alias.column: find "from <table> alias" in the text
    const alias = new RegExp(`(?:from|join)\\s+([\\w."]+)\\s+(?:as\\s+)?${owner}\\b`, 'i').exec(
      text,
    );
    if (alias) {
      const name = (alias[1] ?? '').replace(/"/g, '').split('.').pop()?.toLowerCase();
      for (const t of tables)
        if (t.n.toLowerCase() === name)
          for (const c of t.c) pool.push(`${word.slice(0, dot)}.${sqlQuote(c.n)}`);
    }
  } else {
    pool = [...SQL_KEYWORDS.map((k) => k.toUpperCase())];
    for (const t of tables) pool.push(sqlQuote(t.n), `${sqlQuote(t.s)}.${sqlQuote(t.n)}`);
    const mentioned = tables.filter((t) => new RegExp(`\\b${t.n}\\b`, 'i').test(text));
    for (const t of mentioned.length ? mentioned : tables.slice(0, 20))
      for (const c of t.c) pool.push(sqlQuote(c.n));
  }
  const seen = new Set();
  return pool
    .filter((item) => item.toLowerCase().startsWith(lower) && item.toLowerCase() !== lower)
    .filter((item) => (seen.has(item) ? false : (seen.add(item), true)))
    .slice(0, 10);
}
function currentSqlWord(/** @type {HTMLTextAreaElement} */ area) {
  const before = area.value.slice(0, area.selectionStart);
  const match = /[A-Za-z0-9_."]*$/.exec(before);
  return match ? match[0] : '';
}
function hideSqlSuggestions() {
  const box = document.getElementById('sqlsug');
  if (box) box.hidden = true;
  sqlSuggestions = [];
}
function showSqlSuggestions(force = false) {
  const area = document.getElementById('sql');
  const box = document.getElementById('sqlsug');
  if (!(area instanceof HTMLTextAreaElement) || !box) return;
  const word = currentSqlWord(area);
  if (!word && !force) return hideSqlSuggestions();
  if (word.length < 1 && !force) return hideSqlSuggestions();
  sqlSuggestions = sqlCandidates(word, area.value);
  if (!sqlSuggestions.length) return hideSqlSuggestions();
  sqlSelected = 0;
  box.replaceChildren();
  sqlSuggestions.forEach((item, index) => {
    const li = document.createElement('li');
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', String(index === 0));
    li.dataset.i = String(index);
    const name = document.createElement('span');
    name.textContent = item;
    const kind = document.createElement('small');
    kind.textContent = SQL_KEYWORDS.includes(item.toLowerCase())
      ? 'keyword'
      : item.includes('.')
        ? 'name'
        : 'name';
    li.append(name, kind);
    box.append(li);
  });
  // Place near the caret line (approximate: last line of text before the caret).
  const lines = area.value.slice(0, area.selectionStart).split('\n').length;
  box.style.top = `${Math.min(10 + lines * 18, area.clientHeight)}px`;
  box.style.left = '16px';
  box.hidden = false;
}
function acceptSqlSuggestion(/** @type {number} */ index) {
  const area = document.getElementById('sql');
  if (!(area instanceof HTMLTextAreaElement)) return;
  const pick = sqlSuggestions[index];
  if (!pick) return;
  const word = currentSqlWord(area);
  const start = area.selectionStart - word.length;
  area.setRangeText(pick, start, area.selectionStart, 'end');
  hideSqlSuggestions();
  syncSqlHighlight();
  area.focus();
}
function moveSqlSelection(/** @type {number} */ delta) {
  const box = document.getElementById('sqlsug');
  if (!box || !sqlSuggestions.length) return;
  sqlSelected = (sqlSelected + delta + sqlSuggestions.length) % sqlSuggestions.length;
  box
    .querySelectorAll('li')
    .forEach((li, index) => li.setAttribute('aria-selected', String(index === sqlSelected)));
  box.querySelectorAll('li')[sqlSelected]?.scrollIntoView({ block: 'nearest' });
}
function insertSql(/** @type {string} */ text, replace = false) {
  const area = document.getElementById('sql');
  if (!(area instanceof HTMLTextAreaElement)) return;
  if (replace || !area.value.trim()) area.value = text;
  else area.value = `${area.value.replace(/\s+$/, '')}\n\n${text}`;
  hideSqlSuggestions();
  syncSqlHighlight();
  area.focus();
}
function formatSql(/** @type {string} */ text) {
  const major =
    /\b(from|where|group by|order by|having|limit|offset|union all|union|returning|values|set)\b/gi;
  const joins = /\b((?:left |right |full |inner |cross )?(?:outer )?join)\b/gi;
  return text
    .replace(/\s+/g, ' ')
    .trim()
    .replace(major, (m) => `\n${m.toUpperCase()}`)
    .replace(joins, (m) => `\n${m.toUpperCase()}`)
    .replace(
      /\b(select|insert into|update|delete from|create table|alter table|drop table)\b/gi,
      (m) => m.toUpperCase(),
    )
    .replace(/\b(and|or)\b/gi, (m) => `\n  ${m.toUpperCase()}`)
    .replace(/^\n/, '');
}
function firstTable() {
  return sqlHints?.tables[0];
}
function sqlSnippet(/** @type {string} */ kind) {
  const t = firstTable();
  const name = t ? `${sqlQuote(t.s)}.${sqlQuote(t.n)}` : 'my_table';
  const cols = t?.c.map((c) => sqlQuote(c.n)) ?? ['id', 'name'];
  const second = sqlHints?.tables[1];
  const other = second ? `${sqlQuote(second.s)}.${sqlQuote(second.n)}` : 'other_table';
  switch (kind) {
    case 'select':
      return `SELECT ${cols.slice(0, 5).join(', ')}\nFROM ${name}\nWHERE 1 = 1\nORDER BY 1\nLIMIT 100;`;
    case 'count':
      return `SELECT COUNT(*) AS total\nFROM ${name};`;
    case 'join':
      return `SELECT a.*, b.*\nFROM ${name} a\nJOIN ${other} b ON b.id = a.id\nLIMIT 100;`;
    case 'insert':
      return `INSERT INTO ${name} (${cols.slice(0, 3).join(', ')})\nVALUES (${cols
        .slice(0, 3)
        .map(() => "'…'")
        .join(', ')});`;
    case 'update':
      return `UPDATE ${name}\nSET ${cols[1] ?? 'name'} = '…'\nWHERE ${cols[0] ?? 'id'} = 1;`;
    default:
      return 'CREATE TABLE new_table (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP\n);';
  }
}
function fillSqlBuilder() {
  const select = document.getElementById('sqlb-table');
  if (!(select instanceof HTMLSelectElement)) return;
  select.replaceChildren();
  for (const t of sqlHints?.tables ?? []) {
    const option = document.createElement('option');
    option.value = `${t.s}\u0000${t.n}`;
    option.textContent = `${t.s}.${t.n}`;
    select.append(option);
  }
  renderBuilderColumns();
}
function builderTable() {
  const select = document.getElementById('sqlb-table');
  if (!(select instanceof HTMLSelectElement)) return undefined;
  const [schema, name] = select.value.split('\u0000');
  return sqlHints?.tables.find((t) => t.s === schema && t.n === name);
}
function renderBuilderColumns() {
  const holder = document.getElementById('sqlb-cols');
  const order = document.getElementById('sqlb-order');
  const t = builderTable();
  if (!holder || !(order instanceof HTMLSelectElement)) return;
  holder.replaceChildren();
  order.replaceChildren(new Option('(none)', ''));
  for (const c of t?.c ?? []) {
    const label = document.createElement('label');
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.value = c.n;
    box.checked = true;
    label.append(box, ` ${c.n}`);
    holder.append(label);
    order.append(new Option(c.n, c.n));
  }
  document.getElementById('sqlb-where')?.replaceChildren();
}
function addBuilderCondition() {
  const holder = document.getElementById('sqlb-where');
  const t = builderTable();
  if (!holder || !t) return;
  const row = document.createElement('div');
  row.className = 'sqlb-cond';
  const col = document.createElement('select');
  col.className = 'inp';
  for (const c of t.c) col.append(new Option(c.n, c.n));
  const op = document.createElement('select');
  op.className = 'inp';
  for (const o of ['=', '<>', '>', '<', '>=', '<=', 'LIKE', 'IS NULL', 'IS NOT NULL'])
    op.append(new Option(o, o));
  const val = document.createElement('input');
  val.className = 'inp';
  val.placeholder = 'value';
  row.append(col, op, val);
  holder.append(row);
}
function buildSelect() {
  const t = builderTable();
  if (!t) return '';
  const cols = Array.from(document.querySelectorAll('#sqlb-cols input:checked')).map((b) =>
    sqlQuote(/** @type {HTMLInputElement} */ (b).value),
  );
  let sql = `SELECT ${cols.length && cols.length < t.c.length ? cols.join(', ') : '*'}\nFROM ${sqlQuote(t.s)}.${sqlQuote(t.n)}`;
  const conds = Array.from(document.querySelectorAll('#sqlb-where .sqlb-cond'))
    .map((row) => {
      const [c, o, v] = /** @type {Array<HTMLSelectElement | HTMLInputElement>} */ (
        Array.from(row.children)
      );
      if (!c || !o || !v) return '';
      if (o.value.startsWith('IS ')) return `${sqlQuote(c.value)} ${o.value}`;
      if (!v.value) return '';
      const numeric = /^-?\d+(\.\d+)?$/.test(v.value);
      return `${sqlQuote(c.value)} ${o.value} ${numeric ? v.value : `'${v.value.replace(/'/g, "''")}'`}`;
    })
    .filter(Boolean);
  if (conds.length) sql += `\nWHERE ${conds.join('\n  AND ')}`;
  const order = /** @type {HTMLSelectElement | null} */ (document.getElementById('sqlb-order'))
    ?.value;
  const dir =
    /** @type {HTMLSelectElement | null} */ (document.getElementById('sqlb-dir'))?.value ?? 'ASC';
  if (order) sql += `\nORDER BY ${sqlQuote(order)} ${dir}`;
  const limit = Number(
    /** @type {HTMLInputElement | null} */ (document.getElementById('sqlb-limit'))?.value,
  );
  sql += `\nLIMIT ${Number.isFinite(limit) && limit > 0 ? Math.min(limit, 5000) : 100};`;
  return sql;
}
function initSqlEditor() {
  const area = document.getElementById('sql');
  if (!(area instanceof HTMLTextAreaElement) || area.dataset.ready) return;
  area.dataset.ready = '1';
  if (win.cuadbPrefillSql) {
    area.value = String(win.cuadbPrefillSql);
    win.cuadbPrefillSql = '';
  }
  syncSqlHighlight();
  void loadSqlHints();
  area.addEventListener('input', () => {
    syncSqlHighlight();
    showSqlSuggestions();
  });
  area.addEventListener('scroll', syncSqlHighlight);
  area.addEventListener('blur', () => setTimeout(hideSqlSuggestions, 150));
  area.addEventListener('keydown', (event) => {
    const open = !(document.getElementById('sqlsug')?.hidden ?? true);
    if (event.ctrlKey && event.code === 'Space') {
      event.preventDefault();
      showSqlSuggestions(true);
    } else if (open && event.key === 'ArrowDown') {
      event.preventDefault();
      moveSqlSelection(1);
    } else if (open && event.key === 'ArrowUp') {
      event.preventDefault();
      moveSqlSelection(-1);
    } else if (
      open &&
      (event.key === 'Tab' || (event.key === 'Enter' && !event.ctrlKey && !event.metaKey))
    ) {
      event.preventDefault();
      acceptSqlSuggestion(sqlSelected);
    } else if (open && event.key === 'Escape') {
      event.stopPropagation();
      hideSqlSuggestions();
    } else if (event.key === 'Tab' && !event.shiftKey) {
      event.preventDefault();
      area.setRangeText('  ', area.selectionStart, area.selectionEnd, 'end');
      syncSqlHighlight();
    }
  });
}
document.addEventListener('mousedown', (event) => {
  const item = event.target instanceof Element ? event.target.closest('#sqlsug li') : null;
  if (item instanceof HTMLElement) {
    event.preventDefault();
    acceptSqlSuggestion(Number(item.dataset.i));
  }
});
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const toggle = target.closest('[data-sql-builder]');
  if (toggle instanceof HTMLElement) {
    const panel = document.getElementById('sqlbuilder');
    if (!panel) return;
    panel.hidden = !panel.hidden;
    toggle.setAttribute('aria-expanded', String(!panel.hidden));
    void loadSqlHints();
    return;
  }
  const snippet = target.closest('[data-sql-snippet]');
  if (snippet instanceof HTMLElement) {
    insertSql(sqlSnippet(snippet.dataset.sqlSnippet ?? 'select'));
    return;
  }
  if (target.closest('[data-sql-format]')) {
    const area = document.getElementById('sql');
    if (area instanceof HTMLTextAreaElement && area.value.trim()) {
      area.value = formatSql(area.value);
      syncSqlHighlight();
    }
    return;
  }
  if (target.closest('[data-sqlb-add]')) return addBuilderCondition();
  if (target.closest('[data-sqlb-use]')) {
    const sql = buildSelect();
    if (sql) insertSql(sql, true);
  }
});
document.addEventListener('change', (event) => {
  if (event.target instanceof Element && event.target.id === 'sqlb-table') renderBuilderColumns();
});
document.addEventListener('htmx:afterSwap', () => initSqlEditor());
initSqlEditor();

document.addEventListener('cuadb:schema-changed', () => {
  const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
  fetchInto(`/catalog/schemas${profile}`, '#tree', 'innerHTML');
});

/* ------------------------------------------------------------ grid cells, headers, filters */
const ICON_PATHS = {
  key: '<circle cx="8" cy="15" r="4"/><path d="M10.8 12.2 20 3m-3 3 3 3"/>',
  link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  cal: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>',
  hash: '<path d="M5 9h14M5 15h14M10 4 8 20M16 4l-2 16"/>',
  text: '<path d="M5 6h14M12 6v13M9 19h6"/>',
  json: '<path d="M9 4C6 4 7 8 5 12c2 4 1 8 4 8M15 4c3 0 2 4 4 8-2 4-1 8-4 8"/>',
  toggle: '<rect x="3" y="8" width="18" height="8" rx="4"/><circle cx="8" cy="12" r="2"/>',
  list: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
  id: '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h4M7 14h10"/>',
  code: '<path d="m8 8-4 4 4 4M16 8l4 4-4 4"/>',
  table: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18M3 15h18M9 4v16"/>',
};
function iconSvg(/** @type {keyof typeof ICON_PATHS} */ name) {
  const span = document.createElement('span');
  span.className = 'hico';
  span.setAttribute('aria-hidden', 'true');
  span.innerHTML = `<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${ICON_PATHS[name]}</svg>`;
  return span;
}
const TLDS =
  'com|org|net|io|dev|app|co|ai|edu|gov|me|info|biz|xyz|cloud|tech|ke|za|ng|uk|de|fr|us|ca|au|in|jp|cn|ru|br';
const URL_RE = new RegExp(
  `^(https?://[^\\s<>"']+|www\\.[^\\s<>"']+|[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*\\.(?:${TLDS})(?::\\d+)?(?:/[^\\s<>"']*)?)$`,
  'i',
);
function safeHref(/** @type {string} */ text) {
  if (!URL_RE.test(text)) return '';
  try {
    const url = new URL(/^https?:\/\//i.test(text) ? text : `https://${text}`);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : '';
  } catch {
    return '';
  }
}
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
function simpleType(/** @type {string} */ native, /** @type {boolean} */ isEnum) {
  if (isEnum) return 'enum';
  const t = native
    .toLowerCase()
    .replace(/\(.*\)/, '')
    .trim();
  /** @type {Record<string, string>} */
  const map = {
    'character varying': 'varchar',
    character: 'char',
    'timestamp with time zone': 'timestamptz',
    'timestamp without time zone': 'timestamp',
    'time without time zone': 'time',
    'time with time zone': 'timetz',
    'double precision': 'float8',
    integer: 'int',
    'user-defined': 'enum',
    boolean: 'boolean',
  };
  return map[t] ?? (t || 'unknown');
}
function columnKind(
  /** @type {string} */ name,
  /** @type {string} */ type,
  /** @type {boolean} */ pk,
  /** @type {boolean} */ isEnum,
  /** @type {boolean} */ numeric,
) {
  const lname = name.toLowerCase();
  if (type === 'boolean' || type === 'bool') return 'bool';
  if (isEnum) return 'enum';
  if (/(^|_)code$/.test(lname)) return 'code';
  if (type === 'uuid' || pk || lname === 'id' || /_id$/.test(lname)) return 'id';
  if (/timestamp|^date$|^time/.test(type)) return 'date';
  if (type.includes('json')) return 'json';
  if (numeric) return 'num';
  return 'text';
}
const KIND_ICON = /** @type {Record<string, keyof typeof ICON_PATHS>} */ ({
  bool: 'toggle',
  enum: 'list',
  code: 'code',
  id: 'id',
  date: 'cal',
  json: 'json',
  num: 'hash',
  text: 'text',
});
function applyBool(/** @type {Element} */ el) {
  const raw = (el.getAttribute('data-raw') ?? el.textContent ?? '').toLowerCase();
  const yes = raw === 'true' || raw === 't';
  const mode = String(S.boolFmt || 'dot');
  el.className = `bv ${yes ? 'y' : 'n'}`;
  el.setAttribute('data-fmt', mode);
  el.replaceChildren();
  if (mode === 'dot') {
    const dot = document.createElement('i');
    dot.className = 'bdot';
    dot.setAttribute('aria-hidden', 'true');
    const label = document.createElement('span');
    label.className = 'sr-only';
    label.textContent = yes ? 'true' : 'false';
    el.append(dot, label);
    el.setAttribute('title', yes ? 'true' : 'false');
  } else {
    el.textContent =
      mode === 'caps'
        ? yes
          ? 'TRUE'
          : 'FALSE'
        : mode === 'yesno'
          ? yes
            ? 'Yes'
            : 'No'
          : yes
            ? 'true'
            : 'false';
    el.removeAttribute('title');
  }
}
/** Rich, display-only cells: links, ids, codes, booleans, enums and typed headers. */
function decorateGrid(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('table.grid').forEach((node) => {
    const table = /** @type {HTMLTableElement} */ (node);
    const ths = Array.from(table.querySelectorAll('thead th'));
    const rows = Array.from(table.querySelectorAll('tbody tr[data-row]'));
    ths.forEach((th, index) => {
      if (!(th instanceof HTMLElement) || !th.dataset.col) return;
      const name = th.dataset.col;
      const type = (th.dataset.type ?? '').toLowerCase();
      const cells = rows
        .map((tr) => tr.children[index])
        .filter((cell) => cell instanceof HTMLElement);
      const isEnum = cells.some((cell) => cell.querySelector('.ev'));
      const isFk = cells.some((cell) => cell.querySelector('a.fk'));
      const kind = columnKind(
        name,
        type,
        th.dataset.pk === '1',
        isEnum,
        th.classList.contains('num'),
      );
      th.dataset.kind = kind;
      th.dataset.fk = isFk ? '1' : '0';
      // Header: [icon name ........ type  sort] with both ends flush to the cell edges.
      const iconName = th.dataset.pk === '1' ? 'key' : isFk ? 'link' : KIND_ICON[kind];
      const small = th.querySelector('small');
      if (small && !th.querySelector('.tyb')) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'tyb';
        button.dataset.native = small.textContent ?? '';
        button.textContent = simpleType(small.textContent ?? '', isEnum);
        small.replaceWith(button);
      } else if (small === null) {
        const button = th.querySelector('.tyb');
        if (button instanceof HTMLElement && isEnum) button.textContent = 'enum';
      }
      if (!th.querySelector('.thw')) {
        const left = document.createElement('div');
        left.className = 'thl';
        const right = document.createElement('div');
        right.className = 'thr';
        const wrap = document.createElement('div');
        wrap.className = 'thw';
        wrap.append(left, right);
        for (const child of Array.from(th.childNodes)) {
          if (child instanceof Element && child.classList.contains('rzh')) continue;
          if (child instanceof Element && child.classList.contains('tyb')) right.append(child);
          else if (child instanceof Element && child.classList.contains('s')) child.remove();
          else if (child instanceof Text && !child.textContent?.trim()) child.remove();
          else left.append(child);
        }
        const sind = document.createElement('span');
        sind.className = 'sind';
        right.append(sind);
        th.prepend(wrap);
      }
      const thl = th.querySelector('.thl');
      thl?.querySelector('.hico')?.remove();
      if (iconName && thl) {
        const icon = iconSvg(iconName);
        icon.dataset.k = th.dataset.pk === '1' ? 'pk' : isFk ? 'fk' : kind;
        thl.prepend(icon);
      }
      const sortState = th.getAttribute('aria-sort');
      const sind = th.querySelector('.sind');
      if (sind) {
        sind.textContent = sortState === 'ascending' ? '↑' : sortState === 'descending' ? '↓' : '';
        sind.classList.toggle('on', sortState === 'ascending' || sortState === 'descending');
      }
      cells.forEach((cell) => {
        if (!(cell instanceof HTMLElement) || cell.classList.contains('rn')) return;
        cell.classList.toggle('id-col', kind === 'id');
        cell.classList.toggle('cd-col', kind === 'code');
        cell.querySelectorAll('.bv').forEach(applyBool);
        if (cell.querySelector('[data-null]') || cell.querySelector('.truncation')) return;
        const only = cell.children.length === 1 ? cell.firstElementChild : null;
        const plainCode = only?.tagName === 'CODE' && !only.className;
        if (cell.dataset.orig === undefined) {
          if (cell.children.length > 0 && !plainCode) return;
          cell.dataset.orig = cell.textContent ?? '';
        }
        const text = cell.dataset.orig;
        if (!text) return;
        if (
          cell.classList.contains('numeric') ||
          kind === 'num' ||
          kind === 'bool' ||
          kind === 'json'
        ) {
          return;
        }
        cell.replaceChildren();
        if (kind === 'code') {
          const code = document.createElement('code');
          code.className = 'cd';
          code.textContent = text;
          cell.append(code);
          return;
        }
        if (kind === 'id' && UUID_RE.test(text)) {
          const id = document.createElement('span');
          id.className = 'idv';
          id.title = text;
          id.textContent = S.idFmt === 'full' ? text : `${text.slice(0, 8)}…`;
          cell.append(id);
          return;
        }
        const href = S.links === false || kind === 'date' ? '' : safeHref(text.trim());
        if (href) {
          const a = document.createElement('a');
          a.className = 'url';
          a.href = href;
          a.target = '_blank';
          a.rel = 'noopener noreferrer';
          a.textContent = text;
          cell.append(a);
          return;
        }
        cell.textContent = text;
      });
    });
  });
}
document.addEventListener(
  'click',
  (event) => {
    const target = event.target;
    if (target instanceof Element && target.closest('a.url')) event.stopPropagation();
  },
  true,
);

/* column type dropdown */
/** @type {HTMLElement | null} */
let typeTip = null;
/** @type {HTMLElement | null} */
let typeTipOwner = null;
let typeTipPinned = false;
function hideTypeTip() {
  typeTip?.remove();
  typeTip = null;
  typeTipOwner = null;
  typeTipPinned = false;
}
function showTypeTip(/** @type {HTMLElement} */ button, pin = false) {
  if (typeTipOwner === button && !pin) return;
  hideTypeTip();
  const th = button.closest('th');
  if (!(th instanceof HTMLElement)) return;
  const tip = document.createElement('div');
  tip.className = 'tyinfo';
  tip.setAttribute('role', 'tooltip');
  const key = th.dataset.pk === '1' ? 'Primary key' : th.dataset.fk === '1' ? 'Foreign key' : '—';
  const rowsData = [
    ['Column', th.dataset.col ?? ''],
    ['Type', button.dataset.native || button.textContent || ''],
    ['Shown as', th.dataset.kind ?? ''],
    ['Nullable', th.dataset.nullable ? 'yes' : 'no'],
    ['Key', key],
    ['Required on insert', th.dataset.required ? 'yes' : 'no'],
    ['Editable', th.dataset.editable ? 'yes' : 'read-only'],
  ];
  for (const [label, value] of rowsData) {
    const row = document.createElement('div');
    const a = document.createElement('span');
    a.textContent = label ?? '';
    const b = document.createElement('b');
    b.textContent = value ?? '';
    row.append(a, b);
    tip.append(row);
  }
  document.body.append(tip);
  const rect = button.getBoundingClientRect();
  tip.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - tip.offsetWidth - 8))}px`;
  tip.style.top = `${Math.min(rect.bottom + 4, innerHeight - tip.offsetHeight - 8)}px`;
  typeTip = tip;
  typeTipOwner = button;
  typeTipPinned = pin;
}
document.addEventListener('mouseover', (event) => {
  const button = event.target instanceof Element ? event.target.closest('.tyb') : null;
  if (button instanceof HTMLElement) showTypeTip(button);
  else if (
    !typeTipPinned &&
    typeTip &&
    !(event.target instanceof Element && event.target.closest('.tyinfo'))
  )
    hideTypeTip();
});
document.addEventListener(
  'click',
  (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest('.tyb');
    if (button instanceof HTMLElement) {
      event.preventDefault();
      event.stopPropagation();
      if (typeTipOwner === button && typeTipPinned) hideTypeTip();
      else showTypeTip(button, true);
      return;
    }
    // Anywhere else in a header cell sorts the column.
    const th = target.closest('th[data-col]');
    if (
      th instanceof HTMLElement &&
      !target.closest('a.sortlink, .rzh, input, button') &&
      th.closest('table.grid')
    ) {
      th.querySelector('a.sortlink')?.dispatchEvent(
        new MouseEvent('click', { bubbles: true, cancelable: true }),
      );
      return;
    }
    if (typeTip && !target.closest('.tyinfo')) hideTypeTip();
  },
  true,
);
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') hideTypeTip();
});

/* filters panel (applies to the rows loaded on this page; "Open as SQL" covers the whole table) */
/** @type {Array<{ col: number, kind: string, name: string, el: HTMLInputElement | HTMLSelectElement, el2?: HTMLInputElement }>} */
let filterControls = [];
function buildFilterPanel() {
  const pop = document.getElementById('fpop');
  const table = document.querySelector('#catalog-panel table.grid');
  if (!pop || !(table instanceof HTMLTableElement)) return;
  filterControls = [];
  pop.replaceChildren();
  const head = document.createElement('div');
  head.className = 'fp-h';
  head.innerHTML = '<b>Filters</b><span class="mu">rows loaded on this page</span>';
  pop.append(head);
  const list = document.createElement('div');
  list.className = 'fp-list';
  const rows = Array.from(table.querySelectorAll('tbody tr[data-row]'));
  const ths = Array.from(table.querySelectorAll('thead th'));
  ths.forEach((th, index) => {
    if (!(th instanceof HTMLElement) || !th.dataset.col) return;
    const kind = th.dataset.kind ?? 'text';
    const distinct = new Set();
    rows.forEach((tr) => {
      const cell = tr.children[index];
      if (cell instanceof HTMLElement && !cell.querySelector('[data-null]'))
        distinct.add(cell.dataset.detail ?? '');
    });
    const row = document.createElement('label');
    row.className = 'fp-row';
    const label = document.createElement('span');
    label.className = 'fp-l';
    label.textContent = th.dataset.col;
    row.append(label);
    /** @type {HTMLInputElement | HTMLSelectElement} */
    let control;
    /** @type {HTMLInputElement | undefined} */
    let second;
    let mode = 'text';
    if (kind === 'bool') {
      mode = 'bool';
      const select = document.createElement('select');
      select.className = 'inp';
      select.append(
        new Option('Any', ''),
        new Option('True', 'true'),
        new Option('False', 'false'),
      );
      control = select;
    } else if (
      (kind === 'enum' ||
        (kind === 'text' && distinct.size > 1 && distinct.size <= 12 && rows.length >= 5)) &&
      distinct.size <= 40
    ) {
      mode = 'eq';
      const select = document.createElement('select');
      select.className = 'inp';
      select.append(new Option('Any', ''));
      Array.from(distinct)
        .sort()
        .forEach((value) => select.append(new Option(String(value), String(value))));
      control = select;
    } else if (
      kind === 'num' ||
      (kind === 'id' &&
        !UUID_RE.test(String(Array.from(distinct)[0] ?? '')) &&
        distinct.size &&
        !Number.isNaN(Number(Array.from(distinct)[0])))
    ) {
      mode = 'range';
      const a = document.createElement('input');
      a.className = 'inp';
      a.type = 'number';
      a.placeholder = 'min';
      second = document.createElement('input');
      second.className = 'inp';
      second.type = 'number';
      second.placeholder = 'max';
      control = a;
    } else if (kind === 'date') {
      mode = 'date';
      const a = document.createElement('input');
      a.className = 'inp';
      a.type = 'date';
      second = document.createElement('input');
      second.className = 'inp';
      second.type = 'date';
      control = a;
    } else {
      const input = document.createElement('input');
      input.className = 'inp';
      input.type = 'search';
      input.placeholder = 'contains…';
      control = input;
    }
    const holder = document.createElement('span');
    holder.className = 'fp-c';
    holder.append(control);
    if (second) holder.append(second);
    row.append(holder);
    list.append(row);
    filterControls.push({ col: index, kind: mode, name: th.dataset.col, el: control, el2: second });
  });
  pop.append(list);
  const foot = document.createElement('div');
  foot.className = 'fp-f';
  foot.innerHTML =
    '<span class="fp-n mu" id="fp-n"></span><span class="sp"></span><button class="btn" type="button" data-fp-clear>Clear</button>';
  if (document.querySelector('.app')?.getAttribute('data-sql') === 'true') {
    const sql = document.createElement('button');
    sql.className = 'btn on';
    sql.type = 'button';
    sql.setAttribute('data-fp-sql', '');
    sql.textContent = 'Open as SQL';
    sql.title = 'Run this filter against the whole table in the SQL workspace';
    foot.append(sql);
  }
  pop.append(foot);
  applyFilters();
}
function filterValue(
  /** @type {{ el: HTMLInputElement | HTMLSelectElement, el2?: HTMLInputElement }} */ c,
) {
  return { a: c.el.value.trim(), b: c.el2?.value.trim() ?? '' };
}
function applyFilters() {
  const table = document.querySelector('#catalog-panel table.grid');
  if (!(table instanceof HTMLTableElement)) return;
  const active = filterControls.filter((c) => {
    const v = filterValue(c);
    return v.a !== '' || v.b !== '';
  });
  const rows = Array.from(table.querySelectorAll('tbody tr[data-row]'));
  let shown = 0;
  for (const tr of rows) {
    let ok = true;
    for (const c of active) {
      const cell = tr.children[c.col];
      const detail =
        cell instanceof HTMLElement && !cell.querySelector('[data-null]')
          ? (cell.dataset.detail ?? '')
          : null;
      const v = filterValue(c);
      if (detail === null) {
        ok = false;
        break;
      }
      if (c.kind === 'bool') ok = (v.a === 'true') === (detail === 'true' || detail === 't');
      else if (c.kind === 'eq') ok = detail === v.a;
      else if (c.kind === 'range') {
        const n = Number(detail);
        ok =
          !Number.isNaN(n) && (v.a === '' || n >= Number(v.a)) && (v.b === '' || n <= Number(v.b));
      } else if (c.kind === 'date') {
        const day = detail.slice(0, 10);
        ok = (v.a === '' || day >= v.a) && (v.b === '' || day <= v.b);
      } else ok = detail.toLowerCase().includes(v.a.toLowerCase());
      if (!ok) break;
    }
    /** @type {HTMLElement} */ (tr).hidden = !ok;
    if (ok) shown += 1;
  }
  const counter = document.getElementById('fp-n');
  if (counter)
    counter.textContent = active.length
      ? `${shown} of ${rows.length} rows match`
      : `${rows.length} rows`;
  document.querySelector('[data-pop="fpop"]')?.classList.toggle('on', active.length > 0);
}
function filtersAsSql() {
  const panel = document.getElementById('catalog-panel');
  const qualified = (panel?.dataset.table ?? '').split('.');
  const table =
    qualified.length >= 2
      ? `${sqlQuote(qualified[0] ?? '')}.${sqlQuote(qualified.slice(1).join('.'))}`
      : sqlQuote(qualified[0] ?? '');
  const lit = (/** @type {string} */ v) => `'${v.replace(/'/g, "''")}'`;
  const conds = [];
  for (const c of filterControls) {
    const v = filterValue(c);
    if (v.a === '' && v.b === '') continue;
    const col = sqlQuote(c.name);
    if (c.kind === 'bool') conds.push(`${col} IS ${v.a === 'true' ? 'TRUE' : 'FALSE'}`);
    else if (c.kind === 'eq') conds.push(`${col} = ${lit(v.a)}`);
    else if (c.kind === 'range') {
      if (v.a !== '') conds.push(`${col} >= ${Number(v.a)}`);
      if (v.b !== '') conds.push(`${col} <= ${Number(v.b)}`);
    } else if (c.kind === 'date') {
      if (v.a !== '') conds.push(`${col} >= ${lit(v.a)}`);
      if (v.b !== '')
        conds.push(
          `${col} < ${lit(new Date(Date.parse(v.b) + 86400000).toISOString().slice(0, 10))}`,
        );
    } else conds.push(`CAST(${col} AS TEXT) LIKE ${lit(`%${v.a}%`)}`);
  }
  return `SELECT *\nFROM ${table}${conds.length ? `\nWHERE ${conds.join('\n  AND ')}` : ''}\nLIMIT 100;`;
}
document.addEventListener(
  'click',
  (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (target.closest('[data-pop="fpop"]')) {
      buildFilterPanel();
      return;
    }
    if (target.closest('[data-fp-clear]')) {
      filterControls.forEach((c) => {
        c.el.value = '';
        if (c.el2) c.el2.value = '';
      });
      applyFilters();
      return;
    }
    if (target.closest('[data-fp-sql]')) {
      win.cuadbPrefillSql = filtersAsSql();
      closePops();
      const open = document.getElementById('sqlb');
      if (open) open.click();
      else void copyText(win.cuadbPrefillSql, 'SQL copied');
    }
  },
  true,
);
document.addEventListener('input', (event) => {
  if (event.target instanceof Element && event.target.closest('#fpop')) applyFilters();
});
document.addEventListener('change', (event) => {
  if (event.target instanceof Element && event.target.closest('#fpop')) applyFilters();
});

/* overlay horizontal scrollbar (shows on hover, takes no layout space) */
function initHScroll(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('.wrap > .gw').forEach((node) => {
    const gw = /** @type {HTMLElement} */ (node);
    const wrap = gw.parentElement;
    if (!wrap || gw.dataset.hs) return;
    gw.dataset.hs = '1';
    wrap.classList.add('hs-wrap');
    const bar = document.createElement('div');
    bar.className = 'hbar';
    const thumb = document.createElement('div');
    thumb.className = 'hthumb';
    bar.append(thumb);
    wrap.append(bar);
    const update = () => {
      const overflow = gw.scrollWidth - gw.clientWidth;
      bar.hidden = overflow <= 1;
      if (bar.hidden) return;
      const track = bar.clientWidth;
      const size = Math.max(36, (gw.clientWidth / gw.scrollWidth) * track);
      const left = (gw.scrollLeft / overflow) * (track - size);
      thumb.style.width = `${size}px`;
      thumb.style.transform = `translateX(${left}px)`;
    };
    gw.addEventListener('scroll', update, { passive: true });
    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(update);
      observer.observe(gw);
      const table = gw.querySelector('table');
      if (table) observer.observe(table);
    }
    thumb.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      thumb.setPointerCapture(event.pointerId);
      bar.classList.add('drag');
      const startX = event.clientX;
      const startLeft = gw.scrollLeft;
      const ratio =
        (gw.scrollWidth - gw.clientWidth) / Math.max(1, bar.clientWidth - thumb.offsetWidth);
      const move = (/** @type {PointerEvent} */ e) => {
        gw.scrollLeft = startLeft + (e.clientX - startX) * ratio;
      };
      const stop = () => {
        bar.classList.remove('drag');
        thumb.removeEventListener('pointermove', move);
        thumb.removeEventListener('pointerup', stop);
        thumb.removeEventListener('pointercancel', stop);
      };
      thumb.addEventListener('pointermove', move);
      thumb.addEventListener('pointerup', stop);
      thumb.addEventListener('pointercancel', stop);
    });
    bar.addEventListener('pointerdown', (event) => {
      if (event.target !== bar) return;
      const rect = thumb.getBoundingClientRect();
      gw.scrollLeft += event.clientX < rect.left ? -gw.clientWidth * 0.8 : gw.clientWidth * 0.8;
    });
    update();
  });
}

/* SQL runner output: copy text, CSV/JSON, and copyable errors */
function tableText(
  /** @type {HTMLTableElement} */ table,
  /** @type {'csv' | 'tsv' | 'json'} */ format,
) {
  const heads = Array.from(table.querySelectorAll('thead th:not(.rn)')).map(
    (th) => th.textContent?.trim() ?? '',
  );
  const rows = Array.from(table.querySelectorAll('tbody tr')).map((tr) =>
    Array.from(tr.querySelectorAll('td:not(.rn)')).map((td) =>
      td.querySelector('[data-null]') ? null : (td.textContent ?? ''),
    ),
  );
  if (format === 'json') {
    return JSON.stringify(
      rows.map((r) => Object.fromEntries(heads.map((h, i) => [h, r[i] ?? null]))),
      null,
      2,
    );
  }
  const cell = (/** @type {string | null} */ v) => {
    const text = v ?? '';
    if (format === 'tsv') return text.replace(/\t/g, ' ').replace(/\r?\n/g, ' ');
    return /[",\n\r]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
  };
  const sep = format === 'tsv' ? '\t' : ',';
  return [heads.map(cell).join(sep), ...rows.map((r) => r.map(cell).join(sep))].join('\n');
}
function decorateSqlOutput(/** @type {Element} */ out) {
  out
    .querySelectorAll('.notice.error, .notice.denied, .notice.refused, .cuadb-error')
    .forEach((box) => {
      if (box.querySelector('.copy-err')) return;
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'btn copy-err';
      button.textContent = 'Copy error';
      button.addEventListener(
        'click',
        () =>
          void copyText((box.textContent ?? '').replace('Copy error', '').trim(), 'Error copied'),
      );
      box.append(button);
    });
  out.querySelectorAll('.sqlstep, #sql-result').forEach((step) => {
    const table = step.querySelector('table.grid');
    if (!(table instanceof HTMLTableElement) || step.querySelector('.res-tools')) return;
    const tools = document.createElement('div');
    tools.className = 'res-tools';
    for (const [label, format] of /** @type {Array<[string, 'csv' | 'tsv' | 'json']>} */ ([
      ['Copy CSV', 'csv'],
      ['Copy TSV', 'tsv'],
      ['Copy JSON', 'json'],
    ])) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'btn';
      button.textContent = label;
      button.addEventListener(
        'click',
        () => void copyText(tableText(table, format), `${label.replace('Copy ', '')} copied`),
      );
      tools.append(button);
    }
    const count = table.querySelectorAll('tbody tr').length;
    const info = document.createElement('span');
    info.className = 'mu';
    info.textContent = `${count} row${count === 1 ? '' : 's'}`;
    tools.prepend(info);
    table.closest('.gw')?.before(tools);
  });
  if (!out.querySelector('.out-all') && out.textContent?.trim()) {
    const all = document.createElement('button');
    all.type = 'button';
    all.className = 'btn out-all';
    all.textContent = 'Copy all output';
    all.addEventListener('click', () => {
      const parts = [];
      out
        .querySelectorAll('.sql-head, .sqlh')
        .forEach((h) => parts.push(h.textContent?.trim() ?? ''));
      out.querySelectorAll('.sqlstep').forEach((step, i) => {
        parts.push(`-- ${i + 1}. ${step.querySelector('.sqlstep-h code')?.textContent ?? ''}`);
        const err = step.querySelector('.notice');
        if (err) parts.push(err.textContent?.replace('Copy error', '').trim() ?? '');
        const table = step.querySelector('table.grid');
        if (table instanceof HTMLTableElement) parts.push(tableText(table, 'tsv'));
      });
      const single = out.querySelector('#sql-result > .gw table.grid');
      if (single instanceof HTMLTableElement) parts.push(tableText(single, 'tsv'));
      void copyText(parts.filter(Boolean).join('\n\n'), 'Output copied');
    });
    out.prepend(all);
  }
}
document.addEventListener('htmx:afterSwap', (event) => {
  const target = event.target;
  if (target instanceof Element && (target.id === 'sqlout' || target.id === 'oa-out'))
    decorateSqlOutput(target);
  formatDates(document);
  decorateGrid(document);
  initHScroll(document);
});
document.addEventListener('htmx:afterSettle', () => {
  decorateGrid(document);
  initHScroll(document);
});
gridReady = true;
decorateGrid(document);
initHScroll(document);

/* real connection checks: the dot and database name reflect what is actually connected */
let connTimer = 0;
async function checkConnection(retries = 3) {
  const dot = document.getElementById('cdot');
  const cname = document.getElementById('cname');
  const dname = document.getElementById('dname');
  const button = document.getElementById('cbtn');
  if (!dot || !button) return;
  dot.classList.remove('off');
  dot.classList.add('pend');
  button.setAttribute('title', 'Checking the connection…');
  try {
    const base = String(win.cuadbProfile || '').split('~')[0];
    const response = await fetch(
      `/connection/profiles${base ? `?profile=${encodeURIComponent(base)}` : ''}`,
      {
        cache: 'no-store',
      },
    );
    if (!response.ok) throw new Error('status');
    const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
    const selected = doc.querySelector('[data-profile][aria-selected="true"]');
    if (!(selected instanceof HTMLElement)) throw new Error('none');
    const state = selected.dataset.state ?? '';
    if (/checking/i.test(state) && retries > 0) {
      setTimeout(() => void checkConnection(retries - 1), 1500);
      return;
    }
    const ready = selected.dataset.ready === 'true';
    dot.classList.remove('pend');
    dot.classList.toggle('off', !ready);
    button.classList.toggle('bad', !ready);
    if (cname) cname.textContent = selected.dataset.label ?? cname.textContent ?? 'Connection';
    button.setAttribute(
      'title',
      `${selected.dataset.label ?? 'Connection'} · ${state || (ready ? 'Connected' : 'Not connected')}`,
    );
    if (dname) {
      const derived = String(win.cuadbProfile || '').includes('~') ? currentDb.db : '';
      dname.textContent = derived || selected.dataset.db || 'database';
      dname.parentElement?.classList.toggle('bad', !ready);
      dname.parentElement?.setAttribute(
        'title',
        ready ? `Connected to ${dname.textContent}` : 'Not connected',
      );
    }
  } catch {
    dot.classList.remove('pend');
    dot.classList.add('off');
    button.classList.add('bad');
    button.setAttribute('title', 'The server could not be reached');
  }
}
function scheduleConnectionChecks() {
  clearInterval(connTimer);
  connTimer = window.setInterval(() => {
    if (!document.hidden) void checkConnection(1);
  }, 20000);
}
scheduleConnectionChecks();
window.addEventListener('focus', () => void checkConnection(1));
setTimeout(() => void checkConnection(), 600);

/* ------------------------------------------------------------ row sheet building blocks */
function relativeFrom(/** @type {Date} */ when) {
  const seconds = (when.getTime() - Date.now()) / 1000;
  const units = /** @type {Array<[Intl.RelativeTimeFormatUnit, number]>} */ ([
    ['year', 31536000],
    ['month', 2592000],
    ['week', 604800],
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
  ]);
  const pick = units.find(([, size]) => Math.abs(seconds) >= size);
  const fmt = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
  return pick
    ? fmt.format(Math.round(seconds / pick[1]), pick[0])
    : fmt.format(Math.round(seconds), 'second');
}
function sheetTitle(
  /** @type {string} */ main,
  /** @type {string} */ sub,
  /** @type {'table' | 'link'} */ icon,
) {
  const box = document.createElement('span');
  box.className = 'sh-title';
  const mark = iconSvg(icon);
  mark.classList.add('sh-mark');
  const text = document.createElement('span');
  text.className = 'sh-text';
  const strong = document.createElement('b');
  strong.textContent = main;
  const small = document.createElement('small');
  small.textContent = sub;
  text.append(strong, small);
  box.append(mark, text);
  return box;
}
/**
 * One field of the sheet: name on the left edge, type and tools on the right edge,
 * the value shown below in a form that fits its kind.
 */
function sheetField(
  /** @type {{ name: string, type: string, native: string, kind: string, pk: boolean, fk: boolean, value: HTMLElement, tools: HTMLElement[] }} */ f,
) {
  const kv = document.createElement('div');
  kv.className = 'kv kvf';
  kv.dataset.kind = f.kind;
  const head = document.createElement('div');
  head.className = 'kvh';
  const left = document.createElement('span');
  left.className = 'kvn';
  const iconName = f.pk ? 'key' : f.fk ? 'link' : (KIND_ICON[f.kind] ?? 'text');
  const icon = iconSvg(iconName);
  icon.dataset.k = f.pk ? 'pk' : f.fk ? 'fk' : f.kind;
  const name = document.createElement('b');
  name.textContent = f.name;
  name.title = f.name;
  left.append(icon, name);
  const right = document.createElement('span');
  right.className = 'kvr';
  const type = document.createElement('span');
  type.className = 'kvt';
  type.textContent = f.type;
  type.title = f.native;
  right.append(...f.tools, type);
  head.append(left, right);
  kv.append(head, f.value);
  return kv;
}
function sheetValue(
  /** @type {string} */ kind,
  /** @type {string} */ text,
  /** @type {boolean} */ isNull,
  /** @type {string} */ native,
  /** @type {HTMLAnchorElement | null} */ fkLink,
) {
  const box = document.createElement('div');
  box.className = `kvv kv-${kind}`;
  if (isNull) {
    const none = document.createElement('span');
    none.className = 'kvnull';
    none.textContent = 'NULL';
    box.append(none);
    return box;
  }
  if (fkLink) {
    const a = document.createElement('a');
    a.className = 'fk kvfk';
    a.href = fkLink.getAttribute('href') ?? '#';
    a.title = 'Open the referenced row';
    const label = document.createElement('span');
    label.textContent = text;
    const arrow = document.createElement('i');
    arrow.textContent = '→';
    a.append(label, arrow);
    box.append(a);
    return box;
  }
  if (kind === 'bool') {
    const yes = ['true', 't'].includes(text.toLowerCase());
    const mode = String(S.boolFmt || 'dot');
    const el = document.createElement('span');
    el.className = `bv ${yes ? 'y' : 'n'} kvbool`;
    el.setAttribute('data-fmt', mode);
    if (mode === 'dot' || mode === 'text') {
      const dot = document.createElement('i');
      dot.className = 'bdot';
      el.append(dot, document.createTextNode(yes ? 'true' : 'false'));
      el.setAttribute('data-fmt', 'dot');
    } else {
      el.textContent = mode === 'caps' ? (yes ? 'TRUE' : 'FALSE') : yes ? 'Yes' : 'No';
    }
    box.append(el);
    return box;
  }
  if (kind === 'enum') {
    const el = document.createElement('span');
    el.className = 'ev kvenum';
    el.textContent = text;
    box.append(el);
    return box;
  }
  if (kind === 'id') {
    const el = document.createElement('span');
    el.className = 'kvid';
    el.textContent = text;
    box.append(el);
    return box;
  }
  if (kind === 'code') {
    const el = document.createElement('code');
    el.className = 'cd kvcode';
    el.textContent = text;
    box.append(el);
    return box;
  }
  if (kind === 'date') {
    const when = new Date(text.replace(' ', 'T').replace(/([+-]\d\d)$/, '$1:00'));
    if (!Number.isNaN(when.getTime()) && /\d{4}-\d\d-\d\d/.test(text)) {
      const main = document.createElement('span');
      main.className = 'kvdate';
      main.textContent = when.toLocaleString(undefined, { dateStyle: 'full', timeStyle: 'medium' });
      const sub = document.createElement('small');
      sub.className = 'kvsub';
      sub.textContent = `${relativeFrom(when)} · ${text}`;
      box.append(main, sub);
      return box;
    }
  }
  if (kind === 'json' || /json/i.test(native)) {
    const pre = document.createElement('pre');
    pre.className = 'kvj';
    try {
      pre.textContent = JSON.stringify(JSON.parse(text), null, 2);
    } catch {
      pre.textContent = text;
    }
    box.append(pre);
    return box;
  }
  if (kind === 'num') {
    const el = document.createElement('span');
    el.className = 'kvnum';
    el.textContent = text;
    box.append(el);
    return box;
  }
  const href = S.links === false ? '' : safeHref(text.trim());
  if (href) {
    const a = document.createElement('a');
    a.className = 'url kvurl';
    a.href = href;
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
    a.textContent = text;
    box.append(a);
    return box;
  }
  if (text.includes('\n') || text.length > 90) {
    const pre = document.createElement('pre');
    pre.className = 'kvtext';
    pre.textContent = text;
    box.append(pre);
    return box;
  }
  const el = document.createElement('span');
  el.className = 'kvtext';
  el.textContent = text;
  box.append(el);
  return box;
}

/* ------------------------------------------------------------ draggable sheet width */
function initSheetResize() {
  const sheet = document.getElementById('insp');
  if (!sheet || document.getElementById('insp-rs')) return;
  const parent = sheet.parentElement;
  if (!parent) return;
  try {
    const saved = Number(localStorage.getItem('cuadb:shw'));
    if (saved >= 300 && saved <= 1200)
      document.documentElement.style.setProperty('--shw', `${saved}px`);
  } catch {}
  const handle = document.createElement('div');
  handle.id = 'insp-rs';
  handle.setAttribute('role', 'separator');
  handle.setAttribute('aria-orientation', 'vertical');
  handle.setAttribute('aria-label', 'Resize details sheet');
  handle.title = 'Drag to resize · double-click to reset';
  handle.hidden = !sheet.classList.contains('show');
  sheet.before(handle);
  new MutationObserver(() => {
    handle.hidden = !sheet.classList.contains('show');
  }).observe(sheet, { attributes: true, attributeFilter: ['class'] });
  const setWidth = (/** @type {number} */ width, /** @type {boolean} */ persist) => {
    const max = Math.max(320, Math.min(1200, parent.clientWidth - 120));
    const clamped = Math.round(Math.max(300, Math.min(max, width)));
    document.documentElement.style.setProperty('--shw', `${clamped}px`);
    if (persist) {
      try {
        localStorage.setItem('cuadb:shw', String(clamped));
      } catch {}
    }
    return clamped;
  };
  handle.addEventListener('pointerdown', (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add('drag');
    document.documentElement.classList.add('sheet-drag');
    const right = parent.getBoundingClientRect().right;
    let last = 0;
    const move = (/** @type {PointerEvent} */ e) => {
      last = setWidth(right - e.clientX, false);
    };
    const stop = () => {
      handle.classList.remove('drag');
      document.documentElement.classList.remove('sheet-drag');
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', stop);
      handle.removeEventListener('pointercancel', stop);
      if (last) setWidth(last, true);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', stop);
    handle.addEventListener('pointercancel', stop);
  });
  handle.addEventListener('dblclick', () => {
    document.documentElement.style.removeProperty('--shw');
    try {
      localStorage.removeItem('cuadb:shw');
    } catch {}
  });
  handle.addEventListener('keydown', () => {});
}
initSheetResize();
document.addEventListener('htmx:afterSwap', initSheetResize);

/* ------------------------------------------------------------ custom selects */
/** @type {{ select: HTMLSelectElement, menu: HTMLElement, button: HTMLElement } | null} */
let openSelect = null;
const CHEVRON =
  '<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>';
function closeSelectMenu() {
  if (!openSelect) return;
  openSelect.menu.remove();
  openSelect.button.setAttribute('aria-expanded', 'false');
  openSelect = null;
}
function selectLabel(/** @type {HTMLSelectElement} */ select) {
  const option = select.options[select.selectedIndex];
  return option ? option.textContent || '' : '';
}
function syncCustomSelect(/** @type {HTMLSelectElement} */ select) {
  const wrap = select.nextElementSibling;
  if (!(wrap instanceof HTMLElement) || !wrap.classList.contains('cs')) return;
  const label = wrap.querySelector('.cs-v');
  if (label) {
    const text = selectLabel(select);
    label.textContent = text;
    label.classList.toggle('ph', !text);
  }
  const button = wrap.querySelector('.cs-btn');
  if (button instanceof HTMLButtonElement) button.disabled = select.disabled;
}
function chooseOption(/** @type {HTMLSelectElement} */ select, /** @type {number} */ index) {
  if (index < 0 || index >= select.options.length || select.options[index]?.disabled) return;
  const changed = select.selectedIndex !== index;
  select.selectedIndex = index;
  syncCustomSelect(select);
  if (changed) {
    select.dispatchEvent(new Event('input', { bubbles: true }));
    select.dispatchEvent(new Event('change', { bubbles: true }));
  }
}
function openCustomSelect(
  /** @type {HTMLSelectElement} */ select,
  /** @type {HTMLElement} */ button,
) {
  closeSelectMenu();
  if (select.disabled) return;
  const menu = document.createElement('div');
  menu.className = 'cs-menu';
  menu.setAttribute('role', 'listbox');
  const rect = button.getBoundingClientRect();
  menu.style.minWidth = `${Math.max(rect.width, 120)}px`;
  let active = select.selectedIndex;
  const rows = /** @type {HTMLElement[]} */ ([]);
  Array.from(select.options).forEach((option, index) => {
    const row = document.createElement('div');
    row.className = 'cs-opt';
    row.setAttribute('role', 'option');
    row.setAttribute('aria-selected', String(index === select.selectedIndex));
    row.dataset.i = String(index);
    if (option.disabled) row.classList.add('dis');
    const text = document.createElement('span');
    text.textContent = option.textContent || '';
    row.append(text);
    if (index === select.selectedIndex) {
      const check = document.createElement('i');
      check.className = 'cs-ck';
      check.innerHTML =
        '<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m5 12 5 5 9-10"/></svg>';
      row.append(check);
    }
    rows.push(row);
    menu.append(row);
  });
  document.body.append(menu);
  const place = () => {
    const below = innerHeight - rect.bottom - 8;
    const above = rect.top - 8;
    const height = Math.min(menu.scrollHeight, 320);
    const up = below < height && above > below;
    menu.style.maxHeight = `${Math.max(120, Math.min(320, up ? above : below))}px`;
    menu.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - menu.offsetWidth - 8))}px`;
    menu.style.top = up
      ? `${Math.max(8, rect.top - menu.offsetHeight - 4)}px`
      : `${rect.bottom + 4}px`;
  };
  place();
  const highlight = (/** @type {number} */ index) => {
    active = index;
    rows.forEach((r, i) => r.classList.toggle('act', i === index));
    rows[index]?.scrollIntoView({ block: 'nearest' });
  };
  highlight(Math.max(0, active));
  menu.addEventListener('mousedown', (event) => event.preventDefault());
  menu.addEventListener('click', (event) => {
    const row = event.target instanceof Element ? event.target.closest('.cs-opt') : null;
    if (!(row instanceof HTMLElement)) return;
    chooseOption(select, Number(row.dataset.i));
    closeSelectMenu();
    button.focus();
  });
  menu.addEventListener('mousemove', (event) => {
    const row = event.target instanceof Element ? event.target.closest('.cs-opt') : null;
    if (row instanceof HTMLElement) highlight(Number(row.dataset.i));
  });
  menu.addEventListener('keydown', () => {});
  button.setAttribute('aria-expanded', 'true');
  openSelect = { select, menu, button };
  const keys = (/** @type {KeyboardEvent} */ event) => {
    if (!openSelect || openSelect.menu !== menu) {
      document.removeEventListener('keydown', keys, true);
      return;
    }
    const last = rows.length - 1;
    if (event.key === 'ArrowDown') highlight(Math.min(last, active + 1));
    else if (event.key === 'ArrowUp') highlight(Math.max(0, active - 1));
    else if (event.key === 'Home') highlight(0);
    else if (event.key === 'End') highlight(last);
    else if (event.key === 'Enter' || event.key === ' ') {
      chooseOption(select, active);
      closeSelectMenu();
      button.focus();
    } else if (event.key === 'Escape' || event.key === 'Tab') {
      closeSelectMenu();
      if (event.key === 'Escape') button.focus();
      document.removeEventListener('keydown', keys, true);
      if (event.key === 'Escape') event.stopPropagation();
      return;
    } else if (event.key.length === 1) {
      const needle = event.key.toLowerCase();
      const from = rows.findIndex(
        (_, i) =>
          i > active && (select.options[i]?.textContent ?? '').toLowerCase().startsWith(needle),
      );
      const any =
        from >= 0
          ? from
          : rows.findIndex((_, i) =>
              (select.options[i]?.textContent ?? '').toLowerCase().startsWith(needle),
            );
      if (any >= 0) highlight(any);
      else return;
    } else return;
    event.preventDefault();
    event.stopPropagation();
  };
  document.addEventListener('keydown', keys, true);
}
function enhanceSelect(/** @type {HTMLSelectElement} */ select) {
  if (select.dataset.cs || select.multiple || select.size > 1) return;
  select.dataset.cs = '1';
  const wrap = document.createElement('span');
  wrap.className = `cs ${select.classList.contains('sel') ? 'cs-sm' : ''}`;
  const button = document.createElement('button');
  button.type = 'button';
  button.className = `cs-btn ${Array.from(select.classList).join(' ')}`;
  button.setAttribute('aria-haspopup', 'listbox');
  button.setAttribute('aria-expanded', 'false');
  const label = select.getAttribute('aria-label');
  if (label) button.setAttribute('aria-label', label);
  const value = document.createElement('span');
  value.className = 'cs-v';
  const arrow = document.createElement('span');
  arrow.className = 'cs-ar';
  arrow.innerHTML = CHEVRON;
  button.append(value, arrow);
  wrap.append(button);
  select.classList.add('cs-native');
  select.tabIndex = -1;
  select.after(wrap);
  // Code that assigns `.value` (restoring a form, syncing settings) must update the control.
  const desc = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
  if (desc?.set && desc.get) {
    const { get, set } = desc;
    Object.defineProperty(select, 'value', {
      configurable: true,
      get() {
        return get.call(this);
      },
      set(v) {
        set.call(this, v);
        syncCustomSelect(this);
      },
    });
  }
  new MutationObserver(() => syncCustomSelect(select)).observe(select, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ['disabled'],
  });
  button.addEventListener('click', () => {
    if (openSelect?.select === select) closeSelectMenu();
    else openCustomSelect(select, button);
  });
  button.addEventListener('keydown', (event) => {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
      event.preventDefault();
      openCustomSelect(select, button);
    }
  });
  select.addEventListener('change', () => syncCustomSelect(select));
  syncCustomSelect(select);
}
function enhanceSelects(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('select:not([data-cs])').forEach((node) => {
    if (node instanceof HTMLSelectElement) enhanceSelect(node);
  });
}
document.addEventListener('mousedown', (event) => {
  if (openSelect && event.target instanceof Element && !event.target.closest('.cs-menu, .cs-btn'))
    closeSelectMenu();
});
window.addEventListener('resize', () => closeSelectMenu());
document.addEventListener(
  'scroll',
  (event) => {
    if (openSelect && !(event.target instanceof Node && openSelect.menu.contains(event.target)))
      closeSelectMenu();
  },
  true,
);

/* ------------------------------------------------------------ custom calendar */
/** @type {{ input: HTMLInputElement, el: HTMLElement } | null} */
let openCalendar = null;
const DAY_MS = 86400000;
function isoDay(/** @type {Date} */ d) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
function parseDay(/** @type {string} */ text) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text.trim());
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isNaN(d.getTime()) ? null : d;
}
function closeCalendar() {
  openCalendar?.el.remove();
  openCalendar = null;
}
function openDatePicker(/** @type {HTMLInputElement} */ input) {
  if (openCalendar?.input === input) return;
  closeCalendar();
  const el = document.createElement('div');
  el.className = 'dp';
  el.setAttribute('role', 'dialog');
  el.setAttribute('aria-label', 'Choose a date');
  document.body.append(el);
  const selected = parseDay(input.value);
  let view = new Date(
    (selected ?? new Date()).getFullYear(),
    (selected ?? new Date()).getMonth(),
    1,
  );
  const locale = navigator.language || 'en';
  let first = 0;
  try {
    // @ts-ignore weekInfo is not in every lib.dom version
    const info = new Intl.Locale(locale).weekInfo ?? new Intl.Locale(locale).getWeekInfo?.();
    first = info ? info.firstDay % 7 : 0;
  } catch {
    first = 0;
  }
  const render = () => {
    el.replaceChildren();
    const head = document.createElement('div');
    head.className = 'dp-h';
    const prev = document.createElement('button');
    prev.type = 'button';
    prev.className = 'dp-nav';
    prev.setAttribute('aria-label', 'Previous month');
    prev.textContent = '‹';
    const next = document.createElement('button');
    next.type = 'button';
    next.className = 'dp-nav';
    next.setAttribute('aria-label', 'Next month');
    next.textContent = '›';
    const title = document.createElement('button');
    title.type = 'button';
    title.className = 'dp-title';
    title.textContent = view.toLocaleDateString(locale, { month: 'long', year: 'numeric' });
    head.append(prev, title, next);
    el.append(head);
    prev.addEventListener('click', () => {
      view = new Date(view.getFullYear(), view.getMonth() - 1, 1);
      render();
    });
    next.addEventListener('click', () => {
      view = new Date(view.getFullYear(), view.getMonth() + 1, 1);
      render();
    });
    title.addEventListener('click', () => {
      // Jump a year ahead on repeated clicks of the title: quick year changes.
      view = new Date(view.getFullYear() + 1, view.getMonth(), 1);
      render();
    });
    const grid = document.createElement('div');
    grid.className = 'dp-g';
    for (let i = 0; i < 7; i += 1) {
      const dow = document.createElement('span');
      dow.className = 'dp-w';
      const day = new Date(2023, 0, 1 + ((first + i) % 7));
      dow.textContent = day.toLocaleDateString(locale, { weekday: 'narrow' });
      grid.append(dow);
    }
    const start = new Date(view.getFullYear(), view.getMonth(), 1);
    const offset = (start.getDay() - first + 7) % 7;
    const cursor = new Date(start.getTime() - offset * DAY_MS);
    const todayIso = isoDay(new Date());
    for (let i = 0; i < 42; i += 1) {
      const d = new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() + i);
      const cell = document.createElement('button');
      cell.type = 'button';
      cell.className = 'dp-d';
      if (d.getMonth() !== view.getMonth()) cell.classList.add('out');
      const iso = isoDay(d);
      if (iso === todayIso) cell.classList.add('today');
      if (selected && iso === isoDay(selected)) cell.classList.add('sel');
      cell.textContent = String(d.getDate());
      cell.setAttribute('aria-label', d.toLocaleDateString(locale, { dateStyle: 'full' }));
      cell.addEventListener('click', () => {
        input.value = iso;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        closeCalendar();
        input.focus();
      });
      grid.append(cell);
    }
    el.append(grid);
    const foot = document.createElement('div');
    foot.className = 'dp-f';
    for (const [label, action] of /** @type {Array<[string, () => void]>} */ ([
      [
        'Today',
        () => {
          input.value = isoDay(new Date());
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
          closeCalendar();
        },
      ],
      [
        'Clear',
        () => {
          input.value = '';
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
          closeCalendar();
        },
      ],
    ])) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'btn';
      b.textContent = label;
      b.addEventListener('click', action);
      foot.append(b);
    }
    el.append(foot);
  };
  render();
  const rect = input.getBoundingClientRect();
  const place = () => {
    const h = el.offsetHeight;
    const below = innerHeight - rect.bottom;
    const top = below >= h + 8 || rect.top < h ? rect.bottom + 4 : rect.top - h - 4;
    el.style.top = `${Math.max(8, Math.min(top, innerHeight - h - 8))}px`;
    el.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - el.offsetWidth - 8))}px`;
  };
  place();
  openCalendar = { input, el };
}
function enhanceDate(/** @type {HTMLInputElement} */ input) {
  if (input.dataset.dp) return;
  input.dataset.dp = '1';
  const value = input.value;
  input.type = 'text';
  input.value = value;
  input.placeholder = input.placeholder || 'YYYY-MM-DD';
  input.autocomplete = 'off';
  input.inputMode = 'numeric';
  input.classList.add('dp-input');
  input.addEventListener('focus', () => openDatePicker(input));
  input.addEventListener('click', () => openDatePicker(input));
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && openCalendar) {
      event.stopPropagation();
      closeCalendar();
    } else if (event.key === 'ArrowDown') openDatePicker(input);
  });
}
function enhanceDates(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('input[type="date"]:not([data-dp])').forEach((node) => {
    if (node instanceof HTMLInputElement) enhanceDate(node);
  });
}
document.addEventListener('mousedown', (event) => {
  if (
    openCalendar &&
    event.target instanceof Element &&
    !event.target.closest('.dp') &&
    event.target !== openCalendar.input
  )
    closeCalendar();
});
window.addEventListener('resize', closeCalendar);

/* enhance what is on the page now and everything added later */
function enhanceControls(/** @type {ParentNode} */ root = document) {
  enhanceSelects(root);
  enhanceDates(root);
}
enhanceControls(document);
new MutationObserver((records) => {
  for (const record of records) {
    record.addedNodes.forEach((node) => {
      if (
        node instanceof HTMLSelectElement ||
        node instanceof HTMLInputElement ||
        node instanceof Element
      )
        enhanceControls(node instanceof Element ? node : document);
      if (node instanceof HTMLSelectElement) enhanceSelect(node);
      if (node instanceof HTMLInputElement && node.type === 'date') enhanceDate(node);
    });
  }
}).observe(document.body, { childList: true, subtree: true });

/* ------------------------------------------------------------ editor: key lookups and value suggestions */
/** @type {{ input: HTMLInputElement | HTMLTextAreaElement, menu: HTMLElement } | null} */
let lookupOpen = null;
let lookupTimer = 0;
let lookupSeq = 0;
function lookupClose() {
  if (!lookupOpen) return;
  lookupOpen.menu.remove();
  lookupOpen.input.setAttribute('aria-expanded', 'false');
  lookupOpen = null;
}
function lookupUrl(/** @type {Record<string, string>} */ extra) {
  const panel = document.getElementById('catalog-panel');
  const params = new URLSearchParams(extra);
  if (panel?.dataset.prof) params.set('profile', panel.dataset.prof);
  return `/data/lookup?${params}`;
}
function lookupShow(
  /** @type {HTMLInputElement | HTMLTextAreaElement} */ input,
  /** @type {Array<{ value: string, label?: string }>} */ items,
  /** @type {(value: string, label: string) => void} */ pick,
) {
  lookupClose();
  if (!items.length) return;
  const menu = document.createElement('div');
  menu.className = 'cs-menu lk-menu';
  menu.setAttribute('role', 'listbox');
  const rect = input.getBoundingClientRect();
  menu.style.minWidth = `${rect.width}px`;
  menu.style.maxWidth = `${Math.max(rect.width, 360)}px`;
  let active = 0;
  const rows = items.map((item, index) => {
    const row = document.createElement('div');
    row.className = 'cs-opt lk-opt';
    row.setAttribute('role', 'option');
    const value = document.createElement('b');
    value.textContent = item.value;
    row.append(value);
    if (item.label) {
      const label = document.createElement('small');
      label.textContent = item.label;
      row.append(label);
    }
    row.addEventListener('mousedown', (event) => {
      event.preventDefault();
      pick(item.value, item.label ?? '');
      lookupClose();
    });
    row.addEventListener('mousemove', () => {
      active = index;
      rows.forEach((r, i) => r.classList.toggle('act', i === active));
    });
    menu.append(row);
    return row;
  });
  rows[0]?.classList.add('act');
  document.body.append(menu);
  const below = innerHeight - rect.bottom - 8;
  const height = Math.min(menu.scrollHeight, 260);
  const up = below < height && rect.top > below;
  menu.style.maxHeight = `${Math.max(100, Math.min(260, up ? rect.top - 8 : below))}px`;
  menu.style.left = `${Math.max(8, Math.min(rect.left, innerWidth - menu.offsetWidth - 8))}px`;
  menu.style.top = up
    ? `${Math.max(8, rect.top - menu.offsetHeight - 4)}px`
    : `${rect.bottom + 4}px`;
  input.setAttribute('aria-expanded', 'true');
  lookupOpen = { input, menu };
  const host = /** @type {HTMLElement} */ (input);
  /** @param {KeyboardEvent} event */
  const keys = (event) => {
    if (!lookupOpen || lookupOpen.menu !== menu) return host.removeEventListener('keydown', keys);
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      active = (active + (event.key === 'ArrowDown' ? 1 : -1) + rows.length) % rows.length;
      rows.forEach((r, i) => r.classList.toggle('act', i === active));
      rows[active]?.scrollIntoView({ block: 'nearest' });
      event.preventDefault();
    } else if (event.key === 'Enter' && !(input instanceof HTMLTextAreaElement && !event.ctrlKey)) {
      const item = items[active];
      if (item) {
        pick(item.value, item.label ?? '');
        event.preventDefault();
      }
      lookupClose();
    } else if (event.key === 'Escape') {
      lookupClose();
      event.stopPropagation();
    }
  };
  host.addEventListener('keydown', keys);
}
function lookupState(
  /** @type {HTMLElement} */ box,
  /** @type {string} */ state,
  /** @type {string} */ text,
) {
  box.dataset.state = state;
  const hint = box.querySelector('.lk-hint');
  if (hint) hint.textContent = text;
}
async function lookupFk(/** @type {HTMLInputElement} */ input, /** @type {boolean} */ verify) {
  const box = input.closest('.lk');
  if (!(box instanceof HTMLElement)) return;
  const { lkSchema = '', lkRelation = '', lkColumn = '' } = box.dataset;
  const seq = (lookupSeq += 1);
  const q = input.value.trim();
  const target = `${lkSchema}.${lkRelation}`;
  if (!q) {
    lookupState(box, '', `References ${target}(${lkColumn})`);
    if (!verify) {
      // Empty search: show a few rows of the referenced table to pick from.
    } else return;
  }
  try {
    const response = await fetch(
      lookupUrl({
        mode: 'fk',
        schema: lkSchema,
        relation: lkRelation,
        column: lkColumn,
        q,
        exact: verify ? '1' : '0',
      }),
    );
    if (seq !== lookupSeq) return;
    if (!response.ok) throw new Error('lookup');
    const data = await response.json();
    if (verify) {
      if (data.found) lookupState(box, 'ok', `✓ ${target}${data.label ? ` · ${data.label}` : ''}`);
      else
        lookupState(box, 'bad', `No row in ${target} has ${lkColumn} “${q}”. Search and pick one.`);
      return;
    }
    if (document.activeElement !== input) return;
    lookupShow(input, data.items ?? [], (value, label) => {
      input.value = value;
      lookupState(box, 'ok', `✓ ${target}${label ? ` · ${label}` : ''}`);
      input.dispatchEvent(new Event('change', { bubbles: true }));
    });
    if (q && !(data.items ?? []).length)
      lookupState(box, 'bad', `Nothing in ${target} matches “${q}”.`);
  } catch {
    lookupState(box, '', 'Search is unavailable right now.');
  }
}
async function lookupValues(/** @type {HTMLInputElement | HTMLTextAreaElement} */ input) {
  const panel = document.getElementById('catalog-panel');
  const q = input.value.trim();
  if (!panel || !q || input.disabled || input.readOnly) return lookupClose();
  const seq = (lookupSeq += 1);
  try {
    const response = await fetch(
      lookupUrl({
        mode: 'values',
        schema: panel.dataset.schema ?? '',
        relation: panel.dataset.relation ?? '',
        column: input.name,
        q,
      }),
    );
    if (seq !== lookupSeq || !response.ok) return;
    const data = await response.json();
    if (document.activeElement !== input) return;
    const items = /** @type {string[]} */ (data.items ?? []).filter((v) => v !== input.value);
    lookupShow(
      input,
      items.map((value) => ({ value })),
      (value) => {
        input.value = value;
        input.dispatchEvent(new Event('input', { bubbles: true }));
      },
    );
  } catch {
    /* suggestions are optional */
  }
}
document.addEventListener('input', (event) => {
  const el = event.target;
  if (
    !(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) ||
    !el.closest('#row-form')
  )
    return;
  const lk = el.closest('.lk');
  clearTimeout(lookupTimer);
  if (lk instanceof HTMLElement && el instanceof HTMLInputElement) {
    lookupState(lk, '', 'Searching…');
    lookupTimer = window.setTimeout(() => void lookupFk(el, false), 180);
    return;
  }
  if (el.hasAttribute('data-suggest')) {
    lookupTimer = window.setTimeout(() => void lookupValues(el), 200);
  }
});
document.addEventListener(
  'focusin',
  (event) => {
    const el = event.target;
    if (el instanceof HTMLInputElement && el.closest('.lk') && el.closest('#row-form') && !el.value)
      void lookupFk(el, false);
  },
  true,
);
document.addEventListener(
  'focusout',
  (event) => {
    const el = event.target;
    if (el instanceof HTMLInputElement && el.closest('.lk') && el.closest('#row-form')) {
      setTimeout(() => {
        if (lookupOpen?.input === el) lookupClose();
        if (el.value.trim()) void lookupFk(el, true);
        else {
          const box = el.closest('.lk');
          if (box instanceof HTMLElement)
            lookupState(
              box,
              '',
              `References ${box.dataset.lkSchema}.${box.dataset.lkRelation}(${box.dataset.lkColumn})`,
            );
        }
      }, 120);
    } else if (lookupOpen && el === lookupOpen.input) setTimeout(lookupClose, 120);
  },
  true,
);
document.addEventListener('mousedown', (event) => {
  if (
    lookupOpen &&
    event.target instanceof Element &&
    !event.target.closest('.lk-menu') &&
    event.target !== lookupOpen.input
  )
    lookupClose();
});
// When the editor opens on an existing row, verify the current foreign-key values once.
document.addEventListener('htmx:afterSettle', (event) => {
  if (!(event.target instanceof Element) || event.target.id !== 'insp') return;
  setTimeout(() => {
    document.querySelectorAll('#row-form .lk input').forEach((input) => {
      if (input instanceof HTMLInputElement && input.value.trim()) void lookupFk(input, true);
    });
  }, 400);
});

/* ------------------------------------------------------------ connection wizard */
function connForm() {
  const form = document.getElementById('conn-form');
  return form instanceof HTMLFormElement ? form : null;
}
function applyConnMethod(/** @type {HTMLFormElement} */ form) {
  const sqlite = form.dataset.engine === 'sqlite';
  const method = form.dataset.method === 'url' ? 'url' : 'details';
  const off = (/** @type {string} */ selector, /** @type {boolean} */ disabled) =>
    form.querySelectorAll(selector).forEach((el) => {
      /** @type {HTMLInputElement} */ (el).disabled = disabled;
    });
  off('input[name="url"]', sqlite || method !== 'url');
  off(
    'input[name="host"],input[name="port"],input[name="user"],input[name="password"],input[name="database"],select[name="tls"]',
    sqlite || method !== 'details',
  );
  off('input[name="file"]', !sqlite);
  const engine = form.querySelector('input[type="hidden"][name="engine"]');
  if (engine instanceof HTMLInputElement) engine.value = form.dataset.engine ?? 'postgresql';
  form
    .querySelectorAll('[data-pick-engine]')
    .forEach((card) =>
      card.setAttribute(
        'aria-checked',
        String(card.getAttribute('data-pick-engine') === form.dataset.engine),
      ),
    );
  form
    .querySelectorAll('[data-pick-method]')
    .forEach((card) =>
      card.setAttribute('aria-checked', String(card.getAttribute('data-pick-method') === method)),
    );
  form
    .querySelectorAll('[data-cw-method]')
    .forEach((tab) =>
      tab.setAttribute('aria-selected', String(tab.getAttribute('data-cw-method') === method)),
    );
  const message = document.getElementById('modal-msg');
  if (message) message.replaceChildren();
}
function initConnForm() {
  const form = connForm();
  if (form && !form.dataset.ready) {
    form.dataset.ready = '1';
    applyConnMethod(form);
    if (form.dataset.step === '2')
      /** @type {HTMLElement | null} */ (form.querySelector('input[name="label"]'))?.focus();
  }
}
document.addEventListener(
  'submit',
  (event) => {
    const form = event.target;
    if (
      form instanceof HTMLFormElement &&
      form.getAttribute('id') === 'conn-form' &&
      form.dataset.step === '1'
    ) {
      event.preventDefault();
      event.stopImmediatePropagation();
      form.dataset.step = '2';
      applyConnMethod(form);
      /** @type {HTMLElement | null} */ (form.querySelector('input[name="label"]'))?.focus();
    }
  },
  true,
);
document.addEventListener('click', async (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const form = connForm();
  if (!form || !target.closest('#conn-form')) return;
  const engine = target.closest('[data-pick-engine]');
  if (engine instanceof HTMLElement) {
    form.dataset.engine = engine.dataset.pickEngine ?? 'postgresql';
    applyConnMethod(form);
    return;
  }
  const method = target.closest('[data-pick-method],[data-cw-method]');
  if (method instanceof HTMLElement) {
    form.dataset.method = method.dataset.pickMethod ?? method.dataset.cwMethod ?? 'details';
    applyConnMethod(form);
    return;
  }
  if (target.closest('[data-cw-next]')) {
    form.dataset.step = '2';
    applyConnMethod(form);
    /** @type {HTMLElement | null} */ (form.querySelector('input[name="label"]'))?.focus();
    return;
  }
  if (target.closest('[data-cw-back]')) {
    form.dataset.step = '1';
    applyConnMethod(form);
    return;
  }
  const reveal = target.closest('[data-reveal]');
  if (reveal instanceof HTMLButtonElement) {
    const which = reveal.dataset.reveal;
    const input = form.querySelector(
      which === 'url' ? 'input[name="url"]' : 'input[name="password"]',
    );
    if (!(input instanceof HTMLInputElement)) return;
    if (reveal.dataset.shown === '1') {
      reveal.dataset.shown = '0';
      reveal.textContent = 'Show';
      if (which === 'password') input.type = 'password';
      else input.value = input.dataset.masked ?? input.value;
      return;
    }
    reveal.disabled = true;
    try {
      const response = await fetch(
        `/admin/connections/reveal?id=${encodeURIComponent(form.dataset.id ?? '')}`,
        {
          cache: 'no-store',
        },
      );
      if (!response.ok) throw new Error('refused');
      const data = await response.json();
      if (which === 'url') {
        input.dataset.masked = input.value;
        input.value = data.url ?? '';
      } else {
        input.type = 'text';
        input.value = data.password ?? '';
      }
      reveal.dataset.shown = '1';
      reveal.textContent = 'Hide';
    } catch {
      toast('Only an administrator or the local instance can reveal a saved password.');
    } finally {
      reveal.disabled = false;
    }
  }
});

/* connection cards: real status for every connection */
async function fillConnectionStates() {
  const cards = Array.from(document.querySelectorAll('.cx[data-id]'));
  if (!cards.length) return;
  try {
    const response = await fetch('/connection/profiles', { cache: 'no-store' });
    if (!response.ok) throw new Error('status');
    const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
    const active = String(win.cuadbProfile || '').split('~')[0];
    let again = false;
    for (const card of cards) {
      const id = card.getAttribute('data-id') ?? '';
      const item = doc.querySelector(`[data-profile="${CSS.escape(id)}"]`);
      const chip = card.querySelector('.cx-st');
      if (!(chip instanceof HTMLElement) || !(item instanceof HTMLElement)) continue;
      const ready = item.dataset.ready === 'true';
      const state = item.dataset.state ?? '';
      const checking = /checking/i.test(state);
      again ||= checking;
      chip.dataset.st = checking ? 'pend' : ready ? 'ok' : 'bad';
      const dot = chip.querySelector('.dot');
      dot?.classList.toggle('pend', checking);
      dot?.classList.toggle('off', !checking && !ready);
      const label = chip.querySelector('span');
      if (label)
        label.textContent = `${checking ? 'Checking…' : state || (ready ? 'Connected' : 'Unavailable')}${active === id ? ' · current' : ''}`;
    }
    if (again) setTimeout(() => void fillConnectionStates(), 2000);
  } catch {
    cards.forEach((card) => {
      const chip = card.querySelector('.cx-st');
      if (chip instanceof HTMLElement) {
        chip.dataset.st = 'bad';
        const label = chip.querySelector('span');
        if (label) label.textContent = 'Unknown';
      }
    });
  }
}
document.addEventListener('htmx:afterSwap', (event) => {
  const target = event.target;
  if (target instanceof Element && target.id === 'modal') initConnForm();
  if (
    target instanceof Element &&
    (target.id === 'mainbody' || target.querySelector?.('.cx[data-id]'))
  ) {
    setTimeout(() => void fillConnectionStates(), 50);
  }
});
if (document.querySelector('.cx[data-id]')) void fillConnectionStates();

/* ------------------------------------------------------------ SQL workspace: sheet, builder, examples, history, AI, output */
const isSqlite = () => /sqlite/i.test(String(currentDb.engine || ''));
function setEditorSql(
  /** @type {string} */ text,
  /** @type {'replace' | 'append'} */ mode = 'replace',
) {
  const area = document.getElementById('sql');
  if (!(area instanceof HTMLTextAreaElement)) return;
  area.value =
    mode === 'append' && area.value.trim() ? `${area.value.replace(/\s+$/, '')}\n\n${text}` : text;
  syncSqlHighlight();
}
function editorSql() {
  const area = document.getElementById('sql');
  return area instanceof HTMLTextAreaElement ? area.value : '';
}
function el(
  /** @type {string} */ tag,
  /** @type {string} */ cls = '',
  /** @type {string} */ text = '',
) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text) node.textContent = text;
  return node;
}
function button(
  /** @type {string} */ label,
  /** @type {() => void} */ onClick,
  /** @type {string} */ cls = 'btn',
) {
  const b = /** @type {HTMLButtonElement} */ (el('button', cls, label));
  b.type = 'button';
  b.addEventListener('click', onClick);
  return b;
}
function readLocal(/** @type {string} */ key, /** @type {any} */ fallback) {
  try {
    return JSON.parse(localStorage.getItem(key) || 'null') ?? fallback;
  } catch {
    return fallback;
  }
}
function writeLocal(/** @type {string} */ key, /** @type {any} */ value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage can be unavailable */
  }
}

/* ---- the side sheet ---- */
const SHEET_TABS = /** @type {Array<[string, string]>} */ ([
  ['builder', 'Builder'],
  ['examples', 'Examples'],
  ['history', 'History'],
  ['ai', '✦ AI'],
]);
let sheetTab = 'builder';
function closeSheet() {
  const sheet = document.getElementById('sqlsheet');
  if (sheet) sheet.hidden = true;
}
function openSheet(/** @type {string} */ tab) {
  const sheet = document.getElementById('sqlsheet');
  if (!sheet) return;
  if (!sheet.hidden && sheetTab === tab) return closeSheet();
  sheetTab = tab;
  sheet.hidden = false;
  renderSheet();
  void loadSqlHints().then(() => {
    if (!sheet.hidden && (sheetTab === 'builder' || sheetTab === 'examples')) renderSheet();
  });
}
function renderSheet() {
  const sheet = document.getElementById('sqlsheet');
  if (!sheet) return;
  sheet.replaceChildren();
  const top = el('div', 'sh-top');
  const tabs = el('nav', 'oa-tabs');
  for (const [id, label] of SHEET_TABS) {
    const tab = /** @type {HTMLButtonElement} */ (
      el('button', `oa-tab${id === sheetTab ? ' on' : ''}`, label)
    );
    tab.type = 'button';
    tab.addEventListener('click', () => {
      sheetTab = id;
      renderSheet();
    });
    tabs.append(tab);
  }
  const close = button('✕', closeSheet, 'oa-x');
  close.setAttribute('aria-label', 'Close');
  top.append(tabs, close);
  const body = el('div', 'sh-body');
  sheet.append(top, body);
  if (sheetTab === 'builder') renderBuilderPane(body);
  else if (sheetTab === 'examples') renderExamplesPane(body);
  else if (sheetTab === 'history') renderHistoryPane(body);
  else renderAiPane(body);
  enhanceControls(sheet);
}

/* ---- query builder: options on the left, code written in real time ---- */
const qb = {
  table: '',
  mode: 'rows',
  cols: /** @type {Set<string> | null} */ (null),
  conds: /** @type {Array<{ col: string, op: string, val: string }>} */ ([]),
  match: 'AND',
  fn: 'COUNT',
  fnCol: '',
  group: /** @type {Set<string>} */ (new Set()),
  order: '',
  dir: 'ASC',
  limit: '100',
  sync: true,
};
function qbTable() {
  const tables = sqlHints?.tables ?? [];
  return tables.find((t) => `${t.s}.${t.n}` === qb.table) ?? tables[0];
}
function sqlValue(
  /** @type {{ n: string, t: string } | undefined} */ column,
  /** @type {string} */ raw,
) {
  const v = raw.trim();
  const type = (column?.t ?? '').toLowerCase();
  if (/^(bool)/.test(type) && /^(true|false)$/i.test(v)) return v.toUpperCase();
  if (/int|numeric|decimal|real|double|float|serial|money/.test(type) && /^-?\d+(\.\d+)?$/.test(v))
    return v;
  return `'${v.replace(/'/g, "''")}'`;
}
function buildQuery() {
  const t = qbTable();
  if (!t) return '';
  const name = isSqlite() && t.s === 'main' ? sqlQuote(t.n) : `${sqlQuote(t.s)}.${sqlQuote(t.n)}`;
  const col = (/** @type {string} */ n) => t.c.find((c) => c.n === n);
  const where = qb.conds
    .map((c) => {
      const column = col(c.col);
      const q = sqlQuote(c.col);
      if (c.op === 'IS NULL' || c.op === 'IS NOT NULL') return `${q} ${c.op}`;
      if (c.val.trim() === '') return '';
      if (c.op === 'IN')
        return `${q} IN (${c.val
          .split(',')
          .map((x) => sqlValue(column, x))
          .join(', ')})`;
      if (c.op === 'LIKE' || c.op === 'ILIKE') {
        const text = c.val.includes('%') ? c.val : `%${c.val}%`;
        return `${q} ${c.op === 'ILIKE' && isSqlite() ? 'LIKE' : c.op} '${text.replace(/'/g, "''")}'`;
      }
      return `${q} ${c.op} ${sqlValue(column, c.val)}`;
    })
    .filter(Boolean);
  const lines = [];
  const limit = Math.min(Math.max(Number(qb.limit) || 100, 1), 5000);
  if (qb.mode === 'agg') {
    const groups = Array.from(qb.group).filter((g) => col(g));
    const target = qb.fn === 'COUNT' && !qb.fnCol ? '*' : sqlQuote(qb.fnCol || t.c[0]?.n || '*');
    const alias = `${qb.fn.toLowerCase()}_${target === '*' ? 'all' : qb.fnCol}`;
    lines.push(
      `SELECT ${[...groups.map(sqlQuote), `${qb.fn}(${target}) AS ${sqlQuote(alias)}`].join(', ')}`,
    );
    lines.push(`FROM ${name}`);
    if (where.length) lines.push(`WHERE ${where.join(`\n  ${qb.match} `)}`);
    if (groups.length) lines.push(`GROUP BY ${groups.map(sqlQuote).join(', ')}`);
    if (qb.order) lines.push(`ORDER BY ${sqlQuote(qb.order)} ${qb.dir}`);
    else if (groups.length) lines.push(`ORDER BY ${sqlQuote(alias)} DESC`);
  } else {
    const chosen = qb.cols ? t.c.filter((c) => qb.cols?.has(c.n)).map((c) => sqlQuote(c.n)) : [];
    lines.push(`SELECT ${chosen.length && chosen.length < t.c.length ? chosen.join(', ') : '*'}`);
    lines.push(`FROM ${name}`);
    if (where.length) lines.push(`WHERE ${where.join(`\n  ${qb.match} `)}`);
    if (qb.order) lines.push(`ORDER BY ${sqlQuote(qb.order)} ${qb.dir}`);
  }
  lines.push(`LIMIT ${limit};`);
  return lines.join('\n');
}
function qbChanged() {
  const sql = buildQuery();
  const pre = document.getElementById('qb-pre');
  if (pre) pre.textContent = sql || '— choose a table —';
  if (qb.sync && sql) setEditorSql(sql);
}
function renderBuilderPane(/** @type {HTMLElement} */ body) {
  const tables = sqlHints?.tables ?? [];
  if (!tables.length) {
    body.append(
      el('p', 'ai-note', sqlHints ? 'No tables were found to build from.' : 'Loading tables…'),
    );
    return;
  }
  if (!qb.table || !tables.some((t) => `${t.s}.${t.n}` === qb.table))
    qb.table = `${tables[0]?.s}.${tables[0]?.n}`;
  const t = qbTable();
  if (!t) return;
  const field = (/** @type {string} */ label, /** @type {HTMLElement} */ control) => {
    const wrap = el('label', 'qb-f');
    wrap.append(el('span', '', label), control);
    return wrap;
  };
  const select = (
    /** @type {Array<[string, string]>} */ options,
    /** @type {string} */ value,
    /** @type {(v: string) => void} */ onChange,
  ) => {
    const s = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
    for (const [v, l] of options) s.append(new Option(l, v));
    s.value = value;
    s.addEventListener('change', () => onChange(s.value));
    return s;
  };
  body.append(el('div', 'sh-sec', 'Source'));
  body.append(
    field(
      'Table',
      select(
        tables.map((x) => [`${x.s}.${x.n}`, `${x.s}.${x.n}`]),
        qb.table,
        (v) => {
          qb.table = v;
          qb.cols = null;
          qb.conds = [];
          qb.group = new Set();
          qb.order = '';
          qb.fnCol = '';
          renderSheet();
          qbChanged();
        },
      ),
    ),
  );
  const modeSeg = el('div', 'seg');
  for (const [v, l] of /** @type {Array<[string, string]>} */ ([
    ['rows', 'Rows'],
    ['agg', 'Aggregate'],
  ])) {
    const b = /** @type {HTMLButtonElement} */ (el('button', qb.mode === v ? 'on' : '', l));
    b.type = 'button';
    b.addEventListener('click', () => {
      qb.mode = v;
      renderSheet();
      qbChanged();
    });
    modeSeg.append(b);
  }
  body.append(field('Shape', modeSeg));
  const colNames = /** @type {Array<[string, string]>} */ (t.c.map((c) => [c.n, c.n]));
  if (qb.mode === 'rows') {
    const wrap = el('div', 'qb-cols');
    for (const c of t.c) {
      const label = el('label');
      const box = /** @type {HTMLInputElement} */ (el('input'));
      box.type = 'checkbox';
      box.checked = qb.cols ? qb.cols.has(c.n) : true;
      box.addEventListener('change', () => {
        qb.cols ??= new Set(t.c.map((x) => x.n));
        if (box.checked) qb.cols.add(c.n);
        else qb.cols.delete(c.n);
        qbChanged();
      });
      label.append(box, document.createTextNode(c.n));
      wrap.append(label);
    }
    body.append(field('Columns', wrap));
  } else {
    body.append(el('div', 'qb-two'));
    const two = /** @type {HTMLElement} */ (body.lastElementChild);
    two.append(
      field(
        'Function',
        select(
          ['COUNT', 'SUM', 'AVG', 'MIN', 'MAX'].map((x) => [x, x]),
          qb.fn,
          (v) => {
            qb.fn = v;
            qbChanged();
          },
        ),
      ),
      field(
        'Of column',
        select([['', qb.fn === 'COUNT' ? '* (all rows)' : '—'], ...colNames], qb.fnCol, (v) => {
          qb.fnCol = v;
          qbChanged();
        }),
      ),
    );
    const wrap = el('div', 'qb-cols');
    for (const c of t.c) {
      const label = el('label');
      const box = /** @type {HTMLInputElement} */ (el('input'));
      box.type = 'checkbox';
      box.checked = qb.group.has(c.n);
      box.addEventListener('change', () => {
        if (box.checked) qb.group.add(c.n);
        else qb.group.delete(c.n);
        qbChanged();
      });
      label.append(box, document.createTextNode(c.n));
      wrap.append(label);
    }
    body.append(field('Group by', wrap));
  }
  body.append(el('div', 'sh-sec', 'Conditions'));
  const matchSeg = el('div', 'seg');
  for (const v of ['AND', 'OR']) {
    const b = /** @type {HTMLButtonElement} */ (
      el('button', qb.match === v ? 'on' : '', v === 'AND' ? 'Match all' : 'Match any')
    );
    b.type = 'button';
    b.addEventListener('click', () => {
      qb.match = v;
      renderSheet();
      qbChanged();
    });
    matchSeg.append(b);
  }
  body.append(matchSeg);
  const list = el('div');
  list.style.marginTop = '8px';
  const OPS = ['=', '<>', '>', '<', '>=', '<=', 'LIKE', 'ILIKE', 'IN', 'IS NULL', 'IS NOT NULL'];
  const drawConds = () => {
    list.replaceChildren();
    qb.conds.forEach((c, index) => {
      const row = el('div', 'qb-cond');
      const col = select(colNames, c.col, (v) => {
        c.col = v;
        qbChanged();
      });
      const op = select(
        OPS.map((o) => [o, o]),
        c.op,
        (v) => {
          c.op = v;
          value.disabled = v.startsWith('IS ');
          qbChanged();
        },
      );
      const value = /** @type {HTMLInputElement} */ (el('input', 'inp'));
      value.value = c.val;
      value.placeholder = c.op === 'IN' ? 'a, b, c' : 'value';
      value.disabled = c.op.startsWith('IS ');
      value.addEventListener('input', () => {
        c.val = value.value;
        qbChanged();
      });
      const x = button(
        '×',
        () => {
          qb.conds.splice(index, 1);
          drawConds();
          qbChanged();
        },
        'x',
      );
      x.setAttribute('aria-label', 'Remove condition');
      row.append(col, op, value, x);
      list.append(row);
    });
    enhanceControls(list);
  };
  drawConds();
  body.append(list);
  body.append(
    button('＋ Condition', () => {
      qb.conds.push({ col: t.c[0]?.n ?? '', op: '=', val: '' });
      drawConds();
      qbChanged();
    }),
  );
  body.append(el('div', 'sh-sec', 'Order and size'));
  const two = el('div', 'qb-two');
  two.append(
    field(
      'Order by',
      select([['', '(none)'], ...colNames], qb.order, (v) => {
        qb.order = v;
        qbChanged();
      }),
    ),
    field(
      'Direction',
      select(
        [
          ['ASC', 'Ascending'],
          ['DESC', 'Descending'],
        ],
        qb.dir,
        (v) => {
          qb.dir = v;
          qbChanged();
        },
      ),
    ),
  );
  body.append(two);
  const limit = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  limit.value = qb.limit;
  limit.inputMode = 'numeric';
  limit.addEventListener('input', () => {
    qb.limit = limit.value;
    qbChanged();
  });
  body.append(field('Limit', limit));
  body.append(el('div', 'sh-sec', 'Query'));
  const pre = el('pre', 'qb-pre');
  pre.id = 'qb-pre';
  pre.textContent = buildQuery();
  body.append(pre);
  const actions = el('div', 'sh-actions');
  const sync = el('label', 'sh-switch');
  const box = /** @type {HTMLInputElement} */ (el('input'));
  box.type = 'checkbox';
  box.className = 'tg';
  box.checked = qb.sync;
  box.addEventListener('change', () => {
    qb.sync = box.checked;
    if (qb.sync) qbChanged();
  });
  sync.append(box, document.createTextNode('Sync to editor'));
  actions.append(
    sync,
    button('Generate', () => setEditorSql(buildQuery())),
    button(
      'Run',
      () => {
        setEditorSql(buildQuery());
        /** @type {HTMLFormElement | null} */ (document.getElementById('sqlform'))?.requestSubmit();
      },
      'btn on',
    ),
  );
  body.append(actions);
  if (qb.sync) setEditorSql(buildQuery());
}

/* ---- examples and smart suggestions ---- */
function suggestions() {
  const tables = sqlHints?.tables ?? [];
  const text = editorSql().toLowerCase();
  const full = (/** @type {{ s: string, n: string }} */ t) =>
    isSqlite() && t.s === 'main' ? sqlQuote(t.n) : `${sqlQuote(t.s)}.${sqlQuote(t.n)}`;
  /** @type {Array<{ title: string, why: string, sql: string }>} */
  const out = [];
  const mentioned = tables.filter((t) => new RegExp(`\\b${t.n}\\b`, 'i').test(text));
  const focus = mentioned.length ? mentioned.slice(0, 2) : tables.slice(0, 3);
  for (const t of focus) {
    const q = full(t);
    out.push({
      title: `Latest rows in ${t.n}`,
      why: 'sample',
      sql: `SELECT *\nFROM ${q}\nORDER BY 1 DESC\nLIMIT 50;`,
    });
    const time = t.c.find(
      (c) => /timestamp|date|time/i.test(c.t) || /(_at|_on|date|time)$/i.test(c.n),
    );
    if (time)
      out.push({
        title: `${t.n} by ${time.n}, newest first`,
        why: 'time column',
        sql: `SELECT *\nFROM ${q}\nORDER BY ${sqlQuote(time.n)} DESC\nLIMIT 50;`,
      });
    const cat = t.c.find((c) =>
      /^(status|type|kind|role|plan|category|state|tier|country|region)$/i.test(c.n),
    );
    if (cat)
      out.push({
        title: `${t.n}: count by ${cat.n}`,
        why: 'chart-ready',
        sql: `SELECT ${sqlQuote(cat.n)}, COUNT(*) AS total\nFROM ${q}\nGROUP BY ${sqlQuote(cat.n)}\nORDER BY total DESC;`,
      });
    const key = t.c.find((c) => /^(email|name|slug|code|username)$/i.test(c.n));
    if (key)
      out.push({
        title: `Duplicates of ${key.n} in ${t.n}`,
        why: 'data quality',
        sql: `SELECT ${sqlQuote(key.n)}, COUNT(*) AS copies\nFROM ${q}\nGROUP BY ${sqlQuote(key.n)}\nHAVING COUNT(*) > 1\nORDER BY copies DESC;`,
      });
    out.push({
      title: `Null report for ${t.n}`,
      why: 'data quality',
      sql: `SELECT\n  COUNT(*) AS total,\n${t.c
        .slice(0, 8)
        .map((c) => `  COUNT(${sqlQuote(c.n)}) AS ${sqlQuote(`${c.n}_filled`)}`)
        .join(',\n')}\nFROM ${q};`,
    });
    for (const c of t.c.filter((x) => /_id$/i.test(x.n)).slice(0, 2)) {
      const base = c.n.replace(/_id$/i, '').toLowerCase();
      const target = tables.find(
        (o) => o !== t && [base, `${base}s`, `${base}es`].includes(o.n.toLowerCase()),
      );
      if (target)
        out.push({
          title: `${t.n} joined to ${target.n}`,
          why: `via ${c.n}`,
          sql: `SELECT a.*, b.*\nFROM ${q} a\nJOIN ${full(target)} b ON b.id = a.${sqlQuote(c.n)}\nLIMIT 100;`,
        });
    }
  }
  return out.slice(0, 10);
}
function exampleQueries() {
  const sqlite = isSqlite();
  /** @type {Array<{ title: string, why: string, sql: string }>} */
  const list = sqlite
    ? [
        {
          title: 'List tables',
          why: 'schema',
          sql: "SELECT name, type FROM sqlite_master WHERE type IN ('table','view') ORDER BY name;",
        },
        {
          title: 'Columns of a table',
          why: 'schema',
          sql: "SELECT * FROM pragma_table_info('table_name');",
        },
        {
          title: 'Database size',
          why: 'size',
          sql: 'SELECT page_count * page_size AS bytes FROM pragma_page_count(), pragma_page_size();',
        },
      ]
    : [
        {
          title: 'List tables with size',
          why: 'schema',
          sql: 'SELECT schemaname, relname, n_live_tup AS rows,\n  pg_size_pretty(pg_total_relation_size(relid)) AS size\nFROM pg_stat_user_tables\nORDER BY pg_total_relation_size(relid) DESC\nLIMIT 50;',
        },
        {
          title: 'Columns of a table',
          why: 'schema',
          sql: "SELECT column_name, data_type, is_nullable, column_default\nFROM information_schema.columns\nWHERE table_schema = 'public' AND table_name = 'table_name'\nORDER BY ordinal_position;",
        },
        {
          title: 'Indexes',
          why: 'performance',
          sql: "SELECT schemaname, tablename, indexname, indexdef\nFROM pg_indexes\nWHERE schemaname NOT IN ('pg_catalog','information_schema')\nORDER BY tablename;",
        },
        {
          title: 'Running queries',
          why: 'activity',
          sql: "SELECT pid, state, now() - query_start AS running_for, left(query, 120) AS query\nFROM pg_stat_activity\nWHERE state <> 'idle'\nORDER BY query_start;",
        },
        {
          title: 'Unused indexes',
          why: 'performance',
          sql: 'SELECT relname AS table, indexrelname AS index, idx_scan\nFROM pg_stat_user_indexes\nWHERE idx_scan = 0\nORDER BY relname;',
        },
      ];
  return list;
}
function renderExamplesPane(/** @type {HTMLElement} */ body) {
  const card = (/** @type {{ title: string, why: string, sql: string }} */ item) => {
    const box = el('div', 'ex-item');
    const t = el('div', 'ex-t');
    t.append(el('b', '', item.title), el('small', '', item.why));
    box.append(t, el('pre', 'ex-code', item.sql));
    const act = el('div', 'ex-act');
    act.append(
      button('Use', () => setEditorSql(item.sql)),
      button('Add', () => setEditorSql(item.sql, 'append')),
      button(
        'Run',
        () => {
          setEditorSql(item.sql);
          /** @type {HTMLFormElement | null} */ (
            document.getElementById('sqlform')
          )?.requestSubmit();
        },
        'btn on',
      ),
    );
    box.append(act);
    return box;
  };
  const smart = suggestions();
  body.append(
    el('div', 'sh-sec', sqlHints ? 'Suggested for you' : 'Suggested for you · loading tables…'),
  );
  if (smart.length) smart.forEach((s) => body.append(card(s)));
  else body.append(el('p', 'ai-note', 'Write or pick a table and suggestions appear here.'));
  body.append(el('div', 'sh-sec', 'Starter snippets'));
  for (const [kind, title] of /** @type {Array<[string, string]>} */ ([
    ['select', 'SELECT with filter and order'],
    ['count', 'Count rows'],
    ['join', 'Join two tables'],
    ['insert', 'Insert a row'],
    ['update', 'Update rows'],
    ['create', 'Create a table'],
  ]))
    body.append(card({ title, why: 'snippet', sql: sqlSnippet(kind) }));
  body.append(el('div', 'sh-sec', isSqlite() ? 'SQLite examples' : 'PostgreSQL examples'));
  exampleQueries().forEach((e) => body.append(card(e)));
}

/* ---- recent and saved queries ---- */
const HIST_KEY = 'cuadb:sqlh';
const SAVED_KEY = 'cuadb:sqls';
function recordHistory(
  /** @type {string} */ sql,
  /** @type {boolean} */ ok,
  /** @type {string} */ summary,
) {
  const text = sql.trim();
  if (!text) return;
  const list = /** @type {Array<{ sql: string, ok: boolean, t: number, sum: string }>} */ (
    readLocal(HIST_KEY, [])
  );
  const next = [
    { sql: text, ok, t: Date.now(), sum: summary },
    ...list.filter((h) => h.sql !== text),
  ].slice(0, 50);
  writeLocal(HIST_KEY, next);
}
function ago(/** @type {number} */ t) {
  const s = Math.max(1, Math.round((Date.now() - t) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return `${Math.round(s / 86400)}d ago`;
}
let histMode = 'recent';
function renderHistoryPane(/** @type {HTMLElement} */ body) {
  const tabs = el('div', 'hist-tabs');
  const seg = el('div', 'seg');
  for (const [v, l] of /** @type {Array<[string, string]>} */ ([
    ['recent', 'Recent'],
    ['saved', 'Saved'],
    ['analyses', 'Analyses'],
  ])) {
    const b = /** @type {HTMLButtonElement} */ (el('button', histMode === v ? 'on' : '', l));
    b.type = 'button';
    b.addEventListener('click', () => {
      histMode = v;
      renderSheet();
    });
    seg.append(b);
  }
  const find = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  find.type = 'search';
  find.placeholder = 'Search…';
  tabs.append(seg, find);
  body.append(tabs);
  const list = el('div');
  body.append(list);
  const draw = () => {
    list.replaceChildren();
    const needle = find.value.toLowerCase();
    const saved = /** @type {Array<{ name: string, sql: string }>} */ (readLocal(SAVED_KEY, []));
    if (histMode === 'analyses') {
      const all =
        /** @type {Array<{ id: string, t: number, title: string, cards: DashCard[] }>} */ (
          readLocal(SAVED_ANALYSES, [])
        );
      const rows = all.filter((a) =>
        `${a.title} ${a.cards.map((c) => c.title).join(' ')}`.toLowerCase().includes(needle),
      );
      if (!rows.length)
        list.append(
          el('p', 'ai-note', 'Ask the agent for analytics, then Save a dashboard to keep it here.'),
        );
      for (const a of rows) {
        const box = el('div', 'ex-item');
        const t = el('div', 'ex-t');
        t.append(el('b', '', a.title), el('small', '', `${a.cards.length} cards · ${ago(a.t)}`));
        box.append(t, el('div', 'ex-code', a.cards.map((c) => c.title).join(' · ')));
        const act = el('div', 'ex-act');
        act.append(
          button(
            'Open',
            () => {
              list.replaceChildren();
              list.append(button('← Back to analyses', () => draw()));
              renderDashboard(list, { title: a.title, cards: a.cards }, { saved: true });
            },
            'btn on',
          ),
          button('Delete', () => {
            writeLocal(
              SAVED_ANALYSES,
              all.filter((x) => x.id !== a.id),
            );
            draw();
          }),
        );
        box.append(act);
        list.append(box);
      }
      return;
    }
    if (histMode === 'saved') {
      const rows = saved.filter((s) => `${s.name} ${s.sql}`.toLowerCase().includes(needle));
      if (!rows.length)
        list.append(el('p', 'ai-note', 'Nothing saved yet. Star a recent query to keep it here.'));
      for (const s of rows) {
        const box = el('div', 'ex-item');
        const t = el('div', 'ex-t');
        t.append(el('b', '', s.name));
        box.append(t, el('pre', 'ex-code', s.sql));
        const act = el('div', 'ex-act');
        act.append(
          button('Use', () => setEditorSql(s.sql)),
          button('Add', () => setEditorSql(s.sql, 'append')),
          button('Delete', () => {
            writeLocal(
              SAVED_KEY,
              saved.filter((x) => x !== s && x.sql !== s.sql),
            );
            draw();
          }),
        );
        box.append(act);
        list.append(box);
      }
      return;
    }
    const recent = /** @type {Array<{ sql: string, ok: boolean, t: number, sum: string }>} */ (
      readLocal(HIST_KEY, [])
    );
    const rows = recent.filter((h) => h.sql.toLowerCase().includes(needle));
    if (!rows.length) list.append(el('p', 'ai-note', 'Queries you run appear here.'));
    for (const h of rows) {
      const box = el('div', 'ex-item');
      const t = el('div', 'ex-t');
      const state = el('b', '', h.ok ? '✓' : '✕');
      state.style.color = h.ok ? 'var(--ok)' : '#e0617a';
      t.append(state, el('small', '', `${ago(h.t)}${h.sum ? ` · ${h.sum}` : ''}`));
      box.append(t, el('pre', 'ex-code', h.sql));
      const act = el('div', 'ex-act');
      const isSaved = saved.some((s) => s.sql === h.sql);
      act.append(
        button('Use', () => setEditorSql(h.sql)),
        button(
          'Run',
          () => {
            setEditorSql(h.sql);
            /** @type {HTMLFormElement | null} */ (
              document.getElementById('sqlform')
            )?.requestSubmit();
          },
          'btn on',
        ),
        button(isSaved ? '★ Saved' : '☆ Save', () => {
          if (isSaved) return;
          const name = h.sql.split('\n')[0]?.slice(0, 48) ?? 'Query';
          writeLocal(SAVED_KEY, [{ name, sql: h.sql }, ...saved].slice(0, 100));
          draw();
        }),
        button('Remove', () => {
          writeLocal(
            HIST_KEY,
            recent.filter((x) => x.sql !== h.sql),
          );
          draw();
        }),
      );
      box.append(act);
      list.append(box);
    }
  };
  find.addEventListener('input', draw);
  draw();
}

/* ---- AI assistant (Gemini, called straight from the browser with your own key) ---- */
const AI_KEY = 'cuadb:ai';

function aiConfigured() {
  return Boolean(aiConfig().key);
}
function syncAiClass() {
  document.documentElement.classList.toggle('ai-on', aiConfigured());
}

/* ---- output: summary, views (table, chart, stats), sorting and filtering ---- */
/** @type {number} */
let sqlStarted = 0;
/** @type {string} */
let sqlSubmitted = '';
function outputTables(/** @type {Element} */ out) {
  return /** @type {HTMLTableElement[]} */ (Array.from(out.querySelectorAll('table.grid')));
}
function tableData(/** @type {HTMLTableElement} */ table) {
  const heads = Array.from(table.querySelectorAll('thead th:not(.rn)')).map((th) =>
    th instanceof HTMLElement && th.dataset.col ? th.dataset.col : (th.textContent ?? '').trim(),
  );
  const rows = Array.from(table.querySelectorAll('tbody tr')).map((tr) =>
    Array.from(tr.querySelectorAll('td:not(.rn)')).map((td) =>
      td.querySelector('[data-null]') ? null : (td.textContent ?? ''),
    ),
  );
  return { heads, rows };
}
const isNum = (/** @type {string | null} */ v) =>
  v !== null && v.trim() !== '' && Number.isFinite(Number(v));
function numericColumns(/** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ d) {
  return d.heads
    .map((_, i) => i)
    .filter((i) => {
      const values = d.rows.map((r) => r[i] ?? null).filter((v) => v !== null);
      return values.length > 0 && values.filter(isNum).length / values.length >= 0.8;
    });
}
const CHART_COLORS = ['#4f8cf0', '#43c0a0', '#e0a24f', '#c27ad9', '#e0617a', '#8aa64a'];
const chartState = { type: 'bar', x: '', ys: /** @type {string[]} */ ([]), source: 0 };
function svgNode(
  /** @type {string} */ tag,
  /** @type {Record<string, string | number>} */ attrs = {},
  /** @type {string} */ text = '',
) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  if (text) node.textContent = text;
  return node;
}
function shortNum(/** @type {number} */ n) {
  const a = Math.abs(n);
  if (a >= 1e9) return `${+(n / 1e9).toFixed(1)}B`;
  if (a >= 1e6) return `${+(n / 1e6).toFixed(1)}M`;
  if (a >= 1e4) return `${+(n / 1e3).toFixed(1)}k`;
  return String(+n.toFixed(2));
}
function drawChart(
  /** @type {HTMLElement} */ host,
  /** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ d,
) {
  host.replaceChildren();
  const xi = Math.max(0, d.heads.indexOf(chartState.x));
  const yi = chartState.ys.map((y) => d.heads.indexOf(y)).filter((i) => i >= 0);
  if (!yi.length)
    return host.append(el('p', 'none', 'Pick at least one numeric column for the Y axis.'));
  const rows = d.rows.slice(0, 60);
  const W = 720;
  const H = 300;
  const pad = { l: 52, r: 14, t: 12, b: 54 };
  const svg = svgNode('svg', {
    viewBox: `0 0 ${W} ${H}`,
    class: 'chart',
    role: 'img',
    'aria-label': 'Chart of the result',
  });
  const labels = rows.map((r) => r[xi] ?? '∅');
  if (chartState.type === 'pie') {
    const vals = rows.map((r) => Math.max(0, Number(r[yi[0] ?? 0]) || 0));
    const total = vals.reduce((a, b) => a + b, 0) || 1;
    let angle = -Math.PI / 2;
    const cx = 170;
    const cy = H / 2;
    const rad = 120;
    vals.slice(0, 12).forEach((v, i) => {
      const next = angle + (v / total) * Math.PI * 2;
      const large = next - angle > Math.PI ? 1 : 0;
      const path = svgNode('path', {
        d: `M${cx} ${cy} L${cx + rad * Math.cos(angle)} ${cy + rad * Math.sin(angle)} A${rad} ${rad} 0 ${large} 1 ${cx + rad * Math.cos(next)} ${cy + rad * Math.sin(next)} Z`,
        fill: CHART_COLORS[i % CHART_COLORS.length] ?? '#888',
      });
      path.append(svgNode('title', {}, `${labels[i]}: ${v} (${Math.round((v / total) * 100)}%)`));
      svg.append(path);
      svg.append(
        svgNode(
          'text',
          { x: 340, y: 30 + i * 20 },
          `${(labels[i] ?? '').slice(0, 28)} · ${shortNum(v)} (${Math.round((v / total) * 100)}%)`,
        ),
      );
      svg.append(
        svgNode('rect', {
          x: 324,
          y: 21 + i * 20,
          width: 10,
          height: 10,
          rx: 2,
          fill: CHART_COLORS[i % CHART_COLORS.length] ?? '#888',
        }),
      );
      angle = next;
    });
    host.append(svg);
    return;
  }
  const series = yi.map((i) => rows.map((r) => Number(r[i]) || 0));
  const max = Math.max(1e-9, ...series.flat(), 0);
  const min = Math.min(0, ...series.flat());
  const span = max - min || 1;
  const x0 = pad.l;
  const x1 = W - pad.r;
  const y0 = H - pad.b;
  const y1 = pad.t;
  const yAt = (/** @type {number} */ v) => y0 - ((v - min) / span) * (y0 - y1);
  for (let g = 0; g <= 4; g += 1) {
    const v = min + (span * g) / 4;
    svg.append(svgNode('line', { x1: x0, x2: x1, y1: yAt(v), y2: yAt(v), class: 'gl' }));
    svg.append(svgNode('text', { x: x0 - 6, y: yAt(v) + 3, 'text-anchor': 'end' }, shortNum(v)));
  }
  svg.append(svgNode('line', { x1: x0, x2: x0, y1, y2: y0, class: 'ax' }));
  svg.append(svgNode('line', { x1: x0, x2: x1, y1: y0, y2: y0, class: 'ax' }));
  const n = Math.max(1, rows.length);
  const slot = (x1 - x0) / n;
  const every = Math.ceil(n / 14);
  labels.forEach((label, i) => {
    if (i % every) return;
    const text = svgNode(
      'text',
      {
        x: x0 + slot * i + slot / 2,
        y: y0 + 14,
        'text-anchor': 'end',
        transform: `rotate(-35 ${x0 + slot * i + slot / 2} ${y0 + 14})`,
      },
      label.length > 14 ? `${label.slice(0, 13)}…` : label,
    );
    svg.append(text);
  });
  series.forEach((vals, s) => {
    const color = CHART_COLORS[s % CHART_COLORS.length] ?? '#888';
    if (chartState.type === 'bar') {
      const bw = (slot * 0.76) / series.length;
      vals.forEach((v, i) => {
        const rect = svgNode('rect', {
          x: x0 + slot * i + slot * 0.12 + bw * s,
          y: Math.min(yAt(v), yAt(0)),
          width: Math.max(1, bw - 1),
          height: Math.max(1, Math.abs(yAt(v) - yAt(0))),
          rx: 2,
          fill: color,
        });
        rect.append(svgNode('title', {}, `${labels[i]} · ${d.heads[yi[s] ?? 0]}: ${v}`));
        svg.append(rect);
      });
    } else {
      const pts = vals.map((v, i) => [x0 + slot * i + slot / 2, yAt(v)]);
      const line = pts.map((p, i) => `${i ? 'L' : 'M'}${p[0]} ${p[1]}`).join(' ');
      if (chartState.type === 'area' && pts.length) {
        svg.append(
          svgNode('path', {
            d: `${line} L${pts[pts.length - 1]?.[0]} ${y0} L${pts[0]?.[0]} ${y0} Z`,
            fill: color,
            'fill-opacity': 0.18,
          }),
        );
      }
      svg.append(
        svgNode('path', {
          d: line,
          fill: 'none',
          stroke: color,
          'stroke-width': 2,
          'stroke-linejoin': 'round',
        }),
      );
      pts.forEach((p, i) => {
        const dot = svgNode('circle', { cx: p[0] ?? 0, cy: p[1] ?? 0, r: 3, fill: color });
        dot.append(svgNode('title', {}, `${labels[i]} · ${d.heads[yi[s] ?? 0]}: ${vals[i]}`));
        svg.append(dot);
      });
    }
  });
  host.append(svg);
  const legend = el('div', 'chart-legend');
  yi.forEach((i, s) => {
    const item = el('span');
    const sw = el('i');
    sw.style.background = CHART_COLORS[s % CHART_COLORS.length] ?? '#888';
    item.append(sw, document.createTextNode(d.heads[i] ?? ''));
    legend.append(item);
  });
  host.append(legend);
  if (d.rows.length > rows.length)
    host.append(el('p', 'ai-note', `Showing the first ${rows.length} of ${d.rows.length} rows.`));
}
function statsTable(/** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ d) {
  const table = el('table', 'stats');
  const head = el('tr');
  for (const h of [
    'Column',
    'Kind',
    'Filled',
    'Null',
    'Distinct',
    'Min',
    'Max',
    'Avg',
    'Most common',
  ])
    head.append(el('th', '', h));
  table.append(head);
  d.heads.forEach((name, i) => {
    const values = d.rows.map((r) => r[i] ?? null);
    const filled = values.filter((v) => v !== null);
    const numeric = filled.length > 0 && filled.filter(isNum).length / filled.length >= 0.8;
    const counts = new Map();
    filled.forEach((v) => counts.set(v, (counts.get(v) ?? 0) + 1));
    const top = Array.from(counts.entries()).sort((a, b) => b[1] - a[1])[0];
    const nums = filled.filter(isNum).map(Number);
    const sorted = (numeric ? nums : filled)
      .slice()
      .sort((a, b) => (numeric ? Number(a) - Number(b) : String(a).localeCompare(String(b))));
    const row = el('tr');
    const cells = [
      name,
      numeric ? 'number' : 'text',
      String(filled.length),
      String(values.length - filled.length),
      String(counts.size),
      sorted.length ? String(sorted[0]).slice(0, 24) : '—',
      sorted.length ? String(sorted[sorted.length - 1]).slice(0, 24) : '—',
      numeric && nums.length ? shortNum(nums.reduce((a, b) => a + b, 0) / nums.length) : '—',
      top ? `${String(top[0]).slice(0, 28)} (${top[1]})` : '—',
    ];
    cells.forEach((c, k) => {
      const td = el('td', k >= 2 && k <= 7 ? 'n' : '', c);
      row.append(td);
    });
    table.append(row);
  });
  return table;
}

function sortResultTable(/** @type {HTMLTableElement} */ table, /** @type {HTMLElement} */ th) {
  const index = Array.from(th.parentElement?.children ?? []).indexOf(th);
  const current = th.getAttribute('aria-sort');
  const next =
    current === 'ascending' ? 'descending' : current === 'descending' ? 'none' : 'ascending';
  table.querySelectorAll('thead th').forEach((h) => h.removeAttribute('aria-sort'));
  if (next !== 'none') th.setAttribute('aria-sort', next);
  const body = table.tBodies[0];
  if (!body) return;
  const rows = Array.from(body.rows);
  rows.forEach((r, i) => {
    r.dataset.i0 ??= String(i);
  });
  rows.sort((a, b) => {
    if (next === 'none') return Number(a.dataset.i0) - Number(b.dataset.i0);
    const av = a.cells[index]?.textContent?.trim() ?? '';
    const bv = b.cells[index]?.textContent?.trim() ?? '';
    const numeric = isNum(av) && isNum(bv);
    const cmp = numeric
      ? Number(av) - Number(bv)
      : av.localeCompare(bv, undefined, { numeric: true });
    return next === 'ascending' ? cmp : -cmp;
  });
  rows.forEach((r) => body.append(r));
  decorateGrid(table.closest('.wrap') ?? document);
}

document.addEventListener('htmx:beforeRequest', (event) => {
  const form = /** @type {Any} */ (event).detail?.elt;
  if (form instanceof HTMLElement && form.id === 'sqlform') {
    sqlStarted = performance.now();
    sqlSubmitted = editorSql();
    const sum = document.getElementById('sql-sum');
    if (sum) {
      sum.textContent = 'Running…';
      sum.className = 'sql-sum run';
    }
    closeSheetIfNarrow();
  }
});
function closeSheetIfNarrow() {
  if (innerWidth < 900) closeSheet();
}
document.addEventListener('htmx:afterSwap', (event) => {
  const target = event.target;
  if (target instanceof HTMLElement && target.id === 'sqlout') onSqlOutput(target);
});
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const sheetBtn = target.closest('[data-sheet]');
  if (sheetBtn instanceof HTMLElement) {
    openSheet(sheetBtn.dataset.sheet ?? 'builder');
    return;
  }
  const view = target.closest('[data-sqlview]');
  if (view instanceof HTMLElement) {
    setSqlView(view.dataset.sqlview ?? 'table');
    return;
  }
  const ask = target.closest('[data-ai-table]');
  if (ask instanceof HTMLElement) {
    const table = document.getElementById('catalog-panel')?.dataset.table ?? '';
    win.cuadbPrefillSql = `SELECT * FROM ${table.split('.').map(sqlQuote).join('.')} LIMIT 100;`;
    win.cuadbOpenAi = `Tell me about the table ${table}: what it likely stores, useful queries and anything worth checking.`;
    document.getElementById('sqlb')?.click();
  }
});
document.addEventListener('input', (event) => {
  if (event.target instanceof Element && event.target.id === 'sql-find') filterResults();
});
// Opening the workspace from a table: prefill and optionally start an AI request.
document.addEventListener('htmx:afterSettle', () => {
  if (win.cuadbOpenAi && document.getElementById('sqlsheet')) {
    const job = String(win.cuadbOpenAi);
    win.cuadbOpenAi = '';
    askAi('Describe the table and suggest practical queries.', job);
  }
});
// The settings dialog hosts the same AI configuration.
document.addEventListener('click', (event) => {
  const tab = event.target instanceof Element ? event.target.closest('[data-sp="ai"]') : null;
  if (tab) {
    const host = document.getElementById('ai-config');
    if (host) renderAiConfig(host, () => renderAiConfig(host, () => {}));
  }
});

/* ---- AI providers: Gemini, OpenAI, Anthropic, or any OpenAI-compatible endpoint ---- */
const AI_PROVIDERS =
  /** @type {Record<string, { label: string, models: string[], needsKey: boolean, base: string, keyUrl: string }>} */ ({
    gemini: {
      label: 'Google Gemini',
      models: [
        'gemini-2.0-flash',
        'gemini-2.5-flash',
        'gemini-2.5-pro',
        'gemini-1.5-flash',
        'gemini-1.5-pro',
      ],
      needsKey: true,
      base: 'https://generativelanguage.googleapis.com',
      keyUrl: 'https://aistudio.google.com/apikey',
    },
    openai: {
      label: 'OpenAI',
      models: ['gpt-4o-mini', 'gpt-4o', 'gpt-4.1-mini', 'gpt-4.1', 'o4-mini'],
      needsKey: true,
      base: 'https://api.openai.com/v1',
      keyUrl: 'https://platform.openai.com/api-keys',
    },
    anthropic: {
      label: 'Anthropic Claude',
      models: [
        'claude-sonnet-5-5',
        'claude-haiku-5-5',
        'claude-opus-5-5',
        'claude-3-5-haiku-latest',
      ],
      needsKey: true,
      base: 'https://api.anthropic.com',
      keyUrl: 'https://console.anthropic.com/settings/keys',
    },
    custom: {
      label: 'OpenAI-compatible (Ollama, LM Studio, OpenRouter…)',
      models: ['llama3.1', 'qwen2.5-coder', 'mistral'],
      needsKey: false,
      base: 'http://localhost:11434/v1',
      keyUrl: '',
    },
  });
function aiConfig() {
  const saved = readLocal(AI_KEY, {});
  const provider = AI_PROVIDERS[saved.provider] ? saved.provider : 'gemini';
  const preset = /** @type {NonNullable<typeof AI_PROVIDERS[string]>} */ (AI_PROVIDERS[provider]);
  return /** @type {{ provider: string, key: string, model: string, schema: boolean, base: string, tools: string, sample: boolean }} */ ({
    provider,
    key: String(saved.key ?? ''),
    model: String(saved.model || preset.models[0] || ''),
    schema: saved.schema !== false,
    base: String(saved.base || preset.base),
    tools: String(saved.tools || 'auto'),
    sample: saved.sample === true,
  });
}
function aiConfigured() {
  const cfg = aiConfig();
  const preset = AI_PROVIDERS[cfg.provider];
  return (
    Boolean(cfg.key) ||
    (preset
      ? !preset.needsKey && Boolean(cfg.model) && Boolean(readLocal(AI_KEY, {}).provider)
      : false)
  );
}
function renderAiConfig(/** @type {HTMLElement} */ host, /** @type {() => void} */ onSaved) {
  const cfg = aiConfig();
  host.replaceChildren();
  host.append(
    el(
      'p',
      'ai-note',
      'Choose a provider and model, and add your own API key. The key stays in this browser and is sent only to the provider you pick. Row data is never sent; table and column names are, unless you turn that off.',
    ),
  );
  const field = (/** @type {string} */ label, /** @type {HTMLElement} */ control) => {
    const wrap = el('label', 'qb-f');
    wrap.append(el('span', '', label), control);
    return wrap;
  };
  const provider = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
  for (const [id, p] of Object.entries(AI_PROVIDERS)) provider.append(new Option(p.label, id));
  provider.value = cfg.provider;
  const model = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  model.value = cfg.model;
  model.setAttribute('list', 'ai-models');
  model.placeholder = 'Pick or type any model id';
  const list = el('datalist');
  list.id = 'ai-models';
  const fillModels = () => {
    list.replaceChildren();
    for (const m of AI_PROVIDERS[provider.value]?.models ?? []) list.append(new Option(m, m));
  };
  fillModels();
  const key = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  key.type = 'password';
  key.value = cfg.key;
  key.autocomplete = 'off';
  const base = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  base.value = cfg.base;
  const keyRow = field('API key', key);
  const baseRow = field('Endpoint (OpenAI-compatible)', base);
  const sync = () => {
    const preset = AI_PROVIDERS[provider.value];
    key.placeholder = preset?.needsKey ? 'API key' : 'API key (optional for local models)';
    baseRow.hidden = provider.value !== 'custom';
  };
  sync();
  provider.addEventListener('change', () => {
    const preset = AI_PROVIDERS[provider.value];
    model.value = preset?.models[0] ?? '';
    base.value = preset?.base ?? '';
    fillModels();
    sync();
  });
  const schema = /** @type {HTMLInputElement} */ (el('input'));
  schema.type = 'checkbox';
  schema.checked = cfg.schema;
  const schemaRow = el('label', 'sh-switch');
  schemaRow.append(schema, document.createTextNode('Share table and column names with the AI'));
  const toolSel = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
  for (const [v, l] of [
    ['auto', 'Auto-detect (recommended)'],
    ['native', 'Tool calling'],
    ['text', 'Text only'],
  ])
    toolSel.append(new Option(l, v));
  toolSel.value = cfg.tools;
  const sample = /** @type {HTMLInputElement} */ (el('input'));
  sample.type = 'checkbox';
  sample.checked = cfg.sample;
  const sampleRow = el('label', 'sh-switch');
  sampleRow.append(
    sample,
    document.createTextNode(
      'Let the agent read up to 5 sample rows (sends that data to the provider)',
    ),
  );
  host.append(
    field('Provider', provider),
    field('Model', model),
    list,
    keyRow,
    baseRow,
    field('Agent mode', toolSel),
    schemaRow,
    sampleRow,
  );
  const actions = el('div', 'sh-actions');
  actions.style.marginTop = '10px';
  const status = el('span', 'ai-note');
  actions.append(
    button(
      'Save',
      () => {
        writeLocal(AI_KEY, {
          provider: provider.value,
          key: key.value.trim(),
          model: model.value.trim() || AI_PROVIDERS[provider.value]?.models[0] || '',
          schema: schema.checked,
          base: base.value.trim(),
        });
        syncAiClass();
        toast(
          aiConfigured()
            ? `AI ready · ${AI_PROVIDERS[provider.value]?.label}`
            : 'AI is off (no key)',
        );
        onSaved();
      },
      'btn on',
    ),
    button('Remove key', () => {
      writeLocal(AI_KEY, { ...readLocal(AI_KEY, {}), key: '' });
      key.value = '';
      syncAiClass();
      onSaved();
    }),
    status,
  );
  host.append(actions);
  const url = AI_PROVIDERS[provider.value]?.keyUrl;
  if (url) {
    const link = el('p', 'ai-note');
    link.append(document.createTextNode('Get a key at '));
    const a = /** @type {HTMLAnchorElement} */ (el('a', 'url', url.replace('https://', '')));
    a.href = url;
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
    link.append(a);
    host.append(link);
  }
  enhanceControls(host);
}

document.addEventListener('cuadb:tx', (event) => {
  const id = String(/** @type {Any} */ (event).detail?.id ?? '');
  win.cuadbTx = id;
  const holder = document.getElementById('sql-tx');
  if (!holder) return;
  if (id) fetchInto(`/tx/status?tx=${encodeURIComponent(id)}`, '#sql-tx', 'innerHTML');
  else holder.replaceChildren();
});
// Clicking anywhere outside the tool sheet closes it; its state (builder choices, editor) is kept.
document.addEventListener('mousedown', (event) => {
  const sheet = document.getElementById('sqlsheet');
  if (!sheet || sheet.hidden || !(event.target instanceof Element)) return;
  if (event.target.closest('#sqlsheet, [data-sheet], .cs-menu, .dp, .lk-menu, #modal, #spop'))
    return;
  closeSheet();
});
syncAiClass();

/* ---- output panes: one tab per statement, summary per output ---- */
let paneIndex = 0;
function outPanes() {
  return /** @type {HTMLElement[]} */ (Array.from(document.querySelectorAll('#sqlout .opane')));
}
function activePane() {
  return /** @type {HTMLElement | null} */ (document.querySelector('#sqlout .opane:not([hidden])'));
}
function paneTable(/** @type {Element | null} */ pane) {
  const table = pane?.querySelector('table.grid');
  return table instanceof HTMLTableElement ? table : null;
}
function updateOutputSummary(/** @type {HTMLElement | null} */ pane) {
  const box = document.getElementById('sql-osum');
  if (!box) return;
  if (!pane) {
    box.hidden = true;
    return;
  }
  box.hidden = false;
  const failed = pane.dataset.status === 'failed';
  const st = document.getElementById('os-st');
  if (st) {
    st.textContent = failed ? '✕' : '✓';
    st.className = `os-st ${failed ? 'bad' : 'ok'}`;
  }
  const code = document.getElementById('os-sql');
  if (code) {
    code.textContent = pane.dataset.sql ?? '';
    code.title = pane.dataset.sql ?? '';
  }
  const meta = document.getElementById('os-meta');
  if (meta) meta.textContent = failed ? 'failed' : (pane.dataset.meta ?? '');
  const hasTable = Boolean(paneTable(pane));
  document.getElementById('sql-views')?.toggleAttribute('hidden', !hasTable);
  document.getElementById('sql-find')?.toggleAttribute('hidden', !hasTable);
  box.querySelectorAll('[data-osum-copy]').forEach((b) => b.toggleAttribute('hidden', !hasTable));
}
function setSqlView(/** @type {string} */ view) {
  const pane = activePane();
  document
    .querySelectorAll('#sql-views [data-sqlview]')
    .forEach((b) => b.classList.toggle('on', b.getAttribute('data-sqlview') === view));
  if (!pane) return;
  pane.dataset.pv = view;
  const find = document.getElementById('sql-find');
  if (find) find.hidden = view !== 'table' || !paneTable(pane);
  if (view === 'chart' || view === 'stats') renderViewPane(pane, view);
}
function renderViewPane(/** @type {HTMLElement} */ pane, /** @type {'chart' | 'stats'} */ view) {
  let host = pane.querySelector(':scope > .sqlview');
  if (!(host instanceof HTMLElement)) {
    host = el('div', 'sqlview');
    pane.append(host);
  }
  host.replaceChildren();
  const table = paneTable(pane);
  if (!table) {
    host.append(el('p', 'none', 'This statement returned no rows to analyse.'));
    return;
  }
  const data = tableData(table);
  const header = el('div', 'vh');
  if (view === 'stats') {
    host.append(header, statsTable(data));
    return;
  }
  const nums = numericColumns(data);
  if (!data.heads.includes(chartState.x))
    chartState.x = data.heads[nums.length && nums[0] === 0 && data.heads.length > 1 ? 1 : 0] ?? '';
  chartState.ys = chartState.ys.filter((y) => data.heads.includes(y));
  if (!chartState.ys.length)
    chartState.ys = nums
      .filter((i) => data.heads[i] !== chartState.x)
      .slice(0, 1)
      .map((i) => data.heads[i] ?? '');
  const type = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
  for (const [v, l] of [
    ['bar', 'Bar'],
    ['line', 'Line'],
    ['area', 'Area'],
    ['pie', 'Pie'],
  ])
    type.append(new Option(l, v));
  type.value = chartState.type;
  const xs = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
  data.heads.forEach((h) => xs.append(new Option(h, h)));
  xs.value = chartState.x;
  const chart = el('div');
  const redraw = () => drawChart(chart, data);
  type.addEventListener('change', () => {
    chartState.type = type.value;
    redraw();
  });
  xs.addEventListener('change', () => {
    chartState.x = xs.value;
    redraw();
  });
  const label = (/** @type {string} */ text, /** @type {HTMLElement} */ control) => {
    const l = el('label');
    l.append(document.createTextNode(text), control);
    return l;
  };
  header.append(label('Type', type), label('X axis', xs));
  const ys = el('div', 'ys');
  for (const i of nums) {
    const name = data.heads[i] ?? '';
    const l = el('label');
    const box = /** @type {HTMLInputElement} */ (el('input'));
    box.type = 'checkbox';
    box.checked = chartState.ys.includes(name);
    box.addEventListener('change', () => {
      chartState.ys = box.checked
        ? [...chartState.ys, name]
        : chartState.ys.filter((y) => y !== name);
      redraw();
    });
    l.append(box, document.createTextNode(name));
    ys.append(l);
  }
  header.append(ys);
  host.append(header, chart);
  if (!nums.length)
    chart.append(
      el(
        'p',
        'none',
        'No numeric column in this result. Try COUNT(*), SUM(…) or AVG(…) with GROUP BY.',
      ),
    );
  else redraw();
  enhanceControls(host);
}
function filterResults() {
  const pane = activePane();
  const input = document.getElementById('sql-find');
  const table = paneTable(pane);
  if (!table || !(input instanceof HTMLInputElement)) return;
  const needle = input.value.trim().toLowerCase();
  let shown = 0;
  const rows = Array.from(table.querySelectorAll('tbody tr'));
  rows.forEach((tr) => {
    const ok = !needle || (tr.textContent ?? '').toLowerCase().includes(needle);
    /** @type {HTMLElement} */ (tr).hidden = !ok;
    if (ok) shown += 1;
  });
  const meta = document.getElementById('os-meta');
  if (meta && pane)
    meta.textContent = needle ? `${shown} of ${rows.length} rows match` : (pane.dataset.meta ?? '');
}
function selectPane(/** @type {number} */ index) {
  const list = outPanes();
  if (!list.length) return updateOutputSummary(null);
  paneIndex = Math.max(0, Math.min(index, list.length - 1));
  list.forEach((p, i) => {
    p.hidden = i !== paneIndex;
  });
  document.querySelectorAll('#sql-otabs .otab').forEach((b, i) => {
    b.classList.toggle('on', i === paneIndex);
    b.setAttribute('aria-selected', String(i === paneIndex));
  });
  const pane = list[paneIndex];
  if (!pane) return;
  const find = document.getElementById('sql-find');
  if (find instanceof HTMLInputElement) find.value = '';
  // Tables in a hidden pane are laid out only when they are first shown.
  enhanceTables(pane);
  decorateGrid(pane);
  formatDates(pane);
  formatNumbers(pane);
  initHScroll(pane);
  updateOutputSummary(pane);
  setSqlView(pane.dataset.pv ?? 'table');
}
function buildOutputTabs(/** @type {HTMLElement} */ out) {
  const nav = document.getElementById('sql-otabs');
  if (!nav) return;
  const list = outPanes();
  nav.replaceChildren();
  nav.hidden = list.length < 2;
  list.forEach((pane, i) => {
    const sql = pane.dataset.sql ?? '';
    const table = /\b(?:from|into|update|table|join)\s+("?[\w.]+"?(?:\."?[\w]+"?)?)/i
      .exec(sql)?.[1]
      ?.replace(/"/g, '');
    const label = `${pane.dataset.kind ?? 'SQL'}${table ? ` ${table}` : ''}`;
    const tab = /** @type {HTMLButtonElement} */ (el('button', 'otab'));
    tab.type = 'button';
    tab.setAttribute('role', 'tab');
    tab.title = sql;
    const dot = el('i', `od ${pane.dataset.status === 'failed' ? 'bad' : 'ok'}`);
    const text = el(
      'span',
      '',
      `${i + 1} · ${label.length > 30 ? `${label.slice(0, 29)}…` : label}`,
    );
    tab.append(dot, text);
    tab.addEventListener('click', () => selectPane(i));
    nav.append(tab);
  });
  void out;
}
function onSqlOutput(/** @type {HTMLElement} */ out) {
  const sum = document.getElementById('sql-sum');
  const run = out.querySelector('.orun');
  const tabs = document.getElementById('sql-otabs');
  if (!(run instanceof HTMLElement)) {
    // An error fragment from the server (not a result): show it as it is.
    if (tabs) tabs.hidden = true;
    updateOutputSummary(null);
    const text = (out.textContent ?? '').replace(/Copy error/g, '').trim();
    if (sum) {
      sum.textContent = text ? `✕ ${text.split('\n')[0]}` : '✕ The statement failed';
      sum.className = 'sql-sum failed';
    }
    recordHistory(sqlSubmitted, false, text.split('\n')[0] ?? '');
    noteRunResult(false, sqlSubmitted, '', text.slice(0, 400));
    return;
  }
  const list = outPanes();
  const failedAny =
    run.dataset.status === 'failed' || list.some((p) => p.dataset.status === 'failed');
  const ms = sqlStarted ? Math.round(performance.now() - sqlStarted) : 0;
  const headline = run.dataset.headline || list[0]?.dataset.meta || 'Done';
  if (sum) {
    sum.textContent = `${failedAny ? '✕' : '✓'} ${headline}`;
    sum.className = `sql-sum ${failedAny ? 'failed' : 'ok'}`;
    sum.title = `${headline}${ms ? ` · ${ms} ms round trip` : ''}`;
  }
  buildOutputTabs(out);
  list.forEach((pane) => {
    const table = paneTable(pane);
    if (!table || table.dataset.sortable) return;
    table.dataset.sortable = '1';
    table.querySelector('thead')?.addEventListener('click', (event) => {
      const th = event.target instanceof Element ? event.target.closest('th') : null;
      if (
        th instanceof HTMLElement &&
        !th.classList.contains('rn') &&
        !(event.target instanceof Element && event.target.closest('.tyb, .rzh'))
      )
        sortResultTable(table, th);
    });
  });
  const firstFailed = list.findIndex((p) => p.dataset.status === 'failed');
  selectPane(firstFailed >= 0 ? firstFailed : 0);
  recordHistory(sqlSubmitted, !failedAny, headline);
  noteRunResult(
    !failedAny,
    sqlSubmitted,
    headline,
    (out.querySelector('.opane[data-status="failed"] .notice')?.textContent ?? '')
      .replace(/Copy error|✦ Fix with AI/g, '')
      .trim(),
  );
  if (failedAny && aiConfigured()) {
    out.querySelectorAll('.opane[data-status="failed"] .notice').forEach((box) => {
      if (box.querySelector('.fix-ai')) return;
      box.append(
        button(
          '✦ Fix with AI',
          () =>
            askAi(
              'Fix this failing query. Explain the cause in one sentence, then give the corrected SQL.',
              `Query:\n${sqlSubmitted}\n\nError:\n${(box.textContent ?? '').replace(/Copy error|✦ Fix with AI/g, '').trim()}`,
            ),
          'btn fix-ai',
        ),
      );
    });
  }
}
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const copy = target.closest('[data-osum-copy]');
  if (copy instanceof HTMLElement) {
    const table = paneTable(activePane());
    if (!table) return;
    const format = /** @type {'csv' | 'tsv' | 'json'} */ (copy.dataset.osumCopy ?? 'csv');
    void copyText(tableText(table, format), `${format.toUpperCase()} copied`);
  }
});

/* ------------------------------------------------------------ AI agent: chat, tools, history */
const CAP_KEY = 'cuadb:aicap';
const CHAT_PREFIX = 'cuadb:aichat:';
/** @typedef {{ id: string, name: string, args: any }} AiCall */
/** @typedef {{ role: 'user' | 'assistant' | 'tool', text?: string, calls?: AiCall[], raw?: any, results?: Array<{ id: string, name: string, result: any }> }} TurnMsg */
/** @typedef {{ role: 'user' | 'assistant' | 'note', text: string, tools?: Array<{ name: string, args: any, summary: string }>, suggestions?: string[] }} ChatMsg */
/** @typedef {{ id: string, title: string, t: number, msgs: ChatMsg[] }} Chat */
const agent = {
  running: false,
  /** @type {AbortController | null} */
  abort: null,
  /** @type {Chat | null} */
  chat: null,
  /** @type {any} */
  schema: null,
  schemaFor: '',
  /** @type {{ sql: string, ok: boolean, summary: string, error: string } | null} */
  lastRun: null,
};
const profileBase = () => String(win.cuadbProfile || 'default').split('~')[0] ?? 'default';
const chatsKey = () => `${CHAT_PREFIX}${profileBase()}`;
function loadChats() {
  return /** @type {Chat[]} */ (readLocal(chatsKey(), []));
}
function saveChats(/** @type {Chat[]} */ chats) {
  writeLocal(chatsKey(), chats.slice(0, 20));
}
function currentChat() {
  if (agent.chat) return agent.chat;
  const chats = loadChats();
  agent.chat = chats[0] ?? newChat();
  return agent.chat;
}
function newChat() {
  /** @type {Chat} */
  const chat = {
    id: Math.random().toString(36).slice(2, 10),
    title: 'New chat',
    t: Date.now(),
    msgs: [],
  };
  agent.chat = chat;
  return chat;
}
function persistChat() {
  const chat = agent.chat;
  if (!chat || !chat.msgs.length) return;
  const chats = loadChats().filter((c) => c.id !== chat.id);
  chat.t = Date.now();
  saveChats([chat, ...chats]);
}

/* ---- capability: do native tools work with this provider and model? ---- */
const capKey = () => `${aiConfig().provider}:${aiConfig().model}`;
function toolMode() {
  const forced = aiConfig().tools;
  if (forced === 'native' || forced === 'text') return forced;
  return /** @type {Record<string, string>} */ (readLocal(CAP_KEY, {}))[capKey()] === 'text'
    ? 'text'
    : 'native';
}
function rememberCap(/** @type {'native' | 'text'} */ mode) {
  const all = /** @type {Record<string, string>} */ (readLocal(CAP_KEY, {}));
  all[capKey()] = mode;
  writeLocal(CAP_KEY, all);
}

/* ---- the tools the agent can call ---- */
const TOOLS = /** @type {Array<{ name: string, description: string, parameters: any }>} */ ([
  {
    name: 'list_schemas',
    description:
      'List every schema in the connected database with how many tables and views each holds.',
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'list_tables',
    description: 'List the tables and views inside one schema.',
    parameters: {
      type: 'object',
      properties: { schema: { type: 'string' } },
      required: ['schema'],
    },
  },
  {
    name: 'describe_table',
    description:
      'Get the columns (name, type, nullability, primary key) and foreign keys of one table or view. Always do this before writing SQL that uses a table.',
    parameters: {
      type: 'object',
      properties: { schema: { type: 'string' }, table: { type: 'string' } },
      required: ['schema', 'table'],
    },
  },
  {
    name: 'search_schema',
    description:
      'Find schemas, tables and columns whose names contain a word (for example "loan" or "email"). Use this when you are not sure where something lives.',
    parameters: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] },
  },
  {
    name: 'validate_sql',
    description:
      'Dry-run a query with EXPLAIN to learn whether it is valid, without changing data or returning rows. Returns ok or the database error. Call it on every query before you show it to the user.',
    parameters: { type: 'object', properties: { sql: { type: 'string' } }, required: ['sql'] },
  },
  {
    name: 'sample_rows',
    description:
      'Read up to 5 rows from a table to see what the data looks like. Only works if the user allowed it in AI settings.',
    parameters: {
      type: 'object',
      properties: {
        schema: { type: 'string' },
        table: { type: 'string' },
        limit: { type: 'integer' },
      },
      required: ['schema', 'table'],
    },
  },
  {
    name: 'get_last_run',
    description:
      'Get the outcome (success or the error message, and row counts, never the rows) of the last query the user ran in the editor.',
    parameters: { type: 'object', properties: {} },
  },
  {
    name: 'get_editor_sql',
    description: 'Get the SQL currently in the editor.',
    parameters: { type: 'object', properties: {} },
  },
]);
async function getJson(
  /** @type {string} */ path,
  /** @type {Record<string, string>} */ params = {},
) {
  const q = new URLSearchParams(params);
  const base = String(win.cuadbProfile || '');
  if (base) q.set('profile', base);
  const response = await fetch(`${path}?${q}`, { cache: 'no-store' });
  if (!response.ok) throw new Error(`${path} failed (${response.status})`);
  return response.json();
}
async function aiSchemaTree() {
  const key = String(win.cuadbProfile || '');
  if (agent.schema && agent.schemaFor === key) return agent.schema;
  agent.schema = await getJson('/ai/schema');
  agent.schemaFor = key;
  return agent.schema;
}
async function aiRunSql(/** @type {string} */ sql) {
  const body = new URLSearchParams({ sql });
  const profile = String(win.cuadbProfile || '');
  if (profile) body.set('profile', profile);
  const response = await fetch('/sql/execute', {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded', 'x-csrf-token': csrfToken() },
    body,
  });
  const html = await response.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const failedPane = doc.querySelector(
    '.opane[data-status="failed"] .notice, .notice.error, .notice.denied, .cuadb-error',
  );
  return {
    ok: response.ok && !failedPane,
    error: failedPane
      ? (failedPane.textContent ?? '')
          .replace(/Copy error|✦ Fix with AI/g, '')
          .trim()
          .slice(0, 600)
      : '',
    doc,
  };
}
const clip = (/** @type {any} */ value, max = 6000) => {
  const text = typeof value === 'string' ? value : JSON.stringify(value);
  return text.length > max ? `${text.slice(0, max)}… (truncated)` : text;
};
/** @type {(name: string, args: any) => Promise<any>} */
var runTool = async function (/** @type {string} */ name, /** @type {any} */ args) {
  const a = args ?? {};
  switch (name) {
    case 'list_schemas': {
      const tree = await aiSchemaTree();
      return {
        schemas: tree.schemas.map((/** @type {any} */ s) => ({
          schema: s.name,
          tables: s.tables.length,
        })),
        truncated: tree.truncated,
      };
    }
    case 'list_tables': {
      const tree = await aiSchemaTree();
      const found = tree.schemas.find(
        (/** @type {any} */ s) => s.name.toLowerCase() === String(a.schema ?? '').toLowerCase(),
      );
      if (!found) return { error: `No schema named "${a.schema}". Call list_schemas.` };
      return {
        schema: found.name,
        tables: found.tables.map(
          (/** @type {any} */ t) => `${t.n}${t.k === 'table' ? '' : ` (${t.k})`}`,
        ),
      };
    }
    case 'describe_table': {
      try {
        const d = await getJson('/ai/describe', {
          schema: String(a.schema ?? ''),
          relation: String(a.table ?? a.relation ?? ''),
        });
        return {
          table: `${d.schema}.${d.relation}`,
          columns: d.columns.map(
            (/** @type {any} */ c) =>
              `${c.name} ${c.type}${c.primary_key ? ' PK' : ''}${c.nullable ? '' : ' NOT NULL'}${c.comment ? ` -- ${c.comment}` : ''}`,
          ),
          foreign_keys: d.foreign_keys.map(
            (/** @type {any} */ f) => `${f.columns.join(', ')} -> ${f.references}`,
          ),
        };
      } catch {
        return {
          error: `Could not describe ${a.schema}.${a.table}. Check the names with list_tables or search_schema.`,
        };
      }
    }
    case 'search_schema': {
      try {
        const r = await getJson('/ai/search', { q: String(a.query ?? '') });
        return {
          hits: r.hits.map((/** @type {any} */ h) =>
            h.column
              ? `${h.schema}.${h.table}.${h.column} (${h.type})`
              : `${h.schema}.${h.table} [${h.kind}] matched ${h.match}`,
          ),
        };
      } catch {
        return { error: 'Search failed. Try list_schemas and list_tables.' };
      }
    }
    case 'validate_sql': {
      const sql = String(a.sql ?? '').trim();
      if (!sql) return { ok: false, error: 'No SQL given.' };
      const statements = splitStatementsLite(sql).slice(0, 5);
      const notes = [];
      for (const statement of statements) {
        const first = statement.trim().split(/\s+/)[0]?.toLowerCase() ?? '';
        if (!['select', 'with', 'insert', 'update', 'delete', 'values', 'table'].includes(first)) {
          notes.push(`${first.toUpperCase()}: cannot be dry-run; syntax not checked`);
          continue;
        }
        const r = await aiRunSql(`EXPLAIN ${statement.replace(/;+\s*$/, '')}`);
        if (!r.ok)
          return {
            ok: false,
            error: r.error || 'The database rejected the query.',
            statement: statement.slice(0, 200),
          };
      }
      return {
        ok: true,
        note: notes.length
          ? notes.join('; ')
          : 'Valid: the database planned the query without error.',
      };
    }
    case 'sample_rows': {
      if (!aiConfig().sample)
        return { error: 'The user has not allowed row samples. Work from the schema only.' };
      const limit = Math.min(Math.max(Number(a.limit) || 3, 1), 5);
      const table = `${sqlQuote(String(a.schema ?? ''))}.${sqlQuote(String(a.table ?? ''))}`;
      const r = await aiRunSql(`SELECT * FROM ${table} LIMIT ${limit}`);
      if (!r.ok) return { error: r.error };
      const t = r.doc.querySelector('table.grid');
      if (!(t instanceof HTMLTableElement)) return { rows: [] };
      const data = tableData(t);
      return {
        columns: data.heads.slice(0, 12),
        rows: data.rows
          .slice(0, limit)
          .map((row) => row.slice(0, 12).map((v) => (v ?? 'NULL').slice(0, 60))),
      };
    }
    case 'get_last_run':
      return agent.lastRun ?? { note: 'Nothing has been run in the editor yet.' };
    case 'get_editor_sql':
      return { sql: editorSql().slice(0, 4000) };
    default:
      return { error: `Unknown tool ${name}.` };
  }
};
function splitStatementsLite(/** @type {string} */ sql) {
  const out = [];
  let cur = '';
  let quote = '';
  for (const ch of sql) {
    if (quote) {
      cur += ch;
      if (ch === quote) quote = '';
    } else if (ch === "'" || ch === '"') {
      quote = ch;
      cur += ch;
    } else if (ch === ';') {
      if (cur.trim()) out.push(cur.trim());
      cur = '';
    } else cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

/* ---- talking to the model (native tools or a text protocol) ---- */
async function agentSystem(/** @type {boolean} */ native) {
  let overview = '';
  try {
    const tree = await aiSchemaTree();
    const count = tree.schemas.reduce(
      (/** @type {number} */ n, /** @type {any} */ s) => n + s.tables.length,
      0,
    );
    overview = tree.schemas
      .map((/** @type {any} */ s) => `${s.name} (${s.tables.length})`)
      .join(', ');
    overview = `The database has ${tree.schemas.length} schema(s) and ${count} relation(s): ${overview.slice(0, 1500)}`;
  } catch {
    overview = 'Schema overview unavailable; use the tools.';
  }
  const lines = [
    'You are the SQL agent inside CuaDB, a database explorer used by developers.',
    `Dialect: ${isSqlite() ? 'SQLite' : 'PostgreSQL'}. ${overview}`,
    'How to work: never guess table or column names. Use the tools to look at the schema, search for what the user means, and describe every table you will use. Then write the SQL, call validate_sql, and if it fails read the error, fix it and validate again. Only then show the query.',
    'Prefer read-only SELECT. Do not change data unless the user clearly asks, and say plainly when a query changes data.',
    'Answer briefly. Show the final SQL in exactly one ```sql block. After the answer add a ```suggestions block with 2 to 4 short messages the user might send next, one per line, written as the user would say them.',
    'If the user tells you a query failed or gives you an error, use the tools to find the cause and give a corrected, validated query.',
    'You can explore relationships: use find_relationships to see what a table points to and what points to it, find_join_path to connect any two tables through foreign keys (also across schemas and through intermediate tables), and suggest_joins to propose inferred joins when no foreign-key path exists; say clearly when a join is inferred rather than declared. You may join tables directly to answer a question.',
    'For analytics, insights, a dashboard, or when the user asks what they can learn, explore first (analytics_ideas, find_join_path, describe_table), validate every query, then answer with a short explanation and ONE ```analytics block of JSON: {"title":"…","cards":[{"title":"…","sql":"SELECT …","chart":"bar|line|area|pie|stat|table","x":"column","y":["column"],"note":"how to read it"}]}. Use at most 6 cards. Every card is a single read-only SELECT; alias columns so x and y match them (stat cards return one value). The app runs the cards and draws the charts on the user\'s screen, so you do not need to read the data. Offer to interpret results once the user runs them.',
    'Unless data-aware mode is on, you cannot read rows: reason from schema, column names and types, and the aggregate queries you propose.',
  ];
  if (!native) {
    lines.push(
      'You can call tools. To call one, reply with ONLY a line like: TOOL: {"name":"describe_table","args":{"schema":"public","table":"orders"}} and nothing else. You will receive the result in the next message and can call another tool. When you have enough information, answer normally.',
      `Tools: ${TOOLS.map((t) => `${t.name}(${Object.keys(t.parameters.properties).join(', ')}) - ${t.description}`).join(' | ')}`,
    );
  }
  return lines.join('\n');
}
/** @returns {Promise<{ text: string, calls: AiCall[], raw: any }>} */
async function modelStep(
  /** @type {string} */ system,
  /** @type {TurnMsg[]} */ turn,
  /** @type {boolean} */ native,
  /** @type {AbortSignal} */ signal,
) {
  const cfg = aiConfig();
  const preset = AI_PROVIDERS[cfg.provider];
  if (!preset) throw new Error('Choose an AI provider first.');
  const post = async (
    /** @type {string} */ url,
    /** @type {Record<string, string>} */ headers,
    /** @type {any} */ body,
  ) => {
    const response = await fetch(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...headers },
      body: JSON.stringify(body),
      signal,
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) {
      const message = data?.error?.message ?? data?.error ?? data?.message;
      const error = new Error(
        typeof message === 'string' ? message : `The AI request failed (${response.status}).`,
      );
      /** @type {any} */ (error).status = response.status;
      throw error;
    }
    return data;
  };
  const fnDecl = TOOLS.map((t) => ({
    name: t.name,
    description: t.description,
    parameters: t.parameters,
  }));
  if (cfg.provider === 'gemini') {
    const contents = turn.map((m) => {
      if (m.role === 'user') return { role: 'user', parts: [{ text: m.text ?? '' }] };
      if (m.role === 'assistant')
        return { role: 'model', parts: m.raw ?? [{ text: m.text ?? '' }] };
      return {
        role: 'user',
        parts: (m.results ?? []).map((r) => ({
          functionResponse: { name: r.name, response: { result: r.result } },
        })),
      };
    });
    const body = {
      systemInstruction: { parts: [{ text: system }] },
      contents,
      generationConfig: { temperature: 0.2, maxOutputTokens: 2500 },
      ...(native ? { tools: [{ functionDeclarations: fnDecl }] } : {}),
    };
    const data = await post(
      `${preset.base}/v1beta/models/${encodeURIComponent(cfg.model)}:generateContent?key=${encodeURIComponent(cfg.key)}`,
      {},
      body,
    );
    const parts = data?.candidates?.[0]?.content?.parts ?? [];
    return {
      text: parts.map((/** @type {any} */ p) => p.text ?? '').join(''),
      calls: parts
        .filter((/** @type {any} */ p) => p.functionCall)
        .map((/** @type {any} */ p, /** @type {number} */ i) => ({
          id: `g${i}`,
          name: p.functionCall.name,
          args: p.functionCall.args ?? {},
        })),
      raw: parts,
    };
  }
  if (cfg.provider === 'anthropic') {
    const messages = turn.map((m) => {
      if (m.role === 'user') return { role: 'user', content: m.text ?? '' };
      if (m.role === 'assistant') return { role: 'assistant', content: m.raw ?? m.text ?? '' };
      return {
        role: 'user',
        content: (m.results ?? []).map((r) => ({
          type: 'tool_result',
          tool_use_id: r.id,
          content: clip(r.result),
        })),
      };
    });
    const data = await post(
      `${preset.base}/v1/messages`,
      {
        'x-api-key': cfg.key,
        'anthropic-version': '2023-06-01',
        'anthropic-dangerous-direct-browser-access': 'true',
      },
      {
        model: cfg.model,
        max_tokens: 2500,
        temperature: 0.2,
        system,
        messages,
        ...(native
          ? {
              tools: TOOLS.map((t) => ({
                name: t.name,
                description: t.description,
                input_schema: t.parameters,
              })),
            }
          : {}),
      },
    );
    const blocks = data?.content ?? [];
    return {
      text: blocks
        .filter((/** @type {any} */ b) => b.type === 'text')
        .map((/** @type {any} */ b) => b.text)
        .join(''),
      calls: blocks
        .filter((/** @type {any} */ b) => b.type === 'tool_use')
        .map((/** @type {any} */ b) => ({ id: b.id, name: b.name, args: b.input ?? {} })),
      raw: blocks,
    };
  }
  // OpenAI and OpenAI-compatible servers
  const base = (cfg.provider === 'custom' ? cfg.base : preset.base).replace(/\/$/, '');
  /** @type {Record<string, string>} */
  const headers = cfg.key ? { authorization: `Bearer ${cfg.key}` } : {};
  /** @type {any[]} */
  const messages = [{ role: 'system', content: system }];
  for (const m of turn) {
    if (m.role === 'user') messages.push({ role: 'user', content: m.text ?? '' });
    else if (m.role === 'assistant')
      messages.push(m.raw ?? { role: 'assistant', content: m.text ?? '' });
    else
      for (const r of m.results ?? [])
        messages.push({ role: 'tool', tool_call_id: r.id, content: clip(r.result) });
  }
  const data = await post(`${base}/chat/completions`, headers, {
    model: cfg.model,
    temperature: 0.2,
    messages,
    ...(native
      ? {
          tools: TOOLS.map((t) => ({
            type: 'function',
            function: { name: t.name, description: t.description, parameters: t.parameters },
          })),
        }
      : {}),
  });
  const message = data?.choices?.[0]?.message ?? {};
  return {
    text: message.content ?? '',
    calls: (message.tool_calls ?? []).map((/** @type {any} */ c) => {
      let args = {};
      try {
        args = JSON.parse(c.function?.arguments || '{}');
      } catch {
        args = {};
      }
      return { id: c.id, name: c.function?.name, args };
    }),
    raw: message,
  };
}
/** Text protocol for models without tool calling: `TOOL: {...}` lines. */
function parseTextCall(/** @type {string} */ text) {
  const m = /TOOL:\s*(\{[\s\S]*\})/.exec(text);
  if (!m) return null;
  try {
    const parsed = JSON.parse(m[1] ?? '');
    if (parsed && typeof parsed.name === 'string')
      return { id: 't0', name: parsed.name, args: parsed.args ?? {} };
  } catch {
    /* fall through */
  }
  return null;
}
const toolsUnsupported = (/** @type {any} */ error) =>
  /tool|function|not support|unsupported|schema/i.test(String(error?.message ?? '')) &&
  [400, 404, 422, 501].includes(Number(error?.status ?? 400));

/* ---- the agent turn ---- */
/** @returns {Promise<{ text: string, tools: Array<{ name: string, args: any, summary: string }> }>} */
async function runAgentTurn(
  /** @type {TurnMsg[]} */ turn,
  /** @type {(tool: { name: string, args: any, summary: string, done: boolean }) => void} */ onTool,
  /** @type {AbortSignal} */ signal,
) {
  let native = toolMode() === 'native';
  const used = /** @type {Array<{ name: string, args: any, summary: string }>} */ ([]);
  for (let step = 0; step < 8; step += 1) {
    const system = await agentSystem(native);
    /** @type {{ text: string, calls: AiCall[], raw: any }} */
    let reply;
    try {
      reply = await modelStep(system, turn, native, signal);
    } catch (error) {
      if (
        native &&
        step === 0 &&
        toolMode() === 'native' &&
        aiConfig().tools === 'auto' &&
        toolsUnsupported(error)
      ) {
        rememberCap('text');
        native = false;
        syncAgentBadge();
        continue;
      }
      throw error;
    }
    const calls = native
      ? reply.calls
      : [parseTextCall(reply.text)].filter(Boolean).map((c) => /** @type {AiCall} */ (c));
    if (!calls.length) {
      if (native && aiConfig().tools === 'auto') rememberCap('native');
      return { text: reply.text, tools: used };
    }
    turn.push({ role: 'assistant', text: reply.text, calls, raw: native ? reply.raw : undefined });
    const results = [];
    for (const call of calls) {
      onTool({ name: call.name, args: call.args, summary: 'working…', done: false });
      let result;
      try {
        result = await runTool(call.name, call.args);
      } catch (error) {
        result = { error: error instanceof Error ? error.message : 'The tool failed.' };
      }
      const summary = summarizeTool(call.name, result);
      used.push({ name: call.name, args: call.args, summary });
      onTool({ name: call.name, args: call.args, summary, done: true });
      results.push({ id: call.id, name: call.name, result });
    }
    if (native) turn.push({ role: 'tool', results });
    else
      turn.push({
        role: 'user',
        text: results.map((r) => `TOOL RESULT (${r.name}): ${clip(r.result, 4000)}`).join('\n'),
      });
  }
  return {
    text: 'I looked at the schema several times but could not finish. Tell me a bit more about what you want, or name the table.',
    tools: used,
  };
}
var summarizeTool = function (/** @type {string} */ name, /** @type {any} */ result) {
  if (result?.error) return `error: ${String(result.error).slice(0, 80)}`;
  if (name === 'list_schemas') return `${result.schemas?.length ?? 0} schemas`;
  if (name === 'list_tables') return `${result.tables?.length ?? 0} tables`;
  if (name === 'describe_table')
    return `${result.columns?.length ?? 0} columns, ${result.foreign_keys?.length ?? 0} foreign keys`;
  if (name === 'search_schema') return `${result.hits?.length ?? 0} matches`;
  if (name === 'validate_sql')
    return result.ok ? 'valid ✓' : `invalid: ${String(result.error).slice(0, 70)}`;
  if (name === 'sample_rows') return `${result.rows?.length ?? 0} rows`;
  return 'done';
};
var toolLabel = function (/** @type {string} */ name, /** @type {any} */ args) {
  const a = args ?? {};
  switch (name) {
    case 'list_schemas':
      return 'Listing schemas';
    case 'list_tables':
      return `Listing tables in ${a.schema}`;
    case 'describe_table':
      return `Reading ${a.schema}.${a.table ?? a.relation}`;
    case 'search_schema':
      return `Searching for “${a.query}”`;
    case 'validate_sql':
      return 'Checking the query';
    case 'sample_rows':
      return `Sampling ${a.schema}.${a.table}`;
    case 'get_last_run':
      return 'Checking your last run';
    case 'get_editor_sql':
      return 'Reading the editor';
    default:
      return name;
  }
};

/* ---- chat rendering ---- */
function inlineMarkdown(/** @type {HTMLElement} */ host, /** @type {string} */ text) {
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    host.append(document.createTextNode(text.slice(last, m.index)));
    const token = m[0];
    if (token.startsWith('`')) host.append(el('code', 'cd', token.slice(1, -1)));
    else host.append(el('strong', '', token.slice(2, -2)));
    last = (m.index ?? 0) + token.length;
  }
  host.append(document.createTextNode(text.slice(last)));
}
var renderMarkdown = function (/** @type {HTMLElement} */ host, /** @type {string} */ text) {
  const parts = text.split(/```(?:sql|SQL|postgresql|sqlite)?\n?([\s\S]*?)```/);
  parts.forEach((part, index) => {
    const value = part.trim();
    if (!value) return;
    if (index % 2 === 1) {
      const box = el('div', 'ai-code');
      box.append(el('pre', '', value));
      const act = el('div', 'sh-actions');
      act.append(
        button('Use', () => setEditorSql(value)),
        button('Add', () => setEditorSql(value, 'append')),
        button('Copy', () => void copyText(value, 'SQL copied')),
        button(
          'Run',
          () => {
            setEditorSql(value);
            /** @type {HTMLFormElement | null} */ (
              document.getElementById('sqlform')
            )?.requestSubmit();
          },
          'btn on',
        ),
      );
      box.append(act);
      host.append(box);
      return;
    }
    for (const block of value.split(/\n{2,}/)) {
      const lines = block.split('\n');
      if (lines.every((l) => /^\s*[-*•]\s+/.test(l))) {
        const ul = el('ul', 'ai-ul');
        for (const l of lines) {
          const li = el('li');
          inlineMarkdown(li, l.replace(/^\s*[-*•]\s+/, ''));
          ul.append(li);
        }
        host.append(ul);
      } else {
        const p = el('p', 'ai-msg');
        inlineMarkdown(p, block);
        host.append(p);
      }
    }
  });
};
function parseSuggestions(/** @type {string} */ text) {
  const m = /```suggestions\n([\s\S]*?)```/i.exec(text);
  const items = m
    ? (m[1] ?? '')
        .split('\n')
        .map((l) => l.replace(/^\s*[-*\d.)]+\s*/, '').trim())
        .filter(Boolean)
        .slice(0, 4)
    : [];
  return { clean: text.replace(/```suggestions\n[\s\S]*?```/gi, '').trim(), items };
}
function fallbackSuggestions(/** @type {string} */ text) {
  if (/```sql/i.test(text))
    return ['Run it', 'Explain this query', 'Make it faster', 'Add a filter'];
  return ['What tables are in this database?', 'Explain my current query', 'Find tables about…'];
}
function toolChips(/** @type {Array<{ name: string, args: any, summary: string }>} */ tools) {
  const box = el('details', 'ai-tools');
  const ok = tools.filter((t) => !/^error/.test(t.summary)).length;
  box.append(
    el(
      'summary',
      '',
      `Used ${tools.length} tool${tools.length === 1 ? '' : 's'}${ok === tools.length ? '' : ` · ${tools.length - ok} failed`}`,
    ),
  );
  for (const t of tools) {
    const row = el('div', 'ai-tool');
    row.append(el('span', '', toolLabel(t.name, t.args)), el('small', 'mu', t.summary));
    box.append(row);
  }
  return box;
}
let chatHost = /** @type {HTMLElement | null} */ (null);
function scrollChat() {
  const log = chatHost?.querySelector('.ai-log');
  if (log) log.scrollTop = log.scrollHeight;
}
function appendBubble(/** @type {ChatMsg} */ msg, /** @type {boolean} */ last) {
  const log = chatHost?.querySelector('.ai-log');
  if (!log) return;
  const row = el('div', `ai-row ${msg.role}`);
  if (msg.role === 'user') {
    row.append(el('div', 'ai-bub', msg.text));
  } else if (msg.role === 'note') {
    row.append(el('div', 'ai-note-row', msg.text));
  } else {
    const bub = el('div', 'ai-bub');
    if (msg.tools?.length) bub.append(toolChips(msg.tools));
    renderMarkdown(bub, msg.text);
    row.append(bub);
    if (last && msg.suggestions?.length) {
      const chips = el('div', 'ai-chips');
      msg.suggestions.forEach((s, i) => {
        const chip = button(s, () => void sendChat(s), 'ai-chip');
        chip.style.setProperty('--i', String(i));
        chips.append(chip);
      });
      row.append(chips);
    }
  }
  log.append(row);
  scrollChat();
}
function syncAgentBadge() {
  const badge = chatHost?.querySelector('.ai-cap');
  if (!badge) return;
  const mode = toolMode();
  badge.textContent = mode === 'native' ? 'Tools' : 'Text mode';
  badge.setAttribute(
    'title',
    mode === 'native'
      ? 'This model calls tools to read your schema.'
      : 'This model has no tool calling, so the agent asks for information through text commands.',
  );
  badge.classList.toggle('text', mode === 'text');
}
async function sendChat(/** @type {string} */ raw) {
  const text = raw.trim();
  if (!text || agent.running || !chatHost) return;
  const chat = currentChat();
  const log = chatHost.querySelector('.ai-log');
  if (!log) return;
  log.querySelector('.ai-empty')?.remove();
  chat.msgs.forEach((m) => {
    if (m.suggestions) m.suggestions = undefined;
  });
  log.querySelectorAll('.ai-chips').forEach((c) => c.remove());
  if (chat.title === 'New chat') chat.title = text.slice(0, 48);
  /** @type {ChatMsg} */
  const userMsg = { role: 'user', text };
  chat.msgs.push(userMsg);
  appendBubble(userMsg, false);
  persistChat();
  agent.running = true;
  agent.abort = new AbortController();
  const send = /** @type {HTMLButtonElement | null} */ (chatHost.querySelector('.ai-send'));
  if (send) send.dataset.state = 'stop';
  const live = el('div', 'ai-row assistant');
  const liveBub = el('div', 'ai-bub');
  const status = el('div', 'ai-live');
  status.textContent = 'Thinking…';
  const dots = el('span', 'ai-dots');
  dots.append(el('i'), el('i'), el('i'));
  liveBub.append(dots, status);
  live.append(liveBub);
  log.append(live);
  scrollChat();
  // The conversation so far, plus notes about what happened to queries the agent proposed.
  /** @type {TurnMsg[]} */
  const turn = [];
  for (const m of chat.msgs.slice(-14)) {
    if (m.role === 'note') turn.push({ role: 'user', text: `[Editor event] ${m.text}` });
    else turn.push({ role: m.role === 'user' ? 'user' : 'assistant', text: m.text });
  }
  try {
    const result = await runAgentTurn(
      turn,
      (tool) => {
        status.textContent = tool.done
          ? `${toolLabel(tool.name, tool.args)} · ${tool.summary}`
          : `${toolLabel(tool.name, tool.args)}…`;
        scrollChat();
      },
      agent.abort.signal,
    );
    const { clean, items } = parseSuggestions(result.text);
    /** @type {ChatMsg} */
    const reply = {
      role: 'assistant',
      text: clean || 'Done.',
      tools: result.tools,
      suggestions: items.length ? items : fallbackSuggestions(clean),
    };
    chat.msgs.push(reply);
    live.remove();
    appendBubble(reply, true);
  } catch (error) {
    live.remove();
    const aborted = error instanceof DOMException && error.name === 'AbortError';
    const raw = error instanceof Error ? error.message : '';
    const message = aborted
      ? 'Stopped.'
      : error instanceof TypeError || /failed to fetch|networkerror/i.test(raw)
        ? 'Could not reach the AI endpoint. Check that it is running, the address is right, and that it allows requests from this page.'
        : raw || 'The AI request failed.';
    /** @type {ChatMsg} */
    const note = { role: 'note', text: message };
    appendBubble(note, false);
    if (!aborted) chat.msgs.push(note);
  } finally {
    agent.running = false;
    agent.abort = null;
    if (send) send.dataset.state = 'send';
    persistChat();
    syncAgentBadge();
  }
}
/** The agent learns what happened when the user ran a query. */
function noteRunResult(
  /** @type {boolean} */ ok,
  /** @type {string} */ sql,
  /** @type {string} */ summary,
  /** @type {string} */ error,
) {
  agent.lastRun = { sql: sql.slice(0, 1500), ok, summary, error };
  const chat = agent.chat;
  if (!chat || !chat.msgs.length) return;
  const last = chat.msgs[chat.msgs.length - 1];
  // Only report runs that follow an assistant answer that proposed SQL.
  const proposed = chat.msgs
    .slice(-4)
    .some((m) => m.role === 'assistant' && /```sql/i.test(m.text));
  if (!proposed || last?.role === 'user') return;
  /** @type {ChatMsg} */
  const note = {
    role: 'note',
    text: ok
      ? `You ran a query: ✓ ${summary}`
      : `You ran a query and it failed: ${error.slice(0, 300)}`,
  };
  chat.msgs.push(note);
  persistChat();
  if (chatHost?.isConnected) {
    appendBubble(note, false);
    if (!ok) {
      const log = chatHost.querySelector('.ai-log');
      const chips = el('div', 'ai-chips');
      chips.append(
        button(
          'Fix this error',
          () => void sendChat(`That query failed with: ${error.slice(0, 400)}. Please fix it.`),
          'ai-chip',
        ),
      );
      log?.append(chips);
      scrollChat();
    }
  }
}
function renderAiPane(/** @type {HTMLElement} */ body) {
  if (!aiConfigured()) {
    const host = el('div');
    body.append(host);
    renderAiConfig(host, () => renderSheet());
    return;
  }
  const cfg = aiConfig();
  body.classList.add('ai-body');
  const wrap = el('div', 'ai-chat');
  chatHost = wrap;
  // top bar: provider, model, capability, history, new chat, settings
  const bar = el('div', 'ai-bar');
  const provider = /** @type {HTMLSelectElement} */ (el('select', 'inp'));
  for (const [id, p] of Object.entries(AI_PROVIDERS)) provider.append(new Option(p.label, id));
  provider.value = cfg.provider;
  const model = /** @type {HTMLInputElement} */ (el('input', 'inp'));
  model.value = cfg.model;
  model.setAttribute('list', 'ai-models-pane');
  const list = el('datalist');
  list.id = 'ai-models-pane';
  const fill = () => {
    list.replaceChildren();
    for (const m of AI_PROVIDERS[provider.value]?.models ?? []) list.append(new Option(m, m));
  };
  fill();
  provider.addEventListener('change', () => {
    const preset = AI_PROVIDERS[provider.value];
    model.value = preset?.models[0] ?? '';
    writeLocal(AI_KEY, {
      ...readLocal(AI_KEY, {}),
      provider: provider.value,
      model: model.value,
      base: preset?.base ?? '',
    });
    fill();
    syncAgentBadge();
    if (preset?.needsKey && !readLocal(AI_KEY, {}).key) renderSheet();
  });
  const persistModel = () => {
    writeLocal(AI_KEY, {
      ...readLocal(AI_KEY, {}),
      provider: provider.value,
      model: model.value.trim(),
    });
    syncAgentBadge();
  };
  model.addEventListener('change', persistModel);
  model.addEventListener('input', persistModel);
  const gear = button('⚙', () => {
    body.replaceChildren();
    body.classList.remove('ai-body');
    const host = el('div');
    body.append(host);
    renderAiConfig(host, () => renderSheet());
  });
  gear.title = 'Key, endpoint, tool mode and privacy options';
  bar.append(provider, model, list, gear);
  const head = el('div', 'ai-head');
  const cap = el('span', 'ai-cap');
  const hist = button('History', () => showChatHistory(wrap), 'btn');
  const fresh = button(
    '＋ New',
    () => {
      if (agent.running) return;
      newChat();
      renderSheet();
    },
    'btn',
  );
  head.append(cap, el('span', 'sp'), hist, fresh);
  const log = el('div', 'ai-log');
  const chat = currentChat();
  if (!chat.msgs.length) {
    const empty = el('div', 'ai-empty');
    empty.append(
      el('b', '', 'Ask about your data'),
      el(
        'p',
        'ai-note',
        'The agent can read your schemas and tables, search by name, check its queries before showing them, and hear back when you run them.',
      ),
    );
    const chips = el('div', 'ai-chips');
    for (const s of [
      'Suggest analytics for this database',
      'How are the main tables related?',
      'Build a dashboard from the biggest table',
      'Explain my current query',
    ]) {
      const chip = button(s, () => void sendChat(s), 'ai-chip');
      chip.style.setProperty('--i', String(chips.children.length));
      chips.append(chip);
    }
    empty.append(chips);
    log.append(empty);
  }
  const { box: composer, input, send: sendBtn } = buildComposer();
  sendBtn.addEventListener('click', () => {
    if (agent.running) return agent.abort?.abort();
    const value = input.value;
    input.value = '';
    input.dispatchEvent(new Event('input'));
    void sendChat(value);
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      sendBtn.click();
    }
  });
  wrap.append(bar, head, log, composer);
  body.append(wrap);
  chat.msgs.forEach((m, i) => appendBubble(m, i === chat.msgs.length - 1));
  syncAgentBadge();
  enhanceControls(bar);
  if (agent.running) sendBtn.dataset.state = 'stop';
  const pending = aiPrompt;
  if (pending) {
    aiPrompt = '';
    void sendChat(pending);
  }
}
function showChatHistory(/** @type {HTMLElement} */ wrap) {
  const log = wrap.querySelector('.ai-log');
  if (!log) return;
  log.replaceChildren();
  const chats = loadChats();
  const title = el('div', 'sh-sec', 'Previous chats');
  log.append(title);
  if (!chats.length) log.append(el('p', 'ai-note', 'No saved chats yet.'));
  for (const c of chats) {
    const row = el('div', 'ex-item');
    const t = el('div', 'ex-t');
    t.append(
      el('b', '', c.title),
      el('small', '', `${ago(c.t)} · ${c.msgs.filter((m) => m.role === 'user').length} messages`),
    );
    row.append(t);
    const act = el('div', 'ex-act');
    act.append(
      button('Open', () => {
        agent.chat = c;
        renderSheet();
      }),
      button('Delete', () => {
        saveChats(loadChats().filter((x) => x.id !== c.id));
        if (agent.chat?.id === c.id) agent.chat = null;
        showChatHistory(wrap);
      }),
    );
    row.append(act);
    log.append(row);
  }
  log.append(button('Back to chat', () => renderSheet()));
}
let aiPrompt = '';
function askAi(/** @type {string} */ _task, /** @type {string} */ user) {
  aiPrompt = user;
  const sheet = document.getElementById('sqlsheet');
  if (!sheet) return;
  sheetTab = 'ai';
  sheet.hidden = false;
  renderSheet();
}

/* ------------------------------------------------------------ agent v2: relationships, analytics dashboards, resizable sheet */
/* ---- relationship graph and join discovery ---- */
/** @type {{ key: string, edges: Array<{ from: string, cols: string[], to: string, tcols: string[] }> } | null} */
let fkCache = null;
async function fkEdges() {
  const key = String(win.cuadbProfile || '');
  if (fkCache?.key === key) return fkCache.edges;
  const data = await getJson('/ai/relations');
  fkCache = { key, edges: data.edges ?? [] };
  return fkCache.edges;
}
const normTable = (/** @type {string} */ t) => {
  const parts = String(t ?? '')
    .replace(/"/g, '')
    .split('.');
  return parts.length >= 2 ? `${parts[0]}.${parts.slice(1).join('.')}` : `public.${parts[0]}`;
};
async function relationshipsOf(/** @type {string} */ table) {
  const edges = await fkEdges();
  const t = normTable(table).toLowerCase();
  return {
    references: edges
      .filter((e) => e.from.toLowerCase() === t)
      .map((e) => `${e.from}(${e.cols.join(', ')}) -> ${e.to}(${e.tcols.join(', ')})`),
    referenced_by: edges
      .filter((e) => e.to.toLowerCase() === t)
      .map((e) => `${e.from}(${e.cols.join(', ')}) -> ${e.to}(${e.tcols.join(', ')})`),
  };
}
async function joinPath(/** @type {string} */ from, /** @type {string} */ to) {
  const edges = await fkEdges();
  const a = normTable(from).toLowerCase();
  const b = normTable(to).toLowerCase();
  if (a === b) return { path: [], note: 'Same table.' };
  /** @type {Map<string, Array<{ next: string, label: string, on: string }>>} */
  const adj = new Map();
  const add = (
    /** @type {string} */ k,
    /** @type {{ next: string, label: string, on: string }} */ v,
  ) => adj.set(k, [...(adj.get(k) ?? []), v]);
  for (const e of edges) {
    const f = e.from.toLowerCase();
    const t = e.to.toLowerCase();
    const on = e.cols
      .map((c, i) => `${e.from}.${c} = ${e.to}.${e.tcols[i] ?? e.tcols[0]}`)
      .join(' AND ');
    add(f, {
      next: t,
      label: `${e.from}(${e.cols.join(', ')}) -> ${e.to}(${e.tcols.join(', ')})`,
      on,
    });
    add(t, {
      next: f,
      label: `${e.to}(${e.tcols.join(', ')}) <- ${e.from}(${e.cols.join(', ')})`,
      on,
    });
  }
  /** @type {Map<string, { prev: string, step: { label: string, on: string } } | null>} */
  const seen = new Map([[a, null]]);
  const queue = [a];
  while (queue.length) {
    const cur = queue.shift() ?? '';
    if (cur === b) break;
    for (const n of adj.get(cur) ?? []) {
      if (seen.has(n.next) || seen.size > 800) continue;
      seen.set(n.next, { prev: cur, step: { label: n.label, on: n.on } });
      queue.push(n.next);
    }
  }
  if (!seen.has(b)) return null;
  const steps = [];
  const tables = [b];
  for (let cur = b; seen.get(cur); ) {
    const node = /** @type {{ prev: string, step: { label: string, on: string } }} */ (
      seen.get(cur)
    );
    steps.unshift(node.step);
    cur = node.prev;
    tables.unshift(cur);
  }
  return { tables, joins: steps.map((s) => s.label), on: steps.map((s) => s.on) };
}
const baseType = (/** @type {string} */ t) =>
  String(t)
    .toLowerCase()
    .replace(/\(.*\)/, '')
    .replace(/\s+/g, ' ')
    .trim();
const typeFamily = (/** @type {string} */ t) => {
  const b = baseType(t);
  if (/int|serial|numeric|decimal|real|double|float|money/.test(b)) return 'num';
  if (/char|text|citext|name/.test(b)) return 'text';
  if (/uuid/.test(b)) return 'uuid';
  if (/time|date/.test(b)) return 'time';
  if (/bool/.test(b)) return 'bool';
  return b;
};
async function suggestJoins(/** @type {string} */ a, /** @type {string} */ b) {
  const [sa, ta] = normTable(a).split('.');
  const [sb, tb] = normTable(b).split('.');
  const [da, db] = await Promise.all([
    getJson('/ai/describe', { schema: sa ?? '', relation: ta ?? '' }),
    getJson('/ai/describe', { schema: sb ?? '', relation: tb ?? '' }),
  ]);
  /** @type {Array<{ on: string, why: string, score: number }>} */
  const out = [];
  const singular = (/** @type {string} */ n) =>
    n
      .toLowerCase()
      .replace(/(ies)$/, 'y')
      .replace(/(es|s)$/, '');
  for (const ca of da.columns) {
    for (const cb of db.columns) {
      if (typeFamily(ca.type) !== typeFamily(cb.type)) continue;
      const sameName = ca.name.toLowerCase() === cb.name.toLowerCase();
      const generic =
        /^(id|name|created_at|updated_at|deleted_at|description|status|type|notes?)$/i.test(
          ca.name,
        );
      let score = 0;
      let why = '';
      if (sameName && !generic) {
        score = /(_id|_code|_key|_uuid)$/i.test(ca.name) ? 0.85 : 0.6;
        why = `same column name "${ca.name}" and compatible type`;
      }
      // x_id in one table and id in a table called x
      const refA = /^(.+)_id$/i.exec(ca.name);
      if (refA && cb.primary_key && singular(tb ?? '') === singular(refA[1] ?? '')) {
        score = Math.max(score, 0.9);
        why = `${ca.name} looks like a reference to ${tb}.${cb.name}`;
      }
      const refB = /^(.+)_id$/i.exec(cb.name);
      if (refB && ca.primary_key && singular(ta ?? '') === singular(refB[1] ?? '')) {
        score = Math.max(score, 0.9);
        why = `${cb.name} looks like a reference to ${ta}.${ca.name}`;
      }
      if (score > 0)
        out.push({ on: `${sa}.${ta}.${ca.name} = ${sb}.${tb}.${cb.name}`, why, score });
    }
  }
  out.sort((x, y) => y.score - x.score);
  return out.slice(0, 6).map((o) => ({
    join_on: o.on,
    why: o.why,
    confidence: o.score >= 0.85 ? 'high' : o.score >= 0.6 ? 'medium' : 'low',
  }));
}
async function analyticsIdeas(/** @type {string} */ schema, /** @type {string} */ table) {
  const d = await getJson('/ai/describe', { schema, relation: table });
  const q = `${sqlQuote(schema)}.${sqlQuote(table)}`;
  const cols = /** @type {Array<{ name: string, type: string, primary_key: boolean }>} */ (
    d.columns
  );
  /** @type {Array<{ title: string, chart: string, x?: string, y?: string[], sql: string }>} */
  const ideas = [];
  ideas.push({
    title: `Row count of ${table}`,
    chart: 'stat',
    sql: `SELECT COUNT(*) AS total_rows\nFROM ${q}`,
  });
  const time = cols.find((c) => typeFamily(c.type) === 'time');
  const cats = cols
    .filter(
      (c) =>
        (typeFamily(c.type) === 'text' &&
          /^(status|type|kind|role|plan|category|state|tier|country|region|currency|currency_code|gender|channel|source)$/i.test(
            c.name,
          )) ||
        /_(status|type|kind|code)$/i.test(c.name),
    )
    .slice(0, 2);
  const nums = cols
    .filter((c) => typeFamily(c.type) === 'num' && !c.primary_key && !/_id$/i.test(c.name))
    .slice(0, 2);
  if (time) {
    ideas.push({
      title: `${table} created per month`,
      chart: 'line',
      x: 'month',
      y: ['total'],
      sql: `SELECT date_trunc('month', ${sqlQuote(time.name)}) AS month, COUNT(*) AS total\nFROM ${q}\nGROUP BY 1\nORDER BY 1`,
    });
  }
  for (const c of cats) {
    ideas.push({
      title: `${table} by ${c.name}`,
      chart: 'bar',
      x: c.name,
      y: ['total'],
      sql: `SELECT ${sqlQuote(c.name)}, COUNT(*) AS total\nFROM ${q}\nGROUP BY 1\nORDER BY total DESC\nLIMIT 15`,
    });
    for (const n of nums.slice(0, 1)) {
      ideas.push({
        title: `Average ${n.name} by ${c.name}`,
        chart: 'bar',
        x: c.name,
        y: ['average'],
        sql: `SELECT ${sqlQuote(c.name)}, AVG(${sqlQuote(n.name)}) AS average\nFROM ${q}\nGROUP BY 1\nORDER BY average DESC\nLIMIT 15`,
      });
    }
  }
  for (const n of nums) {
    ideas.push({
      title: `${n.name}: spread`,
      chart: 'table',
      sql: `SELECT MIN(${sqlQuote(n.name)}) AS min, AVG(${sqlQuote(n.name)}) AS avg, MAX(${sqlQuote(n.name)}) AS max, COUNT(${sqlQuote(n.name)}) AS filled, COUNT(*) - COUNT(${sqlQuote(n.name)}) AS missing\nFROM ${q}`,
    });
  }
  return ideas.slice(0, 8);
}
function dataToolsAllowed() {
  return aiConfig().sample;
}
const DATA_OFF = {
  error:
    'Data access is off. Build the query and show it to the user: they can run it and the result stays on their screen. (Data-aware mode can be enabled in AI settings.)',
};
async function profileColumn(
  /** @type {string} */ schema,
  /** @type {string} */ table,
  /** @type {string} */ column,
) {
  const t = `${sqlQuote(schema)}.${sqlQuote(table)}`;
  const c = sqlQuote(column);
  const text = isSqlite() ? `CAST(${c} AS TEXT)` : `${c}::text`;
  const stats = await aiRunSql(
    `SELECT COUNT(*) AS total, COUNT(${c}) AS filled, COUNT(DISTINCT ${c}) AS distinct_values, MIN(${text}) AS min_value, MAX(${text}) AS max_value FROM ${t}`,
  );
  if (!stats.ok) return { error: stats.error };
  const top = await aiRunSql(
    `SELECT ${text} AS value, COUNT(*) AS n FROM ${t} WHERE ${c} IS NOT NULL GROUP BY 1 ORDER BY n DESC LIMIT 5`,
  );
  const st = stats.doc.querySelector('table.grid');
  const tp = top.doc.querySelector('table.grid');
  return {
    stats:
      st instanceof HTMLTableElement
        ? Object.fromEntries(
            tableData(st).heads.map((h, i) => [h, tableData(st).rows[0]?.[i] ?? null]),
          )
        : {},
    top_values:
      tp instanceof HTMLTableElement
        ? tableData(tp).rows.map((r) => `${(r[0] ?? 'NULL').slice(0, 40)} (${r[1]})`)
        : [],
  };
}
TOOLS.push(
  {
    name: 'find_relationships',
    description:
      'List the foreign keys leaving a table and the tables whose foreign keys point to it. Use it to explore how tables relate.',
    parameters: {
      type: 'object',
      properties: { schema: { type: 'string' }, table: { type: 'string' } },
      required: ['schema', 'table'],
    },
  },
  {
    name: 'find_join_path',
    description:
      'Find how two tables connect through foreign keys, even across schemas and through intermediate tables. Returns the chain of joins and the ON conditions.',
    parameters: {
      type: 'object',
      properties: {
        from: { type: 'string', description: 'schema.table' },
        to: { type: 'string', description: 'schema.table' },
      },
      required: ['from', 'to'],
    },
  },
  {
    name: 'suggest_joins',
    description:
      'For two tables with no foreign-key path, suggest inferred joins from matching column names and types. These are guesses; tell the user they are inferred.',
    parameters: {
      type: 'object',
      properties: {
        a: { type: 'string', description: 'schema.table' },
        b: { type: 'string', description: 'schema.table' },
      },
      required: ['a', 'b'],
    },
  },
  {
    name: 'analytics_ideas',
    description:
      'Get ready-made analytics ideas (counts, trends over time, breakdowns, spreads) with SQL for one table, based on its columns.',
    parameters: {
      type: 'object',
      properties: { schema: { type: 'string' }, table: { type: 'string' } },
      required: ['schema', 'table'],
    },
  },
  {
    name: 'profile_column',
    description:
      'Statistics for one column: row count, filled, distinct, min, max and the five most common values. Only available when data-aware mode is on.',
    parameters: {
      type: 'object',
      properties: {
        schema: { type: 'string' },
        table: { type: 'string' },
        column: { type: 'string' },
      },
      required: ['schema', 'table', 'column'],
    },
  },
  {
    name: 'run_query',
    description:
      'Run a read-only SELECT and read back up to 20 rows. Only available when data-aware mode is on; otherwise build the query and let the user run it.',
    parameters: { type: 'object', properties: { sql: { type: 'string' } }, required: ['sql'] },
  },
);
const baseRunTool = runTool;
runTool = async function (/** @type {string} */ name, /** @type {any} */ args) {
  const a = args ?? {};
  try {
    switch (name) {
      case 'find_relationships':
        return await relationshipsOf(`${a.schema}.${a.table}`);
      case 'find_join_path': {
        const p = /** @type {any} */ (await joinPath(a.from, a.to));
        if (!p)
          return {
            path: null,
            note: 'No foreign-key path between these tables. Try suggest_joins for inferred joins.',
          };
        return {
          tables: p.tables,
          joins: p.joins,
          on: p.on,
          hint: `FROM ${p.tables[0]}${p.tables
            .slice(1)
            .map((/** @type {string} */ t, /** @type {number} */ i) => ` JOIN ${t} ON ${p.on[i]}`)
            .join('')}`,
        };
      }
      case 'suggest_joins':
        return {
          inferred: await suggestJoins(a.a, a.b),
          note: 'Inferred from column names and types, not declared foreign keys.',
        };
      case 'analytics_ideas':
        return { ideas: await analyticsIdeas(String(a.schema ?? ''), String(a.table ?? '')) };
      case 'profile_column':
        return dataToolsAllowed()
          ? await profileColumn(
              String(a.schema ?? ''),
              String(a.table ?? ''),
              String(a.column ?? ''),
            )
          : DATA_OFF;
      case 'run_query': {
        if (!dataToolsAllowed()) return DATA_OFF;
        const sql = String(a.sql ?? '').trim();
        if (!/^(select|with|values|table)\b/i.test(sql) || /;\s*\S/.test(sql.replace(/;\s*$/, '')))
          return { error: 'Only a single read-only SELECT is allowed here.' };
        const r = await aiRunSql(sql);
        if (!r.ok) return { error: r.error };
        const t = r.doc.querySelector('table.grid');
        if (!(t instanceof HTMLTableElement)) return { rows: [] };
        const d = tableData(t);
        return {
          columns: d.heads.slice(0, 12),
          rows: d.rows
            .slice(0, 20)
            .map((row) => row.slice(0, 12).map((v) => (v ?? 'NULL').slice(0, 60))),
          total_rows: d.rows.length,
        };
      }
      default:
        return await baseRunTool(name, args);
    }
  } catch (error) {
    return { error: error instanceof Error ? error.message : 'The tool failed.' };
  }
};
const baseSummarize = summarizeTool;
summarizeTool = function (/** @type {string} */ name, /** @type {any} */ result) {
  if (result?.error) return `error: ${String(result.error).slice(0, 80)}`;
  if (name === 'find_relationships')
    return `${result.references?.length ?? 0} out, ${result.referenced_by?.length ?? 0} in`;
  if (name === 'find_join_path')
    return result.joins ? `${result.joins.length} join(s)` : 'no FK path';
  if (name === 'suggest_joins') return `${result.inferred?.length ?? 0} candidates`;
  if (name === 'analytics_ideas') return `${result.ideas?.length ?? 0} ideas`;
  if (name === 'profile_column') return `${result.stats?.distinct_values ?? '?'} distinct`;
  if (name === 'run_query') return `${result.total_rows ?? 0} rows`;
  return baseSummarize(name, result);
};
const baseLabel = toolLabel;
toolLabel = function (/** @type {string} */ name, /** @type {any} */ args) {
  const a = args ?? {};
  switch (name) {
    case 'find_relationships':
      return `Finding relationships of ${a.schema}.${a.table}`;
    case 'find_join_path':
      return `Finding a join path ${a.from} → ${a.to}`;
    case 'suggest_joins':
      return `Inferring joins ${a.a} ↔ ${a.b}`;
    case 'analytics_ideas':
      return `Thinking of analytics for ${a.schema}.${a.table}`;
    case 'profile_column':
      return `Profiling ${a.schema}.${a.table}.${a.column}`;
    case 'run_query':
      return 'Running a read-only query';
    default:
      return baseLabel(name, args);
  }
};

/* ---- analytics dashboards the agent can draw in the chat ---- */
const SAVED_ANALYSES = 'cuadb:analyses';
const CHARTS = ['bar', 'line', 'area', 'pie', 'stat', 'table'];
/** @typedef {{ title: string, sql: string, chart: string, x: string, y: string[], note: string }} DashCard */
/** @typedef {{ title: string, cards: DashCard[] }} DashSpec */
function parseDashboard(/** @type {string} */ raw) {
  try {
    const data = JSON.parse(raw);
    const cards = (Array.isArray(data?.cards) ? data.cards : [])
      .slice(0, 8)
      .map((/** @type {any} */ c) => ({
        title: String(c?.title ?? 'Query').slice(0, 80),
        sql: String(c?.sql ?? '').slice(0, 4000),
        chart: CHARTS.includes(String(c?.chart)) ? String(c.chart) : 'auto',
        x: String(c?.x ?? ''),
        y: Array.isArray(c?.y) ? c.y.map(String).slice(0, 4) : c?.y ? [String(c.y)] : [],
        note: String(c?.note ?? '').slice(0, 300),
      }))
      .filter((/** @type {DashCard} */ c) => /^\s*(select|with|values|table)\b/i.test(c.sql));
    if (!cards.length) return null;
    return /** @type {DashSpec} */ ({
      title: String(data?.title ?? 'Analysis').slice(0, 80),
      cards,
    });
  } catch {
    return null;
  }
}
function drawChartSpec(
  /** @type {HTMLElement} */ host,
  /** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ data,
  /** @type {{ chart: string, x: string, y: string[] }} */ spec,
) {
  const prev = { ...chartState, ys: [...chartState.ys] };
  chartState.type = spec.chart;
  chartState.x = spec.x;
  chartState.ys = spec.y;
  try {
    drawChart(host, data);
  } finally {
    Object.assign(chartState, prev);
  }
}
function resultShape(
  /** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ data,
  /** @type {DashCard} */ card,
) {
  const nums = numericColumns(data);
  const firstText = data.heads.findIndex((_, i) => !nums.includes(i));
  const x =
    card.x && data.heads.includes(card.x)
      ? card.x
      : (data.heads[firstText >= 0 ? firstText : 0] ?? '');
  let y = card.y.filter((c) => data.heads.includes(c));
  if (!y.length)
    y = nums
      .map((i) => data.heads[i] ?? '')
      .filter((h) => h !== x)
      .slice(0, 3);
  let chart = card.chart;
  if (chart === 'auto') {
    if (data.rows.length === 1 && data.heads.length <= 2) chart = 'stat';
    else if (!y.length) chart = 'table';
    else if (/date|time|month|day|year|week|_at$/i.test(x)) chart = 'line';
    else chart = 'bar';
  }
  if (chart === 'stat' && data.rows.length > 1) chart = y.length ? 'bar' : 'table';
  return { x, y, chart };
}
function miniTable(/** @type {{ heads: string[], rows: Array<Array<string | null>> }} */ data) {
  const table = el('table', 'mini');
  const head = el('tr');
  data.heads.slice(0, 8).forEach((h) => head.append(el('th', '', h)));
  table.append(head);
  data.rows.slice(0, 12).forEach((r) => {
    const tr = el('tr');
    r.slice(0, 8).forEach((v) =>
      tr.append(v === null ? el('td', 'nul', 'NULL') : el('td', '', v.slice(0, 40))),
    );
    table.append(tr);
  });
  return table;
}
async function runCard(
  /** @type {DashCard} */ card,
  /** @type {HTMLElement} */ body,
  /** @type {HTMLElement} */ actions,
) {
  body.replaceChildren(el('p', 'ai-note', 'Running…'));
  const r = await aiRunSql(card.sql);
  body.replaceChildren();
  if (!r.ok) {
    const err = el('div', 'notice error', r.error || 'The query failed.');
    body.append(err);
    actions.querySelector('[data-fixcard]')?.remove();
    actions.append(
      Object.assign(
        button(
          '✦ Fix',
          () =>
            void sendChat(
              `The analytics card "${card.title}" failed with: ${r.error.slice(0, 300)}. Query:\n${card.sql}\nPlease fix it.`,
            ),
          'btn',
        ),
        {},
      ),
    );
    return;
  }
  const table = r.doc.querySelector('table.grid');
  if (!(table instanceof HTMLTableElement)) {
    body.append(el('p', 'ai-note', 'Done (no rows returned).'));
    return;
  }
  const data = tableData(table);
  const shape = resultShape(data, card);
  if (shape.chart === 'stat') {
    const tile = el('div', 'dash-stat');
    tile.append(
      el('b', '', data.rows[0]?.[data.heads.length > 1 ? 1 : 0] ?? data.rows[0]?.[0] ?? '—'),
      el('span', '', data.heads[data.heads.length > 1 ? 1 : 0] ?? ''),
    );
    body.append(tile);
  } else if (shape.chart === 'table') {
    body.append(miniTable(data));
  } else {
    const chart = el('div');
    body.append(chart);
    drawChartSpec(chart, data, shape);
  }
  body.append(el('p', 'dash-meta', `${data.rows.length} row${data.rows.length === 1 ? '' : 's'}`));
  actions.querySelector('[data-interpret]')?.remove();
  const interpret = button(
    'Interpret',
    () => {
      const cfg = aiConfig();
      const shown = data.rows
        .slice(0, 20)
        .map((row) =>
          row
            .slice(0, 8)
            .map((v) => v ?? 'NULL')
            .join(' | '),
        )
        .join('\n');
      void sendChat(
        cfg.sample
          ? `Interpret this result for me. Card: ${card.title}\nColumns: ${data.heads.join(' | ')}\nRows (${data.rows.length} total):\n${shown}`
          : `Explain how to read the "${card.title}" card (${data.rows.length} rows, columns: ${data.heads.join(', ')}) and what to look for. I have not shared the values.`,
      );
    },
    'btn',
  );
  interpret.setAttribute('data-interpret', '');
  actions.append(interpret);
}
function renderDashboard(
  /** @type {HTMLElement} */ host,
  /** @type {DashSpec} */ spec,
  /** @type {{ saved?: boolean }} */ opts = {},
) {
  const box = el('div', 'dash');
  const head = el('div', 'dash-head');
  head.append(el('b', '', spec.title));
  const headActs = el('span', 'dash-acts');
  const runners = /** @type {Array<() => Promise<void>>} */ ([]);
  headActs.append(
    button(
      'Run all',
      () => void runners.reduce((p, run) => p.then(run), Promise.resolve()),
      'btn on',
    ),
  );
  if (!opts.saved) {
    headActs.append(
      button('Save', () => {
        const all = /** @type {Array<{ id: string, t: number } & DashSpec>} */ (
          readLocal(SAVED_ANALYSES, [])
        );
        if (
          all.some(
            (a) => a.title === spec.title && JSON.stringify(a.cards) === JSON.stringify(spec.cards),
          )
        )
          return toast('Already saved');
        writeLocal(
          SAVED_ANALYSES,
          [{ id: Math.random().toString(36).slice(2, 9), t: Date.now(), ...spec }, ...all].slice(
            0,
            40,
          ),
        );
        toast('Analysis saved · History → Analyses');
      }),
    );
  }
  head.append(headActs);
  box.append(head);
  const grid = el('div', 'dash-grid');
  for (const card of spec.cards) {
    const c = el('div', `dash-card${card.chart === 'stat' ? ' small' : ''}`);
    c.append(el('div', 'dash-t', card.title));
    if (card.note) c.append(el('div', 'dash-note', card.note));
    const body = el('div', 'dash-body');
    body.append(el('p', 'ai-note', 'Not run yet'));
    const acts = el('div', 'dash-card-acts');
    const run = async () => runCard(card, body, acts);
    runners.push(run);
    acts.append(
      button('Run', () => void run(), 'btn on'),
      button('Open in editor', () => setEditorSql(card.sql)),
      button('Copy SQL', () => void copyText(card.sql, 'SQL copied')),
    );
    c.append(body, acts);
    grid.append(c);
  }
  box.append(grid);
  host.append(box);
}
// Teach the chat renderer about ```analytics blocks.
const baseRenderMarkdown = renderMarkdown;
renderMarkdown = function (/** @type {HTMLElement} */ host, /** @type {string} */ text) {
  const pieces = text.split(/```analytics\n?([\s\S]*?)```/);
  pieces.forEach((piece, i) => {
    if (i % 2 === 0) {
      if (piece.trim()) baseRenderMarkdown(host, piece);
      return;
    }
    const spec = parseDashboard(piece);
    if (spec) renderDashboard(host, spec);
    else host.append(el('p', 'ai-note', 'The agent sent an analysis the app could not read.'));
  });
};

/* ---- resizable tool sheet ---- */
function initSqlSheetResize() {
  const sheet = document.getElementById('sqlsheet');
  if (!sheet || document.getElementById('sqlsheet-rs')) return;
  const parent = sheet.parentElement;
  if (!parent) return;
  try {
    const saved = Number(localStorage.getItem('cuadb:qsw'));
    if (saved >= 320 && saved <= 1400)
      document.documentElement.style.setProperty('--qsw', `${saved}px`);
  } catch {
    /* storage unavailable */
  }
  const handle = el('div');
  handle.id = 'sqlsheet-rs';
  handle.setAttribute('role', 'separator');
  handle.setAttribute('aria-orientation', 'vertical');
  handle.setAttribute('aria-label', 'Resize the tool sheet');
  handle.title = 'Drag to resize · double-click to reset';
  handle.hidden = sheet.hidden;
  sheet.before(handle);
  new MutationObserver(() => {
    handle.hidden = sheet.hidden;
  }).observe(sheet, { attributes: true, attributeFilter: ['hidden'] });
  const set = (/** @type {number} */ width, /** @type {boolean} */ persist) => {
    const max = Math.max(360, Math.min(1400, parent.clientWidth - 160));
    const w = Math.round(Math.max(320, Math.min(max, width)));
    document.documentElement.style.setProperty('--qsw', `${w}px`);
    if (persist) {
      try {
        localStorage.setItem('cuadb:qsw', String(w));
      } catch {
        /* storage unavailable */
      }
    }
    return w;
  };
  handle.addEventListener('pointerdown', (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add('drag');
    document.documentElement.classList.add('sheet-drag');
    const right = parent.getBoundingClientRect().right;
    let last = 0;
    const move = (/** @type {PointerEvent} */ e) => {
      last = set(right - e.clientX, false);
    };
    const stop = () => {
      handle.classList.remove('drag');
      document.documentElement.classList.remove('sheet-drag');
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', stop);
      handle.removeEventListener('pointercancel', stop);
      if (last) set(last, true);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', stop);
    handle.addEventListener('pointercancel', stop);
  });
  handle.addEventListener('dblclick', () => {
    document.documentElement.style.removeProperty('--qsw');
    try {
      localStorage.removeItem('cuadb:qsw');
    } catch {
      /* storage unavailable */
    }
  });
}
initSqlSheetResize();
document.addEventListener('htmx:afterSettle', initSqlSheetResize);
document.addEventListener('htmx:afterSwap', initSqlSheetResize);

/* ---- the composer: glowing field, context chips, voice input, morphing send button ---- */
const MIC_SVG =
  '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/></svg>';
const SEND_SVG =
  '<svg class="i-send" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"/></svg><svg class="i-stop" viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2.5"/></svg>';
function buildComposer() {
  const box = el('div', 'ai-composer');
  const field = el('div', 'ai-field');
  const input = /** @type {HTMLTextAreaElement} */ (el('textarea', 'ai-input'));
  input.rows = 1;
  input.placeholder = 'Ask about your data, or say what to analyse…';
  input.setAttribute('aria-label', 'Message the agent');
  const fit = () => {
    input.style.height = 'auto';
    input.style.height = `${Math.min(input.scrollHeight, 150)}px`;
  };
  input.addEventListener('input', fit);
  const ctx = el('div', 'ai-ctx');
  const addCtx = (/** @type {string} */ label, /** @type {() => string} */ make) => {
    const chip = button(
      label,
      () => {
        const text = make();
        if (!text) return toast('Nothing to add yet.');
        input.value = `${input.value}${input.value && !input.value.endsWith('\n') ? '\n' : ''}${text}`;
        fit();
        input.focus();
      },
      'ai-ctxchip',
    );
    ctx.append(chip);
  };
  addCtx('＋ Editor query', () =>
    editorSql().trim() ? `My query:\n\`\`\`sql\n${editorSql().trim()}\n\`\`\`\n` : '',
  );
  addCtx('＋ Last error', () =>
    agent.lastRun && !agent.lastRun.ok ? `It failed with: ${agent.lastRun.error}\n` : '',
  );
  addCtx(
    '✦ Analyse',
    () =>
      'Suggest analytics for this database: explore the related tables and build a small dashboard.',
  );
  const row = el('div', 'ai-frow');
  const mic = /** @type {HTMLButtonElement} */ (el('button', 'ai-mic'));
  mic.type = 'button';
  mic.innerHTML = `${MIC_SVG}<span class="ai-wave" aria-hidden="true"><i></i><i></i><i></i><i></i></span>`;
  mic.title = 'Dictate with your voice';
  mic.setAttribute('aria-label', 'Dictate');
  const Recognition =
    /** @type {any} */ (window).SpeechRecognition ??
    /** @type {any} */ (window).webkitSpeechRecognition;
  mic.hidden = !Recognition;
  /** @type {any} */
  let rec = null;
  mic.addEventListener('click', () => {
    if (rec) {
      rec.stop();
      return;
    }
    const base = input.value;
    try {
      rec = new Recognition();
    } catch {
      return toast('Voice input is not available in this browser.');
    }
    rec.lang = navigator.language || 'en-US';
    rec.interimResults = true;
    rec.continuous = true;
    rec.onresult = (/** @type {any} */ event) => {
      let text = '';
      for (let i = 0; i < event.results.length; i += 1) text += event.results[i][0].transcript;
      input.value = `${base}${base && !/\s$/.test(base) ? ' ' : ''}${text}`;
      fit();
    };
    rec.onerror = (/** @type {any} */ event) => {
      if (event.error === 'not-allowed' || event.error === 'service-not-allowed')
        toast('Allow microphone access to dictate.');
      else if (event.error !== 'aborted' && event.error !== 'no-speech')
        toast(`Voice input stopped (${event.error}).`);
    };
    rec.onend = () => {
      rec = null;
      mic.classList.remove('on');
      box.classList.remove('listening');
    };
    mic.classList.add('on');
    box.classList.add('listening');
    rec.start();
  });
  const send = /** @type {HTMLButtonElement} */ (el('button', 'ai-send'));
  send.type = 'button';
  send.dataset.state = 'send';
  send.innerHTML = SEND_SVG;
  send.title = 'Send (Enter) · Stop while it works';
  send.setAttribute('aria-label', 'Send');
  const hint = el('span', 'ai-hint', 'Enter to send · Shift+Enter for a new line');
  row.append(ctx, el('span', 'sp'), mic, send);
  field.append(input, row);
  box.append(field, hint);
  return { box, input, send };
}
