# Tarefas 004 — Downloader

Spec: `spec.md` · Plano: `plan.md`. Quem: `scout` (haiku, leitura), `worker` (sonnet, edita, nunca commita), `eu` (principal). `[WA]` = só valida com WhatsApp real.
**Build serializado:** `cargo fmt`, `clippy -- -D warnings`, `test`, sempre `-j1`, um por vez. Tarefas "sem build" podem rodar juntas.

Antes de começar: confirmar as decisões 1–6 do topo do `plan.md` (multipart, "ocupado", limites 64/100 MB, `tempfile`).

## Onda 1 — paralelo, sem build (arquivos distintos)
- ✅ **T1 [scout] Confirmar yt-dlp.** Instalar/achar `yt-dlp` (não existe no dev), rodar `--list-formats` num TikTok e num Reel públicos; registrar em `plan.md`: formato sem marca d'água, `--print` útil, mensagens de stderr para login/privado/404/429. Pronto: seção "Achados T1" no plano. Dep.: nenhuma.
- ✅ **T2 [scout] Campos do VideoMessage/DocumentMessage** na lib (`eeb24da`): confirmar `width/height/seconds/jpeg_thumbnail`, `thumbnailWidth/Height`. Pronto: lista curta no plano. Dep.: nenhuma.
- ✅ **T3 [worker] `download/url.rs` + testes.** `validate(&str) -> Result<(Platform,String), DlError>`: https, host exato/subdomínio de `tiktok.com`/`instagram.com`/`instagr.am`, rejeita `@`, IP, porta, `http`; extrai a primeira URL de um texto. Pronto: arquivo e testes escritos (sem rodar cargo). Dep.: nenhuma.
- ✅ **T4 [worker] `download/cookies.rs` + testes.** JSON (array `name,value,domain,path,secure,expirationDate`) → Netscape; descarta domínio de outra plataforma; inválido/vazio → erro; gravação `0600` em `data/cookies/<plat>.txt`. Pronto: escrito. Dep.: nenhuma.
- ✅ **T5 [worker] `config.rs` + `state.rs`.** Chaves `dl_per_min` (1..10, padrão 2), `dl_timeout_s` (padrão 120), `cookies_status_<plat>`; 2º `Limiter` e `Semaphore(1)` no `AppState`. Pronto: getters/setters + teste de faixa. Dep.: nenhuma.
- ✅ **T6 [worker] `.gitignore` + `Cargo.toml`.** Regras `cookies*`, `*cookies*.txt`, `cookies/`; `tempfile` direto; axum `multipart` (se decisão 1 confirmada). Pronto: diff mínimo. Dep.: decisões 1 e 6.

## Onda 2 — build serializado
- ✅ **T7 [eu] Build da onda 1:** registrar `mod download;` com os módulos stub, `fmt` + `clippy -j1` + `test -j1`; corrigir. Pronto: verde. Dep.: T3–T6.
- ✅ **T8 [worker] `download/ytdlp.rs` + testes.** Montar args (URL após `--`, `Command::args`), `fetch` com timeout/`kill_on_drop`, cópia dos cookies no `TempDir`, `classify(stderr) -> DlError`, probe de boot (`--version`, `AtomicBool`, aviso > 60 dias). Testes: args, classificação, timeout com processo fake. Dep.: T1, T7. Sem build do worker; eu rodo na T9.
- ✅ **T9 [eu] Build do T8** (`-j1`). Dep.: T8.

## Onda 3 — integração
- ✅ **T10 [eu] `download/mod.rs`:** `parse_cmd`, `handle` (fluxo do plano), thumbnail/dimensões via ffmpeg, vídeo vs documento, upload, quote. Teste: `parse_cmd`, `DlError::reply`, limite→documento (função pura). Dep.: T2, T3, T5, T9.
- ✅ **T11 [worker] Painel (em paralelo com T10, só edita `panel.rs`):** seção Downloader: arquivo `.json` por plataforma, Salvar/Testar/Remover, status+horário, campo `dl_per_min`; `DefaultBodyLimit` 1 MB; cookies nunca no HTML/log. Dep.: T4, T5, T8.
- ✅ **T12 [worker] `wa.rs` + `main.rs`:** ramo `/baixar` após `wa.rs:80-86` e probe do yt-dlp no boot. Dep.: T10.
- ✅ **T13 [eu] Build e revisão final:** `fmt`, `clippy -D warnings`, `test` (`-j1`); revisar contra a constituição (P2, P4, P5, P8) e que cookies/URL não vão a log. Dep.: T11, T12.

## Validação real
- **T14 [WA] Vídeo TikTok e Reel públicos** (sem marca, melhor qualidade, citar link, `/dl`). Dep.: T13.
- **T15 [WA] Limites:** vídeo > 64 MB como documento; medir teto real do servidor e ajustar; "ocupado"; limite por minuto; chat sem papel ignorado. Dep.: T14.
- **T16 [WA] Cookies:** anexar JSON válido → Testar "válido"; inválido → erro sem gravar; conteúdo que exige login funciona só com cookie. Dep.: T14.
- **T17 [eu] Fechar:** atualizar spec/plano com o medido, `/gerar-prompt`. Dep.: T15, T16.

## Status (2026-11-24)
T1–T13 feitas e verdes (`fmt`, `clippy -D warnings`, `test`: 68 ok). Falta T14–T17 (`[WA]`, com o dono e número de teste).
Fluxo do `/baixar` ficou em `src/download/handle.rs` (não em `mod.rs`). Decisões 1–6 do plano seguiram os padrões (multipart, "ocupado", 64/100 MB, carrossel fora, teste manual, `tempfile` direto), **não confirmadas pelo dono**.
A validar no WA: carrossel (`--playlist-items 1` pega só o 1º item), link curto `vm.tiktok.com` (falhou no T1), upload de ~100 MB em memória, campos do proto de vídeo/documento.

## Resumo de paralelismo
Juntos (sem build): T1, T2, T3, T4, T5, T6 · depois T8 sozinho · T10 e T11 juntos (arquivos diferentes) · serializados: T7, T9, T13.
