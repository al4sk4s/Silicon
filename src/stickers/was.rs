//! Spike B: monta um `.was` (ZIP com Lottie 512x512, 1 imagem por frame). Escrito do zero (P9).
//! shortcut: ZIP "stored" (sem compressão) e sem trust_token; o app pode exigir token (spike B5).

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len().div_ceil(3) * 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// ZIP sem compressão. Entradas pequenas (< 4 GiB), nomes ASCII.
fn zip_stored(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for (name, data) in files {
        let off = out.len() as u32;
        let (crc, len, nl) = (crc32(data), data.len() as u32, name.len() as u16);
        let common = |v: &mut Vec<u8>| {
            v.extend([20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]); // versão, flags, método 0, hora, data
            v.extend(crc.to_le_bytes());
            v.extend(len.to_le_bytes());
            v.extend(len.to_le_bytes());
            v.extend(nl.to_le_bytes());
        };
        out.extend(b"PK\x03\x04");
        common(&mut out);
        out.extend([0, 0]);
        out.extend(name.as_bytes());
        out.extend(data);
        dir.extend(b"PK\x01\x02\x14\x00");
        common(&mut dir);
        dir.extend([0u8; 12]); // extra, comentário, disco, attrs internos/externos(parcial)
        dir.extend(off.to_le_bytes());
        dir.extend(name.as_bytes());
    }
    let (n, dl, doff) = (files.len() as u16, dir.len() as u32, out.len() as u32);
    out.extend(dir);
    out.extend(b"PK\x05\x06\0\0\0\0");
    out.extend(n.to_le_bytes());
    out.extend(n.to_le_bytes());
    out.extend(dl.to_le_bytes());
    out.extend(doff.to_le_bytes());
    out.extend([0, 0]);
    out
}

/// Lottie 512x512: um asset de imagem por frame, cada camada visível só no seu quadro.
fn lottie(frames: &[Vec<u8>], mime: &str, fps: u8) -> String {
    let n = frames.len();
    let assets: Vec<String> = frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            format!(
                r#"{{"id":"f{i}","w":512,"h":512,"u":"","p":"data:{mime};base64,{}","e":1}}"#,
                b64(f)
            )
        })
        .collect();
    let layers: Vec<String> = (0..n)
        .map(|i| {
            format!(
                r#"{{"ddd":0,"ind":{},"ty":2,"nm":"f{i}","refId":"f{i}","sr":1,"ks":{{"o":{{"a":0,"k":100}},"r":{{"a":0,"k":0}},"p":{{"a":0,"k":[256,256,0]}},"a":{{"a":0,"k":[256,256,0]}},"s":{{"a":0,"k":[100,100,100]}}}},"ao":0,"ip":{i},"op":{},"st":0,"bm":0}}"#,
                i + 1,
                i + 1
            )
        })
        .collect();
    format!(
        r#"{{"v":"5.12.1","fr":{fps},"ip":0,"op":{n},"w":512,"h":512,"nm":"silicon","ddd":0,"assets":[{}],"layers":[{}]}}"#,
        assets.join(","),
        layers.join(",")
    )
}

/// `.was` com os frames (todos 512x512, `mime` único). Sem frames = None.
pub fn build(
    frames: &[Vec<u8>],
    mime: &str,
    fps: u8,
    pack: &str,
    publisher: &str,
) -> Option<Vec<u8>> {
    if frames.is_empty() {
        return None;
    }
    let json = lottie(frames, mime, fps).into_bytes();
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let meta = format!(
        r#"{{"sticker-pack-id":"silicon","sticker-pack-name":"{}","sticker-pack-publisher":"{}","is-from-user-created-pack":1}}"#,
        esc(pack),
        esc(publisher)
    );
    Some(zip_stored(&[
        ("animation/animation.json", json.clone()),
        ("animation/animation_secondary.json", json),
        (
            "animation/animation.json.overridden_metadata",
            meta.into_bytes(),
        ),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_e_crc_conhecidos() {
        assert_eq!(b64(b"Man"), "TWFu");
        assert_eq!(b64(b"Ma"), "TWE=");
        assert_eq!(b64(b"M"), "TQ==");
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn was_e_zip_valido_com_lottie() {
        assert!(build(&[], "image/webp", 24, "a", "b").is_none());
        let w = build(&[vec![1, 2, 3], vec![4, 5]], "image/webp", 24, "a\"b", "c").unwrap();
        assert_eq!(&w[..4], b"PK\x03\x04");
        assert!(w.windows(4).any(|x| x == b"PK\x05\x06"));
        let s = String::from_utf8_lossy(&w);
        assert!(s.contains(r#""op":2,"w":512,"h":512"#) && s.contains(r#"a\"b"#));
        assert!(s.contains("data:image/webp;base64,AQID"));
    }
}
