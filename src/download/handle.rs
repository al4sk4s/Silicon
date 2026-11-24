//! Fluxo do `/baixar`: URL -> limite -> semáforo -> yt-dlp -> envio (spec 004).
use super::{DlError, cookies_dir, url, ytdlp};
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

/// Trata `/baixar`. Falha = uma linha citando o comando.
/// Log só com plataforma, resultado e tamanho (nunca URL nem conteúdo).
pub async fn handle(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    arg: &str,
) {
    let chat = ctx.info.source.chat.to_string();
    let fail = |e: DlError| say(ctx, sent, e.reply());
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
            return say(ctx, sent, "Calma, muitos downloads. Tente daqui a pouco.").await;
        }
        crate::stickers::limit::Verdict::DenySilent => return,
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
    let timeout = Duration::from_secs(st.store.dl_timeout_s().unwrap_or(120));
    let got = match ytdlp::fetch(&link, plat, &cookies_dir(), tmp.path(), timeout).await {
        Ok(g) => g,
        Err(e) => {
            st.push_event("download_erro", &chat);
            return fail(e).await;
        }
    };
    if got.size > MAX_TOTAL {
        return fail(DlError::TooBig).await;
    }
    send(ctx, st, sent, &chat, &got, tmp.path()).await;
    // `tmp` cai aqui e apaga o diretório (RF12), em qualquer caminho acima.
}

async fn send(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
    got: &ytdlp::Fetched,
    tmp: &Path,
) {
    use whatsapp_rust::download::MediaType;
    let doc = as_document(got.size);
    let dims = probe_video(&got.path).await;
    let thumb = thumbnail(&got.path, tmp).await;
    // shortcut: lê o arquivo inteiro em memória (≤ 100 MB), trocar por `upload_stream` se pesar.
    let Ok(bytes) = tokio::fs::read(&got.path).await else {
        return say(ctx, sent, DlError::Other.reply()).await;
    };
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
        Err(_) => {
            st.push_event("download_erro_upload", chat);
            return say(ctx, sent, "Não consegui enviar").await;
        }
    };
    let mut m = wa::Message::default();
    let quote = MessageField::some(ctx.build_quote_context());
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
    match ctx.send_message(m).await {
        Ok(r) => {
            sent.add(r.message_id.as_str());
            let kind = if doc {
                "baixar_documento"
            } else {
                "baixar_video"
            };
            st.push_event(kind, chat);
            tracing::info!(bytes = got.size, doc, "download enviado");
        }
        Err(_) => st.push_event("download_erro_envio", chat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
