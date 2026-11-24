//! T3: conversão via ffmpeg (stdin/stdout para imagem, memfd para vídeo).
use std::{process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt, io::AsyncWriteExt, process::Command, sync::Semaphore, time::timeout,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ConvErr {
    /// Escada esgotada ou timeout: "Mídia pesada demais".
    TooBig,
    /// ffmpeg recusou a entrada: "Formato não suportado".
    Unsupported,
    /// ffmpeg ausente/falha ao subir.
    NoFfmpeg,
}

/// Um ffmpeg por vez.
static SEM: Semaphore = Semaphore::const_new(1);

const STATIC_MAX: usize = 100_000;
const ANIM_MAX: usize = 500_000;

/// (q, fps, segundos, escala)
const ANIM_LADDER: [(u8, u8, u8, u16); 6] = [
    (60, 15, 10, 512),
    (40, 15, 10, 512),
    (40, 12, 8, 512),
    (30, 10, 6, 512),
    (30, 10, 4, 512),
    (25, 8, 4, 384),
];

enum Run {
    Timeout,
    NoFfmpeg,
    Fail,
}

/// Saída por pipe vem com o tamanho RIFF zerado: corrige e confere `RIFF....WEBP`.
fn fix_riff(mut out: Vec<u8>) -> Option<Vec<u8>> {
    if out.len() < 12 || &out[..4] != b"RIFF" || &out[8..12] != b"WEBP" {
        return None;
    }
    let n = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&n.to_le_bytes());
    Some(out)
}

fn filter(side: u16) -> String {
    format!("scale={side}:{side},setsar=1,pad=512:512:-1:-1:color=0x00000000,format=rgba")
}

/// Roda o ffmpeg; `stdin` só para imagem. O stderr não é capturado (nunca logamos conteúdo).
async fn run(
    args: Vec<String>,
    fmt: &str,
    stdin: Option<&[u8]>,
    secs: u64,
) -> Result<Vec<u8>, Run> {
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error"])
        .args(&args)
        .args(["-f", fmt, "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Run::NoFfmpeg
            } else {
                Run::Fail
            }
        })?;
    let mut sin = child.stdin.take().ok_or(Run::Fail)?;
    let mut sout = child.stdout.take().ok_or(Run::Fail)?;
    // Escreve em task separada enquanto lê o stdout (evita deadlock); drop fecha o stdin.
    let data = stdin.map(<[u8]>::to_vec);
    tokio::spawn(async move {
        if let Some(d) = data {
            let _ = sin.write_all(&d).await;
        }
    });
    let work = async {
        let mut out = Vec::new();
        sout.read_to_end(&mut out).await.ok()?;
        let st = child.wait().await.ok()?;
        st.success().then_some(out)
    };
    // Estourou: `child` cai e kill_on_drop mata o processo.
    match timeout(Duration::from_secs(secs), work).await {
        Err(_) => Err(Run::Timeout),
        Ok(None) => Err(Run::Fail),
        Ok(Some(out)) => Ok(out),
    }
}

/// Imagem (PNG/JPEG/WebP...) -> WebP 512x512 estático, <= 100.000 B.
pub async fn to_static(bytes: &[u8]) -> Result<Vec<u8>, ConvErr> {
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    for q in [75, 50, 30] {
        let args = ["-i", "pipe:0", "-frames:v", "1", "-vf"]
            .map(String::from)
            .into_iter()
            .chain([filter(512)])
            .chain(["-c:v", "libwebp", "-lossless", "0", "-q:v"].map(String::from))
            .chain([q.to_string()])
            .collect();
        match run(args, "webp", Some(bytes), 5).await {
            Err(Run::NoFfmpeg) => return Err(ConvErr::NoFfmpeg),
            Err(Run::Fail) => return Err(ConvErr::Unsupported),
            Err(Run::Timeout) => continue,
            Ok(out) => {
                let out = fix_riff(out).ok_or(ConvErr::Unsupported)?;
                if out.len() <= STATIC_MAX {
                    return Ok(out);
                }
            }
        }
    }
    Err(ConvErr::TooBig)
}

/// WebP animado + duração (s) da rung vencedora, usada para cortar o áudio igual.
pub struct Anim {
    pub webp: Vec<u8>,
    pub secs: u32,
}

