//! Comandos de figurinha (spec 002). Só responde a comando; nunca inicia conversa (P1).
pub mod check;
pub mod convert;
pub mod exif;
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

/// Tamanho máximo (em caracteres) de nome e autor.
pub const MAX_FIELD: usize = 64;
/// Variantes de resposta (sorteadas, para não soar robótico).
const CALMA: &[&str] = &[
    "Calma, muitas figurinhas agora",
    "Opa, devagar com as figurinhas!",
    "Ei, respira! Já fiz figurinha demais por agora",
    "Segura a onda, tô fazendo figurinha o mais rápido que dá",
    "Muita figurinha de uma vez, espera um pouquinho",
    "Calma aí, a fábrica de figurinhas precisa de um minuto",
    "Vai com calma, já passou do limite por agora",
    "Pera um pouco, tô sobrecarregado de figurinha",
];
const USO_S: &[&str] = &[
    "Responda uma imagem ou vídeo com /s",
    "Cadê a mídia? Responda uma imagem ou vídeo com /s",
    "Preciso de uma imagem ou vídeo: responda a um com /s",
    "Para fazer figurinha, responda uma imagem ou vídeo com /s",
    "Faltou citar a mídia! Responda uma imagem ou vídeo com /s",
    "Me mostra o que virar figurinha: responda uma imagem ou vídeo com /s",
    "Sem mídia não tem figurinha. Responda uma imagem ou vídeo com /s",
    "Ué, responda uma imagem ou vídeo com /s que eu faço a figurinha",
];

const BAIXAR: &[&str] = &[
    "Não consegui baixar",
    "Deu ruim no download dessa mídia",
    "Não rolou baixar isso, tenta de novo?",
    "Falhei ao baixar a mídia, manda outra vez",
    "O download falhou, tenta de novo daqui a pouco",
    "Não consegui pegar essa mídia",
    "Travou na hora de baixar, tenta de novo",
    "Não deu pra baixar, reenvia a mídia",
];
const PESADA: &[&str] = &[
    "Mídia pesada demais",
    "Essa mídia tá pesada demais pra mim",
    "Arquivo grande demais, manda um menor",
    "Pesado demais! Tenta um arquivo menor",
    "Não aguento esse tamanho, manda algo mais leve",
    "Mídia grande demais pra virar figurinha",
    "Muito pesada, tenta uma mais leve",
    "Esse arquivo passou do limite de tamanho",
];
const ENVIAR: &[&str] = &[
    "Não consegui enviar",
    "Não consegui mandar a figurinha",
    "Falhou o envio, tenta de novo",
    "A figurinha não saiu, tenta outra vez",
    "Deu erro ao enviar, tenta de novo",
    "Travei na hora de mandar, repete o comando",
    "Não rolou enviar, tenta mais uma vez",
    "O envio falhou, tenta de novo daqui a pouco",
];
const NAO_SUPORTADA: &[&str] = &[
    "Figurinha não suportada",
    "Esse tipo de figurinha eu não consigo editar",
    "Não consigo renomear essa figurinha",
    "Figurinha de um formato que não suporto",
    "Essa figurinha não dá pra renomear",
    "Não aceito esse tipo de figurinha",
    "Esse formato de figurinha não é suportado",
    "Não sei lidar com essa figurinha",
];
const VIEWONCE: &[&str] = &[
    "Visualização única não é suportada",
    "Não consigo com mídia de visualização única",
    "Visualização única não rola por aqui",
    "Mídia de visualização única eu não pego",
    "Não dá pra usar visualização única",
    "Visualização única não funciona comigo",
    "Mande uma mídia normal, sem visualização única",
    "Não suporto visualização única",
];
const FFMPEG: &[&str] = &[
    "ffmpeg ausente",
    "Estou sem o ffmpeg, não consigo converter",
    "Falta o ffmpeg aqui, avise o dono",
    "Sem ffmpeg não tem figurinha, avise o dono",
    "O ffmpeg não está disponível agora",
    "Não achei o ffmpeg, chama o dono",
    "Conversor fora do ar (ffmpeg), avise o dono",
    "Sem ffmpeg por aqui, fala com o dono",
];
const RENAME_USO_V: &[&str] = &[
    RENAME_USO,
    "Responda uma figurinha com /rename nome / autor, tá?",
    "Cite a figurinha e mande /rename nome / autor",
    "Faltou citar a figurinha: /rename nome / autor",
    "Pra renomear, responda uma figurinha com /rename nome / autor",
    "Use assim, respondendo a figurinha: /rename nome / autor",
    "Me diga o novo nome e autor: /rename nome / autor, respondendo a figurinha",
    "Responda a figurinha com /rename nome / autor que eu troco",
];
const LIMITE_CAMPO: &[&str] = &[
    "Nome e autor aceitam até 64 caracteres",
    "Máximo de 64 caracteres no nome e no autor",
    "Tá grande! Nome e autor vão até 64 caracteres",
    "Encurta aí: nome e autor têm limite de 64 caracteres",
    "Passou de 64 caracteres no nome ou no autor",
    "Nome e autor não podem passar de 64 caracteres",
    "Limite de 64 caracteres por campo, reduz um pouco",
    "Muito longo: até 64 caracteres no nome e no autor",
];
const FORMATO: &[&str] = &[
    "Formato não suportado",
    "Não conheço esse formato de mídia",
    "Esse formato não rola pra figurinha",
    "Não consegui ler essa mídia, o formato é estranho",
    "Formato esquisito, manda em outro (jpg, png, mp4)",
    "Não abri esse arquivo, tenta outro formato",
    "Esse tipo de arquivo eu não entendo",
    "Formato não aceito, tenta outra mídia",
];
/// Reação rápida ao comando (antes da conversão, que pode levar vários segundos).
const REACOES: &[&str] = &["🚰", "⤵️", "⤴️", "↪️", "↩️", "🔄", "🔃", "🔜"];
const GRUPOS: &[&[&str]] = &[
    FORMATO,
    CALMA,
    USO_S,
    BAIXAR,
    PESADA,
    ENVIAR,
    NAO_SUPORTADA,
    VIEWONCE,
    FFMPEG,
    RENAME_USO_V,
    LIMITE_CAMPO,
];

