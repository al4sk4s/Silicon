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

impl DlError {
    /// Uma linha de resposta em PT-BR.
    pub fn reply(&self) -> &'static str {
        match self {
            DlError::BadHost => "Só aceito links de TikTok e Instagram.",
            DlError::NoUrl => "Uso: /baixar <link> (ou responda uma mensagem com o link).",
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
