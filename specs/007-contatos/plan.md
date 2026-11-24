# Plano 007 — Aba Contatos

Spec: `specs/007-contatos/spec.md` · whatsapp-rust @ `eeb24da`.

## Decisões
1. **Cliente acessível ao painel**: `AppState.client: Mutex<Option<Arc<Client>>>`. `wa::run` preenche após `bot.client()` e zera quando a sessão acaba (reinício/shutdown).
2. **Consulta**: `client.contacts().is_on_whatsapp(&[Jid::pn(num)])` → `IsOnWhatsAppResult{jid, lid, is_registered}`. Uma chamada por envio (RF5). Não manda mensagem.
3. **JID gravado**: `lid` (non-AD) se vier, senão `jid` (PN). O `decide` compara a chave exata; a forma real em DM é confirmada no [WA] (T4).
4. **Normalizador** `normalize_phone` em `panel.rs`: tira tudo que não é dígito; ≤11 dígitos → prefixo `55`; resultado precisa ter 10–15 dígitos.
5. **Lista**: `Store::dm_commands()` = linhas de `roles` com papel `comando` e JID terminando em `@lid` ou `@s.whatsapp.net`, com nome. Grupos não entram.
6. **Rotas** `/contacts` (GET lista + form; POST `add` / `del`), atrás do mesmo `guard`/CSRF; nome com `esc()`. Papel diferente de `comando` no JID → 409 (RF8).
7. Link "Contatos" no menu de `page()`; eventos `contato_add` / `contato_del`.

## Arquivos
`src/state.rs`, `src/wa.rs`, `src/config.rs`, `src/panel.rs`.

## Riscos
JID na forma errada → bot mudo no DM (falha segura). Consulta repetida pode parecer spam: um número por envio.
