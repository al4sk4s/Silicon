# Plano 004 — Downloader (o COMO)

Status: rascunho · Spec: `specs/004-downloader/spec.md` (aprovada 2026-11-24) · Depende de 001 e 002 ·
Constituição v1.0.0 · whatsapp-rust `eeb24da` (fixo) · yt-dlp externo (versão registrada no boot).

## Decisões a confirmar
1. **Upload de cookies: arquivo (multipart) como o dono pediu**, não `<textarea>`. Custo:
   ligar `features = ["multipart"]` no axum (puxa o crate `multer`; confirmar licença, P9) e
   `DefaultBodyLimit` de 1 MB só nessa rota. Alternativa sem dependência nova: colar o JSON
   num `<textarea>`. **Padrão deste plano: multipart.**
2. **Segundo `/baixar` durante um download: responde "ocupado"** (`try_acquire`), sem fila.
   Fila acumularia rajada de envios (P8).
3. **Limites de envio (chute a medir no T-WA):** vídeo normal até **64 MB**; acima, documento
   até teto absoluto **100 MB**; acima disso, recusa. O `upload` da lib é em memória (`Vec<u8>`), então
   100 MB = ~100 MB de RAM (+ cifra). Servidor real pode aceitar menos.
4. **Carrossel/foto fora do MVP** (como na spec). Link de carrossel responde "só vídeo por enquanto".
5. **Teste dos cookies é manual** (botão); sem teste periódico (tráfego não pedido, P1/P8).
6. **`tempfile`** só existe como dependência transitiva. Para o diretório temporário, adicionar
   `tempfile` direto (já no lock, licença MIT/Apache) em vez de guard próprio. Confirmar.

## Módulos
- `src/download/mod.rs` — `parse_cmd` (`/baixar`, `/dl`, `/download`) e `handle` (fluxo abaixo).
- `src/download/url.rs` — extrai a primeira URL e valida o host (RF3). Só `https`, host exato ou subdomínio
  de `tiktok.com`, `instagram.com`, `instagr.am`; rejeita `user@host`, IP, porta estranha.
- `src/download/ytdlp.rs` — monta args, roda o processo, classifica erro, probe de boot.
- `src/download/cookies.rs` — converte JSON (Cookie-Editor/EditThisCookie) → Netscape; grava `0600`.
- Tocados: `wa.rs` (ramo novo após `:80-86`), `state.rs` (2º `Limiter`, `Semaphore(1)` do download),
  `main.rs` (probe do yt-dlp ao lado do ffmpeg), `config.rs` (chaves `dl_per_min`, `dl_timeout_s`,
  `cookies_status_<plat>`), `panel.rs` (tela + rotas), `Cargo.toml` (`tempfile`, axum multipart),
  `.gitignore` (`cookies*`, `*cookies*.txt`, `cookies/`).

## Tipos
```rust
enum Platform { TikTok, Instagram }
struct DlCmd { url: Option<String> }
enum DlError { BadHost, NoUrl, LoginRequired, NotFound, Timeout, Missing, TooBig, Busy, Other }
impl DlError { fn reply(&self) -> &'static str } // uma linha PT-BR
struct Fetched { path: PathBuf, size: u64, width: u32, height: u32, secs: u32, watermark: bool }
```

## Fluxo do `/baixar`
`decide` (papel `comando`) → `parse_cmd` → delay 1–2 s (já existe) → URL do argumento ou da
mensagem **citada** → `url::validate` → `Limiter` do downloader (`dl_per_min`, padrão **2**) →
`try_acquire` do semáforo (`Busy`) → `TempDir` → `ytdlp::fetch` → `ffprobe`/`ffmpeg` (largura,
altura, duração, thumbnail JPEG de 1 frame) → `size ≤ 64 MB` ? `video_message` : `document_message`
(+ linha de aviso) → `upload` → `send_message` citando o comando → `Drop` apaga o `TempDir`.
Qualquer erro vira `DlError::reply()`; nada de log com URL/conteúdo (só plataforma, ok/erro, bytes).

## yt-dlp
Args: `-f "bv*+ba/b" --merge-output-format mp4 --no-playlist --max-filesize 100M --no-progress
--quiet -o <tmp>/v.%(ext)s [--cookies <cópia no tmp>] -- <url>`. A URL vem **depois de `--`**.
- Cookies: copiar o arquivo da plataforma para o `TempDir` antes (o yt-dlp reescreve o arquivo
  de cookies); a cópia some com o dir.
- Timeout `dl_timeout_s` (padrão 120) com `kill_on_drop`; stderr só para classificar
  (`login`/`private`/`404`/`429`/`watermark`), nunca vai a log.
