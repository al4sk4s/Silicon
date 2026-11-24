# Tarefas 005 — Figurinhas animadas com som

Spec: `spec.md` · Plano: `plan.md`. Quem: `scout` (haiku, leitura), `worker` (sonnet, edita, nunca commita), `eu` (principal). `[WA]` = só valida com WhatsApp real.
**Build serializado:** `cargo fmt`, `clippy -- -D warnings`, `test`, sempre `-j1`, um por vez. Tarefas "sem build" podem rodar juntas.

## Onda 1 — paralelo, sem build (arquivos distintos)
- [x] **T1 [scout] Nome Rust do campo de áudio.** Achar no código gerado de `waproto` (ou `cargo doc`/`target/`) como se escreve `StickerMessage.audioMessage` (campo direto ou enum do oneof) e o tipo de `media::audio_message(..)`. Pronto: 3–5 linhas no `plan.md`. Dep.: nenhuma.
- [x] **T2 [worker] `config.rs`: `sticker_sound()` e `sticker_sound_max_s()`.** Bool padrão ligado (`!= "0"`); int 1..10 padrão 10, modelo `sticker_per_min` (`config.rs:125-138`), setter validado. Pronto: getters/setters + teste de faixa e padrão (escritos, sem rodar cargo). Dep.: nenhuma.
- [x] **T3 [worker] `convert.rs`: `Anim` e `extract_audio`.** `to_animated` devolve `Anim { webp, secs }` (secs = `-t` da rung vencedora) e chamadores ajustados; `extract_audio(bytes, secs) -> Result<Option<Vec<u8>>, ConvErr>` com `-map 0:a:0? -vn -t -c:a libopus -b:a 32k -ac 1 -f ogg pipe:1`, vazio → `None`, mesmo timeout da 002. Testes com vídeo `lavfi` com e sem áudio (pulam sem ffmpeg). Pronto: escrito. Dep.: nenhuma.

## Onda 2 — build serializado
- [x] **T4 [eu] Build da onda 1** (`fmt`, `clippy -j1`, `test -j1`); corrigir. Pronto: verde. Dep.: T2, T3.
- [x] **T5 [worker] `panel.rs`: checkbox `sticker_sound` e number `sticker_sound_max_s`.** Em `settings_page` (`:318-342`); em `settings_post` (`:348-411`) validar o int **antes** da cadeia de `set` (400 sem gravar), checkbox ausente = "0". Teste junto de `settings_post_saves_sticker_options`. Pronto: escrito. Dep.: T2, T4.

## Onda 3 — integração
- [x] **T6 [eu] `stickers/mod.rs`: `build_sticker` pura, `send_webp` com áudio, fluxo de vídeo, fallback.** Extrair áudio com `min(secs, sticker_sound_max_s)` se a flag estiver ligada; upload `MediaType::Audio`, `media::audio_message`, mover o `AudioMessage` para o `StickerMessage`; falha → sticker mudo + 1 linha (só se a mídia tinha som). Testes: roundtrip prost com campo 26, função pura do fallback. Dep.: T1, T4.
- [x] **T7 [eu] `/rename` mantém o som:** copiar o `audio_message` do sticker citado para o novo (`mod.rs:335-368`); vazio → mudo. Teste de `build_sticker`/cópia. Dep.: T6.
- [x] **T8 [worker] Nota na `specs/002-figurinhas/spec.md`:** "áudio descartado (`-an`)" substituído pela 005. Dep.: T6. (paralelo com T7, só doc)
- [x] **T9 [eu] Build e revisão final** (`-j1`): contra P1, P7 (zero crate), P8 (1 envio), sem áudio em disco/log. Dep.: T5, T7, T8.

## Validação real
- **T10 [WA] Som toca:** vídeo com áudio + `/s` → figurinha com ícone de volume tocando (Android/iOS); decide campo 26 vs 134 (plano B em `build_sticker`). Dep.: T9.
- **T11 [WA] Casos:** vídeo mudo igual à 002; áudio de 10 s aceito ou cortado (ajustar `sticker_sound_max_s`); `/rename` mantém som; flag desligada = 002; falha forçada → mudo + aviso; conta sem a função (mudo sem erro). Dep.: T10.
- **T12 [eu] Fechar:** marcar tarefas, atualizar spec/plano com o medido, `/gerar-prompt`. Dep.: T11.

## Resumo de paralelismo
Juntos (sem build): T1, T2, T3 · depois T4 sozinho · T5 após T4 · T7 e T8 juntos · serializados: T4, T9 (e todo cargo, `-j1`).
