# Tarefas 002 — Figurinhas

Legenda: **[WA]** só com WhatsApp real · **módulo** = tarefa isolada em arquivo próprio ·
**integração** = junta as peças. Build `-j1`. Pré-requisito: spec 001 pronta (T8 e T4 do núcleo).

## Onda A — pura e paralela (arquivos diferentes, sem build entre si)
**T1. [x] `parse_cmd` + `pick_source`** · módulo — `src/stickers/mod.rs`. Aliases `/s` `/fig`
`/figurinha` (`/sx`, `/figura` não valem); `pick_source` pura sobre `wa::Message` (citação de
imagem/vídeo/GIF, wrapper `view_once_message_v2`, flag inline, legenda, sem mídia → erro).
Testes sintéticos. Depende: 001/T4.

**T2. [x] Rate limit** · módulo — `src/stickers/limit.rs`: janela 60 s por chat, `Instant`
injetado, responde uma vez e silencia (P8). Testes. Depende: —

**T3. [x] Conversão** · módulo — `src/stickers/convert.rs`: `to_static` (q75→50→30, ≤ 100.000 B) e
`to_animated` (escada de 6 tentativas, ≤ 500.000 B); imagem por pipe, vídeo por memfd, `kill_on_drop`, timeout
(5/15 s), `Semaphore(1)`, correção do RIFF size. Testes lavfi
(`#[ignore]` sem ffmpeg). Depende: —

**T4. [x] Settings + painel** · módulo — chaves `viewonce_enabled` (0) e `sticker_per_min` (3);
toggle com aviso "avise o grupo antes" (P6). Depende: 001/T5, 001/T7.

**T5. [x] Checagem do ffmpeg no boot** · módulo — procura `libwebp_anim`; ausente → log + resposta
"ffmpeg ausente"; feature `process` do tokio. Depende: —

**T6. [x] Re-export de `MediaType`/`DownloadLimits`** · pesquisa — confirmar caminhos no commit
`eeb24da`. Depende: —

**T7. [x] `memfd` para todo vídeo/GIF** · módulo — `src/stickers/memfd.rs` + crate `libc`
(Linux-only): `memfd_create` sem CLOEXEC, escreve, seek 0, passa `/proc/self/fd/N` ao ffmpeg,
fecha após `wait`. Teste: o fd some após o uso; nada novo no disco. Depende: T3.

**Build 1** (integração): fmt + clippy + test `-j1` após T1–T5, T7.

## Onda B — integração
**T8. [x] `handle`** · integração — rate limit → checagem `viewonce_enabled` (antes do download) →
`client.download` em memória (10 MB img / 32 MB vídeo) → convert (vídeo viewOnce via memfd) →
`client.upload(MediaType::Sticker)` → `StickerMessage` (mimetype, 512x512, `is_animated`,
quote) → `ctx.send_message`. Gancho no ramo `Command` do dispatch. Mensagens curtas de erro.
Depende: T1–T6, T7.

**T9. [x] Teste de zero persistência** · módulo — (feito em `src/stickers/mod.rs`: cobre a conversão; o download foi conferido lendo a lib em `eeb24da`, só usa disco em testes; log conferido por inspeção: `push_event` recebe só tipo e JID) `tests/viewonce_nopersist.rs`: lista recursiva
de `data_dir`, `temp_dir()` e `/dev/shm` antes/depois de `handle_bytes(view_once = true)`
(imagem e vídeo); log sem bytes. Depende: T7, T8.

**Build 2** (integração) após T8–T9.

## Onda C — validação e opcional
**T10. [x] Validação real (viewOnce: ✗ sem media_key, deprecado; resto passou em 20/11/2026)** · integração — figurinha estática e animada abrem no Android/iOS;
aceita sem EXIF; **viewOnce: a citação traz `media_key` e o dispositivo vinculado baixa?**
(risco maior: fazer **primeiro**, antes de polir); GIF do app vem com faststart; tempos.
Depende: T8.

**T11. [x] EXIF: nome e autor ([WA] validado 20/11/2026: estática e animada mostram nome/autor; painel vale só nas novas)** · módulo — `src/stickers/exif.rs` (chunk `EXIF`), campos
`sticker_pack_name`=`Silicon™` e `sticker_publisher`=`@al4sk4s` em Ajustes (RF10); teste de `with_exif` e do
painel; [WA] conferir ao tocar na figurinha. Depende: T10.

**T12. [x] `/rename` ([WA] validado 20/11/2026: `a / b`, `/`, sem `/`, sem citar, >64)** · comando — `parse_rename` + handler (RF11/RF12): cita figurinha, troca nome/autor,
`/rename /` limpa. Teste do parser; [WA] real. Depende: T11.

## Paralelismo
Livres (sem build): T1–T6 juntos, depois T7. Serial: Builds 1–2 (`-j1`). **Fazer T10 do
viewOnce o mais cedo possível**: se o dispositivo vinculado não receber a chave, a feature
precisa ser repensada antes de T7/T9.

## Manutenção 20/11/2026
- `Limiter` agora descarta chats inativos (janela vencida; só `max 0` mantém o aviso); teste `idle_chats_are_dropped`.
- Teste instável **reproduzido e corrigido**: `make_leaves_nothing_on_disk` varria `/tmp` e pegava `silicon-*test*` de outros testes em paralelo e o bot de teste. Agora ignora nomes `silicon-*` e olha só o 1º nível. Antes: 1 falha em 30 execuções do binário sob carga; depois: 0 em 40.
- `handle`/`handle_rename` não se testam com `MessageContext` (montar `Client` exige o feature `test-support`,
  proibido). Extraído `rename_target` (puro) com teste; o fluxo completo segue validado só no WhatsApp real.