/// Troca a mensagem por uma variante sorteada do mesmo grupo; sem grupo, devolve como veio.
fn vary(t: &str) -> &str {
    GRUPOS
        .iter()
        .find(|g| g.contains(&t))
        .map_or(t, |g| variante(g))
}

pub(crate) fn variante(opcoes: &'static [&'static str]) -> &'static str {
    use rand::Rng;
    opcoes[rand::rng().random_range(0..opcoes.len())]
}

const RENAME_USO: &str = "Responda uma figurinha com /rename nome / autor";

/// `/rename nome / autor`. None se não for o comando; Err(resposta curta) se estiver mal formado.
/// `/rename /` zera os dois. Sem `/` é erro (o autor não é opcional); só a 1ª `/` separa.
pub fn parse_rename(text: &str) -> Option<Result<(String, String), &'static str>> {
    let t = text.trim();
    let (first, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    if !first.eq_ignore_ascii_case("/rename") {
        return None;
    }
    let Some((name, publisher)) = rest.split_once('/') else {
        return Some(Err(RENAME_USO));
    };
    let (name, publisher) = (name.trim(), publisher.trim());
    if name.chars().count() > MAX_FIELD || publisher.chars().count() > MAX_FIELD {
        return Some(Err("Nome e autor aceitam até 64 caracteres"));
    }
    Some(Ok((name.to_string(), publisher.to_string())))
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
pub async fn make(kind: Kind, bytes: &[u8], malvin: bool) -> Result<Vec<u8>, &'static str> {
    use convert::ConvErr;
    let r = match kind {
        Kind::Image => convert::to_static(bytes).await,
        Kind::Video => convert::to_animated(bytes, malvin)
            .await
            .map(|(b, fps, q)| {
                tracing::info!(fps, q, "figurinha animada"); // RFA3: sem conteúdo
                b
            }),
    };
    r.map_err(|e| match e {
        ConvErr::TooBig => "Mídia pesada demais",
        ConvErr::Unsupported => FORMATO[0],
        ConvErr::NoFfmpeg => "ffmpeg ausente",
    })
}

async fn reply(ctx: &MessageContext, sent: &crate::wa::Sent, t: &str) {
    if let Ok(r) = ctx.reply_quoting(vary(t)).await {
        sent.add(r.message_id.as_str());
    }
}

/// Limite por chat (`/s` e `/rename` dividem o balde). false = já respondeu ou calou.
async fn allowed(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
) -> bool {
    let max = st.store.sticker_per_min().unwrap_or(3);
    match st.sticker_limit.check(chat, std::time::Instant::now(), max) {
        limit::Verdict::Allow => true,
        limit::Verdict::DenyFirst => {
            reply(ctx, sent, variante(CALMA)).await;
            false
        }
        limit::Verdict::DenySilent => false,
    }
}

