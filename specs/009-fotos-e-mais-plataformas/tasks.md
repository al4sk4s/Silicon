# Tarefas 009 — `/baixar` com fotos, álbum e Facebook

Spec: `spec.md` · Plano: `plan.md`. Quem: `worker` (sonnet, edita, nunca commita), `eu` (principal). `[WA]` = só valida com WhatsApp real.
**Build serializado:** `cargo fmt`, `clippy -- -D warnings`, `test`, sempre `-j1`, um por vez. Workers não rodam cargo.
`gallery-dl` 1.32.21 já instalado em `/usr/local/bin` no servidor (dono, 2026-11-24).

## Onda 1 — em paralelo, sem build (arquivos diferentes)
- [ ] **T1 [worker] `src/download/gallery.rs`:** `Item{path,size,kind}`, `ItemKind::{Image,Video}`, `build_args` (`-D`, `--no-mtime`, `--range 1-10`, `--filesize-max 100M`, `--cookies` opcional, URL só após `--`), `classify(stderr)->DlError`, `fetch(url, plat, cookies_dir, out_dir, timeout)->Result<Vec<Item>,DlError>` (tipo por extensão, ordem por nome, ignora o resto, soma > 100 MB → `TooBig`, `kill_on_drop`), `probe()`/`ok()` com `GALLERY_OK`. Espelhar `ytdlp.rs`. Pronto: arquivo + testes escritos (args, extensões, ordem, teto de 10, classify). Dep.: nenhuma.
- [ ] **T2 [worker] `Platform::Facebook`:** `mod.rs` (variante, `name()` = `facebook`), `url.rs` (`facebook.com`, `fb.com`, `fb.watch`, subdomínios reais), `cookies.rs` (domínio no `json_to_netscape`; sem UI nova). Testes: aceita `m.facebook.com`/`fb.watch`, rejeita `facebook.com.evil.io`. Dep.: nenhuma.
- [ ] **T3 [worker] `src/help.rs`:** `/baixar` long (fotos, álbum, Facebook, ≤ 10 itens) e `NOVIDADES` (+ "Download de fotos e álbuns"). Teste da bio (≤ 2048) continua. Dep.: nenhuma.

## Onda 2 — build serializado
- [ ] **T4 [eu] Build das ondas 1** + `mod gallery;` em `download/mod.rs`; corrigir `match` de `Platform` em todo o código. Pronto: `fmt`, `clippy -j1`, `test -j1` verdes. Dep.: T1, T2, T3.
- [ ] **T5 [eu] `NoVideo` em `ytdlp::classify`** (stderr "no video formats"/"no media" → variante interna, nunca mostrada) + fallback em `handle.rs`: `ytdlp::fetch` → `NoVideo` → sonda `gallery::ok()` (re-sonda como o yt-dlp; ausente → `DlError::Missing`) → `gallery::fetch`. Debug `baixar_gallerydl_ok`/`baixar_erro_gallerydl`. Dep.: T4.
- [ ] **T6 [eu] `send_album`:** 1 item de imagem → `ImageMessage` simples; ≥ 2 itens → pai `AlbumMessage{expected_image_count, expected_video_count}` citando o comando + `MessageKey` do pai (do `SendResult`), filhos com `wrap_as_album_child`, ordem do post, 1–2 s entre envios; vídeo recodificado (`ensure_compatible`) e enviado como `VideoMessage` dentro do álbum; foto > 16 MB e vídeo > 64 MB saem soltos como documento depois; Debug `baixar_album` (nº de itens e bytes). Funções puras de montagem (pai, chave, divisão álbum/solto) com teste. Dep.: T5.
- [ ] **T7 [eu] Build e revisão final** (`-j1`): P1 (só responde a quem chamou), P3, P8 (≤ 10 + vídeos, pausas, sem laço), tempdir apagado em todos os caminhos, sem URL/JID de terceiros no Debug, sem segredo no diff. `cargo build --release -j1`, avisar "build pronto". Dep.: T6.

## Validação real
- [ ] **T8 [WA] Álbum:** post com 2 e com 5 fotos (Instagram e TikTok foto) chega agrupado no celular. Se não agrupar → plano B (fotos soltas) e abrir ajuste. Dep.: T7 + deploy.
- [ ] **T9 [WA] Misto:** carrossel foto+vídeo no mesmo álbum, vídeo toca. Dep.: T8.
- [ ] **T10 [WA] Facebook:** vídeo/Reel público baixa e toca (celular e PC); link privado → resposta de login. Dep.: T7 + deploy.
- [ ] **T11 [WA] Limites:** post com > 10 itens manda 10 e avisa; foto > 16 MB como documento; sem `gallery-dl` no PATH → resposta de indisponível (sem travar). Dep.: T8.
- [ ] **T12 [eu] Fechar:** marcar tarefas, atualizar spec/plano com o medido, commit + push, `/gerar-prompt`. Dep.: T8–T11.

## Resumo de paralelismo
T1, T2, T3 juntas (workers, sem build) · depois T4 · T5 e T6 em sequência (mesmo arquivo `handle.rs`) · T7 serializado · T8–T11 só com WhatsApp real depois do `deploy.sh` do dono.
