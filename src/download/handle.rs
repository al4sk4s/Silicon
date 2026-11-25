//! Fluxo do `/baixar`: URL -> limite -> semáforo -> yt-dlp -> envio (spec 004).
use super::{DlError, cookies_dir, gallery, url, ytdlp};
use std::path::Path;
use std::time::Duration;
use whatsapp_rust::prelude::*;

/// Até aqui vai como vídeo; acima, como documento (chute a medir no [WA], plano decisão 3).
pub const MAX_VIDEO: u64 = 64 * 1024 * 1024;
/// Teto absoluto (o yt-dlp também usa `--max-filesize 100M`).
pub const MAX_TOTAL: u64 = 100 * 1024 * 1024;

/// `/baixar`, `/dl` ou `/download` (sem caixa). Some(resto do texto) se for o comando.
pub fn parse_cmd(text: &str) -> Option<&str> {
    let t = text.trim_start();
    let end = t.find(char::is_whitespace).unwrap_or(t.len());
    let (first, rest) = t.split_at(end);
    ["/baixar", "/dl", "/download"]
        .iter()
        .any(|c| first.eq_ignore_ascii_case(c))
        .then_some(rest.trim())
}

/// Vídeo normal ou documento (função pura).
pub fn as_document(size: u64) -> bool {
    size > MAX_VIDEO
}

async fn say(ctx: &MessageContext, sent: &crate::wa::Sent, t: &str) {
    if let Ok(r) = ctx.reply_quoting(t).await {
        sent.add(r.message_id.as_str());
    }
}

/// Codecs do arquivo (ffprobe, `chave=valor` por fluxo).
async fn probe_codecs(path: &Path) -> Option<String> {
    let p = path.to_string_lossy();
    let out = run(
        "ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,codec_type,pix_fmt",
            "-of",
            "default=noprint_wrappers=1",
            &p,
        ],
    )
    .await?;
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// true se o vídeo não for H.264 yuv420p ou o áudio não for AAC (não toca em todo aparelho).
pub fn needs_transcode(probe: &str) -> bool {
    let (mut name, mut kind, mut pix) = (String::new(), String::new(), String::new());
    let mut bad = false;
    let mut check = |name: &str, kind: &str, pix: &str| {
        if (kind == "video" && (name != "h264" || pix != "yuv420p"))
            || (kind == "audio" && name != "aac")
        {
            bad = true;
        }
    };
    for l in probe.lines() {
        if let Some(v) = l.strip_prefix("codec_name=") {
            check(&name, &kind, &pix);
            (name, kind, pix) = (v.trim().into(), String::new(), String::new());
        } else if let Some(v) = l.strip_prefix("codec_type=") {
            kind = v.trim().into();
        } else if let Some(v) = l.strip_prefix("pix_fmt=") {
            pix = v.trim().into();
        }
    }
    check(&name, &kind, &pix);
    bad
}

/// Recodifica para H.264/AAC + faststart quando o arquivo não toca em todo lugar.
/// Falha mantém o original (melhor tocar mal que não mandar).
async fn ensure_compatible(st: &crate::state::AppState, got: &mut ytdlp::Fetched, tmp: &Path) {
    let Some(probe) = probe_codecs(&got.path).await else {
        st.dbg("baixar_erro_probe", "ffprobe falhou");
        return;
    };
    let resumo = probe
        .lines()
        .filter(|l| !l.starts_with("codec_type"))
        .collect::<Vec<_>>()
        .join(" ");
    if !needs_transcode(&probe) {
        st.dbg("baixar_codec", &format!("ok {resumo}"));
        return;
    }
    st.dbg("baixar_codec", &format!("recodificando: {resumo}"));
    let out = tmp.join("t.mp4");
    let (i, o) = (got.path.to_string_lossy(), out.to_string_lossy());
    let cmd = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            &i,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            &o,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .status();
    let ok = matches!(tokio::time::timeout(Duration::from_secs(120), cmd).await, Ok(Ok(s)) if s.success());
    match tokio::fs::metadata(&out).await {
        Ok(m) if ok && m.len() > 0 => {
            st.dbg(
                "baixar_codec",
                &format!("recodificado {} -> {} bytes", got.size, m.len()),
            );
            *got = ytdlp::Fetched {
                path: out,
                size: m.len(),
            };
        }
        _ => st.dbg(
            "baixar_erro_recodificar",
            "ffmpeg falhou; enviando original",
        ),
    }
}

