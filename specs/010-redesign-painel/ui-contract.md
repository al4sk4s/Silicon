# Contrato HTML/CSS/JS do painel (spec 010)

Fonte única entre Rust (`components.rs`, `layout.rs`, `pages/*`), CSS e JS. Sem `style=` nem `on*=` inline (CSP). Sem dependências externas.

## `<html>`
`<html lang="pt-BR" data-theme="dark|light" data-density="comfortable|compact" data-sidebar="expanded|collapsed">`.
`theme.js` (síncrono no `<head>`) lê `localStorage["silicon.prefs"]` = `{"v":1,"theme":"system|dark|light","accent":"#rrggbb","density":"comfortable|compact","sidebar":"expanded|collapsed"}`, valida (lista branca; inválido → padrão: theme system, accent `#7c8cff`, density comfortable, sidebar expanded), resolve `system` por `prefers-color-scheme`, grava `data-theme` (já resolvido), `data-density`, `data-sidebar` e define via CSSOM `--accent`, `--accent-fg` (texto claro/escuro pelo brilho) no `documentElement`. Expõe `window.SiliconPrefs = {get(), set(partial), reset(), DEFAULTS}` (set valida, grava, reaplica).

## Shell
```
<body><div class="app">
 <aside class="sidebar" id="sidebar">
  <div class="brand"><svg…/><span class="label">Silicon</span></div>
  <nav class="nav">
   <div class="nav-group"><div class="nav-title label">Overview</div>
    <a class="nav-item active" aria-current="page" href="/" data-tip="Dashboard"><svg class="icon"…/><span class="label">Dashboard</span></a>…
  </nav>
  <div class="sidebar-foot"> …nav-items… <button class="nav-item" id="sidebar-toggle" data-tip="Recolher">…</button></div>
 </aside>
 <div class="backdrop" id="backdrop"></div>
 <div class="main">
  <header class="topbar"><button class="icon-btn" id="menu-btn" aria-label="Abrir menu">…</button>
   <h1 class="topbar-title">Título</h1><span class="status" data-state="ok|warn|bad"><i class="dot"></i><span>Conectado</span></span>
   <div class="spacer"></div> …ações / <form class="inline"><button class="btn btn-ghost btn-sm">Sair</button></form></header>
  <main class="content">…</main>
 </div></div>
 <div class="toasts" id="toasts" role="region" aria-live="polite"></div>
 <dialog class="modal" id="confirm-dlg"><form method="dialog"><h2 class="modal-title"></h2><p class="modal-text"></p><div class="modal-actions"><button value="cancel" class="btn btn-ghost">Cancelar</button><button value="ok" class="btn btn-danger">Confirmar</button></div></form></dialog>
```
- Sidebar compacta (`data-sidebar=collapsed`): esconde `.label`, mostra `[data-tip]` como tooltip CSS (`::after`). Em telas ≤ 760px a sidebar vira drawer (`body.drawer-open` abre; `#backdrop` fecha), ignorando collapsed.
- Ícones: `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">` 18px.

## Componentes (classes)
`.btn` + `.btn-primary|.btn-ghost|.btn-danger` + `.btn-sm`; `.icon-btn`; `.input`, `.select`, `.textarea`; `.field` (> `label`, controle, `.hint`, `.err`); `.form-grid` (2 colunas ≥ 700px); `.switch` (`<label class="switch"><input type=checkbox role=switch><span class="track"></span><span class="switch-label">…</span></label>`); `.check`; `.card` (+ `.card-title`); `.badge` + `.ok|.warn|.bad|.info|.muted`; `.status` + `.dot`; `.empty` (> svg, p); `.page-header` (> `.title`, `.desc`, `.actions`); `.section-header`; `.table` (td com `data-label` → vira cartões ≤ 760px); `.list` / `.list-row` (> `.grow`, `.meta`, `.actions`); `.search` (> svg + input); `.flash[data-kind=ok|err|warn|info]` (inline sem JS; JS converte em toast); `.tabs` (> `a.tab[.active]` navegação de Ajustes); `.avatar`; `.mono`, `.muted`, `.stack`, `.row`, `.spacer`, `.inline`, `.grid`; `.bad-text`; `.qr` (container do SVG do QR); `.code-big` (código de pareamento).
Estados: hover, `:focus-visible` (anel com `--accent`), `:disabled`/`[aria-disabled]`, `.is-loading`.

## Dashboard
```
<section id="dash" class="dash-grid" data-editable>
  <article class="widget w-md" data-widget="conexao" data-title="Conexão"><header class="widget-head"><h2>…</h2><div class="widget-tools">(JS injeta botões)</div></header><div class="widget-body" data-poll="/w/conexao" data-interval="10000">…</div></article>
```
Tamanhos `.w-sm` (1 col) `.w-md` (2) `.w-lg` (full) num grid de 4 colunas (2 em tablet, 1 em mobile). Botões: `#dash-edit` (alterna `#dash[data-editing]`), `#dash-reset`. JS guarda em `localStorage["silicon.dash"]` = `{"v":1,"order":[ids],"hidden":[ids],"size":{id:"sm|md|lg"}}` validado contra os ids presentes; em edição mostra tools (↑ ↓ ocultar tamanho; arrastar HTML5 também); widgets ocultos aparecem numa faixa "Adicionar widget" (`#dash-hidden` com botões `data-show="id"`).

## JS (`app.js`, sem dependência)
- `.flash` presente → vira toast e some do fluxo; toasts: `SiliconToast(msg, kind)`; auto-fecha 4s (erro 7s), `role=status`.
- `form[data-confirm]` (valor = mensagem): intercepta submit, abre `#confirm-dlg`; só envia se `ok`. Atributo opcional `data-confirm-title`.
- `[data-filter]` no `input` + `data-filter-target="selector"`: filtra filhos com `data-q` (texto minúsculo) por substring; esconde com `hidden`.
- `form[data-dirty]`: marca `data-changed` quando algo muda (compara com valores iniciais), habilita `[data-save]` só se mudou, avisa `beforeunload` se há mudança e não enviou; ao submeter desabilita `[data-save]` (anti duplo clique) e põe `.is-loading`.
- `[data-poll=url][data-interval=ms]`: GET `fetch(url,{credentials:"same-origin"})`, substitui `innerHTML` do elemento só se a resposta for 200 e não houver foco/edição dentro dele; pausa com `document.hidden`; erro → `data-stale` no elemento (sem toast); 3xx/401/login (`res.redirected` ou url contém `/login`) → `location.reload()`.
- `#sidebar-toggle` alterna `SiliconPrefs.set({sidebar})`; `#menu-btn`/`#backdrop` alternam `body.drawer-open`; Esc fecha.
- Aparência: `[data-pref=theme]` e `[data-pref=density]` e `[data-pref=sidebar]` são `input type=radio` (value = valor); `button[data-accent]` (value cor); `input[data-pref=accent-custom]` (type=color); `#prefs-reset`. Refletem estado atual e aplicam na hora.
- Tudo com `textContent` quando o texto vem de variável JS; `innerHTML` só para fragmento do servidor já escapado.
- Respeita `prefers-reduced-motion`.
