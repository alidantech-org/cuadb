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
    boolFmt: 'dot',
    enumColor: '',
    idFmt: 'short',
    links: true,
    colIcons: true,
    tgroup: true,
    tcounts: true,
    tsums: true,
    tsort: 'name',
    tdir: 'asc',
    ssort: 'name',
    hideEmpty: false,
    contrast: 'normal',
    surface: 'soft',
    uidens: 'normal',
    iconSize: '14',
  };
  /** @type {Record<string, string>} */
  var FONTS = {
    sans: 'system-ui,-apple-system,"Segoe UI",sans-serif',
    inter: 'Inter,"SF Pro Text","Segoe UI Variable Text","Segoe UI",system-ui,sans-serif',
    humanist: '"Segoe UI",Seravek,"Gill Sans Nova",Ubuntu,Calibri,"DejaVu Sans",sans-serif',
    geometric: 'Montserrat,Avenir,"Avenir Next","Century Gothic",Futura,"Trebuchet MS",sans-serif',
    rounded:
      'ui-rounded,"SF Pro Rounded","Hiragino Maru Gothic ProN",Quicksand,Nunito,"Varela Round",system-ui,sans-serif',
    serif: 'ui-serif,Georgia,"Times New Roman",serif',
    slab: 'Rockwell,"Rockwell Nova","Roboto Slab","DejaVu Serif","Sitka Small",serif',
    mono: 'ui-monospace,SFMono-Regular,Menlo,Consolas,monospace',
    code: '"JetBrains Mono","Fira Code","Cascadia Code","SF Mono",ui-monospace,Menlo,monospace',
    dyslexic:
      '"OpenDyslexic","Atkinson Hyperlegible","Lexend","Comic Sans MS","Trebuchet MS",sans-serif',
  };
  /** @type {Record<string, string>} */
  var TONES = { paper: 'light', dim: 'dark', midnight: 'dark' };

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
      TONES[S.theme] === 'dark' ||
      (S.theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    r.dataset.theme = dark ? 'dark' : 'light';
    r.dataset.pal = S.pal;
    r.dataset.tone = TONES[S.theme] ? S.theme : '';
    r.dataset.contrast = S.contrast || 'normal';
    r.dataset.surface = S.surface || 'soft';
    r.dataset.uidens = S.uidens || 'normal';
    r.dataset.icons = S.icons || 'color';
    r.style.setProperty('--icsz', (Number(S.iconSize) || 14) + 'px');
    if (/^#[0-9a-f]{6}$/i.test(S.accent)) r.style.setProperty('--ac', S.accent);
    else r.style.removeProperty('--ac');
    r.style.setProperty('--rh', (HEIGHTS[S.dens] || 24) + 'px');
    r.style.setProperty('--rad', (Number.isFinite(Number(S.rad)) ? Number(S.rad) : 6) + 'px');
    r.style.setProperty('--fs', (Number(S.fs) || 12.5) + 'px');
    r.style.setProperty('--ff', FONTS[S.ff] || FONTS.sans || '');
    r.style.setProperty('--nulls', JSON.stringify(String(S.nulls)));
    if (/^#[0-9a-f]{6}$/i.test(S.enumColor)) r.style.setProperty('--enum', S.enumColor);
    else r.style.removeProperty('--enum');
    r.dataset.bool = S.boolFmt;
    [
      ['zb', S.zb],
      ['nrn', !S.rn],
      ['nty', !S.ty],
      ['dm', S.dmono],
      ['wr', S.wrap],
      ['icm', S.icons === 'mono'],
      ['no-ci', S.colIcons === false],
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
    r.classList.toggle('square', Number(S.rad) === 0);
    ['rows', 'all', 'none'].forEach(function (l) {
      r.classList.toggle('l-' + l, S.lines === l);
    });
  }
  /** @type {any} */ (window).cuadbPrefs = { DEF: DEF, S: S, TP: TP, save: save, apply: apply };
  apply();
})();
