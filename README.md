<div align="center">

<img src="assets/silicon.svg" alt="Ícone do Silicon" width="128">

# Silicon

**Um bot de WhatsApp em Rust, de terminal, feito para morar no seu servidor.**
Figurinhas na hora, alerta de promoção "bugada" e um painel simples para mandar em tudo.

![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange?logo=rust&logoColor=white)
![Versão](https://img.shields.io/badge/vers%C3%A3o-1.0.0-blue)
![Uso](https://img.shields.io/badge/uso-pessoal-lightgrey)

</div>

---

## ✨ O que o Silicon faz

| | |
|---|---|
| 🖼️ **Figurinhas** | Responda uma imagem, vídeo, GIF ou figurinha com `/s` (`/fig`, `/figurinha`): estática ou animada (até 10 s, com som quando tem). `/rename nome / autor` troca os metadados. Mídia de visualização única é processada **só em memória** (função desligada por padrão). |
| 📥 **Downloader** | `/baixar <link>` (`/dl`) baixa vídeos e fotos/carrosséis de TikTok, Instagram e Facebook via `yt-dlp`; acima de 64 MB vai como arquivo. |
| 🔔 **Alerta de bug** | Observa o grupo de promoções e avisa o dono, num grupo privado, quando alguém manda um "bug" (preço errado, promoção que vai sumir). |
| 🙋 **Ajuda** | `/help` e `/help <comando>`; `/updatedc` (dono) atualiza a descrição do grupo com a lista de comandos. |
| 🎛️ **Painel local** | Página com senha, só na sua rede: dashboard, papéis dos chats, regras de detecção, contatos, pareamento por QR/código, ajustes por categoria e aparência (tema, cor, densidade). |
| 🔒 **Sessão que fica** | O QR é lido **uma vez**; a sessão fica salva e o bot reconecta sozinho. |

## 🧭 Como ele se comporta

O bot segue regras que valem para o projeto inteiro (veja a
[constituição](.specify/memory/constitution.md)):

- **Só responde, nunca inicia.** A única exceção é o alerta que ele manda para o *seu* grupo.
- **Chat sem papel é ignorado**, antes de ler ou baixar qualquer coisa. Sem configuração, o bot fica mudo.
- **Só o dono administra** (`/c allow`, `/c deny`, `/c status` e o painel).
- **Mídia de visualização única nunca vai para o disco**: é processada em memória e descartada.
- **Parece gente, não robô:** atraso de 1 a 2 segundos, limite de uso por chat e nada de rajada.
- **Sem ligações.** Nunca.

## 🧩 Specs

| Spec | O que é |
|---|---|
| [001 Núcleo](specs/001-nucleo/spec.md) | sessão (QR), papéis por chat, painel com senha, daemon |
| [002 Figurinhas](specs/002-figurinhas/spec.md) · [004 animadas](specs/004-figurinhas-animadas/spec.md) · [005 som](specs/005-figurinhas-som/spec.md) | `/s`, `/rename` |
| [003 Monitor](specs/003-monitor-promocoes/spec.md) | alerta de "bug" |
| [004 Comandos do dono](specs/004-comandos-do-dono/spec.md) · [007 Contatos](specs/007-contatos/spec.md) | `/c allow|deny|status`, contatos |
| [004 Downloader](specs/004-downloader/spec.md) · [009 Fotos e plataformas](specs/009-fotos-e-mais-plataformas/spec.md) | `/baixar` |
| [005 Pareamento](specs/005-painel-pareamento/spec.md) · [010 Redesign](specs/010-redesign-painel/spec.md) | painel |
| [006 Release](specs/006-release/spec.md) · [008 Ajuda e bio](specs/008-ajuda-e-bio/spec.md) | binários, `/help`, `/updatedc` |

## 🛠️ Stack

- **Rust** (async com `tokio`) em cima da [`whatsapp-rust`](https://github.com/oxidezap/whatsapp-rust), fixada em um commit específico
- **SQLite** para sessão e configuração (arquivos separados)
- **axum** para o painel, com senha `argon2` e HTTPS autoassinado quando sair do loopback
- **ffmpeg** (com o encoder `libwebp_anim`; o bot checa no boot) para as figurinhas
- **yt-dlp** para o `/baixar` (opcional; o bot re-sonda a cada comando)
- **systemd** para rodar 24/7 no servidor

## 📦 Instalação

Baixe da [página de Releases](../../releases/latest) (Linux x86_64):

| Arquivo | Uso |
|---|---|
| `silicon-linux-x86_64` | binário simples, sem extensão (`chmod +x`) |
| `silicon-x86_64.AppImage` | o mesmo, empacotado (precisa de FUSE ou `--appimage-extract-and-run`) |
| `SHA256SUMS` | conferência: `sha256sum -c SHA256SUMS` |

> **Windows:** sem `.exe` por enquanto. O código usa recursos só de Linux (`memfd_create`,
> `/proc`, `localtime_r`). Para compilar: `cargo build --release` (Rust 1.95+).

## 🚀 Como usar

```bash
silicon set-password   # ou pule: o 1º acesso ao painel (loopback) pede a senha na tela
silicon                # sobe o bot; leia o QR uma única vez (ou pareie pelo painel em /pair)
# painel na sua rede: https://<ip-do-servidor>:8787
```

No WhatsApp, num chat liberado: `/s`, `/rename`, `/baixar <link>`, `/help`. Só o dono:
`/c allow`, `/c deny`, `/c status`, `/updatedc`.

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
  de `target/release/silicon` no passo 2. O `ffmpeg` continua sendo dependência do sistema.
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
