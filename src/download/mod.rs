//! Spec 004: `/baixar` (TikTok e Instagram via yt-dlp externo).
pub mod cookies;
pub mod handle;
pub mod url;
pub mod ytdlp;

/// Pasta dos cookies: `<SILICON_DATA_DIR>/cookies` (mesmo padrão do `data_dir` do main).
pub fn cookies_dir() -> std::path::PathBuf {
    let base = std::env::var("SILICON_DATA_DIR").unwrap_or_else(|_| "/var/lib/silicon".into());
    std::path::PathBuf::from(base).join("cookies")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    TikTok,
    Instagram,
}

impl Platform {
    /// Nome estável (chave de config, nome do arquivo de cookies, log).
    pub fn name(self) -> &'static str {
        match self {
            Platform::TikTok => "tiktok",
            Platform::Instagram => "instagram",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlError {
    BadHost,
    NoUrl,
    LoginRequired,
    NotFound,
    Timeout,
    Missing,
    TooBig,
    Busy,
    Other,
}

const BAD_HOST: &[&str] = &[
    "Só aceito links de TikTok e Instagram.",
    "Esse link eu não pego, só TikTok e Instagram.",
    "Link estranho: aqui só vai TikTok ou Instagram.",
    "Não reconheci esse link, manda de TikTok ou Instagram.",
    "Desse site eu não baixo, só TikTok e Instagram.",
];
const NO_URL: &[&str] = &[
    "Uso: /baixar <link> (ou responda uma mensagem com o link).",
    "Cadê o link? Manda /baixar <link> ou responde uma mensagem com ele.",
    "Faltou o link: /baixar <link>, ou responda a mensagem que tem o link.",
    "Não achei link nenhum. Use /baixar <link>.",
    "Sem link não rola. /baixar <link> ou responda a mensagem com ele.",
];

impl DlError {
    /// Uma linha de resposta em PT-BR.
    pub fn reply(&self) -> &'static str {
        match self {
            DlError::BadHost => crate::stickers::variante(BAD_HOST),
            DlError::NoUrl => crate::stickers::variante(NO_URL),
            DlError::LoginRequired => "Esse conteúdo exige login.",
            DlError::NotFound => "Não achei esse vídeo (link quebrado, privado ou removido).",
            DlError::Timeout => "Demorou demais, desisti.",
            DlError::Missing => "yt-dlp ausente.",
            DlError::TooBig => "Vídeo grande demais.",
            DlError::Busy => "Já estou baixando outro, tente de novo daqui a pouco.",
            DlError::Other => "Não consegui baixar esse vídeo.",
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn respostas_de_link_ruim_sao_5() {
        for v in [super::BAD_HOST, super::NO_URL] {
            assert_eq!(v.len(), 5);
            assert!(v.iter().all(|s| !s.is_empty() && !s.starts_with('/')));
        }
    }
}
