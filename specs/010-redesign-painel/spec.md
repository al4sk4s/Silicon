# Spec 010 — Redesign do painel web (rewrite de UI/UX)

Status: **APROVADA pelo dono em 2026-11-25; em implementação** · Depende de: 001, 005, 007, 003, 004 (downloader) · Só muda apresentação; **nenhuma regra do bot muda**.

## Por quê
O painel (`src/panel.rs`, ~1,7 mil linhas) renderiza HTML com um CSS de uma linha embutido, nav de
texto simples e um formulário longo em `/settings`. Funciona, mas é cru e difícil de manter (HTML, CSS e
handlers no mesmo arquivo). O dono quer um painel com cara de produto (referência Raycast/Arc, identidade
própria): limpo, sem excesso de informação, personalizável (tema, cor, densidade, dashboard) e organizado
em código reaproveitável.

## Estado atual (auditoria)
Rotas hoje (todas atrás do `guard`: CSRF/Origin em todo POST → login → cabeçalhos `no-store` + `X-Frame-Options: DENY`):

| Rota | Lê | Altera (via backend existente) |
|---|---|---|
| `/login` GET/POST, `/logout` | sessão | cria senha (1º acesso, só loopback), login c/ limite de tentativas, logout |
| `/` | status, eventos (50), métricas | — |
| `/metrics` (fragmento) | RAM/CPU/threads/uptime | — |
| `/chats` | chats + papéis | `set_role` / `clear_role` (`comando`/`monitorado`/`alerta`, alerta único), adicionar por JID |
| `/contacts` | contatos | adicionar (normaliza telefone), remover |
| `/rules` | regras, ignorados | add/edit/on/off/del regra; add/del ignorado; teste de texto |
| `/alerts` | alertas | — |
| `/debug` | contadores/linhas de debug | — |
| `/pair`, `/pair/state` | estado do pareamento (QR/código/ttl) | `POST` com telefone ou `retry` |
| `/settings` | config | dono (JID), delays, figurinha (limite/min, pack, autor, som, duração, modo de vídeo), monitor (dedupe, alertas/h, retenção, silêncio) |
| `/downloader…` | status de cookies por plataforma | limite/min, salvar (multipart ≤1 MiB), testar, remover |

Fatos que moldam o desenho: não há rota de arquivos estáticos (CSS/JS inline, e `/login` precisa de CSS sem
sessão); não há CSP; só há **agregados** de métricas (sem série temporal) e **lista de eventos** recentes.

## Histórias
- Abro o painel e vejo, num relance, se o bot está conectado, há quanto tempo e se algo exige atenção.
- Troco entre tema escuro/claro/sistema, escolho cor de destaque e densidade; a escolha fica após recarregar.
- Recolho a sidebar para só ícones; a escolha fica.
- Personalizo o dashboard: mostro/oculto, reordeno e redimensiono widgets; restauro o padrão.
- Em Ajustes, edito uma categoria por vez, vejo "alterações pendentes", salvo só ela e recebo feedback.
- Em Chats/Contatos/Regras faço tudo o que já faço hoje, com busca e confirmação nas ações destrutivas.
- No celular, a navegação vira drawer e as listas viram cards; continua utilizável.

## Requisitos funcionais
**Fundação**
- RF1 Design tokens em CSS custom properties (cores, superfícies em níveis, bordas, raios, sombras, tipografia, espaçamento, movimento). Dark e light completos (sem preto/branco puros); cores semânticas (sucesso/aviso/erro/info).
- RF2 Componentes reutilizáveis, definidos uma vez (função Rust que emite HTML + classe CSS): Button, IconButton, Input, NumberInput, Select, Switch, Checkbox, Card, Badge, Tooltip, Modal, Toast, EmptyState, LoadingState, PageHeader, SectionHeader, StatusIndicator, DataTable (vira lista no mobile), SearchInput. Nenhuma página repete HTML/CSS de componente.
- RF3 Ícones: um conjunto SVG inline único (mesmo traço/tamanho), sem emoji como ícone de UI, sem biblioteca/CDN externa.
- RF4 Fonte: pilha do sistema (sem webfont/CDN), tokens para título, seção, corpo, secundário, label, metadado, número, código.

