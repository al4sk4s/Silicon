# Tarefas 008 — Ajuda e bio do grupo

Spec: `spec.md` · Plano: `plan.md`. Quem: `worker` (sonnet, edita, nunca commita), `eu` (principal). `[WA]` = só valida com WhatsApp real.
**Build serializado:** `cargo fmt`, `clippy -- -D warnings`, `test`, sempre `-j1`, um por vez.

Antes de começar: confirmar as decisões 1–3 do topo do `plan.md` (`/comandos`, papel do `/updatedc`, aviso do WhatsApp).

## Onda 1 — sem build
- [ ] **T1 [worker] `src/help.rs`:** `Cmd`, `CMDS` (`/s`, `/rename`, `/baixar`, `/help`, `/c`, `/updatedc`), `NOVIDADES`, `parse_cmd`, `parse_updatedc`, `help_text(arg, is_owner)`, `bio_text()`, 5 variações do cabeçalho. Pronto: arquivo + testes escritos (sem rodar cargo). Dep.: nenhuma.

## Onda 2 — build serializado
- [ ] **T2 [eu] Build do T1** + `mod help;` em `main.rs`; corrigir. Pronto: `fmt`, `clippy -j1`, `test -j1` verdes. Dep.: T1.
- [ ] **T3 [eu] `wa.rs`: ramos `/help` e `/updatedc`,** colisão `/comandos` (sem args → ajuda; `allow|deny|status` → `/c`), atraso P8, Debug (`ajuda_enviada`). Teste: `/comandos allow` segue `Allow`. Dep.: T2.
- [ ] **T4 [eu] `handle_updatedc`:** `fetch_metadata`, comparar, `set_description` com `PreviousDescription::from(id)`, 1 retry no conflito, erros com 5 variações cada, Debug (`bio_*`). Função pura de classificação de erro com teste. Dep.: T2.
- [ ] **T5 [eu] Build e revisão final** (`-j1`): P1 (só responde ao dono), P3, P8 (1 mensagem, sem laço), sem JID/texto no log. Dep.: T3, T4.

## Validação real
- **T6 [WA] Ajuda:** `/help`, `/comandos`, `/help baixar`, `/help dl`, `/help xyz`; membro comum não vê `/c` nem `/updatedc`. Dep.: T5.
- **T7 [WA] Bio:** `/updatedc` do dono troca a descrição; repetir diz "já atualizada"; membro comum sem efeito; grupo restrito a admins com o bot sem poder → 1 linha de erro. Dono aprova o texto das novidades. Dep.: T6.
- **T8 [eu] Fechar:** marcar tarefas, atualizar spec/plano com o medido, `/gerar-prompt`. Dep.: T7.

## Resumo de paralelismo
T1 sozinho (sem build) · depois T2 · T3 e T4 podem ser escritas juntas mas só uma compila por vez · T5 serializado.
