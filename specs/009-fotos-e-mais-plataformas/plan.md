# Plano 009 — `/baixar` com fotos, carrosséis e Facebook (o COMO)

Status: implementado, falta validar com WhatsApp real (T8-T11) (2026-11-24) · Spec: `specs/009-fotos-e-mais-plataformas/spec.md` · Constituição v1.1.0 ·
whatsapp-rust `eeb24da` (pin fixo, sem atualizar).

## Decisões a confirmar
1. **`gallery-dl` como binário externo** (P7: zero crate). Instalação é do dono; o servidor ainda não tem
   (`pacman` não achou). Sem ele, só o caminho yt-dlp funciona e foto responde "indisponível" (como `DlError::Missing`).
2. **Fallback do yt-dlp:** o gallery-dl só roda quando o yt-dlp devolve `NotFound` (inclui "no video formats"); não há variante nova de erro.
3. **Álbum "de verdade"** exige mensagem-pai + filhos associados. Faço do jeito do WhatsApp Web; se o celular
   não agrupar (`[WA]`), o plano B é mandar as fotos soltas, uma a uma (mesma rajada controlada). Decisão após o teste.
3. **Facebook entra só como host** no `validate`. Cookies do Facebook (vídeo privado) ficam **fora** desta
   spec: `Platform::Facebook` não ganha UI de cookies agora (menor mudança). Público funciona sem.

## Fatos da lib (lidos no código, `eeb24da`)
- `wa::message::AlbumMessage { expected_image_count: u32, expected_video_count: u32, context_info }`
  (`whatsapp.proto:3080`), campo `album_message` (83) de `wa::Message`.
- `whatsapp_rust::proto_helpers::wrap_as_album_child(inner: wa::Message, parent_key: wa::MessageKey) -> wa::Message`
  (`wacore/src/proto_helpers.rs:839`): embrulha o filho em `associated_child_message` e põe
  `MessageAssociation { MEDIA_ALBUM, parent_message_key }` no `message_context_info`. Já está em
  `whatsapp_rust` (`lib.rs:434`, `pub use ... proto_helpers`). Não há helper do **pai**: monto `AlbumMessage` à mão.
- `SendResult { message_id, to, .. }` (`send/mod.rs:1006`): serve para montar o `MessageKey` do pai
  (`remote_jid = chat`, `from_me = true`, `id = message_id`).
- Upload de imagem: `client.upload(bytes, MediaType::Image, UploadOptions::new())` (mesmo caminho do vídeo em
  `handle.rs`); `ImageMessage` com `url, direct_path, media_key, file_enc_sha256, file_sha256, file_length,
  media_key_timestamp, mimetype`, `width/height`, `jpeg_thumbnail` opcional.
- Pergunta 2 da spec respondida: o pai existe (`album_message`), só não tem helper.

## Módulos tocados
- **`src/download/gallery.rs` (novo)**: espelho do `ytdlp.rs` para imagens.
  ```rust
  pub struct Item { pub path: PathBuf, pub size: u64, pub kind: ItemKind }   // ItemKind::{Image, Video}
  pub async fn fetch(url, plat, cookies_dir, out_dir, timeout) -> Result<Vec<Item>, DlError>
  pub fn build_args(url, out_dir, cookies: Option<&Path>) -> Vec<String>
  pub fn classify(stderr) -> DlError
  ```
  Args: `-D <out_dir>` (sem subpastas), `--no-mtime`, `--range 1-10`, `--filesize-max 100M`,
  `--cookies <copia>` quando houver, `--` e a URL. Sem shell; `kill_on_drop`; timeout do `/baixar`.
  Tipo pela extensão (`jpg|jpeg|png|webp` = imagem; `mp4|mov|webm` = vídeo); resto é ignorado e apagado com o tempdir.
  Ordem = nome do arquivo (gallery-dl numera). Soma > 100 MB → `TooBig`.
  `probe()`/`ok()` com `GALLERY_OK: AtomicBool`, igual ao `YTDLP_OK`.
- **`src/download/handle.rs`**: fluxo novo depois do semáforo/reação:
  1. `ytdlp::fetch` → vídeo único (fluxo atual, intocado).
  2. Se o erro for `NoVideo` (novo variante interno de `classify`: "no video formats", "no media"), tenta `gallery::fetch`.
  3. `send_album(ctx, st, sent, chat, items)`: fotos primeiro, depois vídeos pelo `send` atual (recodifica
     com `ensure_compatible` por item, `Fetched`-like).
- **`send_album`**: 1 item de imagem → `ImageMessage` simples (sem pai). ≥ 2 → pai
  `AlbumMessage{expected_image_count: n_fotos, expected_video_count: 0}` citando o comando, depois cada foto
  `wrap_as_album_child(img, parent_key)`, atraso 1–2 s entre envios (`pause`, como no resto). Carrossel misto (decisão do
  dono, 2026-11-24): vídeos **entram no mesmo álbum**: `expected_video_count = n_vídeos`, cada vídeo
  recodificado (`ensure_compatible`), enviado como `VideoMessage` e embrulhado com `wrap_as_album_child`; ordem
  original do post. Vídeo > 64 MB não cabe no álbum: sai solto como documento, depois. Foto > 16 MB é descartada com Debug `baixar_foto_grande` (shortcut: rara; documento se aparecer na prática).
