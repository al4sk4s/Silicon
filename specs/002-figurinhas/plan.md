# Plano 002 — Figurinhas (o COMO)

Status: rascunho · Spec: `specs/002-figurinhas/spec.md` · Depende do plano 001 · Constituição
v0.2.0 · whatsapp-rust `eeb24da` · ffmpeg 9.0.2 (libwebp, libwebp_anim). Testes de pipe feitos em ambiente isolado.

Achados que mudam a spec:
- **Vídeo por stdin falha se o `moov` (índice do MP4) estiver no fim** ("partial file",
  rc=183). Com `+faststart` funciona.
- **WebP por stdout sai com o tamanho do cabeçalho RIFF zerado:** corrigir bytes 4..8 em Rust.
- **O crate `image` só gera WebP sem perda** → a estática também vai pelo ffmpeg.

## Decisões a confirmar
1. **Tudo pelo ffmpeg, sem crate de imagem.** `image` 0.25 só tem `WebPEncoder::new_lossless`
   (foto 512x512 sem perda passa de 100 KB); `webp` 0.3.1 compila libwebp em C. O ffmpeg já é
   obrigatório para animadas e converte estática em < 1 s. Custo: ffmpeg também para estáticas.
2. **Vídeo com `moov` no fim não abre por stdin.** Tentar o pipe primeiro; se falhar e a mídia
   **não** for viewOnce, gravar em `/dev/shm` (tmpfs), apagar com guarda `Drop` e repetir.
   Para viewOnce: **decidido pelo dono: (b) `memfd_create`** (RAM pura, sem caminho no disco),
   passado ao ffmpeg como `/proc/self/fd/N` (crate `libc`, Linux-only; o servidor é Linux).
   Detalhes: fd **sem** `MFD_CLOEXEC` para o filho herdar; seek para 0 após escrever; o
   `Semaphore(1)` do ffmpeg impede que outro processo herde o fd por engano; fechar o fd logo
   após o `wait`. Vale só para vídeo/GIF de viewOnce; o resto segue pelo pipe.
3. **Saída por pipe com RIFF zerado:** corrigir `out[4..8] = ((len-8) as u32).to_le_bytes()`.
   Flags VP8X ok (0x12 = animação + alfa); ffprobe leu 45 frames.
4. Padrões: `sticker_per_min = 3` por chat, `viewonce_enabled = 0`.

**Atualização (implementação):** todo vídeo/GIF (não só viewOnce) entra no ffmpeg por `memfd`
(`/proc/self/fd/N`), então `moov` no fim não importa e o fallback `/dev/shm` foi removido; só
imagem usa pipe. `MediaType`/`DownloadLimits`: `whatsapp_rust::download::{MediaType, DownloadLimits}`;
`UploadOptions`/`UploadResponse`: `whatsapp_rust::upload`.

## API confirmada da lib (eeb24da)
- **Citada:** `ctx.message.context_info()` (`wacore/src/proto_helpers.rs:259`) → `ContextInfo`
  com `stanza_id`, `participant`, `quoted_message: MessageField<wa::Message>`. O conteúdo
  citado vem **inline** (com `media_key`, `direct_path`...). `OriginalMessageResolver` só
  resolve `message_secret`: não serve. Sem citação, a fonte é o próprio `ctx.message` (comando
  na legenda, `get_caption()`).
- **ViewOnce:** `quoted.is_view_once()` detecta wrapper `view_once_message{_v2,_v2_extension}` e
  a flag inline `view_once`; `get_base_message()` desembrulha → `base.image_message` /
  `base.video_message`.
- **Download em memória:** `ctx.client.download(&msg, DownloadLimits::new(max_wire, max_plain))
  -> Result<Vec<u8>, ClientDownloadError>` (`src/download.rs:1172`). Recusar antes pelo
  `file_length`. Limites: 10 MB imagem, 32 MB vídeo.
- **Upload:** `ctx.client.upload(webp, MediaType::Sticker, UploadOptions::new()) ->
  UploadResponse { url, direct_path, media_key, file_enc_sha256, file_sha256, file_length,
  media_key_timestamp, .. }` (`src/upload.rs:441`). Conferir o re-export de `MediaType`.
- **StickerMessage** (proto linha 4786): campos acima vêm do `UploadResponse`;
  `mimetype="image/webp"`, `width=height=512`, `is_animated` (conferir com
  `wacore::webp::is_animated`), `context_info = ctx.build_quote_context()`.
