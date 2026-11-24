<div align="center">

<img src="assets/silicon.svg" alt="Ícone do Silicon" width="128">

# Silicon

**Um bot de WhatsApp em Rust, de terminal, feito para morar no seu servidor.**
Figurinhas na hora, alerta de promoção "bugada" e um painel simples para mandar em tudo.

![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-planejamento-blue)
![Uso](https://img.shields.io/badge/uso-pessoal-lightgrey)

</div>

---

> **Em que pé estamos:** o **núcleo (spec 001)** já está implementado e validado no
> WhatsApp real (login, painel, papéis, reconexão). Os **comandos de figurinha (002)** e o
> **alerta de bug (003)** ainda são só especificação: este README descreve o que vamos
> construir, não o que já funciona.

## ✨ O que o Silicon vai fazer

| | |
|---|---|
| 🖼️ **Figurinhas** | Responda uma imagem, vídeo ou GIF com `/s`, `/fig` ou `/figurinha` e receba a figurinha (estática ou animada). Também funciona com mídia de visualização única, **sem guardar nada**. |
| 🔔 **Alerta de bug** | Fica de olho no grupo de promoções e avisa você, num grupo privado, quando alguém mandar um "bug" (preço errado, promoção que vai sumir...). |
| 🎛️ **Painel local** | Uma página protegida por senha, só na sua rede, para escolher quais chats usam o bot e editar as regras de detecção. |
| 🔒 **Sessão que fica** | Você lê o QR Code **uma vez**; a sessão fica salva e o bot reconecta sozinho. |

## 🧭 Como ele se comporta

O bot segue regras que valem para o projeto inteiro (veja a
[constituição](.specify/memory/constitution.md)):

- **Só responde, nunca inicia.** A única exceção é o alerta que ele manda para o *seu* grupo.
- **Chat sem papel é ignorado**, antes de ler ou baixar qualquer coisa. Sem configuração, o bot fica mudo.
- **Só o dono administra** (`/permitir`, `/bloquear` e o painel).
- **Mídia de visualização única nunca vai para o disco**: é processada em memória e descartada.
- **Parece gente, não robô:** atraso de 1 a 2 segundos, limite de uso por chat e nada de rajada.
- **Sem ligações.** Nunca.

## 🧩 Peças do projeto

```
Silicon
├── 001 · Núcleo ............ sessão (QR), papéis por chat, painel com senha, daemon
├── 002 · Figurinhas ........ /s, /fig, /figurinha: estática, animada e visualização única
└── 003 · Monitor ........... detecta "bug" no grupo de promoções e alerta o dono
```

| Spec | Documentos |
|---|---|
| Núcleo | [spec](specs/001-nucleo/spec.md) · [plano](specs/001-nucleo/plan.md) · [tarefas](specs/001-nucleo/tasks.md) |
| Figurinhas | [spec](specs/002-figurinhas/spec.md) · [plano](specs/002-figurinhas/plan.md) · [tarefas](specs/002-figurinhas/tasks.md) |
| Monitor de promoções | [spec](specs/003-monitor-promocoes/spec.md) · [plano](specs/003-monitor-promocoes/plan.md) · [tarefas](specs/003-monitor-promocoes/tasks.md) |

## 🛠️ Stack prevista

- **Rust** (async com `tokio`) em cima da [`whatsapp-rust`](https://github.com/oxidezap/whatsapp-rust), fixada em um commit específico
- **SQLite** para sessão e configuração (arquivos separados)
- **axum** para o painel, com senha `argon2` e HTTPS autoassinado quando sair do loopback
- **ffmpeg** (com o encoder `libwebp_anim`; o bot checa no boot) para converter as figurinhas. Mídia de visualização única (desligada por padrão; ligue no painel e avise o grupo) é convertida só em memória
- **systemd** para rodar 24/7 no servidor

## 🚀 Como vai ser usar

> Isto é o plano; ainda não dá para rodar.

```bash
# 1. defina a senha do painel
silicon set-password   # ou pule: o 1º acesso ao painel (loopback) pede a senha na tela

# 2. suba o bot e leia o QR Code uma única vez
silicon

# 3. abra o painel na sua rede e escolha os chats
#    https://<ip-do-servidor>:8787
```

No WhatsApp, no chat que você liberou:

```text
/s            (respondendo a uma imagem, vídeo ou GIF)
/figurinha    (o mesmo, com o nome por extenso)
/permitir     (só você: libera o chat atual)
/bloquear     (só você: tira a permissão)
```

## 🖥️ Deploy (systemd)

Passo a passo para rodar 24/7 no servidor (unit em [`deploy/silicon.service`](deploy/silicon.service)):

```bash
# 1. usuário e pasta de dados
sudo useradd --system --home /var/lib/silicon --shell /usr/bin/nologin silicon
sudo install -d -o silicon -g silicon -m 0700 /var/lib/silicon

# 2. binário e unit
sudo install -m 0755 target/release/silicon /usr/local/bin/silicon
sudo install -m 0644 deploy/silicon.service /etc/systemd/system/silicon.service
sudo systemctl daemon-reload

# 3. senha do painel e pareamento: no terminal, FORA do systemd
sudo -u silicon env SILICON_DATA_DIR=/var/lib/silicon silicon set-password
sudo -u silicon env SILICON_DATA_DIR=/var/lib/silicon silicon

# 4. depois de parear, Ctrl+C e ligue o serviço
sudo systemctl enable --now silicon
```

- **Binário pronto:** ao empurrar uma tag `vX.Y.Z`, o GitHub Actions publica no Release o
  `silicon-linux-x86_64`, o `silicon-x86_64.AppImage` e o `SHA256SUMS` (spec 006). Use no lugar
  de `target/release/silicon` no passo 2. Windows `.exe` não é suportado (código Unix). O `ffmpeg` continua sendo dependência do sistema.
- **Pareamento pelo painel (spec 005):** em `/pair` o QR aparece em tempo real, com o código de
  pareamento, campo de número (`5511999998888`) e "Tentar de novo". Só a LAN, com senha (P5).
- **Comandos do dono (spec 004):** `/s` funciona em qualquer chat para você; `/c allow`, `/c deny`
  e `/c status` (ou `/comandos`) liberam/removem o chat atual (o grupo todo) e o bot reage com ✅.
- **Pareie pelo painel (`/pair`) ou no terminal:** o QR/código só são impressos quando o stdout é um
  terminal (nunca no journald); pelo painel funcionam também sob o systemd. Rode sempre como usuário `silicon` e com o **mesmo** `SILICON_DATA_DIR`; como outro
  usuário, os arquivos criados não seriam abertos pelo daemon. Para usar pair code em vez do QR,
  defina `SILICON_PAIR_PHONE=<só dígitos, com DDI>` no mesmo comando.
- **Atrás de proxy reverso?** Se o proxy mantém o `Host` original, nada a fazer. Se ele troca o `Host` pelo do upstream, o login dá 403 (checagem de `Origin`): defina `SILICON_PROXY_HOST=<host público>` no serviço. Proxy só na LAN, nunca exposto à internet (P5).
- **Reinicie depois de `set-password`** (`sudo systemctl restart silicon`): o bind é decidido na
  partida. Sem senha, o painel fica só em `127.0.0.1`.
- **Papel de DM por "JID manual" no painel usa a forma `…@lid`** (ex.: `123456789@lid`), não o número.
- **Painel na LAN usa HTTPS autoassinado:** o navegador vai avisar. Confira o fingerprint
  SHA-256 impresso no terminal/journal antes de aceitar.
- **Saiu com código 2?** O celular encerrou a sessão. O systemd não reinicia nesse caso: refaça
  o passo 3 e depois `sudo systemctl start silicon`.

## ⚠️ Avisos importantes

- **Não é um produto oficial.** O Silicon usa um cliente não oficial do WhatsApp. Isso pode
  violar os termos de uso da Meta, e **o número usado pode ser suspenso**. O projeto mitiga o
  risco (só responde, atraso, limites), mas não o elimina. Para uso sério, prefira um **número
  secundário**.
- **A sessão é uma credencial.** O arquivo de sessão dá acesso à conta. Ele **nunca** deve ir
  para o git, para logs ou para backups públicos. O `.gitignore` já bloqueia os formatos comuns.
- **Respeite quem está no grupo.** Ler mídia de visualização única quebra a expectativa de
  quem enviou. Essa função vem **desligada** e só deve ser usada com o conhecimento do grupo.
- Projeto **pessoal**, sem garantia de suporte.

## 🤝 Como o projeto é tocado

O trabalho segue o fluxo **spec → plano → tarefas → código**: primeiro se alinha a ideia em
[`specs/`](specs), depois se escreve o código, sempre respeitando a
[constituição](.specify/memory/constitution.md). O contexto de como o projeto nasceu está em
[`docs/contexto.md`](docs/contexto.md).

## 📄 Licença

Ainda não definida. Enquanto isso, todos os direitos reservados ao autor.