- **`src/download/url.rs` / `mod.rs`**: `Platform::Facebook` (`facebook.com`, `fb.watch`, `fb.com`); `name()` →
  `"facebook"`. `cookies.rs`: ramo `Facebook` com domínio `facebook.com` para o `json_to_netscape` (compila;
  UI de cookies não ganha item). `config.rs`/painel: nada.
- **`src/download/ytdlp.rs`**: `classify` distingue "sem vídeo" (`DlError::NoVideo`, nunca mostrado ao usuário:
  só dispara o fallback). Args do yt-dlp não mudam.
- **`src/help.rs`**: `/baixar` long (fotos, Facebook, ≤ 10 itens) e `NOVIDADES` (+ "Download de fotos e
  álbuns"). Testes de tamanho da bio continuam valendo.
- Sem tocar: `Cargo.toml`, `state.rs`, `panel.rs`, `config.rs`, sessão.

## Instalação do gallery-dl (feito em 2026-11-24)
O GitHub do projeto não publica binário nas releases. Empacotei a versão 1.32.21 (wheel do PyPI, Python puro)
num único executável (`zipapp`, shebang `/usr/bin/env python3`); testado local e no servidor (`--version` ok),
enviado para `~/gallery-dl` do servidor. Falta só instalar no PATH (precisa de `sudo`):
`sudo install -m755 ~/gallery-dl /usr/local/bin/gallery-dl`. Atualizar = repetir com a versão nova.

## Fluxo
`validate` (host) → limite/min → probe → semáforo → reação → `ytdlp::fetch`
→ ok: fluxo de vídeo atual · `NoVideo`: `gallery::fetch` → itens → fotos (álbum) → vídeos →
tempdir apagado. Qualquer outro erro: resposta com as 5 variações já existentes. Um comando = uma rajada
(≤ 10 fotos + vídeos do carrossel, todos limitados por `--range 1-10`).

## Riscos
- **Ban/spam (P8):** álbum de 10 = 11 mensagens numa rajada. Mitigação: teto 10 itens, 1–2 s entre elas,
  limite por minuto e semáforo já existentes; só no chat de quem chamou. Aceitável (uso humano comum), mas é
  o maior risco: começar com **teto 10 e atraso 1,5 s** e olhar `[WA]`.
- **Álbum não agrupa** no celular (o servidor pode exigir `expected_*` ou `messageIndex`): plano B acima.
  Testar com 2 e com 5 fotos.
- **Extrator quebra** (sites mudam): `gallery-dl` e `yt-dlp` precisam de atualização periódica; erro vira a
  resposta genérica + Debug `baixar_erro_gallerydl` com categoria, sem URL de terceiros.
- **Privacidade/segurança:** `--` antes da URL, host validado, sem shell, cookies 0600 em tempdir apagado;
  nome de arquivo do `gallery-dl` nunca vai para o usuário (usamos nomes fixos no envio).
- **Disco/RAM:** foto lida inteira em memória (≤ 16 MB cada, ≤ 100 MB total), igual ao vídeo.
- **Facebook sem cookie:** muita coisa é só com login; resposta `LoginRequired` já existe.

## Config e padrões seguros
Nenhuma chave nova. Teto de 10 itens e 100 MB fixos no código; limite/min, timeout e cookies já existentes.

## Testes
- `cargo test`: `build_args` (URL só depois de `--`, `-D`, `--range`, cookies opcionais); classificação por
  extensão e ordem dos arquivos num tempdir de teste; teto de 10; `classify` do stderr (login, not found,
  no media → `NoVideo`); `validate` aceita `facebook.com`/`fb.watch`/`m.facebook.com` e rejeita
  `facebook.com.evil.io`; montagem do `AlbumMessage` e do `MessageKey` do pai (função pura); 1 foto não gera pai.
- Só WhatsApp real `[WA]`: álbum de 2 e de 5 fotos agrupa no celular; carrossel misto; Facebook público toca;
  post de 12 itens manda 10 + aviso; sem `gallery-dl` instalado → resposta de indisponível.

## Passos (quem faz)
| # | Passo | Quem | Depende |
|---|---|---|---|
| A | `gallery.rs` (args, classify, fetch, ordem/ext) + testes | worker | — |
| B | `Platform::Facebook` em `url.rs`/`mod.rs`/`cookies.rs` + testes | worker | — |
| C | `NoVideo` em `ytdlp::classify` + fallback em `handle.rs` | eu | A, B |
| D | `send_album` (pai + filhos + pausas + documento > 16 MB) + teste da montagem | eu | C |
| E | `help.rs` (texto, `NOVIDADES`) | worker | — |
| F | build `-j1` (fmt, clippy, test) + revisão P1/P3/P8 | eu | A–E |
| G | instalar `gallery-dl` no servidor (dono) + validação `[WA]` + commit/push | dono + eu | F |