/// Largura, altura e duração (s) via ffprobe; falha vira None (o envio segue sem).
async fn probe_video(path: &Path) -> Option<(u32, u32, u32)> {
    let p = path.to_string_lossy();
    let out = run(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:format=duration",
            "-of",
            "default=noprint_wrappers=1",
            &p,
        ],
    )
    .await?;
    parse_probe(&String::from_utf8_lossy(&out))
}

fn parse_probe(s: &str) -> Option<(u32, u32, u32)> {
    let get = |k: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(k)?.strip_prefix('='))
            .map(str::trim)
    };
    let w = get("width")?.parse().ok()?;
    let h = get("height")?.parse().ok()?;
    let d: f64 = get("duration")?.parse().ok()?;
    Some((w, h, d.round() as u32))
}

/// Miniatura JPEG de 1 frame (largura 320) via ffmpeg.
async fn thumbnail(video: &Path, dir: &Path) -> Option<Vec<u8>> {
    let out = dir.join("thumb.jpg");
    let (v, o) = (video.to_string_lossy(), out.to_string_lossy());
    run(
        "ffmpeg",
        &[
            "-y",
            "-v",
            "error",
            "-i",
            &v,
            "-frames:v",
            "1",
            "-vf",
            "scale=320:-2",
            "-q:v",
            "5",
            &o,
        ],
    )
    .await?;
    tokio::fs::read(&out).await.ok()
}

/// Roda um binário auxiliar (30 s, filho morto ao estourar); None se falhar.
async fn run(bin: &str, args: &[&str]) -> Option<Vec<u8>> {
    let cmd = tokio::process::Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(30), cmd).await {
        Ok(Ok(o)) if o.status.success() => Some(o.stdout),
        _ => None,
    }
}

const REACOES: &[&str] = &["⏬", "📥", "⬇️", "🔽", "⏳", "🛰️"];

/// Trata `/baixar`. Falha = uma linha citando o comando.
/// Log só com plataforma, resultado e tamanho (nunca URL nem conteúdo).
pub async fn handle(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    arg: &str,
) {
    let chat = ctx.info.source.chat.to_string();
    st.dbg("baixar_recebido", &format!("chat {chat}"));
    let fail = |e: DlError| {
        st.dbg("baixar_recusa", &format!("{e:?}"));
        say(ctx, sent, e.reply())
    };
    // Link do argumento, senão da mensagem citada (RF2).
    let quoted = ctx
        .message
        .context_info()
        .and_then(|c| c.quoted_message.as_option())
        .map(|q| q.get_base_message())
        .and_then(|b| b.text_content().or_else(|| b.get_caption()));
    let text = if url::first_url(arg).is_some() {
        arg
    } else {
        quoted.unwrap_or("")
    };
    let (plat, link) = match url::validate(text) {
        Ok(v) => v,
        Err(e) => return fail(e).await,
    };
    let max = st.store.dl_per_min().unwrap_or(2);
    match st.dl_limit.check(&chat, std::time::Instant::now(), max) {
        crate::stickers::limit::Verdict::Allow => {}
        crate::stickers::limit::Verdict::DenyFirst => {
            st.dbg("baixar_recusa", "limite por minuto");
            return say(ctx, sent, "Calma, muitos downloads. Tente daqui a pouco.").await;
        }
        crate::stickers::limit::Verdict::DenySilent => {
            st.dbg("baixar_recusa", "limite por minuto (silencioso)");
            return;
        }
    }
    if !ytdlp::ok() {
        ytdlp::probe().await; // o probe do boot pode ter falhado de forma intermitente
    }
    if !ytdlp::ok() {
        return fail(DlError::Missing).await;
    }
    let Ok(_permit) = st.dl_sem.try_acquire() else {
        return fail(DlError::Busy).await;
    };
    let Ok(tmp) = tempfile::tempdir() else {
        return fail(DlError::Other).await;
    };
    // Resposta visual: o usuário vê que está baixando. Falha é ignorada.
    match ctx.react(crate::stickers::variante(REACOES)).await {
        Ok(r) => {
            sent.add(r.message_id.as_str());
            st.dbg("baixar_reacao", "ok");
        }
        Err(e) => st.dbg("baixar_erro_reacao", &e.to_string()),
    }
    let timeout = Duration::from_secs(st.store.dl_timeout_s().unwrap_or(120));
    let mut got = match ytdlp::fetch(&link, plat, &cookies_dir(), tmp.path(), timeout).await {
        Ok(g) => g,
        // Sem vídeo no post (foto/carrossel): tenta o gallery-dl (spec 009).
        Err(DlError::NotFound) => {
            return photos(ctx, st, sent, &chat, &link, plat, tmp.path(), timeout).await;
        }
        Err(e) => {
            st.dbg("baixar_erro_ytdlp", &format!("{e:?} plataforma {plat:?}"));
            st.push_event("download_erro", &chat);
            return fail(e).await;
        }
    };
    st.dbg("baixar_ytdlp_ok", &format!("{} bytes", got.size));
    ensure_compatible(st, &mut got, tmp.path()).await;
    if got.size > MAX_TOTAL {
        return fail(DlError::TooBig).await;
    }
    send(ctx, st, sent, &chat, &got, tmp.path()).await;
    // `tmp` cai aqui e apaga o diretório (RF12), em qualquer caminho acima.
}

