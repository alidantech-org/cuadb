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
document.addEventListener('htmx:configRequest', (event) => {
  const detail = /** @type {CustomEvent} */ (event).detail;
  const token = document.querySelector('meta[name="csrf-token"]')?.getAttribute('content');
  if (token && detail.verb !== 'get') detail.headers['x-csrf-token'] = token;
});

// Server error fragments (4xx/5xx with our error markup) are shown in place.
document.addEventListener('htmx:beforeSwap', (event) => {
  const detail = /** @type {CustomEvent} */ (event).detail;
  if (detail.xhr.status >= 400 && detail.xhr.responseText.includes('class="cuadb-error"')) {
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
function applyPrefs() {
  prefs.apply();
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
  title.append(label, close);
  panel.append(title);
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
  document.getElementById('insp')?.classList.remove('show');
  $$('tr.sel').forEach((el) => el.classList.remove('sel'));
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

document.addEventListener('dblclick', (event) => {
  const cell = event.target instanceof Element ? event.target.closest('td:not(.rn)') : null;
  if (cell instanceof HTMLElement && cell.closest('table.grid')) {
    void copyText(cell.dataset.detail ?? '', 'Cell copied');
  }
});

document.addEventListener('change', (event) => {
  const el = event.target;
  if (!(el instanceof HTMLInputElement)) return;
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
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    setDrawer(true);
    const input = /** @type {HTMLInputElement | null} */ (document.getElementById('f'));
    input?.focus();
    input?.select();
    return;
  }
  if (event.key === 'Escape') {
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
    if (event.target.matches('.sub')) filterTree();
  }
  closeInspector();
});

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
