# Plano 001 — Núcleo (o COMO)

Status: rascunho · Spec: `specs/001-nucleo/spec.md` · Constituição v0.2.0 · whatsapp-rust no
commit `eeb24da` (0.7.0, MSRV 1.95; Rust local 1.100). API
conferida no código da lib.

## Decisões do dono (2026-11-19)
- HTTPS autoassinado na LAN: **aprovado**. Papéis por **JID** (nome só rótulo): **aprovado**.
- Dono = a conta pareada (`is_from_me`): **aprovado**. RF13 (19/11/2026): papéis **só pelo
  painel**; `/permitir` e `/bloquear` por mensagem **não** serão implementados, `decide()`
  não muda (chat sem papel = Drop, sempre).

## Decisões a confirmar (histórico)
1. **HTTPS autoassinado só quando o bind não for loopback.** Em `127.0.0.1` fica HTTP. Na
   LAN, rustls com certificado gerado uma vez pelo `rcgen`, salvo com `0600`. Adiciona
   `axum-server` (tls-rustls) e `rcgen`. Alternativa: HTTP puro (senha em claro na LAN).
   **Recomendado: HTTPS.**
2. **Config só em SQLite** (`silicon.db`, separado do `.db` da sessão), sem TOML. Caminhos
   por flag/env (`SILICON_DATA_DIR`, padrão `/var/lib/silicon`).
3. **Dono = a conta pareada.** O bot roda no número principal, então o comando de admin é
   a mensagem com `is_from_me == true`. `owner_jid` extra é opcional (migração para chip
   secundário). Por isso é obrigatório **ignorar as respostas do próprio bot** (risco 2).
4. **Lista de chats no painel:** grupos via `client.groups().list_participating()` (JID +
   `subject`). Não há API para listar conversas 1:1: campo manual de JID + lista em memória
   (não persistida) dos JIDs que mandaram mensagem recentemente (só JID e horário). Confirmar
   que guardar o JID de chat sem papel não fere a P2 (na leitura do plano, não fere: não é
   parse).
5. **Crate único**, sem workspace multi-crate.

## API confirmada da lib (eeb24da)
- `whatsapp-rust = { git = "https://github.com/oxidezap/whatsapp-rust", rev = "<hash completo>" }`.
  Features padrão (`sqlite-storage`, `sqlite-storage-bundled`, `tokio-transport`,
  `tokio-runtime`, `ureq-client`, `tokio-native`, `signal`) **não incluem voip**. Nunca ativar
  `voip*` nem `test-support`.
- `Bot::builder()`: `.with_backend(SqliteStore::open(path).await?)`,
  `.on_qr_code(|code, ttl| ..)`, `.with_pair_code(PairCodeOptions)` + `.on_pair_code(..)`,
  `.on_message(|ctx: MessageContext| ..)`, `.on_connected(..)`, `.on_logged_out(..)`;
  depois `.build().await?` e `bot.run().await`.
- `ctx.info.source`: `chat: Jid`, `sender: Jid`, `is_from_me`, `is_group`, `sender_alt`
  (LID/PN). Resposta: `ctx.reply(..)` (só após o atraso, P8).
- Próprio JID: `client.pn()` / `client.lid()` (`None` antes do pareamento).
- Reconexão **automática** (`client.set_auto_reconnect(bool)`; backoff com jitter e teto em
  `wacore/src/protocol/reconnect.rs`: até 64 tentativas, base ≤ 900 s). **Não escrever
  backoff próprio**; RF9 vira configuração.
- `MessageContext` já chega decifrado. "Descartar antes de parse" vale para **nosso** parse,
  download e resposta.

## Dependências (fixar versão)
whatsapp-rust (git rev), tokio 1.54 (macros, rt-multi-thread, signal, time), axum 0.8.9,
rusqlite 0.43 (**sem** `bundled`: a lib já força libsqlite3-sys 0.40, compatível), argon2,
rand, tracing 0.1.44 + tracing-subscriber, serde 1.0.229, qrcode (QR unicode no terminal),
axum-server + rcgen (se decisão 1 aceita). Conferir licenças de argon2/qrcode/rcgen/
axum-server (P9). Lock de instância: `std::fs::File::try_lock` (sem crate).

