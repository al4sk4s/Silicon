# Tarefas 001 — Núcleo

Legenda: **[WA]** só se valida com WhatsApp real · **módulo** = tarefa isolada em arquivo próprio
· **integração** = junta as peças. Build/test/clippy: um por vez, `-j1`.

## Onda A — base (serial)
**T1. [x] Esqueleto do crate** · módulo
- Entrega: `Cargo.toml` (crate único; `whatsapp-rust` por git `rev` completo `eeb24da…`; tokio,
  axum 0.8.9, rusqlite 0.43 sem `bundled`, argon2, rand, tracing, tracing-subscriber, serde,
  qrcode, `tower` como dev-dep), `src/main.rs` mínimo, `rust-toolchain`/MSRV 1.95.
- Pronto quando: `cargo check -j1` passa (rodado pela integração). Sem feature `voip*`.
- Depende de: —

**T2. [x] Conferir licenças** · pesquisa (paralelo com tudo)
- Entrega: lista licença por dependência (argon2, qrcode, rcgen, axum-server, tower, rand...).
- Pronto: nenhuma GPL/AGPL/LGPL (P9); `cargo deny`/`cargo tree` ok se disponível.
- Depende de: —

## Onda B — módulos puros (paralelo, arquivos diferentes; sem build entre eles)
**T3. [x] `lock.rs`** · módulo — lock de instância por `File::try_lock`; segundo lock falha
(teste). Depende: T1.

**T4. [x] `roles.rs` + `dispatch::decide`** · módulo — `Role`, `ChatRoles`, `decide()` pura:
sem papel→Drop, monitorado nunca responde, alerta→Drop, `comando` não dá admin, admin só
`from_me`/dono/`sender_alt`. Testes. Depende: T1.

**T5. [x] `config.rs`** · módulo — `Store` SQLite (`silicon.db`, `0600`), migração `user_version`,
tabelas `roles`/`settings`, índice único do `alerta`, defaults (atraso 1–2 s, bind
`127.0.0.1:8080`). Testes em `:memory:`. Depende: T1.

**T6. [x] Auth do painel** · módulo — argon2id hash/verify, sessões (token 32 B, 12 h), rate limit
(5 falhas → 5 min por IP). Testes. Depende: T1.

**Build 1** (integração): fmt + clippy + test `-j1` após T3–T6.

## Onda C — painel e WhatsApp
**T7. [x] Rotas/HTML do painel** · módulo — login/logout, status, `/chats` (grupos + JID manual +
recentes), papéis por **JID** com nome só de rótulo (RF12), `/settings`, checagem de
`Origin`/`Host`, bind forçado a loopback sem senha, cookie `HttpOnly; SameSite=Strict`.
Testes `oneshot` (sem cookie → 303; login ok → cookie). Depende: T5, T6.

**T8. [x] código (falta [WA] no T11) `wa.rs` + handler** · integração — Bot builder, QR no terminal, pair code, `on_message`
ligado a `decide`, atraso aleatório (P8), **set anti-loop** de IDs enviados, `on_logged_out` →
exit 2, grava eventos sem conteúdo.
**[WA]** parear, reconectar sem QR, `list_participating`. Depende: T4, T5.

**Build 2** (integração) após T7–T8.

## Onda D — fechamento
**T9. [x] `main.rs` final** · módulo — `silicon` e `silicon set-password` (stdin sem eco), HTTPS
autoassinado (`rcgen` + `axum-server`, fingerprint no terminal) quando bind ≠ loopback, `0600`
no `.db` da sessão. Depende: T3, T7, T8.

**T10. [x] Deploy** · módulo (paralelo, sem build) — `deploy/silicon.service` (restart limitado,
`RestartPreventExitStatus=2`, `StateDirectoryMode=0700`) + seção no README. Depende: —

**Build 3** + **T11 [WA]** validação real (integração): QR, reinício sem QR,
loop de auto-resposta ausente, LID vs PN, logout pelo celular → exit 2, painel na LAN com
HTTPS e bloqueio por senha.

## Paralelismo
Livres (sem build): T2, T10; T3–T6 juntos. Serial (integração, `-j1`): Builds 1–3.

## Notas da Onda C (19/11/2026)
- T8 sem comandos: `wa::command_reply` é o gancho vazio (002 pluga ali). T9 deve servir o painel
  com `into_make_service_with_connect_info::<SocketAddr>()` (o login usa o IP) e chamar
  `wa::run(state, session_db, pair_phone)`; `owner_jid` vazio = sem dono.
- Não verificado (T11 [WA]): `ctx.reply` → `SendResult.message_id` casa com `info.id` do eco;
  `Jid::to_string()` estável para LID; `list_participating`; ordem pair code vs Connected.

## Notas da Onda D (19/11/2026)
- T9: HTTPS via `rcgen` (ring) + `axum-server` (`tls-rustls-no-provider`) + `rustls` ring; sem `aws-lc`.
  Licenças conferidas (301 crates no `cargo metadata`; só `r-efi` tem LGPL, mas em `OR` com MIT/Apache).
  Cert/chave em `<data_dir>/panel-{cert,key}.pem` (chave 0600); fingerprint SHA-256 no log a cada partida.
  `set-password` usa `stty -echo` (shortcut, só Unix). Argumento inválido sai com 64; erro com 1; 2 só logout.
- Build 3: fmt, clippy `-D warnings` e 22 testes ok. Falta só T11 [WA].

## T11 [WA] — resultados parciais (19/11/2026, número principal, `SILICON_DATA_DIR=/tmp/silicon-t11`)
**T11 continua `[ ]`** — itens 4 (salvar papel/LID vs PN), 5 (anti-loop) e 6 (atraso, queda, logout→2) pendentes.
- [x] 1a Pareamento por QR ok; pasta `0700`, `session.db`/`silicon.db`/`silicon.lock`/`panel-key.pem` `0600`.
  Obs.: `session.db-wal`/`-shm` nascem `0644` (umask do SQLite); a pasta `0700` protege.
- [ ] 1b Pair code (`SILICON_PAIR_PHONE`) e ordem pair code vs Connected: não testado.
- [x] 2 Reinício reconecta sem QR; segunda instância → "outra instância já está rodando" (exit 1).
- [x] 3 Painel loopback: `/chats` sem cookie → 303; login ok; `Origin` errado → 403; 5 falhas → 429 (a 6ª, mesmo com senha certa).
- [x] 3 HTTPS (bind `0.0.0.0:8080`): fingerprint impresso = o servido (`openssl s_client`); cookie `Secure`.
  **Bug achado e corrigido (`67dc085`):** HTTP/2 não manda `Host`, então todo POST sob HTTPS dava 403;
  `same_origin` agora usa `uri.authority()` como fallback (teste `h2_login_uses_authority_not_host`).
  Conferido com curl; **não** conferido num navegador de verdade.
- [x] 3 `list_participating` popula `/chats` (grupos `@g.us` aparecem).
- [ ] 3 "sem senha força loopback": não testado.
- Artefato de teste: processo em background ignora SIGINT; usar SIGTERM. Ctrl+C real não testado.
