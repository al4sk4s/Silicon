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
- RF3 Estática: imagem → WebP 512x512 (estica para o quadrado, sem cortar nem barras; proporção distorcida),
  ≤ 100 KB (usar 100.000 bytes como teto seguro), enviada como `stickerMessage`
  (`mimetype image/webp`; preencher `width`/`height`).
- RF4 Animada: vídeo/GIF → WebP animado, 512x512, **≤ 500 KB** (teto seguro 500.000 B),
  **≤ 10 s**, **frame ≥ 8 ms**, o **primeiro frame deve mostrar a figurinha completa**;
  `isAnimated=true`, loop infinito. Comando-base (derivado, validar no app):
  `ffmpeg -i in -vf "fps=15,scale=512:512,setsar=1,pad=512:512:-1:-1:color=0x00000000,format=rgba" -an -t 10 -c:v libwebp -lossless 0 -q:v 60 -compression_level 6 -loop 0 out.webp`.
  Se passar de 500 KB, reencoda na escada da spec 004 (**duração 10 s e 512x512 nunca mudam**; só baixa `-q:v` 60→25 dentro de cada fps, e depois o fps 24→20→15→12); se nem o piso couber, responde "mídia pesada demais". [Atualizado 23/11/2026: a escada antiga cortava duração e encolhia a figurinha.]
  **Medido (ffmpeg 9.0.2, vídeo de teste 8 s, pior caso):** q60 → 733 KB, q40 → 611 KB,
  q25 muito lento (> 1 min com `-compression_level 6`). Conclusões: só baixar `-q:v` não
  basta; a escada precisa mexer também em fps/duração/escala; usar `-compression_level`
  menor (≈ 3) e **timeout por tentativa** para não travar o daemon; encode animado leva
  segundos, então roda em `spawn_blocking`/processo filho, nunca no loop de eventos.
- RF5 **[DEPRECADO 19/11/2026 — não suportado]** Visualização única (a citação chega sem `media_key`; ver achado do T10). O bot responde "Visualização única não é suportada" e não baixa. Texto original: desembrulhar o wrapper (`get_base_message`), baixar e
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
- RF9 **[REMOVIDO 19/11/2026]** (opção e painel retirados com o RF5). Texto original: opção **`viewonce_enabled`**, **desligada por padrão**, ligada no painel. Ao ligar, o
  painel mostra aviso para avisar o grupo antes (P6).

## Não funcionais
- Conversão estática < 1 s; animada < 10 s no servidor.
- Temporários só em `tmpfs`/memória quando possível; se usar disco para o ffmpeg, arquivo
  apagado no fim, mesmo em erro, e nunca para viewOnce (viewOnce passa por pipe/stdin).
- Dependência de sistema: `ffmpeg` com `libwebp_anim` (instalado no notebook de dev; **servidor
  ainda não conferido**). O bot checa no boot; ausente → o comando responde "ffmpeg ausente".

## Fora do escopo
Pacotes de figurinha, edição de figurinha (texto/recorte; `/rename` só troca nome/autor, RF11), converter figurinha em imagem.

## Critérios de aceite
- Imagem e vídeo respondidos geram figurinhas que abrem e tocam no app.
- viewOnce: após o envio, nenhum arquivo da mídia existe em disco (teste: listar diretório
  de dados/temp antes e depois) e nada no log.
- Com `viewonce_enabled=false`, o comando em viewOnce responde "desativado".
- Chat sem papel `comando` é ignorado.

## Decisões (resolvidas)
1. Imagem: ffmpeg por pipe (stdin→stdout). **Todo vídeo/GIF** (viewOnce ou não): `memfd`
   (RAM, `/proc/self/fd/N`); sem `/dev/shm` nem arquivo temporário. Substitui o fallback do plano.
2. Comando na legenda da mídia vale (RF2); o `/s` também é lido da legenda no `wa.rs`.

## Perguntas em aberto
- Encoder WebP do crate `image` é lossy? Senão usar `webp`/`ffmpeg` também para estáticas.
- Limites de tamanho/duração/frame: **resolvidos** (fonte oficial `WhatsApp/stickers`, que
  descreve packs; o servidor do WhatsApp pode ser mais tolerante ou mais rígido). Ainda
  falta confirmar na prática: se o WhatsApp aceita sem EXIF, e se o limite é 1000 ou 1024
  bytes/KB (usamos o teto menor).
- Mídias de visualização única mais novas podem não baixar para dispositivo vinculado;
  validar com mensagem real.

