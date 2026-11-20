//! Nome e autor da figurinha (spec 002, RF10): chunk `EXIF` dentro do RIFF/WebP.
//! Formato da comunidade (não documentado pelo WhatsApp): TIFF little-endian, tag 0x5741, JSON.

const PACK_ID: &str = "silicon";

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn exif_payload(name: &str, publisher: &str) -> Vec<u8> {
    let json = format!(
        "{{\"sticker-pack-id\":{},\"sticker-pack-name\":{},\"sticker-pack-publisher\":{}}}",
        json_str(PACK_ID),
        json_str(name),
        json_str(publisher)
    );
    let mut out = vec![
        0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, // "II*\0", IFD em 8
        0x01, 0x00, // 1 entrada
        0x41, 0x57, 0x07, 0x00, // tag 0x5741, tipo UNDEFINED
    ];
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(0x16u32.to_le_bytes()); // dados logo após o IFD
    out.extend(json.as_bytes());
    out
}

/// Cabeçalho VP8X para um WebP simples (`VP8 `/`VP8L`); None se não der para ler as dimensões.
fn vp8x_from_simple(chunks: &[(&[u8], &[u8])]) -> Option<Vec<u8>> {
    let (id, d) = chunks.first()?;
    let (w, h, alpha) = match *id {
        b"VP8 " if d.len() >= 10 && d[3..6] == [0x9d, 0x01, 0x2a] => (
            u32::from(u16::from_le_bytes([d[6], d[7]]) & 0x3fff),
            u32::from(u16::from_le_bytes([d[8], d[9]]) & 0x3fff),
            false,
        ),
        b"VP8L" if d.len() >= 5 && d[0] == 0x2f => {
            let b = u32::from_le_bytes([d[1], d[2], d[3], d[4]]);
            ((b & 0x3fff) + 1, ((b >> 14) & 0x3fff) + 1, b >> 28 & 1 == 1)
        }
        _ => return None,
    };
    let mut v = vec![if alpha { 0x10 } else { 0 }, 0, 0, 0];
    v.extend(&(w - 1).to_le_bytes()[..3]);
    v.extend(&(h - 1).to_le_bytes()[..3]);
    Some(v)
}