/// Trata `/s`: limite -> ffmpeg -> fonte -> viewOnce -> download -> converte -> envia.
/// Falha vira resposta curta citando o comando. Log só com tipo e resultado (P4/P6).
pub async fn handle(ctx: &MessageContext, st: &crate::state::AppState, sent: &crate::wa::Sent) {
    let chat = ctx.info.source.chat.to_string();
    let reply = |t: &'static str| reply(ctx, sent, t);
    if !allowed(ctx, st, sent, &chat).await {
        return;
    }
    // o probe do boot falhou sem causa achada (intermitente); tenta de novo antes de desistir
    if !check::ffmpeg_ok() {
        check::probe().await;
    }
    if !check::ffmpeg_ok() {
        return reply("ffmpeg ausente").await;
    }
    let Ok(src) = pick_source(&ctx.message) else {
        return reply(variante(USO_S)).await;
    };
    // Não suportado: a citação vem sem media_key (T10); nem tenta baixar.
    if src.view_once {
        return reply("Visualização única não é suportada").await;
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
    // Resposta visual imediata (já passou o atraso P8); a figurinha vem depois. Falha é ignorada.
    if let Ok(r) = ctx.react(variante(REACOES)).await {
        sent.add(r.message_id.as_str());
    }
    let t0 = std::time::Instant::now();
    let limits = DownloadLimits::new(max_plain + 65_536, max_plain);
    let bytes = match ctx.client.download(dl, limits).await {
        Ok(b) => b,
        Err(e) => {
            // só o erro da lib (sem mídia nem JID); ajuda a achar a causa
            tracing::warn!(view_once = src.view_once, "download falhou: {e}");
            st.push_event("figurinha_erro_download", &chat);
            return reply("Não consegui baixar").await;
        }
    };
    let dl_ms = t0.elapsed().as_millis();
    let malvin = st.store.video_sticker_mode().is_ok_and(|m| m == "malvin");
    let t1 = std::time::Instant::now();
    let webp = match make(src.kind, &bytes, malvin).await {
        Ok(w) => w,
        Err(m) => {
            st.push_event("figurinha_erro", &chat);
            return reply(m).await;
        }
    };
    // Só tempos e tipo (sem conteúdo): mostra onde está a demora.
    tracing::info!(
        video = src.kind == Kind::Video,
        dl_ms,
        conv_ms = t1.elapsed().as_millis(),
        "figurinha: etapas"
    );
    drop(bytes); // viewOnce: a mídia original some da memória aqui
    // EXIF (nome/autor) é enfeite: se falhar, vai o WebP sem ele.
    let (name, publisher) = (
        st.store.sticker_pack_name().unwrap_or_default(),
        st.store.sticker_publisher().unwrap_or_default(),
    );
    let webp = exif::with_exif(&webp, &name, &publisher).unwrap_or_else(|e| {
        tracing::warn!("EXIF não aplicado: {e}");
        webp
    });
    send_sticker(ctx, st, sent, &chat, webp).await;
}

const MAX_STK: u64 = 2 * 1024 * 1024;

/// Figurinha citada pelo `/rename`; Err = resposta pronta. Separada de `handle_rename` porque
/// o `MessageContext` não se monta em teste (exigiria o feature `test-support`, proibido).
fn rename_target(msg: &wa::Message) -> Result<&wa::message::StickerMessage, &'static str> {
    let quoted = msg
        .context_info()
        .and_then(|c| c.quoted_message.as_option())
        .map(|q| q.get_base_message());
    let Some(stk) = quoted.and_then(|q| q.sticker_message.as_option()) else {
        return Err(RENAME_USO);
    };
    if stk.file_length.is_some_and(|n| n > MAX_STK) {
        return Err("Mídia pesada demais");
    }
    Ok(stk)
}

/// `/rename nome / autor` citando uma figurinha: baixa, troca o EXIF e reenvia (RF11/RF12).
pub async fn handle_rename(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    args: Result<(String, String), &'static str>,
) {
    let chat = ctx.info.source.chat.to_string();
    if !allowed(ctx, st, sent, &chat).await {
        return;
    }
    let (name, publisher) = match args {
        Ok(a) => a,
        Err(m) => return reply(ctx, sent, m).await,
    };
    let stk = match rename_target(&ctx.message) {
        Ok(s) => s,
        Err(m) => return reply(ctx, sent, m).await,
    };
    use whatsapp_rust::download::DownloadLimits;
    let limits = DownloadLimits::new(MAX_STK + 65_536, MAX_STK);
    let bytes = match ctx.client.download(stk, limits).await {
        Ok(b) => b,
        Err(e) => {
            // se a citação vier sem media_key (como o viewOnce), cai aqui: ver spec
            tracing::warn!("download da figurinha citada falhou: {e}");
            st.push_event("figurinha_erro_download", &chat);
            return reply(ctx, sent, "Não consegui baixar").await;
        }
    };
    match exif::with_exif(&bytes, &name, &publisher) {
        Ok(w) => send_sticker(ctx, st, sent, &chat, w).await,
        Err(_) => reply(ctx, sent, "Figurinha não suportada").await,
    }
}

