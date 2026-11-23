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

/// Modo "malvin" (como o bot MALVIN-XD): baixa para 320 px e sobe para 512 (canvas 512x512 mantido),
/// 15 fps, qualidade alta primeiro; mais leve e mais borrado. Mesma regra: 10 s inteiros.
const MALVIN_LADDER: [(u8, u8); 7] = [
    (15, 75),
    (15, 50),
    (15, 30),
    (15, 10),
    (15, 0),
    (12, 0),
    (10, 0),
];
const MALVIN_FILTER: &str = "scale=320:320,scale=512:512,setsar=1,format=rgba";

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

/// Uma tentativa da escada. `Some(bytes)` se coube no teto; timeout conta como "não coube".
async fn attempt(
    path: String,
    fps: u8,
    q: u8,
    filter: &'static str,
) -> Result<Option<Vec<u8>>, ConvErr> {
    let args = vec![
        "-i".into(),
        path,
        "-an".into(),
        "-t".into(),
        "10".into(),
        "-vf".into(),
        format!("fps={fps},{filter}"),
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
        Err(Run::NoFfmpeg) => Err(ConvErr::NoFfmpeg),
        Err(Run::Fail) => Err(ConvErr::Unsupported),
        Err(Run::Timeout) => Ok(None),
        Ok(out) => {
            let out = fix_riff(out).ok_or(ConvErr::Unsupported)?;
            Ok((out.len() <= ANIM_MAX).then_some(out))
        }
    }
}

/// Roda os degraus `idxs` ao mesmo tempo (o encoder WebP usa ~1 núcleo por processo).
async fn round(
    path: &str,
    ladder: &'static [(u8, u8)],
    filter: &'static str,
    idxs: &[usize],
) -> Result<Vec<Option<Vec<u8>>>, ConvErr> {
    let tasks: Vec<_> = idxs
        .iter()
        .map(|&i| {
            let (fps, q) = ladder[i];
            tokio::spawn(attempt(path.to_string(), fps, q, filter))
        })
        .collect();
    let mut out = Vec::new();
    for t in tasks {
        out.push(t.await.map_err(|_| ConvErr::NoFfmpeg)??);
    }
    Ok(out)
}

/// Vídeo/GIF -> WebP animado 512x512, <= 500.000 B. Entrada sempre por `memfd::MemFile`.
/// Devolve também (fps, q) usados, para o log.
/// Busca em até 2 rodadas de 3 ffmpeg em paralelo: sonda o topo, o meio e o fim da escada e,
/// se o topo não coube, refina entre a sonda anterior e a primeira que coube (vale o degrau de
/// melhor qualidade que coube, igual à descida um a um, só que ~3x mais rápida).
pub async fn to_animated(bytes: &[u8], malvin: bool) -> Result<(Vec<u8>, u8, u8), ConvErr> {
    let (ladder, filter): (&'static [(u8, u8)], &'static str) = if malvin {
        (&MALVIN_LADDER, MALVIN_FILTER)
    } else {
        (&ANIM_LADDER, FILTER)
    };
    let _g = SEM.acquire().await.map_err(|_| ConvErr::NoFfmpeg)?;
    // Vivo até o fim: o ffmpeg lê de /proc/self/fd/N (moov no fim não importa).
    let mem = super::memfd::MemFile::new(bytes).map_err(|_| ConvErr::Unsupported)?;
    let path = mem.path();
    let n = ladder.len();
    let probes = [0, (n - 1) / 2, n - 1];
    let mut res = round(&path, ladder, filter, &probes).await?;
    let k = res
        .iter()
        .position(Option::is_some)
        .ok_or(ConvErr::TooBig)?;
    let mut pick = (probes[k], res[k].take().ok_or(ConvErr::TooBig)?);
    if k > 0 {
        let between: Vec<usize> = (probes[k - 1] + 1..probes[k]).collect();
        let found = round(&path, ladder, filter, &between).await?;
        if let Some((j, out)) = found
            .into_iter()
            .enumerate()
            .find_map(|(j, o)| o.map(|o| (j, o)))
        {
            pick = (between[j], out);
        }
    }
    drop(mem);
    let (fps, q) = ladder[pick.0];
    Ok((pick.1, fps, q))
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

    #[test]
    fn escada_so_mexe_em_fps_e_q() {
        // 512x512 e 10 s são fixos; cada fps desce o q antes de o fps cair.
        assert!(FILTER.starts_with("scale=512:512") && !FILTER.contains("pad"));
        for w in ANIM_LADDER.windows(2) {
            assert!(w[1].0 < w[0].0 || (w[1].0 == w[0].0 && w[1].1 < w[0].1));
        }
    }

    #[test]
    fn sondas_cobrem_a_escada() {
        // 3 sondas (topo, meio, fim) deixam no máximo 3 degraus por rodada de refino.
        for n in [ANIM_LADDER.len(), MALVIN_LADDER.len()] {
            let p = [0, (n - 1) / 2, n - 1];
            assert!(p.windows(2).all(|w| w[1] - w[0] - 1 <= 3));
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
        let malvin = std::env::var("MODO_MALVIN").is_ok();
        let (out, fps, q) = to_animated(&b, malvin).await.unwrap();
        eprintln!("fps={fps} q={q} bytes={}", out.len());
        assert!(out.len() <= ANIM_MAX);
    }

    #[tokio::test]
    #[ignore = "precisa de ffmpeg e memfd"]
    async fn video_vira_webp_animado() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let (out, _, _) = to_animated(&mkv, false).await.unwrap();
        assert!(out.len() <= ANIM_MAX);
        assert!(out.windows(4).any(|w| w == b"ANIM"));
    }

    // Caminho real do bot: animado do ffmpeg + EXIF continua animado e carrega o EXIF.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn animado_com_exif() {
        let mkv = ffmpeg_gen(&["-c:v", "mpeg4", "-f", "matroska"]).await;
        let (out, _, _) = to_animated(&mkv, false).await.unwrap();
        let w = crate::stickers::exif::with_exif(&out, "Silicon™", "@al4sk4s").unwrap();
        assert!(whatsapp_rust::webp::is_animated(&w));
        assert!(w.windows(4).any(|x| x == b"EXIF"));
    }
}
