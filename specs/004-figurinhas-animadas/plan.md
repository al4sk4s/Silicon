# Plano 004 — Figurinhas animadas melhores

Fonte: spec.md (aprovada 23/11/2026) + plano do `planner`. Sem dependência nova na Fase A.

## Fase A — WebP certo (convert.rs, mod.rs)
- `filter()` fixo: `scale=512:512,setsar=1,format=rgba`, sem `pad`. `-t 10` fixo (nunca corta duração).
- `ANIM_LADDER` só `(fps, q)`: fps 24→20→15→12; em cada fps, q 60→25 antes de baixar o fps.
  `-compression_level` 1–2. Números provisórios até medir com `travar.mp4` (A6).
- `to_animated` devolve `(bytes, q, fps)`; `handle` registra evento `figurinha_anim` (RFA3).
- Nada coube → `TooBig` ("Mídia pesada demais"), nunca um pedaço do vídeo.
- Risco: 10 s sem pad pode estourar o timeout de 15 s/tentativa em CPU fraca → medir.

## Fase B — spike `.was` (fora do caminho de produção)
- `examples/was_spike.rs`: Lottie próprio 512x512 com N frames (assets base64, 1 camada
  por frame com ip/op em sequência) + `animation.json`/`animation_secondary.json`/
  `overridden_metadata`; flag para `trust_token` ausente/lixo. Escrito do zero (P9).
- Envio: `send_sticker(bytes, mime, lottie)` com `application/was`, `is_lottie`, `is_animated`.
- B1 antes: conferir na rev da lib se o upload `MediaType::Sticker` aceita não-WebP, e a
  licença do crate `zip` (ou ZIP à mão).

## Fase C — toggle (só se B5 passar)
- `config.rs`: `video_sticker_mode()` (`webp` padrão | `was`). `panel.rs`: select em Ajustes
  (só aparece com o spike aprovado). `make()`: modo `was` gera `.was`; erro/limite → cai
  em `to_animated` sem avisar.
