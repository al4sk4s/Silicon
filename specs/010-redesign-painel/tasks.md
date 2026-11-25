# Tarefas 010 — Redesign do painel web

Spec: `spec.md` · Plano: `plan.md`. Decisões do dono (25/11/2026): polling do pareamento a 5 s; "atividade de chats" conta grupos **e** conversas diretas; categorias de Ajustes = Geral, Mensagens, Figurinhas, Monitoramento; Debug entra na migração (T17); se o SVG do QR conflitar com a CSP, pode trocar a geração (fill por atributo).

Legenda: **W** = worker (sonnet, não compila nem commita) · **P** = principal (compila, integra, commita) · **S** = scout (só leitura) · `∥` = pode rodar em paralelo (sem cargo) · `▣` = serializado (cargo `-j1`, um por vez).
Fechar cada onda com `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (tudo `-j1`). Commit/push por onda (fast-forward).

## Onda 0 — Base
- [ ] **T1 (S)** Conferir `roles.rs`/store: contagem de chats por papel (grupos + DMs), `alerts(5)`, `dl_status`; listar o que falta para os widgets. Pronto: nota curta no fim do `plan.md`. · dep: —
- [ ] **T2 (P ▣)** Mover `panel.rs` → `panel/` (`mod.rs`, `guard.rs`, `auth.rs`, `layout.rs`, `pages/*`, `tests.rs`) com `git mv`, um submódulo por commit, `pub(super)`. Pronto: **comportamento idêntico**, 20 testes migrados verdes. · dep: —

## Onda 1 — Fundação (T3–T6 em ∥ entre si e com T1)
- [ ] **T3 (W ∥)** `assets/tokens.css`: tokens (cores dark/light com superfícies em níveis, semânticas, raios, sombras, tipografia, espaçamento, movimento, densidade, `prefers-reduced-motion`). Pronto: revisão visual dos dois temas. · dep: —
- [ ] **T4 (W ∥)** `assets/components.css`: botão, input, número, select, switch, checkbox, card, badge, tooltip, modal(`<dialog>`), toast, empty/loading, page/section header, status, tabela→cards no mobile, search; foco visível; sem `style=` nem `!important`. · dep: T3 (nomes dos tokens)
- [ ] **T5 (W ∥)** `assets/icons.svg`/ícones inline (traço único, tamanho padrão) + logo SVG simples do Silicon. · dep: —
- [ ] **T6 (W ∥)** `assets/theme.js`: lê `localStorage["silicon.prefs"]` versionado com lista branca; valor inválido/versão antiga → padrão; aplica `data-theme/accent/density/sidebar` no `<html>`; texto sobre o destaque automático (claro/escuro pelo brilho); tema "sistema". Pronto: função de validação isolada e testável no console. · dep: —
- [ ] **T7 (W ∥)** `assets/app.js` (módulos pequenos): sidebar/drawer, toast a partir de `.flash`, confirmação com `<dialog>` (`data-confirm`), busca (`data-q`), dirty-state (`beforeunload`, botão desabilitado ao enviar), polling (pausa com aba oculta; só injeta fragmento escapado ou `textContent`), dashboard (ordem/visibilidade/tamanho, arrastar + setas, restaurar). · dep: —
- [ ] **T8 (W ▣)** `assets.rs`: rota `/assets/{v}/{nome}` pública, hash por conteúdo (`sha2`), lista fixa, 404, cache imutável, `nosniff`. + CSP no `guard` e nos assets. Testes: abre sem sessão, 404 para nome/versão errados, não contém dado do bot, cabeçalhos presentes. · dep: T2, T3–T7 existirem
- [ ] **T9 (W ▣)** `components.rs` (`btn`, `icon`, `field_*`, `card`, `badge`, `page_header`, `section_header`, `status_dot`, `empty`, `table`, `search`, `confirm_form`, `flash`), tudo com `esc`. Testes: HTML esperado + escape de `<script>`. · dep: T2
- [ ] **T10 (P ▣)** Shell: `layout.rs` com sidebar (grupos da spec, compacta/expandida, tooltip, página atual), topbar (título, conexão, sessão, atalho Ajustes) e `login_page` novo, usando assets; remover `CSS`/`<script>`/`style=` inline do shell. Pronto: todas as páginas ainda abrem no novo shell; testes verdes; CSP ativa sem erro no console. · dep: T8, T9

## Onda 2 — Páginas (W em ∥ para escrever; compilar/testar em fila ▣; dep: T10)
- [ ] **T11 (W)** Chats: busca, tipo grupo/contato, papel, liberar/bloquear/trocar, adicionar por JID (sem `<script>` inline). Testes de CSRF/papéis mantidos.
- [ ] **T12 (W)** Contatos: iniciais, número, adicionar, remover **com confirmação**; `normalize_phone` intacto.
- [ ] **T13 (W)** Regras: lista, busca, ativar/desativar/editar, remover e ignorado **com confirmação**, criação, caixa de teste com resultado visual (casou / nenhuma / inválido).
- [ ] **T14 (W)** Alertas: lista cronológica, hora/regra/resumo, estado vazio.
- [ ] **T15 (W)** Métricas: números com unidade, sem gráfico (só agregados); fragmento `/metrics` mantido.
- [ ] **T16 (W)** Pareamento: ação principal (QR/código/ttl/tentar de novo/número); conectado = só estado e número; polling 5 s. **Se a CSP bloquear o SVG do QR, mudar `qr_svg` para `fill` por atributo.** `[WA]` ver QR vivo e parear.
- [ ] **T17 (W)** **Debug:** categorias, contagens, horários, detalhes; erro/recusa com ícone/rótulo (classe `.bad`, não `style=`); mono só em identificadores; legível com muitas linhas. Teste `debug_page_lists_and_escapes` mantido.
- [ ] **T18 (W)** Downloader: por plataforma, status de cookies, salvar (multipart ≤ 1 MiB), testar, remover **com confirmação**, limite/min; nunca exibir cookie.
- [ ] **T19 (W)** Ajustes por categoria: `parse_<cat>`/`save_<cat>`, `GET/POST /settings/{geral|mensagens|figurinhas|monitor}`, `POST /settings` legado roda os 4 parse e depois os 4 save. Testes novos: categoria não altera campos de outras; mesmas validações/mensagens; categoria sem mudança não envia.
- [ ] **T20 (W)** Dashboard + `GET /w/{nome}`: widgets (conexão, número, uptime, RAM/CPU, eventos, alertas, atividade de chats **grupos+DMs**, downloader, pareamento se desconectado); modo Personalizar; polling 5/10/15 s.
- [ ] **T21 (W)** Aparência (`/appearance`): tema, cor (presets + livre), densidade, sidebar, restaurar padrões.

## Onda 3 — Fechamento
- [ ] **T22 (P ▣)** Rodar tudo: fmt, clippy, test; testes de segurança novos (Origin errado → 403 em **todo** POST; todas as rotas exigem login exceto `/login` e `/assets`; escape de `<script>` em chat/contato/regra; nenhuma página contém hash/cookie/token). · dep: T11–T21
- [ ] **T23 (P)** Peso da primeira carga < 100 KB; acessibilidade (labels, foco, teclado, contraste AA, estado não só por cor); sem erros no console. · dep: T22
- [ ] **T24 (dono, manual)** Checklist no navegador: dashboard personalizado sobrevive a F5; dark/light; cor/densidade/sidebar; Ajustes salva uma categoria sem afetar outras; confirmações; celular com drawer; sem JS os formulários funcionam; `[WA]` QR/pareamento.
- [ ] **T25 (P)** Checklist "toda operação da tabela do spec continua possível" marcado; atualizar `src/help.rs`/bio só se houver mudança visível ao usuário no bot (provavelmente não); atualizar status da spec; `cargo build --release -j1`; avisar "build pronto"; `/gerar-prompt`.

## Ordem e paralelismo
`T1 ∥ T2` (T1 só lê) → `T3,T5,T6,T7 ∥` (T4 logo após T3) → `T8 → T9 → T10` (▣) → `T11–T21` escritos em ∥ por workers, **compilados em fila** pelo principal → `T22 → T23 → T24 → T25`.
Workers nunca rodam cargo nem commitam.
