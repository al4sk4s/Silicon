# Plano 004 — Comandos do dono em qualquer chat

Status: implementado (T1–T3 [x]) · Spec: `specs/004-comandos-do-dono/spec.md` · Depende de: 001, 002 ·
Constituição 1.1.0 · whatsapp-rust @ `eeb24da`.

## Decisões (já no código)
1. **`decide` ganha `Action::Owner`** (`src/dispatch.rs`). O dono é `from_me` ou `owner_jid`, comparado sem o sufixo de dispositivo (`bare`). Ordem dos casos:
   - papel `comando`: `Command{admin: is_owner}`;
   - `monitorado`: `Monitor`;
   - `alerta`: `Drop`;
   - sem papel e dono: `Owner`;
   - qualquer outro caso: `Drop`, antes de qualquer parse (P2).
2. **`on_message`** (`src/wa.rs`): `Owner` vira `(admin=true, owner_only=true)`. Com `owner_only`, só passam `/c`, `stickers::parse_cmd` e `parse_rename`; o resto é ignorado em silêncio.
3. **Parser `parse_chat_cmd`** (`src/wa.rs`): aceita `/c` e `/comandos` com `allow`, `deny` ou `status`. Argumento extra depois do primeiro é ignorado; outro argumento vira `Usage`; `/cx` não casa.
4. **`handle_chat_cmd`**: `set_role`/`clear_role` e depois `reload_roles()`.
   - `allow` em chat sem papel ou já `comando`: reage ✅ (já `comando` não grava de novo).
   - `deny` em chat `comando`: reage ✅; sem papel: só ✅, nada muda.
   - `allow`/`deny` em chat `monitorado`/`alerta`: "Este chat tem outro papel; use o painel."
   - `status`: uma linha com o papel.
   - Falha ao gravar: "Não consegui salvar." Erro de envio: evento `erro_envio`.
5. **P8**: o mesmo `pick_delay(delay_min, delay_max)` roda antes da reação/resposta. O id enviado entra em `Sent` (anti-eco).
6. **Eventos** no painel: `comando_dono_allow` e `comando_dono_deny` com o JID (RF7).

## Arquivos tocados
`src/dispatch.rs` (`Action::Owner`, `bare`, testes), `src/wa.rs` (`ChatCmd`, `parse_chat_cmd`, `handle_chat_cmd`, ramo `owner_only`, teste do parser).

## Testes
- `dispatch`: só o dono passa em chat sem papel; `monitorado` nunca vira comando; `alerta` cai em `Drop`; regras de admin; dono com sufixo de dispositivo.
- `wa`: parser de `/c`.
- [WA] validado em 20/11/2026.

## Riscos / fora
- Em chat `comando`, `/c` exige `admin` (= dono). Membro comum não altera nada (RF6).
- `/chats` só mostrava a mudança com F5; resolvido pelo recarregamento automático da 005 (RF8).
- Fora: liberar uma pessoa só dentro de um grupo, lista de donos, `/c` para `monitorado`/`alerta`.