## Módulos (`src/`)
- `main.rs`: CLI mínima — `silicon` (daemon) e `silicon set-password` (stdin sem eco, grava
  hash argon2). Monta estado, pega o lock, sobe painel e bot.
- `lock.rs`: `<data_dir>/silicon.lock` + `try_lock()`; falha → sai "outra instância rodando".
- `config.rs`: `Store` sobre rusqlite (`Mutex<Connection>`), migração inline (`user_version`),
  arquivos `0600` (inclusive o `.db` da sessão após o open).
- `roles.rs`: `enum Role { Comando, Monitorado, Alerta }`; `ChatRoles =
  Arc<RwLock<HashMap<String, Role>>>` (cache da tabela `roles`, recarregado a cada escrita).
- `dispatch.rs`: `fn decide(roles, chat, sender, from_me, owner) -> Action` **pura e
  testável**; `enum Action { Drop, Command { admin: bool }, Monitor }`. Handler async: chama
  `decide`, parse, `sleep(rand(delay_min..=delay_max))`, `reply`. Ganchos vazios para 002 e 003.
- `auth.rs`: argon2id, sessões, rate limit (T6). `panel.rs`: rotas axum, login (T7).
- `state.rs`: `AppState` compartilhado painel↔bot (store, papéis, sessões, status, eventos, grupos,
  JIDs recentes). Só metadados.
- `wa.rs`: monta o Bot, handlers de QR/pair/connected/logged_out; grava status/eventos no
  `AppState`.

## Fluxo do evento
`on_message(ctx)`: `chat = ctx.info.source.chat.to_string()` → lookup em `ChatRoles` (leitura
curta, sem await com lock). Sem papel → **Drop** (return, sem log de conteúdo, sem tocar em
`ctx.message`). `Monitorado` → gancho 003 (nunca responde). `Alerta` → Drop (só destino).
`Comando` → se for resposta do próprio bot, Drop; senão parse do texto (`/`); sem comando,
Drop. `admin = from_me || sender ∈ {owner_jid, sender_alt}`. Atraso aleatório, resposta.
Evento vai ao ring buffer como (horário, tipo, JID), sem texto (P4).

## Esquema SQLite (`<data_dir>/silicon.db`, 0600)
```sql
CREATE TABLE roles(jid TEXT PRIMARY KEY,
  role TEXT NOT NULL CHECK(role IN('comando','monitorado','alerta')),
  name TEXT, updated_at INTEGER NOT NULL);
CREATE UNIQUE INDEX one_alert ON roles(role) WHERE role='alerta';
CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- keys: password_hash, owner_jid, delay_min_ms(1000), delay_max_ms(2000), bind(127.0.0.1:8080)
```
Eventos recentes só em memória (`VecDeque` de 200). Sem papéis = bot mudo (RF11).

## Painel (axum, HTML puro com `format!`, sem JS)
- Rotas: `GET/POST /login`, `POST /logout`; `GET /` (status, número via `client.pn()`,
  eventos); `GET /chats` (grupos + manuais + recentes, com papel atual); `POST /chats`
  (jid, role|none); `POST /settings` (atrasos, owner_jid).
- Auth: argon2id; sessão = token aleatório de 32 bytes em `HashMap<token, expiry>` (12 h),
  cookie `HttpOnly; SameSite=Strict; Path=/` (+`Secure` sob HTTPS); middleware → `/login`.
- Rate limit: `HashMap<IpAddr,(falhas, bloqueado_até)>`; 5 falhas → IP bloqueado 5 min.
- Sem `password_hash` → força `127.0.0.1` e avisa no log (P5). POSTs conferem `Origin`/`Host` (opcional: env `SILICON_PROXY_HOST` = host público de um proxy que troque o `Host`).
- Nunca mostra conteúdo de mensagem nem dados da sessão.
- T9: servir com `into_make_service_with_connect_info::<SocketAddr>()` (o login usa o IP).
- RF13: papéis só por aqui. `owner_jid` vazio = sem dono.