- **TikTok sem marca d'água:** a seleção `bv*` deve já pegar o formato sem marca. **A confirmar
  no T-H** (README/`--list-formats` real): se o extrator só entregar com marca, `watermark=true`
  e o bot avisa (RF5). Não inventar flag antes de testar.
- Probe de boot: `yt-dlp --version` (timeout 30 s, `AtomicBool`, espelha `stickers/check.rs`);
  guarda a versão e avisa se tiver > ~60 dias. Ausente → `DlError::Missing`.
- yt-dlp precisa do `ffmpeg` para juntar streams; o probe do ffmpeg já existe.
- Um processo por vez via `Semaphore(1)` (RF11), separado do semáforo do ffmpeg.

## Envio (lib)
`media::video_message`/`document_message` (lib `src/media.rs`) **não** preenchem `width/height`
(vídeo) nem `thumbnailWidth/Height` (documento): setar no proto. `jpeg_thumbnail` vem do ffmpeg
(a lib não gera). `streaming_sidecar` omitido (não obrigatório; validar no T-WA). Exemplo de
envio manual: `stickers/mod.rs:372-416`. Quote: `ctx.build_quote_context()`.

## Painel
- Seção "Downloader" em `/settings` (ou `/downloader`): por plataforma, **campo de arquivo** `.json`,
  botão **Salvar** (converte e grava `data/cookies/<plat>.txt`, `0600`; inválido → erro, nada gravado),
  **Testar** (`yt-dlp --simulate` numa URL pública fixa, resultado ok/falhou + horário em `settings`),
  **Remover**; campo **limite por minuto** (`dl_per_min`, inteiro 1..10) no padrão de `settings_post`
  (`panel.rs:348-411`), efeito imediato (lido a cada mensagem).
- POST passa pelo `guard` existente (senha + `same_origin`, P5). Cookies nunca no HTML nem em log: só status.
- Multipart: `DefaultBodyLimit` 1 MB na rota; checar tipo/tamanho; JSON array de objetos com `name`,
  `value`, `domain` (descartar cookies de outros domínios que não da plataforma).

## Config e padrões seguros
`dl_per_min=2`, `dl_timeout_s=120`, sem cookies (sem login), cookies só `0600` em `data/cookies/`
(`data/` já ignorado; ainda assim `.gitignore` ganha `cookies*`). Sem senha no painel = só loopback (já é assim).

## Riscos
- Extratores quebram (TikTok/IG): aviso de versão velha no boot; atualização é do dono (`yt-dlp -U`).
- Conta dos cookies banida: usar secundária; aviso na tela do painel.
- RAM: upload em memória (~100 MB no pior caso); teto absoluto evita pior. `upload_stream` é plano B.
- Limite real do servidor WhatsApp desconhecido: medir no T-WA e ajustar 64/100 MB.
- Mensagem citada pode trazer URL com parâmetros/redirect (`vm.tiktok.com`): validar host **antes**;
  o yt-dlp segue redirect, então o host final pode divergir (aceito; ver pergunta).
- Injeção por URL: sempre `Command::args` + `--`, nunca shell.

## Testes
- `cargo test`: `url::validate` (hosts bons/ruins, `@`, IP, `http`), `cookies` (JSON→Netscape,
  domínio filtrado, inválido), `ytdlp` (args montados, classificação de stderr), limiter do
  downloader, `DlError::reply`, `TempDir` removido após erro/timeout (processo fake `sleep`).
- Só com internet: yt-dlp real contra 1 TikTok e 1 Reel públicos (T-H, manual, `#[ignore]`).
- Só WhatsApp real (T-WA): vídeo normal, documento acima do limite, thumbnail, abre/toca,
  TikTok sem marca, citar link, ocupado, chat sem papel ignorado, cookies Testar → status.

## Passos (quem faz)
| # | Passo | Quem | Depende |
|---|---|---|---|
| H | Confirmar yt-dlp (`--list-formats`/watermark TikTok, `--print`) e campos do VideoMessage | scout | — |
| A | `url.rs` + testes | worker | — |
| B | `cookies.rs` + testes | worker | — |
| C | `ytdlp.rs` (args, erro, probe) + testes | worker | H |
| D | config/state (chaves, 2º limiter, semáforo) | worker | — |
| E | `download/mod.rs` handle + envio | eu | A, C, D |
| F | painel (multipart, testar, limite) | worker | B, C, D |
| G | wa.rs/main.rs/.gitignore + integração | worker | E |
Paralelo sem build: H, A, B, C, D. Build um por vez (`-j1`): fmt/clippy/test a cada leva.