/// Sobe a figurinha WebP e responde citando o comando.
async fn send_sticker(
    ctx: &MessageContext,
    st: &crate::state::AppState,
    sent: &crate::wa::Sent,
    chat: &str,
    webp: Vec<u8>,
) {
    let reply = |t: &'static str| reply(ctx, sent, t);
    let animated = whatsapp_rust::webp::is_animated(&webp);
    let t_up = std::time::Instant::now();
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
            st.push_event("figurinha_erro_upload", chat);
            return reply("Não consegui enviar").await;
        }
    };
    tracing::info!(up_ms = t_up.elapsed().as_millis(), "figurinha: upload");
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
            st.push_event(&format!("figurinha_{kind}"), chat);
        }
        Err(_) => st.push_event("erro_envio", chat),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ha_8_reacoes() {
        assert_eq!(super::REACOES.len(), 8);
    }

    #[test]
    fn variantes_nao_vazias() {
        assert!(super::CALMA.len() >= 5 && super::USO_S.len() >= 5);
        assert!(super::USO_S.iter().all(|t| t.contains("/s")));
        assert!(super::GRUPOS.iter().all(|g| g.len() >= 8));
        assert!(super::LIMITE_CAMPO.iter().all(|t| t.contains("64")));
        assert!(
            super::RENAME_USO_V
                .iter()
                .all(|t| t.contains("/rename nome / autor"))
        );
        assert!(super::GRUPOS.contains(&super::CALMA));
        assert!(super::CALMA.contains(&super::variante(super::CALMA)));
    }

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

    /// Nomes do 1º nível de um diretório, para comparar antes/depois.
    fn tree(dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            // `silicon-*`: outros testes (em paralelo) e o bot de teste criam isso aqui
            if !e.file_name().to_string_lossy().starts_with("silicon-") {
                out.push(e.path().to_string_lossy().into_owned());
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
    // shortcut: compara só o 1º nível de /tmp e /dev/shm (a árvore inteira pegava o bot de
    // teste em /tmp/silicon-*); arquivo novo de outro processo ali ainda dá falso alarme.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn make_leaves_nothing_on_disk() {
        let png = media(&["-frames:v", "1", "-f", "image2pipe", "-c:v", "png"]);
        let mkv = media(&["-c:v", "mpeg4", "-f", "matroska"]);
        let before = snapshot();
        assert!(make(Kind::Image, &png, false).await.is_ok());
        assert!(make(Kind::Video, &mkv, false).await.is_ok());
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
    fn parse_rename_cases() {
        let ok = |t: &str| parse_rename(t).unwrap().unwrap();
        assert_eq!(
            ok("/rename Meu pack / Eu"),
            ("Meu pack".into(), "Eu".into())
        );
        assert_eq!(ok("/RENAME a/b"), ("a".into(), "b".into()));
        assert_eq!(ok("/rename /"), ("".into(), "".into()));
        assert_eq!(ok("/rename  / só autor"), ("".into(), "só autor".into()));
        assert_eq!(ok("/rename a / b / c"), ("a".into(), "b / c".into()));
        assert!(parse_rename("/rename só nome").unwrap().is_err());
        assert!(parse_rename("/rename").unwrap().is_err());
        let longo = format!("/rename {} / x", "a".repeat(65));
        assert!(parse_rename(&longo).unwrap().is_err());
        let limite = format!("/rename {} / x", "a".repeat(64));
        assert!(parse_rename(&limite).unwrap().is_ok());
        for no in ["", "/renamex a/b", "/s", "oi /rename a/b"] {
            assert!(parse_rename(no).is_none(), "{no}");
        }
    }

    #[test]
    fn rename_target_cases() {
        let stk = |len| {
            let mut s = wa::message::StickerMessage::default();
            s.file_length = len;
            let mut m = wa::Message::default();
            m.sticker_message = MessageField::some(s);
            quoting(m)
        };
        assert!(rename_target(&stk(Some(1000))).is_ok());
        assert!(rename_target(&stk(None)).is_ok());
        assert_eq!(
            rename_target(&stk(Some(MAX_STK + 1))).unwrap_err(),
            "Mídia pesada demais"
        );
        assert_eq!(rename_target(&quoting(img(None))).unwrap_err(), RENAME_USO);
        assert_eq!(
            rename_target(&wa::Message::default()).unwrap_err(),
            RENAME_USO
        );
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
