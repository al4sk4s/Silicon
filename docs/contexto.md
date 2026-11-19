# Contexto de origem (conversa inicial)

Resumo das decisões e fatos da conversa que originou o projeto. É ponto de partida, não
verdade final: a fase de spec pode mudar qualquer item.

## Ideia
Bot de WhatsApp **em Rust, de terminal**, para o WhatsApp do dono. Fluxo:
terminal → QR Code (sessão salva, escaneia uma vez) → bot só aceita comandos de chats
permitidos → configuração por página local **ou** pelo próprio terminal/app.
Foco: `/figurinha` e comandos que **referenciam (respondem a) mensagens**.

## Base técnica proposta
- **`whatsapp-rust`** (oxidezap/whatsapp-rust, inspirada em whatsmeow e Baileys): lib async
  para a API do WhatsApp Web. Pareamento por QR e por código de telefone; storage por
  trait, padrão SQLite (sessão persiste no `.db`); mensagens E2E 1:1 e grupo; upload e
  download de mídia; módulo `webp`. **Pré-1.0**: fixar versão.
- Núcleo: `tokio` + `whatsapp-rust` + handler de eventos de mensagem.
- **Whitelist** de JIDs (SQLite ou TOML); handler ignora tudo fora dela.
- **Comandos**: parser de `/figurinha` etc. Mensagem citada vem em `contextInfo`; dali se
  baixa a mídia.
- **Figurinha**: imagem → WebP 512x512 → `stickerMessage`. Vídeo/GIF animado → `ffmpeg`.
- **Config**: (a) página local `axum` em `127.0.0.1`, ou (b) TUI `ratatui`; comandos admin
  também por mensagem do dono (ex.: `/permitir` dentro do chat). Sugestão inicial: página
  local (menos trabalho).

## Visualização única
Não é proteção criptográfica, só a flag `viewOnce`. A mídia chega cifrada normalmente com a
chave dentro da mensagem; o bot baixa/decifra como qualquer imagem. Basta desembrulhar o
`viewOnceMessage` antes de pegar a mídia.
**Ressalva ética**: quebra a expectativa de quem enviou. Só com consentimento do grupo, ou
deixar de fora. Avisar o pessoal antes.

## Risco de banimento (uso real: 1 grupo, 6 pessoas)
Baixo, mas não zero. Clientes não oficiais violam os termos do WhatsApp; a lib é nova e
reimplementa o protocolo. Mitigações:
- Usar **número/chip secundário** (maior proteção) em vez do número principal.
- Atraso de 1–2 s antes de responder.
- **Nunca iniciar conversa**; só responder a comandos.
- Não rodar a sessão em dois lugares; evitar loop de reconexão.
- Backup do `.db` da sessão.

## Dificuldade estimada
Média. O mais chato: conversão de figurinha animada e manutenção quando o protocolo muda.

## Perguntas em aberto (levar para a fase de spec)
- Número principal ou secundário vai ser o bot?
- Config: página local, TUI, comandos por mensagem, ou combinação?
- Figurinha animada (vídeo/GIF) entra no MVP ou fica para depois?
- `viewOnce`: incluir (com aviso ao grupo) ou deixar de fora?
- Outros comandos além de `/figurinha`?
- Onde roda (PC pessoal, servidor 24h)?
