# Plano 010 — Redesign do painel web

Spec: `spec.md` (decisões do dono de 25/11/2026). Linhas citadas são de `src/panel.rs` (1767 linhas) em `main`.
Só apresentação: nenhuma lógica de bot, detector, papéis, pareamento ou downloader muda.

## Decisão a confirmar
Nenhuma diverge da constituição. Ver "Dúvidas" no fim (polling do pareamento 2 s → 5 s; categorias de Ajustes).

## 1. Módulos (`src/panel.rs` → `src/panel/`)
Sem dependência nova (`sha2` já existe; P7). Funções movidas viram `pub(super)`.

| Arquivo | Conteúdo (origem) |
|---|---|
| `mod.rs` | `router` (56–80), tipos `S`/`Fields`, `esc` (23), `is_loopback` (38), `effective_bind` (44) |
| `guard.rs` | `cookie_token` (82), `same_origin` (91), `guard` (108), `redirect` (129) |
| `auth.rs` | login/setup/logout (165–278) |
| `layout.rs` | `page` (144) vira shell com sidebar + topbar; `login_page` (155); sai a `const CSS` (133) |
| `components.rs` | componentes (seção 4) |
| `assets.rs` | rota `/assets/{v}/{nome}` (seção 2) |
| `pages/` | `dashboard` (275–316, 283 index), `chats` (317–424), `contacts` (425–543), `settings` (544–597, 848–976), `rules` (598–696), `debug` (697), `alerts` (727), `pair` (743–847), `downloader` (977–1147), `appearance` (novo, sem POST) |
| `tests.rs` | testes (1148–1767) com `use super::*`; asserts de HTML ajustados **sem afrouxar** |

Migração em ondas; cada passo termina com `cargo test` verde (`-j1`, um build por vez).

## 2. Assets estáticos
- `src/panel/assets/{tokens.css, components.css, theme.js, app.js, icons.svg}` via `include_str!`.
- `GET /assets/{v}/{nome}`: `v` = hash curto (sha2) do conteúdo, calculado uma vez (`LazyLock`). `v` errado ou nome fora da lista fixa → 404. Fora do `guard` (router separado com `merge` depois do `.layer(guard)`); só devolve strings estáticas, sem acesso ao `AppState` → não vaza dado do bot.
- Cabeçalhos: `Cache-Control: public, max-age=31536000, immutable`, `X-Content-Type-Options: nosniff`, content-type correto.
- Ícones: preferir SVG inline via `icon(name)` (evita `<use href>` externo, não verificado no Safari).

## 3. CSP (aprovada)
`default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'` — posta no `guard` e nos assets, **sem nonce/hash**: o bootstrap de tema é `<script src="/assets/…/theme.js">` síncrono no `<head>` (sem flash).
Sai do HTML atual: `<script>` inline em 309 (poll de `/metrics`), 332 (busca de chats), 805 (poll de `/pair/state`); `style=` em 331, 639, 708 (`color:#c33` → classe `.bad` **mais** rótulo/ícone), 1025–1026. Não há `onclick`.
Risco: o SVG do QR (`qrcode` 0.14, `qr_svg` 743) pode trazer `style`/atributos bloqueados pela CSP → verificar no teste e no navegador; se bloquear, gerar as cores por atributo `fill`.

## 4. Componentes e JS
`components.rs`: funções `-> String`, toda string de usuário passa por `esc`: `btn`, `icon`, `field_text/number/select/switch`, `card`, `badge`, `page_header`, `section_header`, `status_dot`, `empty`, `table` (vira cards no mobile só por CSS), `search`, `confirm_form(action,label,msg)` (gera `data-confirm`), `flash(msg,kind)`. Hoje não há redirect: o POST re-renderiza com mensagem → `flash` vira toast com JS e fica inline sem JS (RF26). Tooltip/modal/toast: CSS + JS.
- `theme.js` (pequeno, síncrono): lê `localStorage["silicon.prefs"]` (versão + lista branca de chaves/enums; inválido/versão antiga → padrão), aplica `data-theme/accent/density/sidebar` em `<html>`, calcula cor do texto sobre o destaque pelo brilho, tema "sistema" via `prefers-color-scheme`. Só preferências visuais (RNF4).
- `app.js`: sidebar/drawer, toast, confirmação com `<dialog>` (foco nativo), busca por `data-q`, dirty-state (`beforeunload`, botão desabilitado ao enviar), polling (pausa com `document.hidden`; só injeta fragmento já escapado pelo servidor ou `textContent`), dashboard (ordem/visibilidade/tamanho no `localStorage`, arrastar + botões de seta para teclado, restaurar padrão).
- Sem JS tudo continua funcionando via forms nativos (RNF8).

