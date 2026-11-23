# Tarefas 004 (build sempre `cargo -j1`, um por vez)

## Fase A
- [x] A1 worker: convert.rs (filtro 512 fixo, escada só fps/q, `-t 10`, retorna q/fps)
- [x] A2 worker: mod.rs evento `figurinha_anim` com q/fps (junto de A1)
- [x] A3 worker: testes (nenhum degrau ≠512; 10 s → ≥9,5 s, 512x512, ≤500 KB)
- [x] A4 (feito pelo principal): atualizar spec 002 RF4 e spec 004 com a escada final
- [x] A5 principal: fmt, clippy -D warnings, test
- [ ] A6 [WA] fim: `travar.mp4` + 2 vídeos: medir fps/q e ajustar a escada

## Fase B (REPROVADA, removida)
- [x] B1 scout: conferir upload/is_lottie na lib e licença do `zip`
- [x] B2 worker: `examples/was_spike.rs` (+ teste: ZIP abre, JSON parseia)
- [x] B3 worker: `send_sticker(bytes, mime, lottie)` (sem rota de produção)
- [x] B4 principal: fmt/clippy/test
- [ ] B5 [WA] fim (número principal, 1 envio por teste, espaçados): RFB1–RFB3 e "Resultado do spike"

## Fase C (REPROVADA) -> C': modo silicon|malvin feito
- [x] C1 worker: config `video_sticker_mode`
- [x] C2 worker: painel (select em Ajustes)
- [x] C3 worker: `make()` modo `was` com fallback
- [x] C4 principal: fmt/clippy/test
- [ ] C5 [WA] fim

- [x] C' toggle `video_sticker_mode` silicon|malvin (config, painel, convert)
- [ ] [WA] dono compara os dois modos no vídeo de teste