## Achado do T10 [WA] — viewOnce NÃO baixa (19/11/2026)
Testado no pareamento real (grupo `comando`, 2 imagens de visualização única, `/s` citando): o original chega
ao aparelho vinculado como `<unavailable>` (log da lib: "unrecoverable <unavailable> child (type: ViewOnce)")
e a citação **vem sem `media_key`**. `client.download` falha com `MissingPlaintextHash` (sem chave a lib trata a
mídia como não criptografada e exige `file_sha256`). Resposta ao usuário: "Não consegui baixar"; evento
`figurinha_erro_download`. **RF5/RF9 (viewOnce) não são viáveis como especificados**; decisão do dono pendente
(ex.: remover viewOnce da spec e responder só "não suportado"). Vídeo de visualização única não foi testado
(mesmo mecanismo). Estática/animada normais: ainda por validar.

## Resultado do T11/T12 [WA] (20/11/2026)
Passou tudo no WhatsApp real (dono): nome `Silicon™`/autor `@al4sk4s` aparecem ao tocar na figurinha estática e na animada;
campos trocados no painel valem nas novas; `/rename a / b`, `/rename /`, `/rename só nome` (erro), sem citar (uso) e campo >64
(recusa) funcionaram. A figurinha citada veio com `media_key` (download ok). EXIF aceito pelo app; nada a investigar.

## Resultado do T10 [WA] (20/11/2026)
Fechado: Android e iOS animam, sem duplicata; legenda, `/s` sem citar e rajada ("Calma" uma vez) ok; tempos: imagem 3–3,5 s,
vídeo ~7 s (alvo <10 s).
Imagem não quadrada, vídeo e GIF respondidos com `/s` no grupo de teste: figurinhas **perfeitas** (dono). Mídia é esticada
para 512x512 (decisão do dono: sem corte nem barras, proporção distorcida). Sem EXIF funciona. Pendentes: Android/iOS
lado a lado, rajada/"Calma…", `/s` sem citar, tempos medidos. Probe do ffmpeg: timeout subiu para 30 s (boot falhava
com cache frio) e `/s` refaz o probe se o flag estiver falso.

## Nome e autor da figurinha + `/rename` (pedido do dono, 20/11/2026; implementado em código — falta [WA])
- RF10 Toda figurinha gerada leva EXIF com **nome do pacote** e **autor** (o que aparece ao tocar na figurinha).
  Padrões: nome **`Silicon™`**, autor **`@al4sk4s`**. Dois campos novos em **Ajustes** do painel (`sticker_pack_name`,
  `sticker_publisher`), válidos para as figurinhas novas. Substitui o RF6 "opcional" (T11).
- RF11 Comando **`/rename`**: o usuário responde (cita) uma **figurinha** e manda `/rename campo1 / campo2`. O bot
  baixa a figurinha citada, troca nome=campo1 e autor=campo2 no EXIF e responde com a figurinha nova (mesmo
  limite por chat e mesmo atraso de 1–2 s do `/s`). `/rename /` (só a barra) **limpa os dois** (nome e autor
  vazios, como as figurinhas de hoje). Só em chat com papel `comando`; nunca inicia conversa.
- RF12 Validação do `/rename`: sem citar figurinha → "Responda uma figurinha com /rename nome / autor"; campo
  acima de 64 caracteres (valor a confirmar) → recusa curta; sem a `/` → recusa com o uso. Figurinha animada e
  estática mantêm os bytes do WebP; só o chunk `EXIF` muda.
- Implementado (T11/T12, respostas conservadoras, o dono pode mudar): comando `/rename` sem espaço; `/rename nome`
  sem `/` é erro com o uso (autor não é opcional); só a 1ª `/` separa (o resto vai no autor); limite 64 caracteres
  (também no painel); `@al4sk4s` é texto configurável no painel. `/s` e `/rename` dividem o mesmo limite por chat.
  EXIF: TIFF LE, tag 0x5741, JSON com `sticker-pack-id` fixo `silicon`; VP8X criado se faltar. Não há `strip_exif`:
  `with_exif("", "")` já cobre `/rename /`. Falha no EXIF nunca
  derruba o envio de `/s`. Risco aberto: a figurinha citada pode vir sem `media_key` (download falha) e o WhatsApp
  pode ignorar esse EXIF (fonte só comunitária); ambos dependem do [WA].
- Perguntas ao dono (já respondidas pelo padrão acima): (1) o comando é `/rename` (sem espaço)? (2) `/rename nome` sem `/` vale como só nome (autor
  vazio) ou é erro? (3) o EXIF novo aparece em figurinhas já enviadas? (não: só nas geradas depois). (4) o
  `@al4sk4s` do autor é texto fixo mesmo ou o nome do dono configurado?