/// Monta a mensagem de vídeo (ou documento, se grande) já enviada ao servidor de mídia.
/// `dir` guarda arquivos temporários deste item (miniatura). None = falhou (já registrado no Debug).
async fn build_video(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    chat: &str,
    got: &ytdlp::Fetched,
    dir: &Path,
    quote: MessageField<wa::ContextInfo>,
) -> Option<wa::Message> {
    use whatsapp_rust::download::MediaType;
    let doc = as_document(got.size);
    let dims = probe_video(&got.path).await;
    let thumb = thumbnail(&got.path, dir).await;
    // shortcut: lê o arquivo inteiro em memória (≤ 100 MB), trocar por `upload_stream` se pesar.
    let bytes = tokio::fs::read(&got.path).await.ok()?;
    let kind = if doc {
        MediaType::Document
    } else {
        MediaType::Video
    };
    let up = match ctx
        .client
        .upload(bytes, kind, whatsapp_rust::upload::UploadOptions::new())
        .await
    {
        Ok(u) => u,
        Err(e) => {
            st.dbg("baixar_erro_upload", &e.to_string());
            st.push_event("download_erro_upload", chat);
            return None;
        }
    };
    let mut m = wa::Message::default();
    if doc {
        let mut d = wa::message::DocumentMessage::default();
        d.url = Some(up.url);
        d.direct_path = Some(up.direct_path);
        d.media_key = Some(up.media_key.to_vec());
        d.file_enc_sha256 = Some(up.file_enc_sha256.to_vec());
        d.file_sha256 = Some(up.file_sha256.to_vec());
        d.file_length = Some(up.file_length);
        d.media_key_timestamp = Some(up.media_key_timestamp);
        d.mimetype = Some("video/mp4".into());
        d.file_name = Some("video.mp4".into());
        d.caption = Some("Passou do limite de vídeo, vai como arquivo.".into());
        if let Some(t) = thumb {
            d.jpeg_thumbnail = Some(t);
            d.thumbnail_width = Some(320);
            d.thumbnail_height = dims.map(|(w, h, _)| (320 * h / w.max(1)).max(1));
        }
        d.context_info = quote;
        m.document_message = MessageField::some(d);
    } else {
        let mut v = wa::message::VideoMessage::default();
        v.url = Some(up.url);
        v.direct_path = Some(up.direct_path);
        v.media_key = Some(up.media_key.to_vec());
        v.file_enc_sha256 = Some(up.file_enc_sha256.to_vec());
        v.file_sha256 = Some(up.file_sha256.to_vec());
        v.file_length = Some(up.file_length);
        v.media_key_timestamp = Some(up.media_key_timestamp);
        v.mimetype = Some("video/mp4".into());
        v.streaming_sidecar = up.streaming_sidecar;
        v.jpeg_thumbnail = thumb;
        if let Some((w, h, s)) = dims {
            v.width = Some(w);
            v.height = Some(h);
            v.seconds = Some(s);
        }
        v.context_info = quote;
        m.video_message = MessageField::some(v);
    }
    Some(m)
}

