# Constituição do Silicon

> Versão 1.0.0 · Ratificada em 2026-11-19 · Autor: Al4sk4s
> 1.0.0: ratificada. (0.2.0: P1 ganha exceção do alerta ao dono; P2 ganha papéis; P5 libera LAN com senha.)
> Uma spec, plano ou código que contrarie a constituição está errado — quem muda é a
> spec/plano/código, não a regra. Mudar regra: discutir com o dono e subir a versão
> (MAJOR = remove/inverte · MINOR = adiciona · PATCH = só texto).

## O que é o Silicon
Bot de WhatsApp em Rust, de terminal, de uso pessoal, para um grupo pequeno de conhecidos.
Comandos de figurinha primeiro; outros depois, via spec.

## Princípios

### P1 — Só responde, nunca inicia
O bot nunca manda mensagem por conta própria nem para quem não o chamou. Só responde a um
comando vindo de chat permitido. Reduz risco de ban e de incômodo.
**Única exceção:** o **alerta ao dono** (spec 003): o bot pode enviar mensagem por conta
própria **somente** para o chat de papel `alerta`, que é do dono. Nunca para terceiros,
nunca para o grupo monitorado. Ligações automáticas estão proibidas.

### P2 — Whitelist primeiro
Todo chat tem um papel: `comando` (usa comandos e recebe resposta), `monitorado` (o bot só
**lê**, nunca responde) ou `alerta` (destino dos alertas ao dono). Mensagens de chats **sem
papel** são ignoradas **antes** de qualquer parse, download ou resposta. Padrão seguro:
nenhum papel atribuído = bot mudo.

### P3 — Só o dono administra
Comandos de admin (permitir/bloquear chat, config) só valem se vierem do dono. Estar na
whitelist dá direito de usar comandos públicos, nunca de administrar.

### P4 — Segredos e sessão fora do repositório
A sessão do WhatsApp (`.db`), chaves e tokens nunca entram no git, em log ou em painel.
A sessão é credencial: trate como senha. Backup é responsabilidade do dono.

### P5 — Painel só na rede local, com senha
O painel de configuração é acessível **só na LAN** (nunca exposto à internet, sem
port-forward). Exige **senha** (guardada só como hash), com limite de tentativas. Padrão
ao subir sem senha configurada: escuta apenas em `127.0.0.1`.

### P6 — Respeito a quem usa o grupo
Nada que contrarie expectativa dos membros sem consentimento. Em especial, ler mídia de
visualização única só existe com aviso/consentimento do grupo e atrás de opção **desligada
por padrão** (ver pergunta em aberto em `docs/contexto.md`).

### P7 — Simples e enxuto
Terminal, poucas dependências, sem abstração que ninguém pediu. Dependência nova só com
motivo. Versão de `whatsapp-rust` fixada.

### P8 — Humano, não robô
Atraso de 1–2 s antes de responder; sem rajada de mensagens; sem loop de reconexão; uma
única instância usando a sessão.

### P9 — Licenças limpas
Não copiar código GPL/AGPL/LGPL de outros projetos; reescrever a ideia. Conferir a licença
de cada dependência nova.

## Fluxo de trabalho
Ideia → spec → plano → tarefas → implementação. Sem spec aprovada, sem código de feature.