**Layout e navegação**
- RF5 Sidebar fixa no desktop, agrupada pelas funções reais: Overview (Dashboard); Gerenciamento (Chats, Contatos); Automação (Regras, Alertas); Sistema (Métricas, Debug, Pareamento, Downloader); rodapé (Ajustes, Aparência, Sair). Expandida/compacta (compacta mostra ícones + tooltip), indicador da página atual, estado persistido.
- RF6 Topbar discreta: título/breadcrumb, indicador de conexão, menu da sessão e atalho de Ajustes. Busca global **só** se couber como paleta de navegação por páginas (ver Perguntas em aberto).
- RF7 URLs reais por página (as atuais continuam válidas); recarregar nunca quebra estado; auth preservada. Responsivo: desktop-first; tablet com sidebar adaptável; mobile com drawer, topbar simplificada, formulários em coluna, alvos de toque ≥ 40 px. Largura útil sem `max-width` minúsculo, com colunas de formulário/texto limitadas para leitura.

**Dashboard**
- RF8 `/` vira overview: cabeçalho curto (estado + ações rápidas) e widgets **só com dados que o backend já fornece**: conexão, número pareado, uptime, RAM/CPU/threads, eventos recentes, alertas recentes, atividade de chats (contagem por papel), estado do downloader (cookies por plataforma), pareamento (quando desconectado). Nada de métrica inventada.
- RF9 Modo "Personalizar": mostrar/ocultar, reordenar (arrastar + teclado), tamanho (pequeno/médio/grande onde fizer sentido), restaurar padrão. O modo de edição é temporário.
- RF10 Atualização dinâmica por fragmentos/JSON leves já existentes (`/metrics`, `/pair/state`) ou novos fragmentos atrás de login; intervalos ≥ 5 s, pausam com aba oculta, não tocam em campo em edição; falha de rede = indicador discreto, sem erro ruidoso.

**Ajustes**
- RF11 `/settings` dividido em categorias navegáveis (Geral, Mensagens, Figurinhas, Monitoramento; Downloader fica na própria página). Cada categoria é um formulário **independente** com POST próprio; só campos reais existentes.
- RF12 Controles pelo tipo: switch (booleano), número (inteiros), select (modo de vídeo), texto (JID, pack, autor), horário (silêncio). Slider só onde melhorar (ex.: duração do som), nunca obrigatório.
- RF13 Salvamento: indicador de alterações pendentes por categoria, botão desabilitado durante envio (anti duplo clique), validação no cliente **e** a validação atual do servidor (inalterada), toast de sucesso/erro, aviso ao sair com alterações pendentes, categoria sem alterações não envia request.
- RF14 **Semântica preservada**: mesmos nomes de campos, mesmos limites/erros, mesma persistência. O corte em POSTs por categoria não pode zerar/alterar campos de outras categorias (o handler aceita só os campos da categoria enviada).

**Páginas funcionais** (todas mantêm 100% das operações atuais)
- RF15 Chats: busca, lista com tipo (grupo/contato), papel atual, ações liberar/bloquear/trocar papel, adicionar por JID; feedback visual. Sem leitura de mensagens ou nada além de permissões.
- RF16 Contatos: nome, número, iniciais como identificação visual, remover **com confirmação (modal)**, formulário de adição; normalização e validação de telefone inalteradas.
- RF17 Regras: lista (busca se > N), nome, regex, ativo/inativo, editar/ativar/desativar/remover (remover com confirmação), criação, seção de padrões ignorados, caixa de teste com resultado visual (regra casou / nenhuma / padrão inválido). Detector e precedência inalterados.
- RF18 Alertas: lista cronológica com hora, regra e resumo; estado vazio. Sem conteúdo de mensagem.
- RF19 Métricas: números com unidade; **sem gráficos** enquanto só houver agregados (série temporal só se uma spec futura passar a guardar histórico).
- RF20 Debug: categorias, contagens, horários, detalhes; erro/recusa com marcação visual (ícone/rótulo, não só cor); mono só em identificadores/linhas.
- RF21 Pareamento: prioriza a ação (QR / código / tempo restante / tentar de novo / informar número); conectado mostra só estado e número; atualização automática. Lógica e geração do QR inalteradas.
- RF22 Downloader: por plataforma, status de cookies, salvar (upload), testar, remover (com confirmação), limite/min; conteúdo de cookies nunca exibido.

