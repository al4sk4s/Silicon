# Plano 003 — Monitor de promoções (alerta de bug)

Status: **revisado em 2026-11-20 contra o código de `origin/main` @ `5a5e214` e a lib** · Spec: `specs/003-monitor-promocoes/spec.md` · Depende de: 001, 004 · Constituição 1.1.0 (sem divergência) · whatsapp-rust @ `eeb24da` (fixo)

## Decisões a confirmar
1. **Dependências novas (P7):** `regex` 1.13.1 (regras editáveis exigem regex; motor de tempo linear, sem ReDoS) e `unicode-normalization` 0.1.26 (tirar acento do texto; a std não faz). Licenças MIT/Apache-2.0 (P9). Sem elas, a regra "bug" vira `contains` e some a edição por regex no painel.
2. **`@todos` não está verificado.** A lib só guarda `mentioned_jid` (`Vec<String>`); se menção só em `mentioned_jid` notifica, só o [WA] diz. Plano: o texto leva `@<user>` por pessoa **e** `mentioned_jid`. Se a notificação falhar no [WA], aceitar alerta sem notificação forte (o dono já está no grupo `alerta`) ou ajustar o texto.
3. **Chat `alerta` pode ser grupo ou DM.** Menções só se for `@g.us`; em DM o alerta já notifica sozinho.

## Lib: o que foi confirmado (rev `eeb24da`, arquivo:linha)
- `Client::send_message(to: impl Into<Jid>, wa::Message) -> Result<SendResult, SendError>` (`src/send/mod.rs:1512`); `SendResult.message_id`.
- Texto: `wa::Message{ extended_text_message: MessageField::some(ExtendedTextMessage{ text: Some(..), context_info: MessageField::some(ContextInfo{ mentioned_jid: Vec<String>, .. }), .. }), .. }` (padrão em `examples/demo.rs:211`).
- `MessageExt` (`use whatsapp_rust::MessageExt`): `text_content()`, `get_caption()` (imagem, vídeo, documento), `media_with_caption(&str) -> Option<wa::Message>` (só imagem/vídeo; **clona a mensagem de mídia original, sem baixar nem reenviar**; traz o `context_info` antigo), `set_context_info(ContextInfo) -> bool` (**substitui** o contexto inteiro; sem merge) — `wacore/src/proto_helpers.rs:163-239`.
- `client.groups().fetch_metadata(&Jid) -> GroupMetadata`; `participants[]{ jid, phone_number: Option, lid: Option }`; `jid` vem em PN ou LID conforme o `addressing_mode` do grupo (`src/features/groups.rs:1689, 810`).
- `MessageInfo`: `source.{chat, sender, is_from_me, is_group, sender_alt}`, `id`, `push_name`, `timestamp.as_secs()`. Não existe `sender_pn`/`sender_lid`.
- Próprio JID: `client.pn()` / `client.lid()` (`Option<Jid>`; `None` antes de parear). `get_pn`/`get_lid` **não existem**.
- Plano B de mídia: `Client::download(&dyn Downloadable, DownloadLimits::new(wire, plain))` e `Client::upload(Vec<u8>, MediaType, UploadOptions::default())`.

## Módulos
- `src/detector.rs` (novo, puro): `normaliza(&str)` (NFD, sem marcas, minúsculas); `Detector{regras, ignorar}` compilado com `RegexBuilder::size_limit(64 KiB)`/`dfa_size_limit`; `avalia(&Detector, &str) -> Option<RegraCasada{id, nome}>` (ignorar vence; primeira regra ativa por id). Regra inválida/gigante é recusada com erro legível.
- `src/alerta.rs` (novo, puro): `Dedupe` (`HashMap<u64, Instant>`, chave = hash do 1º link sem query de rastreio, senão do texto normalizado; janela `dedupe_min`, com limpeza de vencidos), `Limite` (`VecDeque<Instant>`, `alertas_hora`), `em_silencio(hora_atual, ini, fim)` (aceita intervalo que cruza meia-noite; vazio = desligado), `formata(...)`.
- `src/monitor.rs` (novo): `trata(st, client, ctx)` — fluxo abaixo; cache de participantes do grupo `alerta` (10 min); envio com retry.
- `src/wa.rs`: no `on_message`, `Action::Monitor` deixa de dar `return` e chama `monitor::trata` num `tokio::spawn` (nunca bloqueia o handler). Mensagens `is_from_me` e eco de `Sent` são ignoradas.
- `src/config.rs`: migração (tabelas `rules`, `ignore`, `alert_log`) + CRUD, `purge` e getters/setters de settings (padrão seguro abaixo). Usa o `Store` existente (`get`/`set`).
- `src/state.rs`: `AppState` ganha `detector: RwLock<Arc<Detector>>`, `dedupe`, `limite` (Mutex). Cada escrita no painel recompila e troca o `Arc` sem reiniciar. Eventos reaproveitam `push_event` (`alerta_enviado`, `alerta_falhou`, `alerta_sem_destino`; JID do chat, sem conteúdo).
- `src/panel.rs`: abas **Regras** (`/rules`: CRUD, ativar/desativar, lista de ignorar, **testar um texto**) e **Alertas** (`/alerts`: histórico). Mesmo login, CSRF/Origin e `esc()` das rotas atuais; links no menu. Ajustes (`/settings`) ganha os 4 settings do monitor.