- **Envio citando:** `ctx.send_message(msg)` (`src/bot.rs:178`); erros de texto por
  `ctx.reply_quoting(texto)`.

## Módulos
- `src/stickers/mod.rs`: `parse_cmd(text) -> bool` (`/s`, `/fig`, `/figurinha`, sem caixa; `/sx`
  e `/figura` não valem); `pick_source(&wa::Message) -> Result<Source, Err>` **pura**
  (`Source { media: Image|Video|Gif, view_once, msg }`); `handle(ctx, store)`: rate limit →
  flag viewOnce → download → convert → upload → envio.
- `src/stickers/convert.rs`: `to_static(bytes)` e `to_animated(bytes)` com
  `tokio::process::Command` (`kill_on_drop`), stdin escrito em task separada com stdout lido em
  paralelo; `timeout` por tentativa (5 s estática, 15 s animada); `Semaphore(1)` global.
- `src/stickers/limit.rs`: `HashMap<String, VecDeque<Instant>>`, janela 60 s, `Instant` injetado.
- `dispatch.rs`: no ramo `Command`, `if stickers::parse_cmd(..) { stickers::handle(..) }` após
  o atraso da P8.
- `config.rs`: chaves `viewonce_enabled`, `sticker_per_min`. `panel.rs`: checkbox viewOnce com
  aviso "avise o grupo antes" (P6) + campo do limite.
- `main.rs`: no boot, `ffmpeg -hide_banner -encoders` procurando `libwebp_anim`; ausente → log e
  o comando responde "ffmpeg ausente". `Cargo.toml`: feature `process` do tokio; nenhum crate novo.

## Conversão (ffmpeg, sempre `-v error -i pipe:0 ... -f webp pipe:1`)
- Filtro comum: `scale=512:512:force_original_aspect_ratio=decrease,pad=512:512:-1:-1:color=0x00000000,format=rgba`.
- **Estática:** `-frames:v 1 -c:v libwebp -lossless 0 -q:v Q`, Q 75→50→30, teto **100.000 B**, 3 tentativas.
- **Animada:** `-an -t D -vf fps=F,<filtro> -c:v libwebp -lossless 0 -compression_level 3 -q:v Q
  -loop 0`. Teto **500.000 B**, máx. **6 tentativas** (q, fps, s, escala): (60,15,10,512),
  (40,15,10,512), (40,12,8,512), (30,10,6,512), (30,10,4,512), (25,8,4,384+pad). Esgotou ou
  timeout → "mídia pesada demais".
- `pad` garante primeiro frame completo 512x512 (validar no app).
- Pós: corrigir RIFF size e conferir `RIFF....WEBP`. GIF chega como `video_message` com
  `gif_playback` → mesmo caminho da animada.

## Zero persistência do viewOnce
- Bytes só em `Vec<u8>`: download → stdin → stdout → upload → `drop`. Sem `/dev/shm` nem
  fallback para arquivo quando `view_once`.
- Log só (horário, chat JID, tipo, tamanho, resultado); nunca bytes, legenda, URL ou chaves;
  stderr do ffmpeg cortado e só em erro.
- `viewonce_enabled = 0` → "visualização única desativada" **antes** do download.

## Mensagens (curtas)
"Responda uma imagem ou vídeo com /s" · "Mídia pesada demais" · "Formato não suportado" ·
"Calma, muitas figurinhas agora" · "Não consegui baixar". Rate limit: responde uma vez e fica
mudo até a janela passar (P8).
Todas as respostas de erro/aviso do bot têm 8 variantes sorteadas por `vary()` em `reply` (pedido do dono, 20/11/2026); o texto da 1ª é o canônico dos testes.

## Testes (`cargo test`, sem rede)
- `parse_cmd`: 3 aliases, caixa, texto depois, falsos positivos.
- `pick_source` com `wa::Message` sintética: citação de imagem/vídeo/GIF; wrapper
  `view_once_message_v2`; flag inline; legenda com comando; sem mídia → erro.
- `limit`: N passa, N+1 bloqueia, libera após 60 s.
- `convert` (`#[ignore]` sem ffmpeg; entradas por lavfi, em memória): PNG/JPEG → estática
  ≤ 100.000 B, 512x512, não animada; MP4 faststart → animada ≤ 500.000 B, RIFF correto; ruído
  pesado 10 s → desce a escada e cabe ou `TooBig`; MP4 sem faststart com `view_once` → erro, sem
  arquivo.
