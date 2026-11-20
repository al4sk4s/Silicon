# Plano 005 — Painel novo e pareamento pelo painel

Status: implementado (T1–T3 [x], T4 aberta) · Spec: `specs/005-painel-pareamento/spec.md` · Depende de: 001 · whatsapp-rust @ `eeb24da`.

## Decisões (já no código)
1. **Visual**: CSS embutido em `page()` (`src/panel.rs`), nav Status / Chats / Pareamento / Ajustes e Sair. Sem CDN nem fonte externa; JS inline e pequeno.
2. **`/chats`**: lista num `BTreeMap` (ordem estável) com os grupos de `st.groups` (preenchidos ao conectar por `client.groups().list_participating()`), `st.recent` (`note_recent`: só JID e horário) e os JIDs que já têm papel. Cada chat tem liga/desliga `comando` e menu de papel (`set_role`/`clear_role`; `alerta` continua único). Busca em JS (`data-q`). Recarrega a cada 5 s com `location.replace` se a busca está vazia e nenhum input/select está em foco (RF8). Ao fim, "Adicionar por JID".
3. **Estado do pareamento** em `state::Pairing{qr, ttl, at, code, phone, exhausted}` atrás de `Mutex`, só em memória (P4). QR e código também aparecem no terminal (`println!`, só se stdout for terminal), nunca no `tracing`.
4. **Reinício da sessão sem sair do processo**: `wa::run` roda um loop `build` + `bot.run()` em `select!` com `st.restart.notified()` (se o restart chega primeiro, `client.shutdown()`). Se o run termina com `ShutdownRequested` ou `exhausted`, espera o botão "Tentar de novo" (corrige o daemon que saía ao esgotar o QR). Outro motivo de fim vira `Err`, para o systemd reiniciar. O número do pair code vem de `pairing.phone`, via `PairCodeOptions`.
5. **Rotas** (todas atrás de `guard`/login):
   - `/pair`: `div#s` que o JS troca a cada 2 s com `fetch('/pair/state')`; se a resposta veio por redirect (sessão expirada), recarrega a página.
   - `/pair/state`: fragmento HTML. O `guard` põe `Cache-Control: no-store` em toda resposta.
   - `POST /pair`: mesmo `same_origin`/CSRF. Já pareado: 409. `action=retry`: só reinicia. Senão `valid_phone` (8–15 dígitos, `main.rs`), 400 se inválido; grava `phone` (vazio = QR) e `notify_one`.
6. **QR em SVG**: `qr_svg` (`qrcode::render::svg`, mínimo 260 px). Fragmento: código, QR com segundos restantes, "O QR expirou", "Aguardando…" ou "Conectado como <número>".
7. **Primeiro acesso** (RF7): sem `password_hash`, `/login` mostra `setup_page` (duas caixas). `first_password` só aceita loopback (senão 403); vazia ou diferente da confirmação dá 400; grava hash argon2 e já abre sessão.

## Arquivos tocados
`src/panel.rs`, `src/state.rs` (`Pairing`, `restart`, `groups`, `recent`, `status`), `src/wa.rs` (loop do `run`, `on_qr_code`, `on_pair_code`, `PairingQrCodesExhausted`), `src/main.rs` (`valid_phone`).

## Testes
`pair_requires_login_validates_phone_and_csrf`, `pair_fragment_shows_qr_then_expired`, `first_access_sets_password`, `chats_post_csrf_and_save`. [WA] validado em 20/11/2026.

## Riscos / fora
- Polling de 2 s/5 s; sem JS só não há QR ao vivo.
- `notify_one` pode acumular e gerar um reinício a mais.
- Primeiro acesso por web não vale atrás de proxy reverso na mesma máquina (peer vira loopback): defina a senha antes de expor.
- T4 (conferir no navegador / LAN por HTTPS autoassinado) aberta.
- Fora: tema escuro, i18n, desparear pelo painel.