**Aparência** (`/appearance`, preferência só do navegador)
- RF23 Tema (escuro/claro/sistema via `prefers-color-scheme`), cor de destaque (presets + seletor livre, com piso de contraste do texto sobre o destaque), densidade (confortável/compacta), sidebar (expandida/compacta), restaurar padrões.
- RF24 Persistência em `localStorage`, versionada, com validação: valor inválido ou versão antiga → padrão seguro. Aplicada por script inline mínimo no `<head>` para evitar flash de tema. **Só** preferências visuais; nunca credencial, cookie, token ou dado do bot.
- RF25 Microinterações curtas (≤ 200 ms) em menus, sidebar, modal, toast, tooltip; `prefers-reduced-motion` desliga.

**Feedback**
- RF26 Padrão único: toast (sucesso/erro/aviso/info) para efêmero; mensagem inline para erro de campo; erros ao usuário sem detalhe interno; detalhe técnico só no log/`/debug` como hoje. Fluxos POST→redirect existentes continuam funcionando sem JS (a mensagem vem por flash/parâmetro já usado, hoje exibido como toast quando houver JS).

## Requisitos não funcionais
- RNF1 **Arquitetura:** server-side HTML em Rust/Axum + CSS e JS modulares **sem build step e sem framework** (sem React/Vue/Svelte/Node). `panel.rs` vira módulo `panel/` (`mod.rs`, `auth`/`guard`, `routes`, `pages/`, `components/`) com CSS/JS/ícones em arquivos próprios embutidos no binário (`include_str!`), servidos por rota estática (`/assets/…`, cache longo com versão no nome). Deploy continua sendo **um binário só**. Sem dependência nova (P7); se surgir necessidade, justificar no plano.
- RNF2 **Assets públicos:** apenas CSS/JS/ícones/fonte do sistema, sem dados do bot, servidos sem sessão (o login precisa deles). Todo o resto continua atrás do `guard`.
- RNF3 **Segurança preservada (testes existentes seguem verdes e são migrados, não apagados):** login, 1º acesso só loopback, sessões, expiração, limite de tentativas, logout, Origin/CSRF em todo POST (inclui `:authority` HTTP/2 e `SILICON_PROXY_HOST`), atributos do cookie, `no-store`, `X-Frame-Options`. Pode **acrescentar** CSP restritiva (`default-src 'self'`; scripts só de `/assets` + o bootstrap de tema com nonce/hash) — nunca afrouxar.
- RNF4 **Privacidade (P4/P6):** nada de conteúdo de mensagem, hash, senha, token, cookie do WhatsApp ou sessão em HTML, JS, logs ou `localStorage`. Todo dado dinâmico escapado (`esc`) ou inserido via `textContent`; sem `innerHTML` com dado do servidor sem escape.
- RNF5 **Rede (P5):** nenhum novo listener, nenhuma chamada externa (sem CDN, fonte ou analytics). Mesma regra de bind.
- RNF6 **Desempenho:** primeira página < 100 KB sem compressão (CSS+JS+ícones); sem polling < 5 s; polling pausa em aba oculta; roda bem no servidor pequeno atual.
- RNF7 **Acessibilidade:** labels associados, foco visível, navegação por teclado, modal com trap/retorno de foco, `aria-*` em switches/tooltips/toasts, contraste AA, estado nunca só por cor, HTML semântico.
- RNF8 **Compatibilidade:** funciona sem JS nas operações (forms nativos); JS só melhora (busca, toasts, dashboard, aparência, confirmações via `<dialog>`). Navegadores atuais (Chromium/Firefox/Safari recentes).
- RNF9 Código: comentários PT-BR, identificadores em inglês; sem lógica de negócio nos módulos de apresentação (handlers só chamam o backend existente).

