//! Comandos de figurinha (spec 002). Só responde a comando; nunca inicia conversa (P1).
pub mod check;
pub mod convert;
pub mod limit;
pub mod memfd;

use whatsapp_rust::prelude::*;

/// `/s`, `/fig` ou `/figurinha` (sem caixa), seguidos de fim de texto ou espaço.
pub fn parse_cmd(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    ["/s", "/fig", "/figurinha"]
        .iter()
        .any(|c| first.eq_ignore_ascii_case(c))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    /// Vídeo ou GIF.
    Video,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub kind: Kind,
    pub view_once: bool,
    /// Mensagem já desembrulhada (base); o download sai dela.
    pub media: wa::Message,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SrcErr {
    NoMedia,
}

/// Fonte da mídia: a mensagem citada, ou a própria (comando na legenda).
pub fn pick_source(msg: &wa::Message) -> Result<Source, SrcErr> {
    let src = msg
        .context_info()
        .and_then(|c| c.quoted_message.as_option())
        .unwrap_or(msg);
    let base = src.get_base_message();
    let kind = if base.image_message.is_set() {
        Kind::Image
    } else if base.video_message.is_set() {
        Kind::Video
    } else {
        return Err(SrcErr::NoMedia);
    };
    Ok(Source {
        kind,
        view_once: src.is_view_once(),
        media: base.clone(),
    })
}

const MAX_IMG: u64 = 10 * 1024 * 1024;
const MAX_VID: u64 = 32 * 1024 * 1024;

/// Mídia -> WebP. Só memória (imagem por pipe, vídeo por memfd); nada vai a disco nem a log.
pub async fn make(kind: Kind, bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    use convert::ConvErr;
    let r = match kind {
        Kind::Image => convert::to_static(bytes).await,
        Kind::Video => convert::to_animated(bytes).await,
    };
    r.map_err(|e| match e {
        ConvErr::TooBig => "Mídia pesada demais",
        ConvErr::Unsupported => "Formato não suportado",
        ConvErr::NoFfmpeg => "ffmpeg ausente",
    })
}

/// Trata `/s`: limite -> ffmpeg -> fonte -> viewOnce -> download -> converte -> envia.
/// Falha vira resposta curta citando o comando. Log só com tipo e resultado (P4/P6).
pub async fn handle(ctx: &MessageContext, st: &crate::state::AppState, sent: &crate::wa::Sent) {
    let chat = ctx.info.source.chat.to_string();
    let reply = |t: &'static str| async move {
        if let Ok(r) = ctx.reply_quoting(t).await {
            sent.add(r.message_id.as_str());
        }
    };
    let max = st.store.sticker_per_min().unwrap_or(3);
    match st
        .sticker_limit
        .check(&chat, std::time::Instant::now(), max)
    {
        limit::Verdict::Allow => {}
        limit::Verdict::DenyFirst => return reply("Calma, muitas figurinhas agora").await,
        limit::Verdict::DenySilent => return,
    }
    if !check::ffmpeg_ok() {
        return reply("ffmpeg ausente").await;
    }
    let Ok(src) = pick_source(&ctx.message) else {
        return reply("Responda uma imagem ou vídeo com /s").await;
    };
    // Antes de baixar qualquer byte (P6).
    if src.view_once && !st.store.viewonce_enabled().unwrap_or(false) {
        return reply("Visualização única desativada").await;
    }
    use whatsapp_rust::download::{DownloadLimits, Downloadable};
    let (max_plain, file_len, dl): (u64, Option<u64>, &dyn Downloadable) = match src.kind {
        Kind::Image => match src.media.image_message.as_option() {
            Some(m) => (MAX_IMG, m.file_length, m),
            None => return reply("Não consegui baixar").await,
        },
        Kind::Video => match src.media.video_message.as_option() {
            Some(m) => (MAX_VID, m.file_length, m),
            None => return reply("Não consegui baixar").await,
        },
    };
    if file_len.is_some_and(|n| n > max_plain) {
        return reply("Mídia pesada demais").await;
    }
    let limits = DownloadLimits::new(max_plain + 65_536, max_plain);
    let Ok(bytes) = ctx.client.download(dl, limits).await else {
        st.push_event("figurinha_erro_download", &chat);
        return reply("Não consegui baixar").await;
    };
    let webp = match make(src.kind, &bytes).await {
        Ok(w) => w,
        Err(m) => {
            st.push_event("figurinha_erro", &chat);
            return reply(m).await;
        }
    };
    drop(bytes); // viewOnce: a mídia original some da memória aqui
    let animated = whatsapp_rust::webp::is_animated(&webp);
    let up = match ctx
        .client
        .upload(
            webp,
            whatsapp_rust::download::MediaType::Sticker,
            whatsapp_rust::upload::UploadOptions::new(),
        )
        .await
    {
        Ok(u) => u,
        Err(_) => {
            st.push_event("figurinha_erro_upload", &chat);
            return reply("Não consegui enviar").await;
        }
    };
    let mut s = wa::message::StickerMessage::default();
    s.url = Some(up.url);
    s.direct_path = Some(up.direct_path);
    s.media_key = Some(up.media_key.to_vec());
    s.file_enc_sha256 = Some(up.file_enc_sha256.to_vec());
    s.file_sha256 = Some(up.file_sha256.to_vec());
    s.file_length = Some(up.file_length);
    s.media_key_timestamp = Some(up.media_key_timestamp);
    s.mimetype = Some("image/webp".into());
    s.width = Some(512);
    s.height = Some(512);
    s.is_animated = Some(animated);
    s.context_info = MessageField::some(ctx.build_quote_context());
    let mut m = wa::Message::default();
    m.sticker_message = MessageField::some(s);
    match ctx.send_message(m).await {
        Ok(r) => {
            sent.add(r.message_id.as_str());
            let kind = if animated { "animada" } else { "estatica" };
            st.push_event(&format!("figurinha_{kind}"), &chat);
        }
        Err(_) => st.push_event("erro_envio", &chat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(view_once: Option<bool>) -> wa::Message {
        let mut i = wa::message::ImageMessage::default();
        i.view_once = view_once;
        let mut m = wa::Message::default();
        m.image_message = MessageField::some(i);
        m
    }

    fn vid(gif: bool) -> wa::Message {
        let mut v = wa::message::VideoMessage::default();
        v.gif_playback = Some(gif);
        let mut m = wa::Message::default();
        m.video_message = MessageField::some(v);
        m
    }

    fn text() -> wa::Message {
        let mut m = wa::Message::default();
        m.conversation = Some("oi".into());
        m
    }

    /// Mensagem de texto `/s` que cita `q`.
    fn quoting(q: wa::Message) -> wa::Message {
        let mut c = wa::ContextInfo::default();
        c.quoted_message = MessageField::some(q);
        let mut e = wa::message::ExtendedTextMessage::default();
        e.text = Some("/s".into());
        e.context_info = MessageField::some(c);
        let mut m = wa::Message::default();
        m.extended_text_message = MessageField::some(e);
        m
    }

    /// Nomes (recursivo) de um diretório, para comparar antes/depois.
    fn tree(dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            out.push(e.path().to_string_lossy().into_owned());
            if e.path().is_dir() {
                tree(&e.path(), out);
            }
        }
    }

    fn snapshot() -> Vec<String> {
        let mut v = Vec::new();
        tree(&std::env::temp_dir(), &mut v);
        tree(std::path::Path::new("/dev/shm"), &mut v);
        v.sort();
        v
    }

    /// Gera mídia de teste em memória (lavfi -> stdout).
    fn media(args: &[&str]) -> Vec<u8> {
        let o = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=10",
                "-t",
                "2",
            ])
            .args(args)
            .arg("pipe:1")
            .output()
            .unwrap();
        assert!(o.status.success());
        o.stdout
    }

    // Zero persistência (RF5): imagem e vídeo não deixam nada em /tmp nem /dev/shm.
    // shortcut: compara a árvore inteira de /tmp; outro processo mexendo lá pode dar falso alarme.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn make_leaves_nothing_on_disk() {
        let png = media(&["-frames:v", "1", "-f", "image2pipe", "-c:v", "png"]);
        let mkv = media(&["-c:v", "mpeg4", "-f", "matroska"]);
        let before = snapshot();
        assert!(make(Kind::Image, &png).await.is_ok());
        assert!(make(Kind::Video, &mkv).await.is_ok());
        assert_eq!(before, snapshot());
    }

    #[test]
    fn parse_cmd_cases() {
        for ok in ["/s", "/FIG", " /figurinha", "/s legenda", "/Fig\tx"] {
            assert!(parse_cmd(ok), "{ok}");
        }
        for no in ["", "s", "/sx", "/figura", "/stickers", "oi /s", "/s/fig"] {
            assert!(!parse_cmd(no), "{no}");
        }
    }

    #[test]
    fn quoted_image() {
        let s = pick_source(&quoting(img(None))).unwrap();
        assert_eq!(s.kind, Kind::Image);
        assert!(!s.view_once);
        assert!(s.media.image_message.is_set());
    }

    #[test]
    fn quoted_video_and_gif() {
        assert_eq!(pick_source(&quoting(vid(false))).unwrap().kind, Kind::Video);
        assert_eq!(pick_source(&quoting(vid(true))).unwrap().kind, Kind::Video);
    }

    #[test]
    fn quoted_view_once_v2_wrapper() {
        let mut w = wa::message::FutureProofMessage::default();
        w.message = MessageField::some(img(None));
        let mut m = wa::Message::default();
        m.view_once_message_v2 = MessageField::some(w);
        let s = pick_source(&quoting(m)).unwrap();
        assert_eq!(s.kind, Kind::Image);
        assert!(s.view_once);
    }

    #[test]
    fn inline_view_once_flag() {
        assert!(pick_source(&quoting(img(Some(true)))).unwrap().view_once);
    }

    #[test]
    fn caption_on_own_image() {
        let s = pick_source(&img(None)).unwrap();
        assert_eq!(s.kind, Kind::Image);
    }

    #[test]
    fn no_media() {
        assert_eq!(pick_source(&text()).unwrap_err(), SrcErr::NoMedia);
        assert_eq!(pick_source(&quoting(text())).unwrap_err(), SrcErr::NoMedia);
    }
}
