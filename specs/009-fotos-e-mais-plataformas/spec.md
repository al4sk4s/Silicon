# Spec 009 — `/baixar` com fotos, carrosséis e mais plataformas

Status: **rascunho** (2026-11-24) · Depende de: 004-downloader, 008 (ajuda) · Constituição v1.1.0

## Por quê
O `/baixar` só pega vídeo (yt-dlp) de TikTok e Instagram. Post só de fotos, carrossel (fotos ou misto) e
vídeos do Facebook não funcionam. O dono quer que "um post com 5 fotos" chegue inteiro, de qualquer plataforma.

## Histórias
- **Membro:** mando `/baixar <link de post com 5 fotos>` e recebo as 5 fotos juntas, como álbum do WhatsApp.
- **Membro:** carrossel misto (fotos + vídeos) chega inteiro: fotos no álbum, vídeos como vídeo.
- **Membro:** `/baixar <link do Facebook>` baixa o vídeo/Reel.

## Requisitos funcionais
- **RF1 Plataformas:** TikTok, Instagram, **Facebook**. Domínio novo = entrada na lista de `validate`.
- **RF2 Ordem:** tenta `yt-dlp`; se não houver vídeo ("no video formats"), cai no `gallery-dl`.
  Carrossel misto: o que for vídeo segue o fluxo atual (H.264/AAC, ffprobe, recodificação); o que for foto
  vai para o álbum.
- **RF3 Álbum:** as fotos saem **juntas**, como álbum do WhatsApp (mensagem pai + filhos associados), uma
  mensagem do bot por item, citando o comando. Teto de **10 itens** por comando (o resto é descartado, com
  1 linha avisando). Atraso 1–2 s entre os envios (P8).
- **RF4 Mídia mista:** fotos primeiro (álbum), depois cada vídeo. Vídeo > 64 MB vai como documento (como hoje).
- **RF5 Imagens:** envia como veio (JPEG/PNG/WebP); só converte para JPEG se o formato não for aceito.
  Foto > 16 MB vai como documento. Limite de 100 MB total por comando (mesmo teto do vídeo).
- **RF6 Cookies:** `gallery-dl` usa o mesmo `data/cookies/<plat>.txt` (cópia 0600 no tempdir, apagada ao fim).
- **RF7 Falhas:** as 5 variações de resposta já existentes (link ruim, não encontrado, login necessário,
  ocupado, tempo esgotado). Sem laço, sem nova tentativa automática.
- **RF8 Limites e limpeza:** mesmo limite por minuto, semáforo e timeout do `/baixar`; tempdir apagado
  sempre; `--` antes da URL; nada de shell.
- **RF9 Debug:** `baixar_gallerydl_ok`, `baixar_erro_gallerydl`, `baixar_album` (nº de itens, bytes),
  sem URL completa de terceiros além do host.
- **RF10 Ajuda:** `/help baixar` e a bio (`NOVIDADES`) citam fotos e Facebook.

## Não funcionais
- **P1/P2/P3:** só responde a quem chamou, em chat com papel `comando`; nunca inicia.
- **P7:** sem crate novo; `gallery-dl` é binário externo, sondado no boot como o `yt-dlp` (re-sonda no comando).
- **P8:** álbum = rajada controlada: ≤ 10 itens, 1–2 s entre eles, 1 comando por vez por chat.

## Fora do escopo
Stories/Reels privados sem cookie, baixar perfil inteiro, conversão de foto em figurinha, X/Pinterest/outros
(só entram se o dono pedir depois), enviar legenda do post.

## Critérios de aceite
1. Post com 5 fotos (Instagram e TikTok foto) → álbum com as 5, num único comando.
2. Carrossel misto → fotos em álbum + vídeos reproduzíveis.
3. Vídeo do Facebook público → baixa e toca (celular e PC).
4. Link sem mídia ou privado → resposta curta (5 variações), sem rastro no disco.
5. Post com > 10 itens → manda 10 e avisa.
6. `cargo test`: parsing da saída do `gallery-dl`, teto de itens, host novo aceito, classificação de erro.
   `[WA]`: álbum aparece agrupado no celular.

## Perguntas em aberto
1. **Instalação:** `gallery-dl` não está no `pacman` do servidor (`target not found`). Opções: `pacman -Sy`
   (base desatualizada?), `pipx install gallery-dl` ou binário do GitHub em `/usr/local/bin`. Qual o dono prefere?
2. **Álbum na lib:** a lib tem `wrap_as_album_child`, mas não vi API pronta para a mensagem *pai*
   (AlbumMessage com contagem esperada). Confirmar no plano; se faltar, montar o proto à mão.
3. Fotos com legenda: mandar a legenda do post na primeira foto? (proposta: não.)
