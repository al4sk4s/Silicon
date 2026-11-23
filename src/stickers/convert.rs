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

/// (fps, q): primeiro baixa a qualidade, depois o fps. Duração (10 s) e 512x512 nunca mudam.
/// Medido com um vídeo real de 10 s e muito movimento (23/11/2026): 15 fps q0 ≈ 480 KB, 12 fps q0 ≈ 385 KB;
/// vídeo assim só cabe nos degraus de baixo (borrado); vídeo calmo para nos de cima.
const ANIM_LADDER: [(u8, u8); 9] = [
    (20, 50),
    (20, 25),
    (15, 40),
    (15, 20),
    (15, 5),
    (15, 0),
    (12, 0),
    (10, 0),
    (8, 0),
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

/// Estica para o quadrado (sem barras nem encolher): a tela é sempre 512x512.
const FILTER: &str = "scale=512:512,setsar=1,format=rgba";

/// Roda o ffmpeg; `stdin` só para imagem. O stderr não é capturado (nunca logamos conteúdo).
async fn run(
    args: Vec<String>,
    stdin: Option<&[u8]>,
    secs: u64,
    fmt: &str,
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
            .chain([FILTER.to_string()])
            .chain(["-c:v", "libwebp", "-lossless", "0", "-q:v"].map(String::from))
            .chain([q.to_string()])
            .collect();
        match run(args, Some(bytes), 5, "webp").await {
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

/// Vídeo/GIF -> WebP animado 512x512, <= 500.000 B. Entrada sempre por `memfd::MemFile`.
/// Devolve também (fps, q) usados, para o log.
pub async fn to_animated(bytes: &[u8]) -> Result<(Vec<u8>, u8, u8), ConvErr> {
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    // Vivo até o fim: o ffmpeg lê de /proc/self/fd/N (moov no fim não importa).
    let mem = super::memfd::MemFile::new(bytes).map_err(|_| ConvErr::Unsupported)?;
    for (fps, q) in ANIM_LADDER {
        let args = vec![
            "-i".into(),
            mem.path(),
            "-an".into(),
            "-t".into(),
            "10".into(),
            "-vf".into(),
            format!("fps={fps},{FILTER}"),
            "-c:v".into(),
            "libwebp".into(),
            "-lossless".into(),
            "0".into(),
            "-compression_level".into(),
            "4".into(),
            "-q:v".into(),
            q.to_string(),
            "-loop".into(),
            "0".into(),
        ];
        match run(args, None, 25, "webp").await {
            Err(Run::NoFfmpeg) => return Err(ConvErr::NoFfmpeg),
            Err(Run::Fail) => return Err(ConvErr::Unsupported),
            Err(Run::Timeout) => continue,
            Ok(out) => {
                let out = fix_riff(out).ok_or(ConvErr::Unsupported)?;
                if out.len() <= ANIM_MAX {
                    return Ok((out, fps, q));
                }
            }
        }
    }
    drop(mem);
    Err(ConvErr::TooBig)
}

/// Corta um fluxo de JPEGs concatenados (cada um começa em FF D8 FF; dentro dos dados o FF vem como FF 00).
fn split_jpegs(d: &[u8]) -> Vec<Vec<u8>> {
    let starts: Vec<usize> = (0..d.len().saturating_sub(2))
        .filter(|&i| d[i..i + 3] == [0xFF, 0xD8, 0xFF])
        .collect();
    (0..starts.len())
        .map(|k| d[starts[k]..*starts.get(k + 1).unwrap_or(&d.len())].to_vec())
        .collect()
}

/// Vídeo/GIF -> frames JPEG 512x512 (10 fps, até 10 s) para o `.was`. Entrada por memfd.
pub async fn to_frames(bytes: &[u8]) -> Result<Vec<Vec<u8>>, ConvErr> {
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    let mem = super::memfd::MemFile::new(bytes).map_err(|_| ConvErr::Unsupported)?;
    let args = ["-i", &mem.path(), "-an", "-t", "10", "-vf"]
        .map(String::from)
        .into_iter()
        .chain([
            format!("fps=10,{FILTER}"),
            "-c:v".into(),
            "mjpeg".into(),
            "-q:v".into(),
            "8".into(),
        ])
        .collect();
    match run(args, None, 20, "image2pipe").await {
        Err(Run::NoFfmpeg) => Err(ConvErr::NoFfmpeg),
        Err(Run::Fail) => Err(ConvErr::Unsupported),
        Err(Run::Timeout) => Err(ConvErr::TooBig),
        Ok(out) => Ok(split_jpegs(&out)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn split_jpegs_separa_por_soi() {
        let d = [0xFF, 0xD8, 0xFF, 1, 0xFF, 0, 2, 0xFF, 0xD8, 0xFF, 3];
        let v = super::split_jpegs(&d);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1], [0xFF, 0xD8, 0xFF, 3]);
        assert!(super::split_jpegs(&[1, 2]).is_empty());
    }

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

    #[test]
    fn escada_so_mexe_em_fps_e_q() {
        // 512x512 e 10 s são fixos; cada fps desce o q antes de o fps cair.
        assert!(FILTER.starts_with("scale=512:512") && !FILTER.contains("pad"));
        for w in ANIM_LADDER.windows(2) {
            assert!(w[1].0 < w[0].0 || (w[1].0 == w[0].0 && w[1].1 < w[0].1));
        }
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

    /// Vídeo real do dono: `VIDEO_TESTE=/caminho.mp4 cargo test -j1 video_real -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg e VIDEO_TESTE"]
    async fn video_real_cabe_inteiro() {
        let b = std::fs::read(std::env::var("VIDEO_TESTE").unwrap()).unwrap();
        let (out, fps, q) = to_animated(&b).await.unwrap();
        eprintln!("fps={fps} q={q} bytes={}", out.len());
        assert!(out.len() <= ANIM_MAX);
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg e memfd"]
    async fn video_vira_frames_jpeg_e_was() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let f = to_frames(&mkv).await.unwrap();
        assert!(f.len() >= 25, "3 s a 10 fps: {}", f.len()); // duração inteira, não pedaço
        let w = crate::stickers::was::build(&f, "image/jpeg", 10, "a", "b").unwrap();
        assert!(w.len() < 3_000_000, "tamanho do .was: {}", w.len());
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg e memfd"]
    async fn video_vira_webp_animado() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let (out, _, _) = to_animated(&mkv).await.unwrap();
        assert!(out.len() <= ANIM_MAX);
        assert!(out.windows(4).any(|w| w == b"ANIM"));
    }

    // Caminho real do bot: animado do ffmpeg + EXIF continua animado e carrega o EXIF.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn animado_com_exif() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let (out, _, _) = to_animated(&mkv).await.unwrap();
        let w = crate::stickers::exif::with_exif(&out, "Silicon™", "@al4sk4s").unwrap();
        assert!(whatsapp_rust::webp::is_animated(&w));
        assert!(w.windows(4).any(|x| x == b"EXIF"));
    }
}
