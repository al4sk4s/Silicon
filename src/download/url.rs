//! Extração e validação da URL de vídeo (parse manual, sem crate `url`).

use super::{DlError, Platform};

/// Primeiro token que começa com `http://` ou `https://`, sem pontuação final.
pub fn first_url(text: &str) -> Option<&str> {
    text.split_whitespace()
        .find(|t| t.starts_with("http://") || t.starts_with("https://"))
        .map(|t| t.trim_end_matches(['.', ',', ')', '!', '?']))
}

/// Pega a primeira URL e só a aceita se for https e de um host permitido.
pub fn validate(text: &str) -> Result<(Platform, String), DlError> {
    let url = first_url(text).ok_or(DlError::NoUrl)?;
    let rest = url.strip_prefix("https://").ok_or(DlError::BadHost)?;
    let host = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];

    // Lista fechada de caracteres: barra `@`, `:` (porta), `[` `]` (IPv6), `\` etc.
    let charset_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    let is_ip = host.chars().all(|c| c.is_ascii_digit() || c == '.');
    if !charset_ok || is_ip {
        return Err(DlError::BadHost);
    }

    let host = host.to_ascii_lowercase();
    // Host exato ou subdomínio real (com o ponto), nunca só sufixo de string.
    let is = |d: &str| host == d || host.ends_with(&format!(".{d}"));
    let platform = if is("tiktok.com") {
        Platform::TikTok
    } else if is("instagram.com") || is("instagr.am") {
        Platform::Instagram
    } else {
        return Err(DlError::BadHost);
    };
    Ok((platform, url.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plat(s: &str) -> Result<Platform, DlError> {
        validate(s).map(|(p, _)| p)
    }

    #[test]
    fn hosts_bons() {
        for u in [
            "https://www.tiktok.com/@a/video/1",
            "https://vm.tiktok.com/abc/",
            "https://vt.tiktok.com/abc/",
            "https://tiktok.com/x",
        ] {
            assert!(matches!(plat(u), Ok(Platform::TikTok)), "{u}");
        }
        for u in [
            "https://www.instagram.com/reel/x",
            "https://instagr.am/p/x",
            "https://WWW.Instagram.com/reel/x",
        ] {
            assert!(matches!(plat(u), Ok(Platform::Instagram)), "{u}");
        }
    }

    #[test]
    fn hosts_maus() {
        for u in [
            "https://evil-tiktok.com/x",
            "https://tiktok.com.evil.com/x",
            "https://tiktok.com@evil.com/x",
            "https://evil.com@tiktok.com/x",
            "https://evil.com/?u=tiktok.com",
            "https://evil.com#tiktok.com",
            "http://www.tiktok.com/x",
            "https://127.0.0.1/x",
            "https://[::1]/x",
            "https://www.tiktok.com:443/x",
            "https://www.tiktok.com:8080/x",
        ] {
            assert!(matches!(plat(u), Err(DlError::BadHost)), "{u}");
        }
    }

    #[test]
    fn extracao() {
        assert!(matches!(validate("oi sem link"), Err(DlError::NoUrl)));
        let (_, u) = validate("olha isso https://vm.tiktok.com/abc/. massa").unwrap();
        assert_eq!(u, "https://vm.tiktok.com/abc/");
        // várias URLs: vale a primeira
        let (p, u) = validate("https://instagr.am/p/1 e https://vm.tiktok.com/2").unwrap();
        assert!(matches!(p, Platform::Instagram));
        assert_eq!(u, "https://instagr.am/p/1");
        assert_eq!(first_url("https://a.b/c)!?"), Some("https://a.b/c"));
    }
}