## Sessão, logout, systemd
`LoggedOut` → loga e sai com **código 2** (precisa QR novo). Unit `silicon.service`:
`User=silicon`, `Environment=SILICON_DATA_DIR=/var/lib/silicon`, `StateDirectory=silicon`,
`StateDirectoryMode=0700`, `Restart=on-failure`, `RestartSec=30`,
`RestartPreventExitStatus=2`, `StartLimitIntervalSec=600`, `StartLimitBurst=5`,
`NoNewPrivileges=yes`, `ProtectSystem=strict`. Sem loop de reinício (P8). O primeiro
pareamento é feito no terminal, fora do systemd.

## Testes
`cargo test`, sem rede:
- `decide()`: sem papel → Drop; monitorado nunca responde; admin só com `from_me`/dono;
  `comando` não dá admin.
- Store em `:memory:`: CRUD de papéis, unicidade do alerta, defaults.
- argon2 hash/verify; rate limiter (5 falhas bloqueiam; libera após o prazo).
- Lock: segundo `try_lock` no mesmo arquivo falha.
- Painel via `tower::ServiceExt::oneshot`: sem cookie → 303 `/login`; login certo → cookie.

Só com WhatsApp real: QR/pair code, reinício sem QR, `list_participating`, atraso visível,
reconexão após queda de rede, logout pelo celular → exit 2.

## Riscos
1. Ban do número principal (aceito). Mitigação: P1/P2/P8 e whitelist vazia por padrão.
2. **Loop de auto-resposta:** dono comanda do próprio número (`is_from_me`) e a resposta do
   bot também é `is_from_me`. Mitigação: set curto em memória dos IDs enviados; a resposta
   nunca começa com `/`.
3. **LID vs PN:** `chat`/`sender` podem vir em LID e o papel salvo com PN não casar.
   Mitigação: casar também com `sender_alt`; o painel salva o JID como a lib entrega. Validar
   com WhatsApp real.
4. API pré-1.0 muda: rev fixado, só atualiza com spec/PR.
5. `.db` da sessão vazar por backup/permissão: `0600`, `StateDirectoryMode=0700`, `.gitignore`.
6. Certificado autoassinado gera aviso no navegador; dono aceita uma vez (fingerprint no
   terminal).

## Fragmentação
| # | Passo | Arquivos | Agente | Depende |
|---|---|---|---|---|
| 1 | Cargo.toml (deps, rev completo), esqueleto `main.rs` | Cargo.toml, src/main.rs | módulo | — |
| 2 | Conferir licenças (argon2, qrcode, rcgen, axum-server) | — | pesquisa | — |
| 3 | `lock.rs` + teste | src/lock.rs | módulo | 1 |
| 4 | `roles.rs` + `dispatch::decide` + testes | src/roles.rs, src/dispatch.rs | módulo | 1 |
| 5 | `config.rs` (esquema, Store, 0600) + testes | src/config.rs | módulo | 1 |
| 6 | Auth: hash, sessões, rate limit + testes | src/auth.rs | módulo | 1 |
| 7 | Rotas/HTML do painel + testes oneshot | src/panel.rs | módulo | 5, 6 |
| 8 | `wa.rs`: builder, QR/pair, handler com `decide`, atraso, set anti-loop | src/wa.rs, src/dispatch.rs | integração | 4, 5 |
| 9 | `main.rs` final + `set-password` + TLS opcional | src/main.rs | módulo | 3, 7, 8 |
| 10 | Unit systemd + seção no README | deploy/silicon.service | módulo | — |

Paralelo (sem build): 2 e 10 a qualquer momento; 3, 4, 5 e 6 juntos após o 1 (arquivos
diferentes); só a integração roda cargo. Serial (um por vez, `cargo -j1`): a integração roda
`build`/`clippy -D warnings`/`test` após cada onda (1; 3–6; 7–8; 9). Bug pesado de
tipos/lifetimes da lib → revisão mais cuidadosa.

## Não verificado
`PairCodeOptions` (`whatsapp_rust::pair_code`) compila; `SendResult.message_id` existe, mas
ainda não se sabe se casa com o `info.id` do eco (T11). Estabilidade de `Jid::to_string()` para LID.
