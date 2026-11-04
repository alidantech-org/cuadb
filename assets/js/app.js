// Browser-only behaviors live here. Data requests and pagination use HTMX.
document.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;

  const toggle = target.closest('[data-toggle-sidebar]');
  if (toggle instanceof HTMLButtonElement) {
    const sidebar = document.getElementById('side');
    if (sidebar) {
      const opened = sidebar.classList.toggle('show');
      toggle.setAttribute('aria-expanded', String(opened));
    }
    return;
  }
  const popTrigger = target.closest('[data-pop]');
  if (popTrigger instanceof HTMLButtonElement) {
    const pop = document.getElementById(popTrigger.dataset.pop ?? '');
    if (pop) {
      const opened = pop.hidden;
      pop.hidden = !opened;
      popTrigger.setAttribute('aria-expanded', String(opened));
    }
    return;
  }
  if (target.closest('[data-columns-all]') || target.closest('[data-columns-none]')) {
    const all = target.closest('[data-columns-all]') !== null;
    const form = /** @type {HTMLFormElement | null} */ (document.querySelector('.row-search'));
    const params = new URLSearchParams(form ? /** @type {any} */ (new FormData(form)) : undefined);
    params.delete('column');
    params.set('pick', '1');
    if (all) {
      document.querySelectorAll('[data-column-choice]').forEach((item, index) => {
        if (!(item instanceof HTMLInputElement)) return;
        item.checked = index < 24;
        if (item.checked) params.append('column', item.dataset.columnChoice ?? '');
      });
    } else {
      document.querySelectorAll('[data-column-choice]').forEach((item) => {
        if (item instanceof HTMLInputElement) item.checked = false;
      });
      params.set('pick', '1');
    }
    if (!all) params.delete('column');
    if (!all)
      document
        .querySelectorAll('[data-column-choice]:checked')
        .forEach((item) => params.append('column', item.getAttribute('data-column-choice') ?? ''));
    /** @type {any} */ (window).htmx?.ajax('GET', `/catalog/data?${params.toString()}`, {
      target: '#catalog-panel',
      swap: 'outerHTML',
    });
    return;
  }
  if (target.closest('[data-compact-toggle]')) {
    const grid = document.querySelector('.data-grid');
    const button = target.closest('[data-compact-toggle]');
    if (grid && button) {
      const compact = grid.classList.toggle('compact');
      button.setAttribute('aria-pressed', String(compact));
      try {
        localStorage.setItem('cuadb.grid.compact', String(compact));
      } catch {}
    }
    return;
  }
});

document.addEventListener('htmx:afterSwap', (event) => {
  if (event.target instanceof Element && event.target.matches('#catalog-panel')) {
    const compact = localStorage.getItem('cuadb.grid.compact') === 'true';
    event.target.querySelector('.data-grid')?.classList.toggle('compact', compact);
    event.target
      .querySelector('[data-compact-toggle]')
      ?.setAttribute('aria-pressed', String(compact));
  }
});

document.addEventListener('keydown', (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    document.getElementById('f')?.focus();
  }
  if (event.key === 'Escape') {
    document.getElementById('side')?.classList.remove('show');
    document.querySelectorAll('.pop.show').forEach((element) => element.classList.remove('show'));
    const inspector = document.getElementById('insp');
    if (inspector) inspector.hidden = true;
    document
      .querySelectorAll('.data-grid tr[aria-selected="true"]')
      .forEach((row) => row.removeAttribute('aria-selected'));
  }
  const status = event.target;
  if (
    status instanceof HTMLElement &&
    status.id === 'connection-indicator' &&
    (event.key === 'Enter' || event.key === ' ')
  ) {
    event.preventDefault();
    /** @type {any} */ (window).htmx?.trigger(status, 'click');
  }
});

document.addEventListener('htmx:beforeSwap', (event) => {
  const detail = /** @type {CustomEvent} */ (event).detail;
  if (detail.xhr.status >= 400 && detail.xhr.responseText.includes('class="cuadb-error"')) {
    detail.shouldSwap = true;
    detail.isError = false;
    detail.swapOverride = 'innerHTML';
  }
});