```sql
CREATE TABLE rules(id INTEGER PRIMARY KEY, nome TEXT NOT NULL, padrao TEXT NOT NULL, ativa INTEGER NOT NULL DEFAULT 1);
INSERT INTO rules(nome,padrao) VALUES('bug','\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao|zinho|ue[im])?\b');
CREATE TABLE ignore(id INTEGER PRIMARY KEY, padrao TEXT NOT NULL);
CREATE TABLE alert_log(ts INTEGER NOT NULL, regra TEXT NOT NULL, resumo TEXT NOT NULL); -- resumo ≤ 80 chars, sem mídia
```
Regex padrão revisada: o texto chega sem acento (`ão`→`ao`). Deve casar: bug, bugs, bugado, BUGADA, bugados, bugou, bugar, bugando, buguei, bugzão, bugzinho. Não casa: debug, debugar, bugatti, ebuggy. Purge de `alert_log` (`ts < agora − retencao_dias`) na partida e a cada hora.

## Fluxo de `monitor::trata`
1. Texto = `text_content()` ou `get_caption()`; sem texto → fim. `Detector.avalia`; sem regra → fim (nada é gravado).
2. `Dedupe` → `Limite` → `em_silencio`: qualquer um barra → descarta (sem fila) e registra evento.
3. Destino = papel `alerta` lido **na hora** (`st.roles`); ausente → `alerta_sem_destino`, nada é enviado. Destino ≠ origem (garantido por papel único por chat).
4. Atraso P8 (`delay_min/max_ms`, mesmo da 001) e envio **só** via `client.send_message(jid_alerta, ..)`. **Nunca** `ctx.reply`/`ctx.react`/`send_message` ao chat de origem; sem read receipt.
5. Texto: `ExtendedTextMessage` com o formato abaixo + `mentioned_jid`. Imagem/vídeo: `media_with_caption(legenda_formatada)` → `set_context_info(ContextInfo{mentioned_jid})` (já descarta o contexto antigo, incluindo citação/encaminhado). `None` (tipo sem mídia) → cai para texto.
6. Menções: se destino `@g.us`, `fetch_metadata` (cache 10 min), `participants[].jid` menos `pn()`/`lid()` do bot; um `@<user>` no texto por pessoa. Falha no `fetch_metadata` → alerta sem menções (não perde o alerta).
7. Falha de envio: até 2 retentativas (5 s e 30 s), depois `alerta_falhou` (sem conteúdo). Erros ficam presos na task; nunca derrubam o daemon.
8. Sucesso: `alert_log` (regra, resumo 80 chars) + `alerta_enviado`; `sent.add(message_id)` para o anti-eco.

## Formato do alerta
```
🔔 Regra: bug
Grupo: <nome do chat monitorado, senão JID>
De: <pushname> (<user>)
Hora: 14:32
<link, se houver>

<texto original>

@<u1> @<u2>
```
Com mídia, o texto vai como legenda, truncada em ~1000 caracteres. Hora local (`state::local_hms`).

## Config e padrões seguros
Settings (tabela `settings` já existente): `dedupe_min`=30, `alertas_hora`=10, `silencio_ini`/`silencio_fim`=vazio (desligado), `retencao_dias`=7. Sem chat `alerta` ou sem chat `monitorado` = monitor inerte (bot mudo). Regra `bug` vem ativa; lista de ignorar vem vazia.

