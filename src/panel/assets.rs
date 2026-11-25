//! Arquivos estáticos do painel (CSS/JS), embutidos no binário e servidos sem sessão
//! (a tela de login precisa deles). Só conteúdo estático: nada do bot passa por aqui.
use crate::panel::*;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

const FILES: [(&str, &str, &str); 4] = [
    (
        "tokens.css",
        include_str!("assets/tokens.css"),
        "text/css; charset=utf-8",
    ),
    (
        "components.css",
        include_str!("assets/components.css"),
        "text/css; charset=utf-8",
    ),
    (
        "theme.js",
        include_str!("assets/theme.js"),
        "text/javascript; charset=utf-8",
    ),
    (
        "app.js",
        include_str!("assets/app.js"),
        "text/javascript; charset=utf-8",
    ),
];

/// Versão = hash do conteúdo: URL nova a cada mudança, então o cache pode ser eterno.
static VER: LazyLock<String> = LazyLock::new(|| {
    let mut h = Sha256::new();
    for (n, c, _) in FILES {
        h.update(n.as_bytes());
        h.update(c.as_bytes());
    }
    h.finalize()
        .iter()
        .take(5)
        .map(|b| format!("{b:02x}"))
        .collect()
});

pub(crate) fn url(name: &str) -> String {
    format!("/assets/{}/{name}", *VER)
}

/// CSP restritiva: nada inline, nada externo.
pub(crate) const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

pub(crate) async fn asset(Path((v, name)): Path<(String, String)>) -> Response {
    let Some((_, body, ctype)) = FILES.iter().find(|(n, _, _)| *n == name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if v != *VER {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [
            (header::CONTENT_TYPE, *ctype),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        *body,
    )
        .into_response()
}