document.addEventListener('input', (event) => {
  const input = event.target;
  if (input instanceof HTMLInputElement && input.matches('[data-column-choice]')) {
    const form = /** @type {HTMLFormElement | null} */ (document.querySelector('.row-search'));
    const params = new URLSearchParams(form ? /** @type {any} */ (new FormData(form)) : undefined);
    params.delete('column');
    params.set('pick', '1');
    params.delete('page');
    params.delete('cursor');
    document
      .querySelectorAll('[data-column-choice]:checked')
      .forEach((item) => params.append('column', item.getAttribute('data-column-choice') ?? ''));
    /** @type {any} */ (window).htmx?.ajax('GET', `/catalog/data?${params.toString()}`, {
      target: '#catalog-panel',
      swap: 'outerHTML',
    });
    return;
  }
  if (!(input instanceof HTMLInputElement) || input.id !== 'f') return;
  const needle = input.value.trim().toLocaleLowerCase();
  document.querySelectorAll('#tree [data-schema-name]').forEach((schema) => {
    const schemaName = schema.getAttribute('data-schema-name')?.toLocaleLowerCase() ?? '';
    const schemaMatches = needle !== '' && schemaName.includes(needle);
    const relations = schema.querySelectorAll('[data-relation-name]');
    let relationMatches = false;
    relations.forEach((relation) => {
      const name = relation.getAttribute('data-relation-name')?.toLocaleLowerCase() ?? '';
      const matches = needle === '' || schemaMatches || name.includes(needle);
      relation.toggleAttribute('hidden', !matches);
      relationMatches ||= matches;
    });
    schema.toggleAttribute('hidden', needle !== '' && !schemaMatches && !relationMatches);
  });
});

document.addEventListener('click', (event) => {
  const link =
    event.target instanceof Element ? event.target.closest('#tree [data-relation-name] a') : null;
  if (!link) return;
  document
    .querySelectorAll('#tree [data-relation-name] a[aria-current]')
    .forEach((item) => item.removeAttribute('aria-current'));
  link.setAttribute('aria-current', 'page');
});

document.addEventListener('click', async (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const cell = target.closest('[data-detail]');
  if (cell instanceof HTMLButtonElement) {
    const panel = document.getElementById('insp');
    const value = panel?.querySelector('[data-detail-value]');
    const type = panel?.querySelector('[data-detail-type]');
    if (panel && value && type) {
      document
        .querySelectorAll('.data-grid tr[aria-selected="true"]')
        .forEach((row) => row.removeAttribute('aria-selected'));
      cell.closest('tr')?.setAttribute('aria-selected', 'true');
      value.textContent = cell.dataset.detail ?? '';
      type.textContent = cell.dataset.type ?? '';
      panel.hidden = false;
    }
    return;
  }
  if (target.closest('[data-close-detail]')) {
    const panel = document.getElementById('insp');
    if (panel) panel.hidden = true;
    document
      .querySelectorAll('.data-grid tr[aria-selected="true"]')
      .forEach((row) => row.removeAttribute('aria-selected'));
    return;
  }
  if (target.closest('[data-copy-detail]')) {
    const value = document.querySelector('#insp [data-detail-value]')?.textContent ?? '';
    try {
      await navigator.clipboard.writeText(value);
      showToast('Copied cell value');
    } catch {
      showToast('Clipboard access is unavailable');
    }
  }
});

/** @type {ReturnType<typeof setTimeout> | undefined} */
let toastTimeout;
/** @param {string} message */
function showToast(message) {
  const toast = document.getElementById('toast');
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => toast.classList.remove('show'), 1800);
}

const splitter = document.getElementById('rs');
const sidebar = document.getElementById('side');
if (splitter && sidebar) {
  const savedWidth = Number(localStorage.getItem('cuadb.sidebar.width'));
  if (savedWidth >= 180 && savedWidth <= 520)
    document.documentElement.style.setProperty('--sw', `${savedWidth}px`);
  let dragging = false;
  splitter.addEventListener('pointerdown', (event) => {
    if (window.matchMedia('(max-width: 760px)').matches) return;
    dragging = true;
    splitter.setPointerCapture(event.pointerId);
  });
  splitter.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    const width = Math.max(180, Math.min(520, event.clientX));
    document.documentElement.style.setProperty('--sw', `${width}px`);
    splitter.setAttribute('aria-valuenow', String(width));
  });
  const stopDrag = () => {
    if (!dragging) return;
    dragging = false;
    const width = Math.max(180, Math.min(520, sidebar.getBoundingClientRect().width));
    localStorage.setItem('cuadb.sidebar.width', String(Math.round(width)));
  };
  splitter.addEventListener('pointerup', stopDrag);
  splitter.addEventListener('pointercancel', stopDrag);
  splitter.addEventListener('dblclick', () => {
    document.documentElement.style.setProperty('--sw', '264px');
    splitter.setAttribute('aria-valuenow', '264');
    localStorage.setItem('cuadb.sidebar.width', '264');
  });
  splitter.addEventListener('keydown', (event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowRight' ? 16 : -16;
    const width = Math.max(180, Math.min(520, sidebar.getBoundingClientRect().width + delta));
    document.documentElement.style.setProperty('--sw', `${width}px`);
    splitter.setAttribute('aria-valuenow', String(width));
    localStorage.setItem('cuadb.sidebar.width', String(width));
  });
}