/// Foto até aqui entra como imagem; acima disso é descartada (limite do WhatsApp para imagem).
/// shortcut: spec 009 RF5 previa documento; foto > 16 MB é rara, só descarta e registra no Debug.
pub const MAX_IMAGE: u64 = 16 * 1024 * 1024;

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

/// Largura e altura via ffprobe (imagem não tem duração).
async fn probe_dims(path: &Path) -> Option<(u32, u32)> {
    let p = path.to_string_lossy();
    let out = run(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "default=noprint_wrappers=1",
            &p,
        ],
    )
    .await?;
    let s = String::from_utf8_lossy(&out);
    let get = |k: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(k)?.strip_prefix('='))
            .and_then(|v| v.trim().parse::<u32>().ok())
    };
    Some((get("width")?, get("height")?))
}

async fn build_image(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    chat: &str,
    item: &gallery::Item,
    dir: &Path,
    quote: MessageField<wa::ContextInfo>,
) -> Option<wa::Message> {
    use whatsapp_rust::download::MediaType;
    let dims = probe_dims(&item.path).await;
    let thumb = thumbnail(&item.path, dir).await;
    let bytes = tokio::fs::read(&item.path).await.ok()?;
    let up = match ctx
        .client
        .upload(
            bytes,
            MediaType::Image,
            whatsapp_rust::upload::UploadOptions::new(),
        )
        .await
    {
        Ok(u) => u,
        Err(e) => {
            st.dbg("baixar_erro_upload", &e.to_string());
            st.push_event("download_erro_upload", chat);
            return None;
        }
    };
    let mut i = wa::message::ImageMessage::default();
    i.url = Some(up.url);
    i.direct_path = Some(up.direct_path);
    i.media_key = Some(up.media_key.to_vec());
    i.file_enc_sha256 = Some(up.file_enc_sha256.to_vec());
    i.file_sha256 = Some(up.file_sha256.to_vec());
    i.file_length = Some(up.file_length);
    i.media_key_timestamp = Some(up.media_key_timestamp);
    i.mimetype = Some(mime_of(&item.path).into());
    i.jpeg_thumbnail = thumb;
    if let Some((w, h)) = dims {
        i.width = Some(w);
        i.height = Some(h);
    }
    i.context_info = quote;
    let mut m = wa::Message::default();
    m.image_message = MessageField::some(i);
    Some(m)
}

/// Vídeo avulso (fluxo do yt-dlp): cita o comando e envia.
/// Fallback do yt-dlp: baixa fotos/carrossel com o gallery-dl e envia (álbum se houver 2+).
#[allow(clippy::too_many_arguments)]
async fn photos(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
    link: &str,
    plat: super::Platform,
    tmp: &Path,
    timeout: Duration,
) {
    if !gallery::ok() {
        gallery::probe().await;
    }
    let fail = |e: DlError| {
        st.dbg(
            "baixar_erro_gallerydl",
            &format!("{e:?} plataforma {plat:?}"),
        );
        st.push_event("download_erro", chat);
        say(ctx, sent, e.reply())
    };
    if !gallery::ok() {
        return fail(DlError::Missing).await;
    }
    match gallery::fetch(link, plat, &cookies_dir(), tmp, timeout).await {
        Ok(items) => {
            st.dbg("baixar_gallerydl_ok", &format!("{} itens", items.len()));
            send_items(ctx, st, sent, chat, items, tmp).await;
        }
        Err(e) => fail(e).await,
    }
}

