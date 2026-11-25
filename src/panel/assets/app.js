// Silicon painel: melhorias progressivas (toast, confirmação, filtro, formulário sujo,
// polling, sidebar, aparência e dashboard editável). Sem JS tudo continua funcionando via forms.
// Texto de variável entra só por textContent; innerHTML apenas com fragmento do servidor (polling).
(function () {
  "use strict";
  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };

  // ---------- Toast ----------
  function toast(msg, kind) {
    var box = $("#toasts");
    if (!box) return;
    kind = ["ok", "err", "warn", "info"].indexOf(kind) >= 0 ? kind : "info";
    var t = document.createElement("div");
    t.className = "toast";
    t.setAttribute("data-kind", kind);
    t.setAttribute("role", "status");
    var s = document.createElement("span");
    s.className = "toast-msg";
    s.textContent = String(msg);
    var b = document.createElement("button");
    b.type = "button";
    b.className = "icon-btn";
    b.setAttribute("aria-label", "Fechar aviso");
    b.textContent = "×";
    var close = function () { if (t.parentNode) t.parentNode.removeChild(t); };
    b.addEventListener("click", close);
    t.appendChild(s);
    t.appendChild(b);
    box.appendChild(t);
    setTimeout(close, kind === "err" ? 7000 : 4000);
  }
  window.SiliconToast = toast;

  function initFlash() {
    if (!$("#toasts")) return; // sem container, a mensagem fica inline
    $$(".flash").forEach(function (f) {
      var m = f.textContent.trim();
      f.parentNode.removeChild(f);
      if (m) toast(m, f.getAttribute("data-kind"));
    });
  }

  // ---------- Confirmação via <dialog> ----------
  function initConfirm() {
    var dlg = $("#confirm-dlg");
    document.addEventListener("submit", function (e) {
      var f = e.target;
      if (!f.hasAttribute || !f.hasAttribute("data-confirm") || f.__confirmed) return;
      var msg = f.getAttribute("data-confirm");
      if (!dlg || typeof dlg.showModal !== "function") {
        if (!window.confirm(msg)) e.preventDefault();
        return;
      }
      e.preventDefault();
      var sub = e.submitter || null;
      var opener = document.activeElement;
      $(".modal-title", dlg).textContent = f.getAttribute("data-confirm-title") || "Confirmar";
      $(".modal-text", dlg).textContent = msg;
      dlg.returnValue = ""; // senão Esc herdaria o "ok" anterior
      dlg.addEventListener("close", function () {
        if (dlg.returnValue === "ok") {
          f.__confirmed = true;
          if (f.requestSubmit) f.requestSubmit(sub || undefined); else f.submit();
          f.__confirmed = false;
        } else if (opener && opener.focus) {
          opener.focus();
        }
      }, { once: true });
      dlg.showModal();
      var cancel = $('button[value="cancel"]', dlg);
      if (cancel) cancel.focus();
    });
  }

  // ---------- Filtro ----------
  function initFilter() {
    function run(inp) {
      var tgt = $(inp.getAttribute("data-filter-target") || "");
      if (!tgt) return;
      var q = inp.value.trim().toLowerCase();
      $$("[data-q]", tgt).forEach(function (el) {
        el.hidden = q !== "" && (el.getAttribute("data-q") || "").indexOf(q) < 0;
      });
    }
    document.addEventListener("input", function (e) {
      if (e.target.hasAttribute && e.target.hasAttribute("data-filter")) run(e.target);
    });
    $$("[data-filter]").forEach(run);
  }

  // ---------- Formulário com alterações pendentes ----------
  function initDirty() {
    var forms = $$("form[data-dirty]");
    if (!forms.length) return;
    var leaving = false;
    var snap = function (f) {
      var p = [];
      new FormData(f).forEach(function (v, k) { p.push(k + "=" + (typeof v === "string" ? v : v.name)); });
      return JSON.stringify(p);
    };
    forms.forEach(function (f) {
      var base = snap(f);
      var saves = $$("[data-save]", f);
      var refresh = function () {
        var ch = snap(f) !== base;
        if (ch) f.setAttribute("data-changed", ""); else f.removeAttribute("data-changed");
        saves.forEach(function (b) { b.disabled = !ch; });
      };
      f.addEventListener("input", refresh);
      f.addEventListener("change", refresh);
      f.addEventListener("reset", function () { setTimeout(refresh, 0); });
      f.addEventListener("submit", function (e) {
        if (!f.hasAttribute("data-changed")) { e.preventDefault(); return; }
        // adiado: o submitter não pode estar desabilitado quando o FormData é montado
        setTimeout(function () {
          if (e.defaultPrevented) return;
          leaving = true;
          saves.forEach(function (b) { b.disabled = true; b.classList.add("is-loading"); });
        }, 0);
      });
      window.addEventListener("pageshow", function (ev) {
        if (ev.persisted) { leaving = false; saves.forEach(function (b) { b.classList.remove("is-loading"); }); refresh(); }
      });
      refresh();
    });
    window.addEventListener("beforeunload", function (e) {
      if (!leaving && forms.some(function (f) { return f.hasAttribute("data-changed"); })) {
        e.preventDefault();
        e.returnValue = "";
      }
    });
  }

  // ---------- Polling ----------
  function initPoll() {
    var els = $$("[data-poll]");
    if (!els.length) return;
    els.forEach(function (el) {
      var url = el.getAttribute("data-poll");
      var ms = Math.max(5000, parseInt(el.getAttribute("data-interval"), 10) || 10000);
      var busy = false, last = null, timer = null;
      function tick() {
        clearTimeout(timer);
        timer = setTimeout(tick, ms);
        if (document.hidden || busy) return;
        busy = true;
        fetch(url, { credentials: "same-origin" }).then(function (res) {
          if (res.redirected || res.status === 401 || /\/login/.test(res.url)) { location.reload(); return null; }
          if (!res.ok) { el.setAttribute("data-stale", ""); return null; }
          return res.text();
        }).then(function (html) {
          if (html === null || html === undefined) return;
          el.removeAttribute("data-stale");
          var a = document.activeElement;
          if (html === last || (a && a !== document.body && el.contains(a))) return;
          last = html;
          el.innerHTML = html; // fragmento do servidor, já escapado
        }).catch(function () {
          el.setAttribute("data-stale", "");
        }).then(function () { busy = false; });
      }
      timer = setTimeout(tick, ms);
      document.addEventListener("visibilitychange", function () { if (!document.hidden) tick(); });
    });
  }

  // ---------- Sidebar / drawer ----------
  function initShell() {
    var P = window.SiliconPrefs;
    var tg = $("#sidebar-toggle");
    if (tg && P) tg.addEventListener("click", function () {
      P.set({ sidebar: P.get().sidebar === "collapsed" ? "expanded" : "collapsed" });
    });
    var mb = $("#menu-btn");
    var setDrawer = function (open) {
      document.body.classList.toggle("drawer-open", open);
      if (mb) mb.setAttribute("aria-expanded", open ? "true" : "false");
    };
    if (mb) mb.addEventListener("click", function () { setDrawer(!document.body.classList.contains("drawer-open")); });
    var bd = $("#backdrop");
    if (bd) bd.addEventListener("click", function () { setDrawer(false); });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && document.body.classList.contains("drawer-open")) {
        setDrawer(false);
        if (mb) mb.focus();
      }
    });
  }

  // ---------- Aparência ----------
  function initAppearance() {
    var P = window.SiliconPrefs;
    if (!P) return;
    var sync = function () {
      var p = P.get();
      $$("input[data-pref]").forEach(function (i) {
        var k = i.getAttribute("data-pref");
        if (i.type === "radio") i.checked = i.value === p[k];
        else if (k === "accent-custom") i.value = p.accent;
      });
      $$("button[data-accent]").forEach(function (b) {
        b.setAttribute("aria-pressed", b.getAttribute("data-accent").toLowerCase() === p.accent ? "true" : "false");
      });
    };
    document.addEventListener("change", function (e) {
      var i = e.target;
      if (!i.getAttribute || i.type !== "radio") return;
      var k = i.getAttribute("data-pref");
      if (k === "theme" || k === "density" || k === "sidebar") { var o = {}; o[k] = i.value; P.set(o); sync(); }
    });
    document.addEventListener("input", function (e) {
      var i = e.target;
      if (i.getAttribute && i.getAttribute("data-pref") === "accent-custom") { P.set({ accent: i.value }); sync(); }
    });
    document.addEventListener("click", function (e) {
      var b = e.target.closest && e.target.closest("button[data-accent], #prefs-reset");
      if (!b) return;
      if (b.id === "prefs-reset") P.reset(); else P.set({ accent: b.getAttribute("data-accent") });
      sync();
    });
    sync();
  }

  // ---------- Dashboard ----------
  function initDash() {
    var dash = $("#dash");
    if (!dash) return;
    var KEY = "silicon.dash", SIZES = ["sm", "md", "lg"], SIZE_LABEL = { sm: "P", md: "M", lg: "G" };
    var ws = {}; // id -> elemento
    var defOrder = [], defSize = {};
    $$(".widget[data-widget]", dash).forEach(function (w) {
      var id = w.getAttribute("data-widget");
      if (ws[id]) return;
      ws[id] = w;
      defOrder.push(id);
      defSize[id] = SIZES.filter(function (s) { return w.classList.contains("w-" + s); })[0] || "md";
    });
    var strip = $("#dash-hidden"), editBtn = $("#dash-edit"), resetBtn = $("#dash-reset");
    var title = function (id) { return ws[id].getAttribute("data-title") || id; };

    // Lê e valida: ids que não existem mais são ignorados; novos entram no fim.
    function load() {
      var st = { order: defOrder.slice(), hidden: [], size: {} };
      try {
        var raw = JSON.parse(localStorage.getItem(KEY));
        if (!raw || raw.v !== 1) return st;
        var ord = [];
        (Array.isArray(raw.order) ? raw.order : []).forEach(function (id) {
          if (typeof id === "string" && ws[id] && ord.indexOf(id) < 0) ord.push(id);
        });
        defOrder.forEach(function (id) { if (ord.indexOf(id) < 0) ord.push(id); });
        st.order = ord;
        if (Array.isArray(raw.hidden)) st.hidden = raw.hidden.filter(function (id, i, a) {
          return typeof id === "string" && ws[id] && a.indexOf(id) === i;
        });
        if (raw.size && typeof raw.size === "object") Object.keys(raw.size).forEach(function (id) {
          if (ws[id] && SIZES.indexOf(raw.size[id]) >= 0) st.size[id] = raw.size[id];
        });
      } catch (e) {}
      return st;
    }
    var state = load();
    var editing = false, dragged = null;

    function save() {
      try {
        localStorage.setItem(KEY, JSON.stringify({ v: 1, order: state.order, hidden: state.hidden, size: state.size }));
      } catch (e) {}
    }
    var sizeOf = function (id) { return state.size[id] || defSize[id]; };

    function btn(label, text, act, id) {
      var b = document.createElement("button");
      b.type = "button";
      b.className = "icon-btn";
      b.textContent = text;
      b.setAttribute("aria-label", label);
      b.setAttribute("data-act", act);
      b.setAttribute("data-id", id);
      return b;
    }

    function render() {
      state.order.forEach(function (id) {
        var w = ws[id];
        dash.insertBefore(w, strip && strip.parentNode === dash ? strip : null);
        w.hidden = state.hidden.indexOf(id) >= 0;
        SIZES.forEach(function (s) { w.classList.toggle("w-" + s, s === sizeOf(id)); });
        w.draggable = editing;
        var tools = $(".widget-tools", w);
        if (tools) {
          tools.textContent = "";
          if (editing) {
            var t = title(id);
            tools.appendChild(btn("Mover " + t + " para cima", "↑", "up", id));
            tools.appendChild(btn("Mover " + t + " para baixo", "↓", "down", id));
            tools.appendChild(btn("Tamanho de " + t + ": " + sizeOf(id) + ". Alterar", SIZE_LABEL[sizeOf(id)], "size", id));
            tools.appendChild(btn("Ocultar " + t, "×", "hide", id));
          }
        }
      });
      if (strip) {
        strip.textContent = "";
        strip.hidden = !(editing && state.hidden.length);
        if (!strip.hidden) {
          var l = document.createElement("span");
          l.className = "muted";
          l.textContent = "Adicionar widget:";
          strip.appendChild(l);
          state.hidden.forEach(function (id) {
            var b = btn("Adicionar " + title(id), "+ " + title(id), "show", id);
            b.className = "btn btn-ghost btn-sm";
            b.setAttribute("data-show", id);
            strip.appendChild(b);
          });
        }
      }
      if (editing) dash.setAttribute("data-editing", ""); else dash.removeAttribute("data-editing");
      if (editBtn) editBtn.setAttribute("aria-pressed", editing ? "true" : "false");
    }

    function move(id, dir) {
      var o = state.order, i = o.indexOf(id), j = i + dir;
      while (j >= 0 && j < o.length && state.hidden.indexOf(o[j]) >= 0) j += dir; // pula ocultos
      if (j < 0 || j >= o.length) return;
      o.splice(i, 1);
      o.splice(j, 0, id);
    }

    function act(a, id) {
      if (a === "up") move(id, -1);
      else if (a === "down") move(id, 1);
      else if (a === "size") state.size[id] = SIZES[(SIZES.indexOf(sizeOf(id)) + 1) % SIZES.length];
      else if (a === "hide") { if (state.hidden.indexOf(id) < 0) state.hidden.push(id); }
      else if (a === "show") state.hidden = state.hidden.filter(function (x) { return x !== id; });
      else return;
      save();
      render();
      // devolve o foco ao botão equivalente (os botões são recriados)
      var next = $('[data-act="' + a + '"][data-id="' + id + '"]') || $('[data-act="show"]') || editBtn;
      if (a === "hide") next = $('[data-act="show"][data-id="' + id + '"]') || editBtn;
      if (next) next.focus();
    }

    dash.addEventListener("click", function (e) {
      var b = e.target.closest && e.target.closest("button[data-act]");
      if (b) act(b.getAttribute("data-act"), b.getAttribute("data-id"));
    });
    if (strip && strip.parentNode !== dash) strip.addEventListener("click", function (e) {
      var b = e.target.closest && e.target.closest("button[data-act]");
      if (b) act(b.getAttribute("data-act"), b.getAttribute("data-id"));
    });
    if (editBtn) editBtn.addEventListener("click", function () { editing = !editing; render(); });
    if (resetBtn) resetBtn.addEventListener("click", function () {
      try { localStorage.removeItem(KEY); } catch (e) {}
      state = { order: defOrder.slice(), hidden: [], size: {} };
      render();
      toast("Dashboard restaurado ao padrão.", "ok");
    });

    // Arrastar e soltar (HTML5); reordena no drop.
    var clearOver = function () { $$(".drag-over", dash).forEach(function (x) { x.classList.remove("drag-over"); }); };
    dash.addEventListener("dragstart", function (e) {
      var w = e.target.closest && e.target.closest(".widget");
      if (!editing || !w) { e.preventDefault(); return; }
      dragged = w.getAttribute("data-widget");
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", dragged); // Firefox exige
      w.classList.add("dragging");
    });
    dash.addEventListener("dragover", function (e) {
      var w = dragged && e.target.closest && e.target.closest(".widget");
      if (!w || w.getAttribute("data-widget") === dragged) return;
      e.preventDefault();
      clearOver();
      w.classList.add("drag-over");
    });
    dash.addEventListener("drop", function (e) {
      var w = dragged && e.target.closest && e.target.closest(".widget");
      if (!w || w.getAttribute("data-widget") === dragged) return;
      e.preventDefault();
      var r = w.getBoundingClientRect();
      var narrow = r.width < dash.clientWidth * 0.9;
      var after = narrow ? e.clientX > r.left + r.width / 2 : e.clientY > r.top + r.height / 2;
      state.order = state.order.filter(function (x) { return x !== dragged; });
      var i = state.order.indexOf(w.getAttribute("data-widget"));
      state.order.splice(after ? i + 1 : i, 0, dragged);
      save();
      render();
    });
    dash.addEventListener("dragend", function () {
      dragged = null;
      clearOver();
      $$(".dragging", dash).forEach(function (x) { x.classList.remove("dragging"); });
    });

    render();
  }

  function init() {
    [initFlash, initConfirm, initFilter, initDirty, initPoll, initShell, initAppearance, initDash].forEach(function (f) {
      try { f(); } catch (e) { /* uma melhoria quebrada não derruba as outras */ }
    });
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init); else init();
})();

// Selects com data-autosubmit enviam o form ao mudar (troca de papel em Chats).
document.addEventListener("change", (e) => {
  const t = e.target;
  if (t && t.matches && t.matches("select[data-autosubmit]") && t.form) t.form.requestSubmit();
});