## 5. Ajustes por categoria (pergunta 5 resolvida)
Hoje `settings_post` (852) grava tudo e **zera o que não veio** (`sticker_sound` ausente → false; pack/publisher ausentes → ""; delays/per_min/owner obrigatórios). Decisão:
- `GET/POST /settings/{cat}`, `cat ∈ geral | mensagens | figurinhas | monitor` (URL real; campo desconhecido ignorado).
- Cada categoria: `parse_<cat>(&st,&f) -> Result<Vals,String>` + `save_<cat>(&st,&Vals)`, reaproveitando as validações atuais sem mudar mensagens/limites:
  geral = `owner_jid`; mensagens = delay mín/máx; figurinhas = per_min, pack, publisher, som, duração, modo de vídeo; monitor = dedupe, alertas/h, retenção, silêncio.
- `POST /settings` legado (sem JS) roda os 4 `parse` e depois os 4 `save` (valida tudo antes de gravar, como hoje) → testes atuais seguem valendo.
- Teste novo: `POST /settings/figurinhas` sem `owner_jid` não altera `owner_jid` nem delays.
- Downloader continua na própria página (`/downloader/limit` etc.).

## 6. Dashboard e atualização dinâmica
Dados já em `AppState` (sem consulta nova cara): `status` (conexão, número), `boot` (uptime), `metricas::atualizar`, `events` (últimos 10), `store.alerts(5)`, contagem por papel (`store.roles()`/`st.roles` — **conferir** que expõe contagem), `dl_status` por plataforma, `pair_fragment` (só desconectado).
Fragmentos novos `GET /w/{nome}` atrás do `guard`, reutilizando a mesma função do render inicial. Polling: `/metrics` 5 s, status 10 s, eventos/alertas 15 s, `/pair/state` 5 s (era 2 s; RNF6).

## 7. Ondas (cada uma deixa o painel funcionando)
1. **Merge/mover:** `git mv` para `panel/mod.rs`; `tests.rs`; um submódulo por commit. (worker, serial)
2. **Assets + CSP + testes** (rota, 404, sem sessão, cabeçalhos). (worker)
3. **CSS tokens/componentes**, **icons.svg**, **theme.js/app.js** — sem cargo, em paralelo. (workers)
4. **`components.rs`** + testes unitários (HTML esperado, escape). (worker)
5. **Shell** (sidebar/topbar/login) — depende de 2–4.
6. **Páginas** (uma por worker, arquivos distintos): chats, contatos, regras, alertas, debug, pareamento, downloader; **ajustes por categoria**; **dashboard + `/w`**; **aparência**.
7. **Polimento/a11y**, medir peso (< 100 KB), checklist manual no navegador (dono).
Tudo que compila passa por **um único cargo `-j1`**; workers entregam sem compilar, o principal compila em fila.

## 8. Riscos e mitigação
- **Regressão de segurança** (CSRF/auth/cabeçalhos): testes atuais migrados, não apagados; novo teste "Origin errado → 403" em todo POST; assets públicos só com estáticos.
- **XSS:** `esc` em tudo; JS usa `textContent`; teste com `<script>` em nome de chat/contato/regra.
- **Privacidade (P4/P6):** nenhum hash/cookie/token/conteúdo na UI nem em `localStorage`.
- **Testes que casam trechos exatos de HTML:** ajustar com cuidado.
- **QR vs CSP** e **peso**: medir/verificar (itens acima).
- **Merge:** esta branch foi atualizada com `origin/main` antes do plano (o painel anterior estava defasado).

## 9. Testes
`cargo test` (unitários de componentes e parsers, integração Axum dos POSTs, assets, CSP, escape, ajustes por categoria). Só no navegador (manual): dashboard drag/teclado, temas, drawer mobile, `<dialog>`, console sem erros, validação do parser de prefs JS (opcional: função pequena testável no console). Nada aqui exige WhatsApp real, exceto ver o QR vivo (`[WA]`).

## Dúvidas para o dono
1. Polling do pareamento a 5 s (era 2 s) está ok?
2. "Atividade de chats" conta também contatos de DM ou só grupos?
3. Nomes das categorias de Ajustes (Geral / Mensagens / Figurinhas / Monitoramento) servem?
4. Se o SVG do QR conflitar com a CSP, aceito mudar a geração (atributos `fill`)?
