# Spec 002 — Figurinhas (estática, animada, visualização única)

Status: **aprovada pelo dono em 2026-11-19** · Depende de: 001

## Por quê
Caso de uso principal: transformar mídia de um chat em figurinha com um comando,
respondendo à mensagem de origem.

## Histórias
- Respondo uma **imagem** com `/s`, `/fig` ou `/figurinha` → o bot responde com a figurinha.
- Respondo um **vídeo/GIF curto** com o mesmo comando → figurinha **animada**.
- Respondo uma mensagem de **visualização única** com o mesmo comando → o bot gera a
  figurinha e **não guarda nada** da mídia original.
- Mando o comando sem responder a nada → o bot orienta o uso (uma linha).

## Requisitos funcionais
- RF1 Comandos: `/s`, `/fig`, `/figurinha` (aliases). Só em chats com papel `comando`.
- RF2 Fonte da mídia: mensagem **citada** (`contextInfo`/`stanza_id`) ou a própria mensagem
  se o comando vier na legenda da mídia.
- RF3 Estática: imagem → WebP 512x512 (proporção preservada, fundo transparente),
  ≤ 100 KB (usar 100.000 bytes como teto seguro), enviada como `stickerMessage`
  (`mimetype image/webp`; preencher `width`/`height`).
- RF4 Animada: vídeo/GIF → WebP animado, 512x512, **≤ 500 KB** (teto seguro 500.000 B),
  **≤ 10 s**, **frame ≥ 8 ms**, o **primeiro frame deve mostrar a figurinha completa**;
  `isAnimated=true`, loop infinito. Comando-base (derivado, validar no app):
  `ffmpeg -i in -vf "fps=15,scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:-1:-1:color=0x00000000,format=rgba" -an -t 10 -c:v libwebp -lossless 0 -q:v 60 -compression_level 6 -loop 0 out.webp`.
  Se passar de 500 KB, reencoda reduzindo nesta ordem: `-q:v` (60→40→25), fps (15→12→10),
  duração, escala; no máximo N tentativas, depois responde "mídia pesada demais".
  **Medido (ffmpeg 9.0.2, vídeo de teste 8 s, pior caso):** q60 → 733 KB, q40 → 611 KB,
  q25 muito lento (> 1 min com `-compression_level 6`). Conclusões: só baixar `-q:v` não
  basta; a escada precisa mexer também em fps/duração/escala; usar `-compression_level`
  menor (≈ 3) e **timeout por tentativa** para não travar o daemon; encode animado leva
  segundos, então roda em `spawn_blocking`/processo filho, nunca no loop de eventos.
- RF5 **Visualização única**: desembrulhar o wrapper (`get_base_message`), baixar e
  converter **somente em memória**. Proibido gravar a mídia original em disco, cache, log
  ou painel. Descartada assim que a figurinha é enviada **ou** a operação falha.
- RF6 Pacote/autor da figurinha configurável no painel via EXIF no WebP (chaves da
  comunidade `sticker-pack-id`, `sticker-pack-name`, `sticker-pack-publisher`; TIFF
  little-endian, tag `0x5741`, JSON UTF-8). **Fonte só comunitária e não é comprovado que
  seja obrigatório** → entra como tarefa separada e opcional; a figurinha sem EXIF deve
  funcionar do mesmo jeito.
- RF7 Falhas (mídia grande, formato inválido, ffmpeg ausente) respondem com mensagem curta;
  nunca travam o daemon; nunca deixam arquivo temporário (apagado em `Drop`/sempre).
- RF8 Limite de uso por chat (ex.: N figurinhas/minuto, configurável) contra rajada (P8).
- RF9 Opção **`viewonce_enabled`**, **desligada por padrão**, ligada no painel. Ao ligar, o
  painel mostra aviso para avisar o grupo antes (P6).

## Não funcionais
- Conversão estática < 1 s; animada < 10 s no servidor.
- Temporários só em `tmpfs`/memória quando possível; se usar disco para o ffmpeg, arquivo
  apagado no fim, mesmo em erro, e nunca para viewOnce (viewOnce passa por pipe/stdin).
- Dependência de sistema: `ffmpeg` com `libwebp` (precisa checar no PC servidor; não está
  instalado no notebook de dev).

## Fora do escopo
Pacotes de figurinha, edição de figurinha (texto/recorte), converter figurinha em imagem.

## Critérios de aceite
- Imagem e vídeo respondidos geram figurinhas que abrem e tocam no app.
- viewOnce: após o envio, nenhum arquivo da mídia existe em disco (teste: listar diretório
  de dados/temp antes e depois) e nada no log.
- Com `viewonce_enabled=false`, o comando em viewOnce responde "desativado".
- Chat sem papel `comando` é ignorado.

## Decisões a confirmar
1. ffmpeg para animadas via pipe (`stdin`→`stdout`) para viewOnce nunca tocar disco.
2. Para viewOnce de **vídeo**, o mesmo vale (pipe). Se o ffmpeg exigir arquivo, a animada de
   viewOnce fica de fora do MVP.

## Perguntas em aberto
- Encoder WebP do crate `image` é lossy? Senão usar `webp`/`ffmpeg` também para estáticas.
- Limites de tamanho/duração/frame: **resolvidos** (fonte oficial `WhatsApp/stickers`, que
  descreve packs; o servidor do WhatsApp pode ser mais tolerante ou mais rígido). Ainda
  falta confirmar na prática: se o WhatsApp aceita sem EXIF, e se o limite é 1000 ou 1024
  bytes/KB (usamos o teto menor).
- Mídias de visualização única mais novas podem não baixar para dispositivo vinculado;
  validar com mensagem real.