/// Devolve o WebP com o chunk `EXIF` (nome/autor) no lugar de qualquer EXIF anterior.
/// Só mexe em cabeçalhos: os bytes da imagem/animação ficam iguais.
pub fn with_exif(webp: &[u8], name: &str, publisher: &str) -> Result<Vec<u8>, &'static str> {
    if webp.len() < 20 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return Err("não é WebP");
    }
    let mut chunks: Vec<(&[u8], &[u8])> = Vec::new();
    let mut off = 12;
    while off + 8 <= webp.len() {
        let len = u32::from_le_bytes(webp[off + 4..off + 8].try_into().unwrap()) as usize;
        let end = off.checked_add(8 + len).ok_or("WebP truncado")?;
        if end > webp.len() {
            return Err("WebP truncado");
        }
        if &webp[off..off + 4] != b"EXIF" {
            chunks.push((&webp[off..off + 4], &webp[off + 8..end]));
        }
        off = end + (len & 1);
    }
    let mut vp8x = match chunks.first() {
        Some((b"VP8X", d)) if d.len() >= 10 => d.to_vec(),
        _ => vp8x_from_simple(&chunks).ok_or("WebP não reconhecido")?,
    };
    vp8x[0] |= 0x08; // bit EXIF
    let exif = exif_payload(name, publisher);
    let mut body = b"WEBP".to_vec();
    let skip = usize::from(matches!(chunks.first(), Some((b"VP8X", _))));
    for (id, d) in std::iter::once((&b"VP8X"[..], &vp8x[..]))
        .chain(chunks[skip..].iter().copied())
        .chain(std::iter::once((&b"EXIF"[..], &exif[..])))
    {
        body.extend(id);
        body.extend((d.len() as u32).to_le_bytes());
        body.extend(d);
        if d.len() & 1 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u24(b: &[u8]) -> u32 {
        u32::from_le_bytes([b[0], b[1], b[2], 0])
    }

    fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for (id, d) in chunks {
            body.extend(*id);
            body.extend((d.len() as u32).to_le_bytes());
            body.extend(d);
            if d.len() & 1 == 1 {
                body.push(0);
            }
        }
        let mut o = b"RIFF".to_vec();
        o.extend((body.len() as u32).to_le_bytes());
        o.extend(body);
        o
    }

    fn find<'a>(w: &'a [u8], want: &[u8; 4]) -> Option<&'a [u8]> {
        let mut off = 12;
        while off + 8 <= w.len() {
            let len = u32::from_le_bytes(w[off + 4..off + 8].try_into().unwrap()) as usize;
            if &w[off..off + 4] == want {
                return Some(&w[off + 8..off + 8 + len]);
            }
            off += 8 + len + (len & 1);
        }
        None
    }

    // VP8 de mentira: tag(3) + start code + 512x512
    fn vp8() -> Vec<u8> {
        vec![0, 0, 0, 0x9d, 0x01, 0x2a, 0x00, 0x02, 0x00, 0x02, 7]
    }

    #[test]
    fn simple_webp_gets_vp8x_and_exif() {
        let w = with_exif(&riff(&[(b"VP8 ", vp8())]), "Silicon™", "@al4sk4s \"x\"").unwrap();
        let x = find(&w, b"VP8X").unwrap();
        assert_eq!(x[0] & 0x08, 0x08);
        assert_eq!(u24(&x[4..7]), 511);
        assert_eq!(u24(&x[7..10]), 511);
        assert_eq!(find(&w, b"VP8 ").unwrap(), &vp8()[..]);
        let e = find(&w, b"EXIF").unwrap();
        assert_eq!(&e[..4], b"II*\0");
        let n = u32::from_le_bytes(e[14..18].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&e[22..]).unwrap();
        assert_eq!(json.len(), n);
        assert!(json.contains("\"sticker-pack-name\":\"Silicon™\""));
        assert!(json.contains("\"sticker-pack-publisher\":\"@al4sk4s \\\"x\\\"\""));
        // RIFF consistente
        let riff_len = u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff_len + 8, w.len());
    }

    #[test]
    fn animated_keeps_chunks_and_replaces_old_exif() {
        let mut x = vec![0x02, 0, 0, 0];
        x.extend([0xff, 0x01, 0, 0xff, 0x01, 0]);
        let src = riff(&[
            (b"VP8X", x),
            (b"ANIM", vec![0; 6]),
            (b"ANMF", vec![1; 17]),
            (b"EXIF", b"velho".to_vec()),
        ]);
        let w = with_exif(&src, "a", "b").unwrap();
        let x = find(&w, b"VP8X").unwrap();
        assert_eq!(x[0], 0x0a); // ANIM + EXIF
        assert!(whatsapp_rust::webp::is_animated(&w));
        assert_eq!(find(&w, b"ANMF").unwrap().len(), 17);
        assert!(!w.windows(5).any(|s| s == b"velho"));
        // trocar de novo (rename) não duplica o chunk
        let w2 = with_exif(&w, "", "").unwrap();
        assert_eq!(w2.windows(4).filter(|s| *s == b"EXIF").count(), 1);
    }

    // WebP real do ffmpeg (estático e animado) continua decodificável depois do EXIF.
    #[tokio::test]
    #[ignore = "precisa de ffmpeg"]
    async fn real_ffmpeg_output_still_decodes() {
        use tokio::io::AsyncWriteExt;
        for (args, anim) in [(&["-frames:v", "1"][..], false), (&["-t", "1"][..], true)] {
            let o = std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=64x64:rate=10",
                ])
                .args(args)
                .args(["-c:v", "libwebp_anim", "-f", "webp", "pipe:1"])
                .output()
                .unwrap();
            assert!(o.status.success());
            let w = with_exif(&o.stdout, "Silicon™", "@al4sk4s").unwrap();
            assert_eq!(whatsapp_rust::webp::is_animated(&w), anim);
            if anim {
                continue; // webp_pipe do ffmpeg não decodifica animado nem sem EXIF
            }
            let mut c = tokio::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-f",
                    "webp_pipe",
                    "-i",
                    "pipe:0",
                    "-f",
                    "null",
                    "-",
                ])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            c.stdin.take().unwrap().write_all(&w).await.unwrap();
            assert!(c.wait().await.unwrap().success());
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(with_exif(b"nao e webp nenhum, mesmo", "a", "b").is_err());
        assert!(with_exif(&riff(&[(b"JUNK", vec![1, 2])]), "a", "b").is_err());
    }
}
