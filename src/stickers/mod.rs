//! Comandos de figurinha (spec 002). Só responde a comando; nunca inicia conversa (P1).
pub mod check;
pub mod convert;
pub mod limit;
pub mod memfd;

use whatsapp_rust::prelude::*;

/// `/s`, `/fig` ou `/figurinha` (sem caixa), seguidos de fim de texto ou espaço.
#[allow(dead_code)] // usado pelo handle (T8)
pub fn parse_cmd(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    ["/s", "/fig", "/figurinha"]
        .iter()
        .any(|c| first.eq_ignore_ascii_case(c))
}

#[allow(dead_code)] // usado pelo handle (T8)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    /// Vídeo ou GIF.
    Video,
}

#[allow(dead_code)] // usado pelo handle (T8)
#[derive(Debug, Clone)]
pub struct Source {
    pub kind: Kind,
    pub view_once: bool,
    /// Mensagem já desembrulhada (base); o download sai dela.
    pub media: wa::Message,
}

#[allow(dead_code)] // usado pelo handle (T8)
#[derive(Debug, PartialEq, Eq)]
pub enum SrcErr {
    NoMedia,
}

/// Fonte da mídia: a mensagem citada, ou a própria (comando na legenda).
#[allow(dead_code)] // usado pelo handle (T8)
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