async fn send(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
    got: &ytdlp::Fetched,
    tmp: &Path,
) {
    let quote = MessageField::some(ctx.build_quote_context());
    let Some(m) = build_video(ctx, st, chat, got, tmp, quote).await else {
        return say(ctx, sent, "Não consegui enviar").await;
    };
    let doc = as_document(got.size);
    match ctx.send_message(m).await {
        Ok(r) => {
            sent.add(r.message_id.as_str());
            let kind = if doc {
                "baixar_documento"
            } else {
                "baixar_video"
            };
            st.push_event(kind, chat);
            st.dbg(
                "baixar_enviado",
                &format!("{} bytes, documento={doc}", got.size),
            );
        }
        Err(e) => {
            st.dbg("baixar_erro_envio", &e.to_string());
            st.push_event("download_erro_envio", chat);
        }
    }
}

/// Entra no álbum? Foto até 16 MB, vídeo até 64 MB (acima, vai solto como documento).
fn in_album(it: &gallery::Item) -> bool {
    match it.kind {
        gallery::ItemKind::Image => it.size <= MAX_IMAGE,
        gallery::ItemKind::Video => !as_document(it.size),
    }
}

/// Chave da mensagem-pai do álbum (mensagem nossa no chat).
fn album_key(chat: &str, id: &str) -> wa::MessageKey {
    let mut k = wa::MessageKey::default();
    k.remote_jid = Some(chat.into());
    k.from_me = Some(true);
    k.id = Some(id.into());
    k
}

/// Pai do álbum com a contagem esperada de fotos e vídeos.
fn album_parent(images: u32, videos: u32, quote: MessageField<wa::ContextInfo>) -> wa::Message {
    let mut a = wa::message::AlbumMessage::default();
    a.expected_image_count = Some(images);
    a.expected_video_count = Some(videos);
    a.context_info = quote;
    let mut m = wa::Message::default();
    m.album_message = MessageField::some(a);
    m
}