## Garantias (constituição)
- **P1/P2:** único envio por conta própria é para o chat `alerta`; `monitorado` nunca recebe nada; `alerta` segue `Drop` no `decide`. Sem ligação, sem feature `voip`.
- **P4/P6:** histórico guarda só resumo de 80 chars, sem mídia, 7 dias; conteúdo das promoções não sai para serviço externo; logs/eventos só com JID e tipo.
- **P8:** atraso + dedupe + limite/hora + silêncio; sem rajada, sem fila acumulada.

## Testes
**`cargo test` (sem rede):** detector (lista de casa/não casa acima, acentos, ignorar vence regra, regra inválida/gigante recusada, desativada ignorada); dedupe (dentro/fora da janela, mesmo link com textos diferentes); limite (11º bloqueado, janela desliza); silêncio (inclui 22h–7h e vazio=off); `formata` (snapshot, truncamento); Store (CRUD de regras, migração em banco existente, purge de 7 dias); painel `oneshot` (sem login redireciona, CSRF barra, regex inválida → 400 sem gravar, "testar texto"); função pura que escolhe o destino (monitorado nunca é destino, origem ≠ destino, sem `alerta` → nenhum envio).
**Só com WhatsApp real [WA] (bot do notebook, comigo presente):** alerta chega com a regra citada; **`@todos` notifica** (menção em `mentioned_jid` com/sem `@` no texto); LID vs PN dos participantes; `media_with_caption` com mídia de dias atrás (CDN expirado?); latência < 5 s; nada é enviado ao grupo monitorado.

## Riscos
1. **Ban:** envio automático 24/7 do número principal. Limites ativos por padrão; recomendado chip secundário (troca de sessão, sem mudar código).
2. **Mídia no CDN expira:** plano B (T8): `download` (limite 16 MiB) → `upload` → nova mensagem; se falhar, só texto.
3. **Menção em LID:** grupo em modo LID pode exigir o `jid` certo em `mentioned_jid`; validar no [WA].
4. **Lib pré-1.0:** rev fixado; a API acima foi lida no código desse rev.
5. **Falso positivo/negativo da regex:** só a palavra `bug`; a lista de ignorar e o "testar texto" do painel reduzem o ajuste às cegas.
6. **`pn()`/`lid()` `None` logo após o boot:** se faltar, não filtra o bot das menções (inofensivo).

## Fragmentação
| # | Passo | Arquivos | Agente | Depende |
|---|---|---|---|---|
| 1 | Deps `regex`=1.13.1, `unicode-normalization`=0.1.26 | Cargo.toml | você | — |
| 2 | `detector.rs` + testes | src/detector.rs | worker | 1 |
| 3 | `alerta.rs` (dedupe, limite, silêncio, formata) + testes | src/alerta.rs | worker | 1 |
| 4 | Migração, CRUD, settings, purge + testes | src/config.rs | worker | — |
| 5 | Painel `/rules` e `/alerts` + settings + oneshot | src/panel.rs, src/state.rs | worker | 2, 4 |
| 6 | `monitor::trata` + gancho no `wa.rs` | src/monitor.rs, src/wa.rs, src/state.rs | você | 2, 3, 4 |
| 7 | Validar com WhatsApp real (T7) e plano B de mídia se precisar (T8) | — | você + dono | 6 |

Paralelo: 2, 3 e 4 juntos (arquivos diferentes, sem build); depois 5 junto com 6 (`state.rs` é compartilhado: um só edita). Só o agente principal roda cargo: `fmt`, `clippy -j1 --all-targets -- -D warnings`, `test -j1 -- --include-ignored` após cada onda.

## Divergências da implementação (2026-11-20)
- Ordem dos filtros: **silêncio → dedupe → limite** (o silêncio não registra nada). Consequência: se o limite/hora barra, o dedupe já registrou a mensagem (não alerta de novo na janela).
- Cache de participantes do `alerta` é um `static` em `monitor.rs` (não campo de `AppState`); só existe um chat `alerta`.
- Novo evento `alerta_barrado` (silêncio/dedupe/limite), só com JID.
- Os campos de `state.rs` (`detector`, `dedupe`, `limite`, `recarrega_detector`) foram escritos pelo agente principal, antes do T5.
- Purge do `alert_log`: `tokio::spawn` em `main.rs` (interval de 1 h, 1º tick na partida), lê `retencao_dias` a cada tick.
- `/alerts` mostra só HH:MM:SS (sem dia) e sem paginação (corte em 100): risco conhecido.