- **Persistência:** listar recursivamente `data_dir`, `temp_dir()` e `/dev/shm` antes/depois de
  `handle_bytes(view_once=true)` e afirmar que nada mudou.
- Build serial, `cargo -j1`.

## Só com WhatsApp real
Figurinha abre e anima no Android e no iOS; aceita **sem EXIF**; a citação de viewOnce traz
`media_key` inline e o download pelo dispositivo vinculado funciona (viewOnce novo pode vir sem
chave); tempo real de upload; o GIF do app vem com faststart.

## EXIF + `/rename` (T11/T12, após T10)
- `src/stickers/exif.rs`: monta o chunk `EXIF` (TIFF LE, tag `0x5741`, JSON com `sticker-pack-name` /
  `sticker-pack-publisher`) e o injeta no RIFF/WebP (VP8X com bit EXIF, chunk antes de `ANMF`/dados conforme o
  formato). Funções puras testáveis: `with_exif(webp, name, publisher) -> Vec<u8>` e `strip_exif`.
- `stickers::handle` chama `with_exif` com os campos do painel (config: `sticker_pack_name`, `sticker_publisher`).
- `/rename`: no despacho (`wa.rs`) ao lado de `/s`; pega `quoted_message.sticker_message`, baixa (mesmo `download`
  com `DownloadLimits`), troca o EXIF, reenvia como figurinha (`is_animated` vem do WebP). Parser puro:
  `parse_rename("nome / autor") -> (String, String)`; `"/"` → vazios. Reusa limitador/atraso do `/s`.
- Riscos: a figurinha citada de outro aparelho pode vir sem `media_key` como o viewOnce (verificar no T10 do
  rename); não verificado se o WhatsApp mostra o nome/autor assim; EXIF não comprovado como obrigatório.

## Riscos
1. Citação de viewOnce pode vir sem chave para dispositivo vinculado → feature pode não
   funcionar; só o teste real responde.
2. ffmpeg travado/lento: timeout, `kill_on_drop`, semáforo de 1.
3. RAM: vídeo de 32 MB + saída, um por vez (aceitável).
4. `Vec` liberado não é zerado (sem `zeroize`; aceito).
5. Limites 1000 vs 1024 e EXIF não comprovados: usamos os tetos menores.

## Fragmentação
| # | Passo | Arquivos | Agente | Depende |
|---|---|---|---|---|
| 1 | `parse_cmd` + `pick_source` + testes | src/stickers/mod.rs | módulo | 001 pronto |
| 2 | `limit.rs` + testes | src/stickers/limit.rs | módulo | — |
| 3 | `convert.rs` (pipe, timeout, escada, RIFF fix) + testes lavfi | src/stickers/convert.rs | módulo | — |
| 4 | Settings no Store + toggle/aviso no painel | src/config.rs, src/panel.rs | módulo | — |
| 5 | Checar ffmpeg no boot; tokio `process` | src/main.rs, Cargo.toml | módulo | — |
| 6 | `handle`: download/upload/StickerMessage/quote + gancho no dispatch | src/stickers/mod.rs, src/dispatch.rs | integração | 1–5 |
| 7 | Teste de zero persistência | tests/viewonce_nopersist.rs | módulo | 3, 6 |
| 8 | Conferir na lib o caminho de `MediaType`/`DownloadLimits` | — | pesquisa | — |
| 9 | EXIF (opcional) | src/stickers/exif.rs | módulo | 6 + teste real |
| 10 | `memfd` para vídeo viewOnce (crate `libc`) + teste: fd some após uso, nada em disco | src/stickers/memfd.rs | módulo | 3 |

Paralelo (sem build): 1, 2, 3, 4, 5 e 8 juntos (arquivos diferentes); só a integração roda cargo.
Serial (`cargo fmt/clippy -D warnings/test -j1`, pela integração): após a onda 1–5, após o 6,
após o 7. Erro pesado de tipos `buffa`/`MessageField` → revisão mais cuidadosa.

## Não verificado
Re-export de `MediaType`; tempo das conversões no servidor; GIF real do WhatsApp; comportamento
no app.

## Achado do T10 [WA] (19/11/2026)
Risco do viewOnce confirmado: citação sem `media_key` → download impossível no dispositivo vinculado
(`MissingPlaintextHash`). O plano de memfd/pipe só para viewOnce perde o motivo; memfd segue para todo vídeo.
Ver spec.md. `stickers::handle` agora loga o erro da lib (`download falhou: …`, sem mídia nem JID).