/// Envia fotos e vídeos do post: 1 item sai simples; 2 ou mais saem como álbum (pai + filhos), com
/// pausa entre os envios (P8). Vídeo grande demais para o álbum vai solto depois, como documento.
async fn send_items(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
    items: Vec<gallery::Item>,
    tmp: &Path,
) {
    let (album, loose): (Vec<_>, Vec<_>) = items.into_iter().partition(in_album);
    let imgs = album
        .iter()
        .filter(|i| i.kind == gallery::ItemKind::Image)
        .count() as u32;
    let vids = album.len() as u32 - imgs;
    st.dbg(
        "baixar_album",
        &format!("{imgs} fotos, {vids} vídeos, {} soltos", loose.len()),
    );
    let quote = || MessageField::some(ctx.build_quote_context());
    let grouped = album.len() >= 2;
    // 1) Sobe tudo antes de enviar: assim o álbum chega de uma vez, como quando se anexa várias fotos.
    let mut msgs = Vec::new();
    for (n, it) in album.iter().enumerate() {
        let dir = tmp.join(format!("i{n}"));
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        // Filho do álbum não cita; item único cita o comando.
        let q = if grouped {
            MessageField::none()
        } else {
            quote()
        };
        let m = match it.kind {
            gallery::ItemKind::Image => build_image(ctx, st, chat, it, &dir, q).await,
            gallery::ItemKind::Video => {
                let mut g = ytdlp::Fetched {
                    path: it.path.clone(),
                    size: it.size,
                };
                ensure_compatible(st, &mut g, &dir).await;
                build_video(ctx, st, chat, &g, &dir, q).await
            }
        };
        msgs.extend(m);
    }
    // 2) Uma pausa (P8) e o álbum sai em sequência curta: pai, depois os filhos.
    crate::wa::pause(st).await;
    let mut ok = 0;
    let mut parent_key = None;
    if grouped && !msgs.is_empty() {
        match ctx.send_message(album_parent(imgs, vids, quote())).await {
            Ok(r) => {
                sent.add(r.message_id.as_str());
                parent_key = Some(album_key(chat, r.message_id.as_str()));
            }
            Err(e) => {
                st.dbg("baixar_erro_envio", &e.to_string());
                return;
            }
        }
    }
    for m in msgs {
        let m = match &parent_key {
            Some(k) => whatsapp_rust::proto_helpers::wrap_as_album_child(m, k.clone()),
            None => m,
        };
        match ctx.send_message(m).await {
            Ok(r) => {
                sent.add(r.message_id.as_str());
                ok += 1;
            }
            Err(e) => st.dbg("baixar_erro_envio", &e.to_string()),
        }
        if parent_key.is_some() {
            tokio::time::sleep(crate::wa::pick_delay(150, 250)).await;
        }
    }
    for (n, it) in loose.iter().enumerate() {
        if it.kind != gallery::ItemKind::Video {
            st.dbg("baixar_foto_grande", &format!("{} bytes ignorada", it.size));
            continue;
        }
        crate::wa::pause(st).await;
        let dir = tmp.join(format!("l{n}"));
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let g = ytdlp::Fetched {
            path: it.path.clone(),
            size: it.size,
        };
        if let Some(m) = build_video(ctx, st, chat, &g, &dir, MessageField::none()).await
            && let Ok(r) = ctx.send_message(m).await
        {
            sent.add(r.message_id.as_str());
            ok += 1;
        }
    }
    if ok == 0 {
        return say(ctx, sent, "Não consegui enviar").await;
    }
    st.push_event("baixar_album", chat);
    st.dbg("baixar_enviado", &format!("{ok} itens enviados"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_codec_incompativel() {
        let ok = "codec_name=h264\ncodec_type=video\npix_fmt=yuv420p\ncodec_name=aac\ncodec_type=audio\n";
        assert!(!needs_transcode(ok));
        assert!(needs_transcode(&ok.replace("h264", "av1")));
        assert!(needs_transcode(&ok.replace("yuv420p", "yuv444p")));
        assert!(needs_transcode(&ok.replace("aac", "opus")));
        assert!(!needs_transcode(
            "codec_name=h264\ncodec_type=video\npix_fmt=yuv420p\n"
        ));
    }

    #[test]
    fn parse_cmd_cases() {
        assert_eq!(parse_cmd("/baixar https://x"), Some("https://x"));
        assert_eq!(parse_cmd("/DL"), Some(""));
        assert_eq!(parse_cmd("/download  a b "), Some("a b"));
        assert_eq!(parse_cmd("/baixarx"), None);
        assert_eq!(parse_cmd("baixar"), None);
        assert_eq!(parse_cmd("/s"), None);
    }

    #[test]
    fn limit_probe_and_replies() {
        assert!(!as_document(MAX_VIDEO));
        assert!(as_document(MAX_VIDEO + 1));
        assert_eq!(
            parse_probe("width=576\nheight=1024\nduration=21.4\n"),
            Some((576, 1024, 21))
        );
        assert_eq!(parse_probe("lixo"), None);
        for e in [
            DlError::BadHost,
            DlError::NoUrl,
            DlError::LoginRequired,
            DlError::NotFound,
            DlError::Timeout,
            DlError::Missing,
            DlError::TooBig,
            DlError::Busy,
            DlError::Other,
        ] {
            assert!(!e.reply().is_empty() && !e.reply().starts_with('/'));
        }
    }

    #[test]
    fn album_e_pai() {
        let it = |kind, size| gallery::Item {
            path: "x".into(),
            size,
            kind,
        };
        assert!(in_album(&it(gallery::ItemKind::Image, MAX_IMAGE)));
        assert!(!in_album(&it(gallery::ItemKind::Image, MAX_IMAGE + 1)));
        assert!(in_album(&it(gallery::ItemKind::Video, MAX_VIDEO)));
        assert!(!in_album(&it(gallery::ItemKind::Video, MAX_VIDEO + 1)));
        let k = album_key("123@g.us", "ABC");
        assert_eq!(k.remote_jid.as_deref(), Some("123@g.us"));
        assert_eq!(k.from_me, Some(true));
        assert_eq!(k.id.as_deref(), Some("ABC"));
        let m = album_parent(3, 1, MessageField::none());
        let a = m.album_message.as_option().unwrap();
        assert_eq!(
            (a.expected_image_count, a.expected_video_count),
            (Some(3), Some(1))
        );
        assert_eq!(mime_of(Path::new("a.PNG")), "image/jpeg"); // extensão só em minúsculas (gallery-dl)
        assert_eq!(mime_of(Path::new("a.png")), "image/png");
    }
}
