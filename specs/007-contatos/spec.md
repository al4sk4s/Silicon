# Spec 007 — Aba Contatos no painel

Status: **aprovada e implementada (T1–T3); T4 [WA] pendente; decisões de produto aprovadas pelo dono em 2026-11-20, texto a revisar** · Depende de: 004, 005 · Constituição 1.1.0 (sem emenda)

## Por quê
Liberar uma pessoa hoje exige que ela mande mensagem e o dono dê `/c allow` no chat dela
(ou cole o JID em `/chats`). O dono quer só colar o **número** num painel e ter a pessoa liberada.

## Histórias
- Abro a aba **Contatos**, colo `11 99999-8888` (nome opcional) e clico Adicionar → o bot confere
  que o número existe no WhatsApp, descobre o JID e dá papel `comando` ao chat privado dele.
- A aba lista só quem **está ativo** (papel `comando` em chat privado), com nome e número.
- Removo um contato → o papel é tirado (igual `/c deny`).
- Número que não existe no WhatsApp → erro na tela; nada é gravado.

## Requisitos funcionais
- RF1 Aba `/contacts` no menu, atrás do login e do CSRF das outras rotas (P5, P6).
- RF2 Número: aceita espaços, parênteses, `-` e `+`; sem código do país assume **55**; resultado
  só dígitos, 10–15. Inválido → 400, nada gravado.
- RF3 Verifica com `contacts().is_on_whatsapp` (consulta ao servidor; **não manda mensagem** ao
  contato, então não é "iniciar conversa", P1). Não registrado → erro. Bot desconectado → erro
  "bot desconectado", nada gravado.
- RF4 Grava papel `comando` no JID do chat privado: o `lid` devolvido pela consulta, senão o JID
  `@s.whatsapp.net`. Nome opcional (escapado com `esc()`).
- RF5 Uma consulta por envio do formulário; nunca lista de números (risco de spam/ban).
- RF6 Lista = chats privados (`@lid`/`@s.whatsapp.net`) com papel `comando`. Grupos, `monitorado`
  e `alerta` não aparecem aqui. Remover = `clear_role` + recarregar papéis.
- RF7 Eventos no painel: `contato_add` / `contato_del` com o JID.
- RF8 Contato que já tem outro papel (`monitorado`/`alerta`) → recusado, como na 004 RF4.

## Fora do escopo
Importar lista/agenda; liberar a pessoa em qualquer chat (só o DM); papel diferente de `comando`;
lidar com a migração futura PN→LID (ver riscos).

## Riscos
- **Forma do JID em DM** (LID vs PN): o `decide` compara a chave exata do chat. Se gravarmos a
  forma errada, o bot fica mudo no DM (falha segura). Confirmar no [WA] qual forma chega.
- Consultas de existência fora do normal podem parecer spam: um número por vez, sem automação.

## Critérios de aceite
- Testes de painel: sem login redireciona; CSRF barra; número inválido → 400 sem gravar; bot
  desconectado não grava; GET lista só DMs com papel `comando`. Teste do normalizador de número.
- [WA] no WhatsApp real: adicionar um número pela aba; essa pessoa manda `/s` no DM e recebe a
  figurinha; remover → volta ao mudo; número inexistente → erro.