## Fora do escopo
- Qualquer mudança em regras do bot, detector, papéis, whitelist, pareamento, downloader, esquema do banco, comandos de WhatsApp.
- Cliente de WhatsApp (ler/enviar mensagem, ver conteúdo), i18n, múltiplos usuários, 2FA.
- SPA/framework JS, build com Node, bibliotecas de gráfico, WebSocket/SSE (polling leve basta).
- Persistir preferências visuais/layout **no servidor** (v1: só navegador; ver pergunta 1).
- Série temporal de métricas e gráficos (exigiria guardar histórico; spec futura).
- Busca global de conteúdo; a paleta de comandos, se entrar, só navega entre páginas.

## Critérios de aceite
1. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` verdes (`-j1`); nenhum teste de painel removido sem equivalente.
2. Testes novos/migrados: `/assets/*` abre sem sessão e não contém dado do bot; todas as demais rotas exigem login; POST com Origin errado → 403 em **todas** as rotas POST; cada POST de Ajustes por categoria valida como antes e **não altera campos de outras categorias**; resposta com `no-store`/`X-Frame-Options`/CSP; HTML escapa nome de chat/contato/regra com `<script>`; nenhuma página contém hash/cookie/token.
3. Teste unitário de cada componente (HTML esperado + escape) e do parser de preferências (inválido → padrão).
4. Manual (navegador, dono): dashboard personalizado sobrevive a F5; tema/cor/densidade/sidebar idem; dark e light legíveis; Ajustes salva uma categoria sem afetar outras; confirmações de remover; pareamento atualiza sozinho; celular com drawer usável; console do navegador sem erros.
5. Todas as operações da tabela "Estado atual" continuam possíveis (checklist no `tasks.md`).
6. Peso da primeira carga dentro do RNF6.
7. Sem JS, as operações de CRUD/ajustes ainda funcionam.
8. Constituição conferida: P1/P2/P3 (nenhuma ação nova do bot), P4 (sem segredo no cliente/log), P5 (LAN+senha), P7 (sem dependência/stack nova).

## Conferência com a constituição
Sem conflito: o redesign é só apresentação sobre a mesma lógica. Pontos de atenção: **P5** (assets públicos não podem vazar dado; CSP só endurece), **P4/P6** (nada de mensagem/hash/cookie na UI nem no `localStorage`), **P7** (sem framework/build; ícones e CSS próprios).

## Entrega em etapas (para o plano/tarefas)
Auditoria → fundação (tokens, componentes, layout, assets) → dashboard → migração página a página (cada uma funcional ao fim) → aparência/preferências → polimento/acessibilidade → testes e checagem visual. Cada etapa deixa o painel funcionando; commits por etapa.

## Decisões do dono (25/11/2026)
1. Preferências visuais e layout: **só `localStorage`** (por navegador).
2. **Sem** paleta Ctrl+K (a busca global sai do escopo; RF6 fica sem ela).
3. **CSP aprovada**: sem `style=`/`onclick=` inline.
4. Confirmação (modal) só para remover **contato, regra, padrão ignorado e cookie**; bloquear chat não confirma.
5. Cor de destaque livre: texto sobre ela escolhido **automaticamente** (claro/escuro) pelo brilho.
6. Logotipo: **símbolo simples em SVG** + nome.

## Perguntas em aberto
- POST por categoria (`/settings/{cat}`) vs `/settings` aceitando subconjunto: decidir no plano.
- Widgets "Alertas recentes" e "Atividade de chats": confirmar no plano que o `AppState` já expõe os dados sem consulta nova cara.
- Alertas/Debug: sem histórico persistido novo; paginação só se a lista passar de ~100 itens.
