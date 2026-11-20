# Spec 005 — Painel novo e pareamento pelo painel

Status: **decisões aprovadas pelo dono em 2026-11-20; texto da spec a revisar** (decisão 4: JS/CSS próprios permitidos) · Depende de: 001

## Por quê
O painel é cru: liberar grupos/contatos exige JID manual, e o login do WhatsApp só aparece no
terminal. Quer-se um visual limpo (tons de cinza, botões simples) e o pareamento visível.

## Histórias
- **Chats:** vejo uma lista de grupos e contatos (grupos do WhatsApp + quem falou recentemente),
  busco por nome e liberto com **um clique** (liga/desliga `comando`; menu para `monitorado`/`alerta`).
- **Pareamento (`/pair`):** enquanto não pareado, vejo o **QR em tempo real** (troca sozinho quando o
  WhatsApp emite outro) e o **código de pareamento** (quando há número configurado).
- QR esgotado/expirado → mostra "QR expirou" e o botão **Tentar de novo** (reinicia o pareamento).
- Campo **número** com exemplo `5511999998888` (DDI+DDD+número, só dígitos): ao enviar, o bot pede
  código de pareamento para esse número e o mostra na tela.
- Já pareado: `/pair` mostra "Conectado como <número>".

## Requisitos funcionais
- RF1 CSS próprio embutido (sem CDN, sem fonte externa); JS próprio pequeno, sem dependência,
  só para busca/atualização (P7). Páginas continuam funcionando sem JS, menos o QR vivo.
- RF2 QR desenhado em **SVG** pelo crate `qrcode` já usado. O estado `pairing {qr, ttl, code, fim}`
  fica só em memória; nunca em disco/log (P4: o QR/código é credencial temporária).
- RF3 `/pair/state` (fragmento HTML, não JSON) atrás de login, consultado a cada 2 s pela página. `no-store`.
- RF4 `POST /pair` {phone?} (CSRF/Origin como as demais): valida (8–15 dígitos), grava o número
  no estado, **reinicia a sessão de pareamento** do bot (sem sair do processo). Vale só se não pareado.
- RF5 Painel segue só LAN com senha (P5); nada novo escuta na rede.
- RF6 Chats: o botão "liberar" usa o mesmo `set_role`/`clear_role`; `alerta` continua único.

## Fora do escopo
Tema escuro, i18n, edição de configurações além do que já existe, desparear pelo painel.

## Critérios de aceite
- Teste: `/pair/state` exige login; `POST /pair` com número inválido → 400; QR vira SVG.
- Teste: Origin errado em `POST /pair` → recusado.
- [WA] com sessão nova: QR aparece, troca, parear funciona; "Tentar de novo" gera QR novo;
  número digitado gera código aceito pelo celular.
