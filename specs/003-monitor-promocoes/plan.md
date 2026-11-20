# Plano 003 — Monitor de promoções (alerta de bug)

Status: rascunho · Spec: `specs/003-monitor-promocoes/spec.md` · Depende de: 001 ·
Constituição 1.1.0 · whatsapp-rust @ `eeb24da`. ## Decisões a confirmar
1. **Alerta = mensagem nova, não "encaminhada".** Texto: `client.send_message(alerta, msg)` com
   `extended_text_message` + `context_info.mentioned_jid`. Imagem/vídeo:
   `MessageExt::media_with_caption(&caption)` (`wacore/src/proto_helpers.rs:203`) **reaproveita
   a referência no CDN, sem baixar nem reenviar a mídia**; depois `set_context_info` com as
   menções. `forward_message` (`src/send/mod.rs:1540`) existe, mas remove as menções e marca
   `is_forwarded` → descartado. Download em memória (`Client::download(.., DownloadLimits)`)
   só se o CDN expirar (risco 2).
2. **@todos.** A lib não tem "mencionar todos" (`group_mentions` é para subgrupo de comunidade).
   Proposta: `client.groups().fetch_metadata(&alerta)` → `participants[].jid` (sem o bot) em
   `mentioned_jid`, cache de 10 min, um `@<user>` no texto por pessoa (grupo pequeno).
   Menção "oculta" (só em `mentioned_jid`) **não verificada**.
3. **Normalização:** `unicode-normalization` 0.1.26 (NFD, tira marcas combinantes, minúsculas).
4. **Regex:** `regex` 1.13.1 com `RegexBuilder::size_limit(64 KiB)` e `dfa_size_limit`. Motor
   de tempo linear (sem ReDoS); limite só barra regra gigante. Regra que não compila é recusada
   no painel.

## Módulos
- `src/detector.rs` (novo, puro): `Regra{id,nome,re}`, `Msg<'a>{texto}`, `normaliza(&str)`,
  `avalia(&Detector,&Msg) -> Option<RegraCasada{id,nome}>`. Aplica a lista de ignorar antes
  (também regex, mesmo limite). Primeira regra ativa que casar vence (ordem por id).
- `src/alerta.rs` (novo, puro): `Dedupe` (`HashMap<u64,Instant>`; chave = hash do texto
  normalizado ou do 1º link sem query de rastreio; janela padrão 30 min), `Limite`
  (`VecDeque` de instantes; 10/h), `em_silencio(agora, ini, fim)` (aceita intervalo que cruza
  a meia-noite), `formata(...)`.
- `src/dispatch.rs`: `Action::Monitor` → `monitor::trata(ctx)` em `tokio::spawn`. Texto de
  `msg.text_content()` ou `msg.get_caption()`. Autor: `ctx.info.source.sender`; pushname:
  `ctx.info.push_name`; hora: `ctx.info.timestamp`. **Nunca** `ctx.send_message`/`reply` (iriam
  ao chat de origem); sempre `client.send_message(jid_alerta, ..)`.
- `src/monitor.rs` (novo): extração, menções (cache), envio com retry.
- `src/config.rs` (migração v2): tabelas `rules`, `ignore`, `alert_log` + settings.
- `src/panel.rs`: `/regras` (CRUD, ativar/desativar, **testar um texto contra as regras**) e
  `/alertas` (histórico). Cada escrita recompila e troca `Arc<RwLock<Detector>>` sem reiniciar.