/// Vídeo/GIF -> WebP animado 512x512, <= 500.000 B. Entrada sempre por `memfd::MemFile`.
pub async fn to_animated(bytes: &[u8]) -> Result<Anim, ConvErr> {
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    // Vivo até o fim: o ffmpeg lê de /proc/self/fd/N (moov no fim não importa).
    let mem = super::memfd::MemFile::new(bytes).map_err(|_| ConvErr::Unsupported)?;
    for (q, fps, secs, side) in ANIM_LADDER {
        let args = vec![
            "-i".into(),
            mem.path(),
            "-an".into(),
            "-t".into(),
            secs.to_string(),
            "-vf".into(),
            format!("fps={fps},{}", filter(side)),
            "-c:v".into(),
            "libwebp".into(),
            "-lossless".into(),
            "0".into(),
            "-compression_level".into(),
            "3".into(),
            "-q:v".into(),
            q.to_string(),
            "-loop".into(),
            "0".into(),
        ];
        match run(args, "webp", None, 15).await {
            Err(Run::NoFfmpeg) => return Err(ConvErr::NoFfmpeg),
            Err(Run::Fail) => return Err(ConvErr::Unsupported),
            Err(Run::Timeout) => continue,
            Ok(out) => {
                let out = fix_riff(out).ok_or(ConvErr::Unsupported)?;
                if out.len() <= ANIM_MAX {
                    return Ok(Anim {
                        webp: out,
                        secs: secs.into(),
                    });
                }
            }
        }
    }
    drop(mem);
    Err(ConvErr::TooBig)
}

/// Primeira trilha de áudio -> Ogg/Opus mono, até `secs` s. Sem áudio -> `Ok(None)`.
pub async fn extract_audio(bytes: &[u8], secs: u32) -> Result<Option<Vec<u8>>, ConvErr> {
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    let mem = super::memfd::MemFile::new(bytes).map_err(|_| ConvErr::Unsupported)?;
    // Sem faixa de áudio o ffmpeg falha ("does not contain any stream"); a mídia já foi lida com
    // sucesso pelo `to_animated`, então falha aqui = sem áudio, não erro (sem ler stderr).
    let probe = ["-i", &mem.path(), "-map", "0:a:0", "-vn", "-t", "0.1"]
        .map(String::from)
        .to_vec();
    match run(probe, "null", None, 10).await {
        Ok(_) => {}
        Err(Run::NoFfmpeg) => return Err(ConvErr::NoFfmpeg),
        Err(Run::Timeout) => return Err(ConvErr::TooBig),
        Err(Run::Fail) => return Ok(None),
    }
    let args = [
        "-i",
        &mem.path(),
        "-map",
        "0:a:0",
        "-vn",
        "-t",
        &secs.to_string(),
        "-c:a",
        "libopus",
        "-b:a",
        "32k",
        "-ac",
        "1",
    ]
    .map(String::from)
    .to_vec();
    match run(args, "ogg", None, 15).await {
        Err(Run::NoFfmpeg) => Err(ConvErr::NoFfmpeg),
        Err(Run::Fail) => Err(ConvErr::Unsupported),
        Err(Run::Timeout) => Err(ConvErr::TooBig),
        Ok(out) => Ok((!out.is_empty()).then_some(out)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_riff_corrige_tamanho_e_valida() {
        let mut v = b"RIFF\0\0\0\0WEBPVP8 xx".to_vec();
        let n = (v.len() - 8) as u32;
        v = fix_riff(v).unwrap();
        assert_eq!(v[4..8], n.to_le_bytes());
        assert!(fix_riff(b"RIFF\0\0\0\0WAVEfmt ".to_vec()).is_none());
        assert!(fix_riff(b"RIFF".to_vec()).is_none());
    }

    async fn ffmpeg_gen(args: &[&str]) -> Vec<u8> {
        let o = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=640x480:rate=15",
                "-t",
                "3",
            ])
            .args(args)
            .arg("pipe:1")
            .output()
            .await
            .unwrap();
        assert!(o.status.success());
        o.stdout
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn png_vira_webp_estatico() {
        let png = ffmpeg_gen(&["-frames:v", "1", "-f", "image2pipe", "-c:v", "png"]).await;
        let out = to_static(&png).await.unwrap();
        assert!(out.len() <= STATIC_MAX);
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        assert!(matches!(&out[12..16], b"VP8 " | b"VP8X" | b"VP8L"));
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg e memfd"]
    async fn video_vira_webp_animado() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-t", "3", "-f", "matroska"]).await;
        let out = to_animated(&mkv).await.unwrap();
        assert!(out.webp.len() <= ANIM_MAX);
        assert!(out.webp.windows(4).any(|w| w == b"ANIM"));
        assert!(out.secs > 0);
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg e memfd"]
    async fn extrai_audio_ogg_ou_none() {
        let com = ffmpeg_gen(&[
            "-f", "lavfi", "-i", "sine=d=3", "-c:v", "mpeg4", "-t", "3", "-f", "matroska",
        ])
        .await;
        let a = extract_audio(&com, 2).await.unwrap().unwrap();
        assert_eq!(&a[..4], b"OggS");
        let sem = ffmpeg_gen(&["-c:v", "mpeg4", "-t", "3", "-f", "matroska"]).await;
        assert_eq!(extract_audio(&sem, 2).await, Ok(None));
    }

    // Caminho real do bot: animado do ffmpeg + EXIF continua animado e carrega o EXIF.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn animado_com_exif() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let out = to_animated(&mkv).await.unwrap();
        let w = crate::stickers::exif::with_exif(&out.webp, "Silicon™", "@al4sk4s").unwrap();
        assert!(whatsapp_rust::webp::is_animated(&w));
        assert!(w.windows(4).any(|x| x == b"EXIF"));
    }
}
