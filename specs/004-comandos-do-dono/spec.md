# Spec 004 — Comandos do dono em qualquer chat

Status: **decisões aprovadas pelo dono em 2026-11-20; texto da spec a revisar** (decisões 2 e 3) · Depende de: 001, 002 · Emenda: constituição 1.1.0

## Por quê
Hoje é preciso abrir o painel e colar o JID para liberar um chat. O dono quer mandar `/s` em
qualquer conversa e liberar o chat atual com um comando.

## Histórias
- Eu (dono) mando `/s` em chat **sem papel** → o bot responde a figurinha; nada é liberado.
- Mando `/c allow` no chat → ele ganha papel `comando` (grupo inteiro: todos os membros usam
  os comandos públicos) e o bot **reage com ✅** à minha mensagem.
- `/c deny` → remove o papel do chat; reação ✅.
- `/c status` → o bot responde uma linha: papel atual do chat (ou "sem papel").

## Requisitos funcionais
- RF1 Remetente = dono (P3: `owner_jid` ou `from_me`, JID sem sufixo de dispositivo). Mensagem de
  outra pessoa em chat sem papel continua ignorada antes de qualquer parse (P2).
- RF2 Em chat **sem papel**, o dono só dispara os comandos de figurinha (`/s`, `/fig`,
  `/figurinha`, `/rename`) e `/c`. Chat `monitorado`/`alerta`: nada muda (nunca responde).
- RF3 `/c` (alias `/comandos`) aceita `allow`, `deny`, `status`. Só o dono; de outra pessoa é
  ignorado em silêncio. Argumento inválido → uso em uma linha.
- RF4 `allow` em chat `monitorado`/`alerta` é recusado ("este chat tem outro papel; use o painel").
  `deny` só remove papel `comando`.
- RF5 Reação ✅ é resposta a comando do dono (não é iniciar conversa); mesmo atraso humano de P8.
- RF6 O mesmo `/c` vale em chat que já tem papel `comando` (só o dono altera; membro comum não).
- RF7 Evento no painel: `comando_dono_allow` / `_deny` com o JID.

## Fora do escopo
Liberar uma pessoa só dentro de um grupo; lista de donos; `/c` para `monitorado`/`alerta`.

## Critérios de aceite
- Teste de `decide`: dono em chat sem papel → permitido; não-dono → Drop; monitorado/alerta → como antes.
- Teste do parser de `/c`. `allow`/`deny` gravam e recarregam os papéis.
- [WA] no WhatsApp real: `/s` do dono em chat sem papel; `/c allow` → ✅ e membros usam `/s`;
  `/c deny` volta ao mudo; não-dono mandando `/c allow` não faz nada.