```sql
CREATE TABLE rules(id INTEGER PRIMARY KEY, nome TEXT NOT NULL, padrao TEXT NOT NULL, ativa INTEGER NOT NULL DEFAULT 1);
INSERT INTO rules(nome,padrao) VALUES('bug','\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao|zinho|ue[im])?\b');
CREATE TABLE ignore(id INTEGER PRIMARY KEY, padrao TEXT NOT NULL);
CREATE TABLE alert_log(ts INTEGER NOT NULL, regra TEXT NOT NULL, resumo TEXT NOT NULL); -- resumo ≤ 80 chars, sem mídia
-- settings: dedupe_min(30) alertas_hora(10) silencio_ini/fim(vazio=off) retencao_dias(7)
```
**Regex padrão revisada.** O texto chega normalizado (sem acento), então `ão` vira `ao` e a
alternativa útil é `zao`. Deve casar: bug, bugs, bugado, BUGADA, bugados, bugou, bugar,
bugando, buguei, bugzão, bugzinho. Não deve casar: debug, debugar, bugatti, ebuggy. `\b` é
Unicode no crate. Purge `DELETE FROM alert_log WHERE ts<?` na partida e a cada hora.

## Formato do alerta
```
🔔 Regra: bug
Grupo: <nome>
De: <pushname> (<user>)
Hora: 14:32
<link se houver>

<texto original>

@<u1> @<u2>
```
Com mídia, o texto vai como legenda, truncada em ~1000 caracteres.

## Garantias
- **P1/P2:** destino vem só do papel `alerta` lido na hora do envio. Sem chat `alerta`, nada é
  enviado e o log registra "sem alerta". Teste: destino ≠ origem; `monitorado` nunca é destino.
- **Falha de envio:** até 2 retentativas (5 s e 30 s), depois desiste e registra evento sem
  conteúdo. Erros ficam presos na task; nunca derrubam o daemon.
- **P8:** atraso 1–2 s (mesmo `delay_*` da 001) + dedupe + limite/hora + silêncio. No
  silêncio a mensagem é descartada (sem fila).
- **RF1:** o fluxo `monitorado` não envia read receipt nem reação.

## Testes `cargo test` (sem rede)
Detector (casa/não casa, acentos, ignorar vence regra, regra inválida recusada, desativada
ignorada); dedupe (dentro/fora da janela, mesmo link com textos diferentes); limite (11º
bloqueado); silêncio (inclui 22h–7h); formata (snapshot); Store (CRUD de regras, purge de 7
dias); destino nunca igual ao monitorado.
**Só com WhatsApp real:** notificação do @todos, menção sem `@` no texto,
`media_with_caption` dias depois (CDN expirado?), LID vs PN dos participantes, latência < 5 s.

## Riscos
1. Ban: envio automático 24/7 do número principal. Limites ativos por padrão; recomendado chip
   secundário.
2. Referência de mídia no CDN pode expirar. Alternativa: `download` (`DownloadLimits` 16 MiB) →
   `upload` → nova mensagem; se falhar, só texto.
3. Lib pré-1.0: rev fixado.
4. Privacidade: histórico guarda só resumo de 80 caracteres, apagado em 7 dias.

## Fragmentação
| # | Passo | Arquivos | Agente | Depende |
|---|---|---|---|---|
| 1 | Deps `regex`=1.13.1, `unicode-normalization`=0.1.26 | Cargo.toml | módulo | 001 pronto |
| 2 | `detector.rs` + testes | src/detector.rs | módulo | 1 |
| 3 | `alerta.rs` (dedupe, limite, silêncio, formata) + testes | src/alerta.rs | módulo | 1 |
| 4 | Migração v2, CRUD, purge + testes | src/config.rs | módulo | 1 |
| 5 | Painel `/regras` e `/alertas` + oneshot | src/panel.rs | módulo | 2, 4 |
| 6 | `monitor::trata` no dispatch: extração, menções, envio com retry | src/dispatch.rs, src/monitor.rs | integração | 2, 3, 4 |
| 7 | Validar com WhatsApp real | — | integração | 6 |

Paralelo: 2, 3 e 4 juntos; depois 5 junto com 6. Só a integração roda cargo. Serial: a integração
roda `cargo build/clippy -D warnings/test -j1` após cada onda (1; 2–4; 5–6). Erro difícil de
tipos com `wa::Message` → revisão mais cuidadosa.

## Não verificado
`mentioned_jid` sem `@user` no texto notifica?; validade da mídia reaproveitada do CDN; caminho
exato até `ctx.info` (campos vieram de `wacore/src/types/message.rs:436-438`).
