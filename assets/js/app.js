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

/* ------------------------------------------------------------ popovers */
const POPS = ['cpop', 'spop', 'colpop', 'exppop'];
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
  if (open && id === 'spop') syncSettings();
}

/* ------------------------------------------------------------ settings */
function syncSettings() {
  const pop = document.getElementById('spop');
  if (!pop) return;
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
  if (S.mode !== lastMode) {
    lastMode = S.mode;
    if (S.mode === 'advanced') loadObjects();
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

/* ------------------------------------------------------------ inspector */
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
    caption.append(name, type);
    const pre = document.createElement('pre');
    pre.textContent = cell?.querySelector('[data-null]') ? 'NULL' : (cell?.dataset.detail ?? '');
    kv.append(caption, pre);
    panel.append(kv);
  });
  panel.classList.add('show');
}
function closeInspector() {
  win.cuadbEditing = false;
  document.getElementById('insp')?.classList.remove('show');
  $$('tr.sel').forEach((el) => el.classList.remove('sel'));
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

function fieldControl(
  /** @type {ReturnType<typeof gridColumns>[number]} */ col,
  /** @type {string} */ value,
) {
  const isBool = col.type === 'boolean' || col.type === 'bool';
  /** @type {HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement} */
  let control;
  if (isBool) {
    control = document.createElement('select');
    for (const option of ['true', 'false']) {
      const o = document.createElement('option');
      o.value = option;
      o.textContent = option;
      control.append(o);
    }
    control.value = ['true', 't', '1'].includes(value.toLowerCase()) ? 'true' : 'false';
  } else if (value.length > 80 || value.includes('\n')) {
    control = document.createElement('textarea');
    control.rows = 4;
    control.value = value;
  } else {
    control = document.createElement('input');
    control.value = value;
    if (/int|numeric|decimal|real|double|float/.test(col.type)) control.inputMode = 'decimal';
  }
  control.className = 'inp';
  control.name = col.name;
  control.setAttribute('autocomplete', 'off');
  return control;
}

/** Show the editor in the inspector panel: a row to edit, or null for a new row. */
function openEditor(/** @type {HTMLTableRowElement | null} */ row) {
  const panel = document.getElementById('insp');
  if (!panel) return;
  const insert = row === null;
  const cols = gridColumns();
  const values = row ? rowValues(row) : {};
  panel.replaceChildren();
  const title = document.createElement('h3');
  const label = document.createElement('span');
  label.textContent = insert ? 'New row' : `Edit row ${row?.dataset.row ?? ''}`;
  const close = document.createElement('button');
  close.type = 'button';
  close.setAttribute('data-close-insp', '');
  close.setAttribute('aria-label', 'Close editor');
  close.textContent = '×';
  title.append(label, close);
  const form = document.createElement('form');
  form.className = 'ed';
  form.id = 'row-form';
  form.dataset.mode = insert ? 'insert' : 'update';
  for (const col of cols) {
    const current = values[col.name];
    const field = document.createElement('div');
    field.className = 'fl';
    const caption = document.createElement('span');
    caption.textContent = `${col.name} · ${col.type}${col.pk ? ' · key' : ''}`;
    const line = document.createElement('div');
    line.className = 'row2';
    const readOnly = !col.editable || (!insert && (col.pk || current?.big));
    if (readOnly) {
      const shown = document.createElement('input');
      shown.className = 'inp';
      shown.readOnly = true;
      shown.value = insert ? '' : current?.value === null ? 'NULL' : (current?.value ?? '');
      shown.placeholder = col.editable ? '' : 'not editable here';
      shown.setAttribute('aria-label', col.name);
      line.append(shown);
    } else {
      const control = fieldControl(col, insert ? '' : (current?.value ?? ''));
      if (insert) {
        if (!(control instanceof HTMLSelectElement))
          control.placeholder = col.pk || !col.required ? 'default' : 'required';
        if (control instanceof HTMLSelectElement) {
          const blank = document.createElement('option');
          blank.value = '';
          blank.textContent = 'default';
          control.prepend(blank);
          control.value = '';
        }
      }
      control.setAttribute('aria-label', col.name);
      line.append(control);
      if (col.nullable) {
        const wrap = document.createElement('label');
        wrap.className = 'nlt';
        const box = document.createElement('input');
        box.type = 'checkbox';
        box.dataset.nullFor = col.name;
        box.checked = !insert && current?.value === null;
        const text = document.createElement('span');
        text.textContent = 'NULL';
        wrap.append(box, text);
        line.append(wrap);
        control.disabled = box.checked;
      }
    }
    field.append(caption, line);
    form.append(field);
  }
  const foot = document.createElement('div');
  foot.className = 'ef';
  const message = document.createElement('div');
  message.id = 'eerr';
  message.setAttribute('role', 'status');
  const cancel = document.createElement('button');
  cancel.type = 'button';
  cancel.className = 'btn';
  cancel.setAttribute('data-close-insp', '');
  cancel.textContent = 'Cancel';
  const save = document.createElement('button');
  save.type = 'submit';
  save.className = 'btn on';
  save.textContent = insert ? 'Insert row' : 'Save changes';
  foot.append(message, cancel, save);
  form.append(foot);
  panel.append(title, form);
  panel.classList.add('show');
  win.cuadbEditing = true;
  /** @type {HTMLElement | null} */ (
    form.querySelector('input:not([readonly]):not(:disabled),select,textarea')
  )?.focus();
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
    if (!control) continue;
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

/* ------------------------------------------------------------ sidebar tree */
function openSchema(/** @type {Element} */ holder, /** @type {boolean} */ open) {
  holder.classList.toggle('o', open);
  holder.setAttribute('aria-expanded', String(open));
}
function autoOpenSchema() {
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
  const inPop = target.closest('.pop');
  if (!inPop && !target.closest('[data-pop],#cbtn,#sbtn')) closePops();

  if (target.closest('[data-modal-close]') || target.id === 'modal') {
    closeModal();
    return;
  }
  if (target.closest('[data-toggle-sidebar]')) {
    setDrawer(!(document.getElementById('side')?.classList.contains('show') ?? false));
    return;
  }
  if (innerWidth < 761 && !target.closest('#side, #menu')) setDrawer(false);
  if (target.closest('#cbtn')) {
    togglePop('cpop');
    return;
  }
  if (target.closest('#sbtn')) {
    togglePop('spop');
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
    closePops();
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
    closePops();
    closeInspector();
    markActiveRow(null);
    if (innerWidth < 761) setDrawer(false);
    const profile = win.cuadbProfile ? `?profile=${encodeURIComponent(win.cuadbProfile)}` : '';
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

document.addEventListener('dblclick', (event) => {
  const cell = event.target instanceof Element ? event.target.closest('td:not(.rn)') : null;
  if (cell instanceof HTMLElement && cell.closest('table.grid')) {
    void copyText(cell.dataset.detail ?? '', 'Cell copied');
  }
});

document.addEventListener('change', (event) => {
  const field = event.target;
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
    }
    if (event.target.matches('.sub')) {
      filterTree();
      loadObjects();
    }
    decorateBars(event.target);
  }
  closeInspector();
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
  if (target.id === 'sql-tx') {
    const open = target.querySelector('[data-tx]');
    win.cuadbTx = open instanceof HTMLElement ? (open.dataset.tx ?? '') : '';
  }
});
document.addEventListener('htmx:afterRequest', (event) => {
  const detail = /** @type {Any} */ (event).detail;
  const path = String(detail.pathInfo?.requestPath ?? '');
  const cancel = document.getElementById('sql-cancel');
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
  const path = String(/** @type {Any} */ (event).detail.pathInfo?.requestPath ?? '');
  if (path.startsWith('/sql/execute'))
    document.getElementById('sql-cancel')?.removeAttribute('hidden');
});
// A successful admin change: say so, then refresh the list it altered.
document.addEventListener('cuadb:changed', () => {
  // htmx fires response-header events before it swaps the body in; wait for the swap.
  setTimeout(afterChange, 80);
});
function afterChange() {
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
  if (inModal) closeModal();
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
