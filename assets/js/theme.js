// Display preferences. Loaded synchronously in <head> so the first paint already uses the
// saved theme. Only non-sensitive choices are stored, never data, credentials or SQL.
(function () {
  var DEF = {
    mode: 'explore',
    theme: 'system',
    pal: 'mist',
    accent: '',
    rad: '6',
    fs: '12.5',
    ff: 'sans',
    dmono: false,
    dens: 'compact',
    lines: 'rows',
    rn: true,
    ty: true,
    zb: false,
    wrap: false,
    nulls: 'NULL',
    sep: false,
    icons: 'color',
    side: 'rows',
    unit: 'auto',
    remember: true,
    psize: '100',
    restore: true,
    confirmDiscard: true,
    autoOpen: true,
    motion: 'system',
    dates: 'raw',
    tgroup: true,
    tcounts: true,
    tsums: true,
    tsort: 'name',
    tdir: 'asc',
    ssort: 'name',
    hideEmpty: false,
  };
  /** @type {Record<string, string>} */
  var FONTS = {
    sans: 'system-ui,-apple-system,"Segoe UI",sans-serif',
    serif: 'ui-serif,Georgia,"Times New Roman",serif',
    mono: 'ui-monospace,SFMono-Regular,Menlo,monospace',
  };
  /** @type {Record<string, number>} */
  var HEIGHTS = { dense: 20, compact: 24, comfortable: 28, roomy: 34 };
  function load(/** @type {string} */ key) {
    try {
      return JSON.parse(localStorage.getItem(key) || 'null');
    } catch (e) {
      return null;
    }
  }
  /** @type {Record<string, any>} */
  var S = Object.assign({}, DEF, load('cuadb:s') || {});
  var TP = load('cuadb:t') || {};
  function save() {
    try {
      localStorage.setItem('cuadb:s', JSON.stringify(S));
      localStorage.setItem('cuadb:t', JSON.stringify(TP));
    } catch (e) {}
  }
  function apply() {
    var r = document.documentElement;
    var dark =
      S.theme === 'dark' ||
      (S.theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    r.dataset.theme = dark ? 'dark' : 'light';
    r.dataset.pal = S.pal;
    if (/^#[0-9a-f]{6}$/i.test(S.accent)) r.style.setProperty('--ac', S.accent);
    else r.style.removeProperty('--ac');
    r.style.setProperty('--rh', (HEIGHTS[S.dens] || 24) + 'px');
    r.style.setProperty('--rad', (Number(S.rad) || 6) + 'px');
    r.style.setProperty('--fs', (Number(S.fs) || 12.5) + 'px');
    r.style.setProperty('--ff', FONTS[S.ff] || FONTS.sans || '');
    r.style.setProperty('--nulls', JSON.stringify(String(S.nulls)));
    [
      ['zb', S.zb],
      ['nrn', !S.rn],
      ['nty', !S.ty],
      ['dm', S.dmono],
      ['wr', S.wrap],
      ['icm', S.icons === 'mono'],
      ['exm', S.mode === 'explore'],
      ['hide-counts', !S.tcounts],
      ['hide-sums', !S.tsums],
      [
        'no-motion',
        S.motion === 'on' ||
          (S.motion === 'system' && window.matchMedia('(prefers-reduced-motion: reduce)').matches),
      ],
    ].forEach(function (pair) {
      r.classList.toggle(pair[0], !!pair[1]);
    });
    ['rows', 'all', 'none'].forEach(function (l) {
      r.classList.toggle('l-' + l, S.lines === l);
    });
  }
  /** @type {any} */ (window).cuadbPrefs = { DEF: DEF, S: S, TP: TP, save: save, apply: apply };
  apply();
})();
