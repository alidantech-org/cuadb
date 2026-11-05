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
const TP = prefs.TP;

/** Ask HTMX for a fragment. */
function fetchInto(/** @type {string} */ url, /** @type {string} */ target, swap = 'outerHTML') {
  win.htmx?.ajax('GET', url, { target, swap });
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
  const title = document.createElement('div');
  title.className = 'sg';
  title.textContent = 'Databases on this server';
  const list = document.createElement('div');
  list.id = 'dblist';
  list.className = 'dblist';
  const loading = document.createElement('div');
  loading.className = 'note';
  loading.textContent = 'Loading…';
  list.append(loading);
  const info = document.createElement('div');
  info.className = 'dbinfo';
  const line = (/** @type {string} */ label, /** @type {string} */ value) => {
    const row = document.createElement('div');
    row.className = 'sr';
    const a = document.createElement('span');
    a.textContent = label;
    const b = document.createElement('b');
    b.textContent = value;
    row.append(a, b);
    return row;
  };
  info.append(
    line('Connection', currentDb.label),
    line('Engine', currentDb.engine),
    line('State', currentDb.state),
  );
  const access = document.createElement('div');
  access.className = 'sr';
  const text = document.createElement('span');
  text.textContent = 'Your access';
  const tag = document.createElement('span');
  tag.className = `tag m-${currentDb.access}`;
  tag.textContent =
    { read: 'read', write: 'can edit', full: 'can edit and run SQL' }[currentDb.access] ?? 'read';
  access.append(text, tag);
  info.append(access);
  pop.append(title, list, info);
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
const POPS = ['cpop', 'dpop', 'opop', 'colpop', 'exppop'];
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
  syncSettings();
  /** @type {HTMLElement | null} */ (dialog.querySelector('.snav.on'))?.focus();
}
function closeSettings() {
  const dialog = document.getElementById('spop');
  if (!dialog?.classList.contains('show')) return;
  dialog.classList.remove('show');
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
  applyTreeView();
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
  const heads = Array.from(table.querySelectorAll('thead th[data-col]'));
  const cells = Array.from(row.querySelectorAll('td:not(.rn)'));
  panel.replaceChildren();
  const title = document.createElement('h3');
  const label = document.createElement('span');
  label.textContent = `Row ${row.dataset.row ?? ''}`;
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
    const kv = document.createElement('div');
    kv.className = 'kv';
    const caption = document.createElement('label');
    const name = document.createElement('span');
    name.textContent = head.getAttribute('data-col') ?? '';
    const type = document.createElement('span');
    type.textContent = head.getAttribute('data-type') ?? '';
    const copy = document.createElement('button');
    copy.type = 'button';
    copy.className = 'cpy';
    copy.setAttribute('data-copy-value', String(index));
    copy.setAttribute('aria-label', `Copy ${name.textContent}`);
    copy.textContent = 'Copy';
    caption.append(name, type, copy);
    if (cell?.querySelector('.truncation') && gridColumns().some((c) => c.pk)) {
      const full = document.createElement('button');
      full.type = 'button';
      full.className = 'cpy';
      full.setAttribute('data-load-full', String(index));
      full.textContent = 'Load full';
      caption.append(full);
    }
    const pre = document.createElement('pre');
    pre.textContent = cell?.querySelector('[data-null]') ? 'NULL' : (cell?.dataset.detail ?? '');
    kv.append(caption, pre);
    panel.append(kv);
  });
  panel.classList.add('show');
  const backdrop = document.getElementById('sheet-bk');
  if (backdrop) backdrop.hidden = false;
  panel.scrollTop = 0;
}
function closeInspector() {
  win.cuadbEditing = false;
  document.getElementById('insp')?.classList.remove('show');
  const backdrop = document.getElementById('sheet-bk');
  if (backdrop) backdrop.hidden = true;
  $$('tr.sel').forEach((el) => el.classList.remove('sel'));
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
  foot.className = 'ef';
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
    const ths = Array.from(headRow.cells);
    // Natural widths come from the browser's own auto layout, measured once.
    const natural = ths.map((th) => Math.ceil(th.getBoundingClientRect().width));
    const group = document.createElement('colgroup');
    const cols = ths.map((th, index) => {
      const col = document.createElement('col');
      const fixed = th.classList.contains('rn');
      const width = fixed ? 46 : Math.max(64, Math.min(420, natural[index] ?? 120));
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
function rememberTable(/** @type {string} */ path) {
  if (!S.remember) return;
  const query = path.split('?')[1] ?? '';
  const params = new URLSearchParams(query);
  const key = tableKey(params);
  if (!key) return;
  const sort = params.get('sort');
  const cols = params.has('pick') ? params.getAll('column') : null;
  if (!sort && !cols) delete TP[key];
  else
    TP[key] = {
      s: sort ? { c: sort, d: params.get('direction') === 'desc' ? -1 : 1 } : null,
      h: cols,
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
    });
    if (box) sortChildren(box, '.relw', S.tsort, S.tdir, relName);
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
}

/** Show timestamps in the viewer's time zone (display only). */
function formatDates(/** @type {ParentNode} */ root = document) {
  root.querySelectorAll('td.ts-value').forEach((node) => {
    const cell = /** @type {HTMLElement} */ (node);
    const span = cell.querySelector('.ts');
    if (!span || cell.querySelector('[data-null]')) return;
    const raw = cell.dataset.detail ?? '';
    if (S.dates !== 'local') {
      if (span.textContent !== raw && cell.dataset.shown) span.textContent = cell.dataset.shown;
      return;
    }
    if (!cell.dataset.shown) cell.dataset.shown = span.textContent ?? '';
    const when = new Date(raw.replace(' ', 'T').replace(/([+-]\d\d)$/, '$1:00'));
    if (!Number.isNaN(when.getTime()) && /\d{4}-\d\d-\d\d/.test(raw)) {
      span.textContent = when.toLocaleString(undefined, {
        dateStyle: 'medium',
        timeStyle: 'medium',
      });
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
  if (!S.restore || document.getElementById('catalog-panel')) return;
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
  if (!S.autoOpen) return;
  const schemas = $$('#tree .sch');
  const first =
    schemas.find((el) => el.getAttribute('data-schema-name') === 'public') ?? schemas[0];
  const button = first?.querySelector('[data-s]');
  if (first && button instanceof HTMLElement && !first.classList.contains('o')) {
    button.click();
  }
}
function filterTree() {
  const input = /** @type {HTMLInputElement | null} */ (document.getElementById('f'));
  const needle = (input?.value ?? '').trim().toLocaleLowerCase();
  $$('#tree .sch').forEach((schema) => {
    const schemaName = (schema.getAttribute('data-schema-name') ?? '').toLocaleLowerCase();
    const schemaHit = needle !== '' && schemaName.includes(needle);
    let any = false;
    schema.querySelectorAll('[data-relation-name]').forEach((relation) => {
      const name = (relation.getAttribute('data-relation-name') ?? '').toLocaleLowerCase();
      const hit = needle === '' || schemaHit || name.includes(needle);
      relation.toggleAttribute('hidden', !hit);
      any ||= hit;
    });
    schema.toggleAttribute('hidden', needle !== '' && !schemaHit && !any);
    if (needle !== '') openSchema(schema, true);
  });
  const flat = document.getElementById('flatbox');
  if (flat) {
    flat.querySelectorAll('[data-relation-name]').forEach((relation) => {
      const name = (relation.getAttribute('data-relation-name') ?? '').toLocaleLowerCase();
      const qual = (relation.getAttribute('data-qual') ?? '').toLocaleLowerCase();
      relation.toggleAttribute(
        'hidden',
        needle !== '' && !name.includes(needle) && !qual.includes(needle),
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
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  // The sheet closes on any click outside it. That click only closes: it never opens another
  // row, so a double-click on a row opens the sheet and the second click closes it.
  if (sheetOpen() && !target.closest('#insp, [data-row-add], .pop')) {
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
    return;
  }
  const popBtn = target.closest('[data-pop]');
  if (popBtn instanceof HTMLElement) {
    togglePop(popBtn.dataset.pop ?? '');
    return;
  }

  const profile = target.closest('[data-profile]');
  if (profile instanceof HTMLElement) {
    const label = profile.dataset.label ?? 'Connection';
    const name = document.getElementById('cname');
    if (name) name.textContent = label;
    win.cuadbProfile = profile.dataset.profile ?? '';
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
    if (holder) openSchema(holder, !holder.classList.contains('o'));
    return;
  }
  const rowLink = target.closest('#tree a.row');
  if (rowLink) {
    markActiveRow(rowLink);
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
    if (view === 'admin-connections') {
      Promise.resolve(
        win.htmx?.ajax('GET', '/admin', { target: '#mainbody', swap: 'innerHTML' }),
      ).then(() =>
        document
          .querySelector('#adm-seg [data-tab="connections"]')
          ?.dispatchEvent(new MouseEvent('click', { bubbles: true })),
      );
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
  if (form instanceof HTMLFormElement && form.id === 'row-form') {
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
  if (el instanceof HTMLInputElement && el.id === 'f') filterTree();
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
      restoreLast();
    }
    if (event.target.matches('.sub')) {
      filterTree();
      loadObjects();
      applyTreeView();
    }
    decorateBars(event.target);
    // A new page replaces the sheet's row; swaps inside the sheet itself (the editor form,
    // its messages) and elsewhere must leave it open.
    if (event.target.id === 'mainbody' || event.target.id === 'catalog-panel') closeInspector();
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
  if (target.id === 'modal') openModal();
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
document.addEventListener('htmx:beforeRequest', (event) => {
  const detailBefore = /** @type {Any} */ (event).detail;
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
function refreshConnections() {
  const list = document.getElementById('cpop');
  if (!list) return;
  Promise.resolve(
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
