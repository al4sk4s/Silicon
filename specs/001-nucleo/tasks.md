# Tarefas 001 — Núcleo

Legenda: **[WA]** só se valida com WhatsApp real · **módulo** = tarefa isolada em arquivo próprio
· **integração** = junta as peças. Build/test/clippy: um por vez, `-j1`.

## Onda A — base (serial)
**T1. Esqueleto do crate** · módulo
- Entrega: `Cargo.toml` (crate único; `whatsapp-rust` por git `rev` completo `eeb24da…`; tokio,
  axum 0.8.9, rusqlite 0.43 sem `bundled`, argon2, rand, tracing, tracing-subscriber, serde,
  qrcode, `tower` como dev-dep), `src/main.rs` mínimo, `rust-toolchain`/MSRV 1.95.
- Pronto quando: `cargo check -j1` passa (rodado pela integração). Sem feature `voip*`.
- Depende de: —

**T2. Conferir licenças** · pesquisa (paralelo com tudo)
- Entrega: lista licença por dependência (argon2, qrcode, rcgen, axum-server, tower, rand...).
- Pronto: nenhuma GPL/AGPL/LGPL (P9); `cargo deny`/`cargo tree` ok se disponível.
- Depende de: —

## Onda B — módulos puros (paralelo, arquivos diferentes; sem build entre eles)
**T3. `lock.rs`** · módulo — lock de instância por `File::try_lock`; segundo lock falha
(teste). Depende: T1.

**T4. `roles.rs` + `dispatch::decide`** · módulo — `Role`, `ChatRoles`, `decide()` pura:
sem papel→Drop, monitorado nunca responde, alerta→Drop, `comando` não dá admin, admin só
`from_me`/dono/`sender_alt`. Testes. Depende: T1.

**T5. `config.rs`** · módulo — `Store` SQLite (`silicon.db`, `0600`), migração `user_version`,
tabelas `roles`/`settings`, índice único do `alerta`, defaults (atraso 1–2 s, bind
`127.0.0.1:8080`). Testes em `:memory:`. Depende: T1.

**T6. Auth do painel** · módulo — argon2id hash/verify, sessões (token 32 B, 12 h), rate limit
(5 falhas → 5 min por IP). Testes. Depende: T1.

**Build 1** (integração): fmt + clippy + test `-j1` após T3–T6.

## Onda C — painel e WhatsApp
**T7. Rotas/HTML do painel** · módulo — login/logout, status, `/chats` (grupos + JID manual +
recentes), papéis por **JID** com nome só de rótulo (RF12), `/settings`, checagem de
`Origin`/`Host`, bind forçado a loopback sem senha, cookie `HttpOnly; SameSite=Strict`.
Testes `oneshot` (sem cookie → 303; login ok → cookie). Depende: T5, T6.

**T8. `wa.rs` + handler** · integração — Bot builder, QR no terminal, pair code, `on_message`
ligado a `decide`, atraso aleatório (P8), **set anti-loop** de IDs enviados, `on_logged_out` →
exit 2, comandos `/permitir` e `/bloquear` do dono (RF13), grava eventos sem conteúdo.
**[WA]** parear, reconectar sem QR, `list_participating`. Depende: T4, T5.

**Build 2** (integração) após T7–T8.

## Onda D — fechamento
**T9. `main.rs` final** · módulo — `silicon` e `silicon set-password` (stdin sem eco), HTTPS
autoassinado (`rcgen` + `axum-server`, fingerprint no terminal) quando bind ≠ loopback, `0600`
no `.db` da sessão. Depende: T3, T7, T8.

**T10. Deploy** · módulo (paralelo, sem build) — `deploy/silicon.service` (restart limitado,
`RestartPreventExitStatus=2`, `StateDirectoryMode=0700`) + seção no README. Depende: —

**Build 3** + **T11 [WA]** validação real (integração): QR, reinício sem QR, `/permitir`,
loop de auto-resposta ausente, LID vs PN, logout pelo celular → exit 2, painel na LAN com
HTTPS e bloqueio por senha.

## Paralelismo
Livres (sem build): T2, T10; T3–T6 juntos. Serial (integração, `-j1`): Builds 1–3.
