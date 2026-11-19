# Spec 001 — Núcleo: sessão, whitelist, painel local

Status: **rascunho para alinhar** · Depende de: constituição v0.1.0

## Por quê
Base de tudo: conectar ao WhatsApp uma vez (QR), manter a sessão, ignorar tudo que não for
de chat permitido e dar ao dono um jeito de configurar sem editar arquivo. O daemon roda 24/7
no PC servidor; a config é feita de outro lugar (notebook) pelo painel.

## Histórias
- Como dono, rodo `silicon` no terminal, escaneio o QR **uma vez** e a sessão persiste.
- Como dono, reinicio o daemon e ele reconecta sem QR.
- Como dono, abro o painel no navegador e marco quais chats são **comando**, quais são
  **monitorados** (spec 003) e qual é o chat de **alerta**.
- Como dono, vejo no painel o estado (conectado, número, últimos eventos).

## Requisitos funcionais
- RF1 Daemon em Rust (`tokio` + `whatsapp-rust`), binário único, roda como serviço (systemd).
- RF2 Primeiro uso: imprime QR no terminal; aceita também pareamento por código de telefone.
- RF3 Sessão em SQLite fora do repositório (caminho configurável), permissão `0600`.
- RF4 Papéis por chat (JID): `comando`, `monitorado`, `alerta`. Sem papel = ignorado.
- RF5 Chat sem papel é descartado **antes** de parse, download ou resposta (P2).
- RF6 Só o **dono** (JID configurado) usa comandos de admin; estar em `comando` não dá admin (P3).
- RF7 Painel local: lista chats/grupos do WhatsApp e permite atribuir papéis; mostra status;
  mostra log recente sem conteúdo de mensagem.
- RF8 Atraso configurável (padrão 1–2 s, aleatório) antes de toda resposta (P8).
- RF9 Reconexão com backoff exponencial e teto; **sem loop apertado** (P8).
- RF10 Uma instância por sessão (lock de arquivo); segunda instância recusa subir.
- RF11 Config (papéis, atraso, caminhos) em arquivo + painel; whitelist vazia = bot mudo.
- RF12 Papéis são ligados ao **JID** do chat (nunca ao nome): renomear o grupo não quebra.
  O nome só aparece como rótulo, atualizado a partir da lib.
- RF13 Habilitar quem pode usar comandos: o dono dá o papel `comando` a um **grupo** ou a
  uma **pessoa** (chat 1:1) **somente pelo painel** (decisão do dono, 19/11/2026). Chat sem
  papel é descartado antes do parse (P2), inclusive mensagem do dono: `/permitir` e
  `/bloquear` por mensagem **não existem** nesta spec (pode virar emenda futura da P2).

## Não funcionais
- Painel acessível **só na LAN**, com **senha** (P5): bind configurável (padrão
  `127.0.0.1`; na LAN só com senha definida), login com hash `argon2`, limite de
  tentativas, sessão por cookie `HttpOnly`/`SameSite=Strict`, nunca em porta exposta à
  internet.
- Logs sem texto de mensagem nem mídia (P4).
- Versão da `whatsapp-rust` fixada por commit (README da lib recomenda Git rev).

## Fora do escopo
Múltiplas contas; multi-usuário; app mobile; painel exposto à internet.

## Critérios de aceite
- QR escaneado uma vez; reinício reconecta sem QR.
- Mensagem de chat sem papel não gera resposta nem download (teste automatizado com
  handler simulado).
- Segunda instância é recusada.
- Painel sem login redireciona para a tela de senha; 5 senhas erradas seguidas bloqueiam
  o IP por alguns minutos.

## Decisões a confirmar
1. **Painel na LAN com senha** (decidido pelo dono). Risco: HTTP puro na LAN trafega a
   senha em claro. Mitigação proposta: HTTPS com certificado autoassinado (`rustls`) — a
   confirmar no plano; senão, aceitar o risco numa LAN doméstica confiável.
2. **Número principal agora, secundário depois**: sessão trocável sem mudar código (só
   apagar o `.db` e parear de novo). O risco do ban recai sobre o número principal (aceito).
3. Painel: HTML simples servido pelo `axum` (sem framework JS) para manter enxuto (P7).

## Perguntas em aberto
- Como o painel descobre os chats (lista de grupos via lib) — validar na API.
- HTTPS autoassinado ou HTTP puro na LAN?
- Como a senha inicial é definida (comando `silicon set-password` no terminal)?
