// Silicon painel: aplica as preferências visuais (tema, destaque, densidade, sidebar)
// ainda no <head>, antes da primeira pintura. Só guarda preferências visuais em localStorage.
(function () {
  var KEY = "silicon.prefs";
  var D = { v: 1, theme: "system", accent: "#7c8cff", density: "comfortable", sidebar: "expanded" };
  var OK = {
    theme: ["system", "dark", "light"],
    density: ["comfortable", "compact"],
    sidebar: ["expanded", "collapsed"],
  };
  var HEX = /^#[0-9a-f]{6}$/i;
  var mq = null;
  try { mq = window.matchMedia("(prefers-color-scheme: dark)"); } catch (e) {}

  // Valida contra lista branca; qualquer campo inválido (ou versão ≠ 1) volta ao padrão.
  function clean(o) {
    var r = { v: 1 };
    var ok = o && typeof o === "object" && o.v === 1;
    Object.keys(OK).forEach(function (k) {
      r[k] = ok && OK[k].indexOf(o[k]) >= 0 ? o[k] : D[k];
    });
    r.accent = ok && typeof o.accent === "string" && HEX.test(o.accent) ? o.accent.toLowerCase() : D.accent;
    return r;
  }

  function lum(h) {
    var c = [1, 3, 5].map(function (i) {
      var v = parseInt(h.substr(i, 2), 16) / 255;
      return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  }

  function get() {
    try { return clean(JSON.parse(localStorage.getItem(KEY))); } catch (e) { return clean(null); }
  }

  function apply(p) {
    try {
      var r = document.documentElement;
      r.setAttribute("data-theme", p.theme === "system" ? (mq && !mq.matches ? "light" : "dark") : p.theme);
      r.setAttribute("data-density", p.density);
      r.setAttribute("data-sidebar", p.sidebar);
      r.style.setProperty("--accent", p.accent);
      // texto sobre o destaque: o que tiver mais contraste (limiar ~0.179)
      r.style.setProperty("--accent-fg", lum(p.accent) > 0.179 ? "#0b0c10" : "#ffffff");
    } catch (e) {}
  }

  function set(partial) {
    var p = clean(Object.assign({}, get(), partial));
    try { localStorage.setItem(KEY, JSON.stringify(p)); } catch (e) {}
    apply(p);
    return p;
  }

  function reset() {
    try { localStorage.removeItem(KEY); } catch (e) {}
    var p = clean(null);
    apply(p);
    return p;
  }

  window.SiliconPrefs = { get: get, set: set, reset: reset, DEFAULTS: D };
  apply(get());
  try {
    if (mq) {
      var onChange = function () { if (get().theme === "system") apply(get()); };
      if (mq.addEventListener) mq.addEventListener("change", onChange);
      else if (mq.addListener) mq.addListener(onChange);
    }
  } catch (e) {}
})();
