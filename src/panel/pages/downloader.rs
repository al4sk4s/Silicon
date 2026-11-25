use crate::panel::*;

pub(crate) const PLATS: [(Platform, &str, &str); 2] = [
    (
        Platform::TikTok,
        "TikTok",
        "https://www.tiktok.com/@hankgreen1/video/7047596209028074758",
    ),
    (
        Platform::Instagram,
        "Instagram",
        "https://www.instagram.com/reel/CDUMkliABpa/",
    ),
];

pub(crate) fn plat_of(name: &str) -> Option<(Platform, &'static str, &'static str)> {
    PLATS.iter().copied().find(|p| p.0.name() == name)
}

pub(crate) fn dl_status(st: &AppState, dir: &std::path::Path, plat: Platform) -> String {
    if !cookies::path(dir, plat).exists() {
        return "sem cookie".into();
    }
    let s = st.store.cookies_status(plat.name()).ok().flatten();
    match s.as_deref().and_then(|v| v.split_once('|')) {
        Some((k @ ("ok" | "falhou"), ts)) => match ts.parse::<u64>() {
            Ok(t) => format!(
                "{} às {}",
                if k == "ok" { "válido" } else { "falhou" },
                crate::state::local_hms(t)
            ),
            Err(_) => "não testado".into(),
        },
        _ => "não testado".into(),
    }
}

/// Só status; o conteúdo dos cookies nunca entra no HTML.
pub(crate) fn dl_page(st: &AppState, dir: &std::path::Path, msg: &str) -> String {
    let per_min = st.store.dl_per_min().unwrap_or(2);
    let mut body = format!(
        "<p>{}</p><p>Use uma conta secundária: a conta dos cookies pode ser banida.</p>",
        esc(msg)
    );
    for (plat, label, _) in PLATS {
        let n = plat.name();
        body += &format!(
            "<h2>{label}</h2><p>Cookies: {}</p>\
             <form method=\"post\" action=\"/downloader/{n}/save\" enctype=\"multipart/form-data\">\
             <input type=\"file\" name=\"file\" accept=\".json\" required> <button>Salvar</button></form>\
             <form method=\"post\" action=\"/downloader/{n}/test\" style=\"display:inline\"><button>Testar</button></form> \
             <form method=\"post\" action=\"/downloader/{n}/remove\" style=\"display:inline\"><button>Remover</button></form>",
            esc(&dl_status(st, dir, plat))
        );
    }
    body += &format!(
        "<h2>Limite</h2><form method=\"post\" action=\"/downloader/limit\">\
         downloads por minuto (1-10): <input type=\"number\" min=\"1\" max=\"10\" name=\"dl_per_min\" value=\"{per_min}\"> \
         <button>Salvar</button></form>"
    );
    page("Downloader", &body)
}

pub(crate) fn dl_reply(st: &AppState, code: StatusCode, msg: &str) -> Response {
    (code, Html(dl_page(st, &cookies_dir(), msg))).into_response()
}

pub(crate) async fn dl_get(State(st): S) -> Html<String> {
    Html(dl_page(&st, &cookies_dir(), ""))
}

pub(crate) async fn dl_limit(State(st): S, Form(f): Fields) -> Response {
    let Some(n) = f
        .get("dl_per_min")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| (1..=10).contains(n))
    else {
        return dl_reply(
            &st,
            StatusCode::BAD_REQUEST,
            "Downloads por minuto deve ser um inteiro de 1 a 10.",
        );
    };
    match st.store.set_dl_per_min(n) {
        Ok(()) => dl_reply(&st, StatusCode::OK, "Salvo."),
        Err(e) => dl_reply(
            &st,
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Erro ao salvar: {e}"),
        ),
    }
}

/// Converte e grava; erro = mensagem fixa, nada gravado e nada do conteúdo vaza.
pub(crate) fn apply_upload(
    dir: &std::path::Path,
    plat: Platform,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let json = std::str::from_utf8(bytes).map_err(|_| "Arquivo inválido.")?;
    let txt = cookies::json_to_netscape(json, plat)
        .map_err(|_| "JSON inválido ou sem cookies desta plataforma.")?;
    cookies::save(dir, plat, &txt).map_err(|_| "Não foi possível gravar.")?;
    Ok(())
}

pub(crate) async fn dl_save(State(st): S, Path(name): Path<String>, mut mp: Multipart) -> Response {
    let Some((plat, ..)) = plat_of(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut data = None;
    while let Ok(Some(field)) = mp.next_field().await {
        if field.name() == Some("file") {
            data = field.bytes().await.ok(); // acima de 1 MB vira erro => None
            break;
        }
    }
    let Some(data) = data else {
        return dl_reply(
            &st,
            StatusCode::BAD_REQUEST,
            "Arquivo ausente ou maior que 1 MB.",
        );
    };
    match apply_upload(&cookies_dir(), plat, &data) {
        Ok(()) => {
            let _ = st.store.set_cookies_status(plat.name(), "");
            dl_reply(&st, StatusCode::OK, "Cookies salvos.")
        }
        Err(m) => dl_reply(&st, StatusCode::BAD_REQUEST, m),
    }
}

pub(crate) async fn dl_test(State(st): S, Path(name): Path<String>) -> Response {
    let Some((plat, _, url)) = plat_of(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(_permit) = st.dl_sem.try_acquire() else {
        return dl_reply(&st, StatusCode::OK, "Ocupado, tente de novo.");
    };
    let ok = match tempfile::tempdir() {
        Ok(tmp) => ytdlp::fetch(
            url,
            plat,
            &cookies_dir(),
            tmp.path(),
            std::time::Duration::from_secs(60),
        )
        .await
        .is_ok(),
        Err(_) => false,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let v = format!("{}|{now}", if ok { "ok" } else { "falhou" });
    let _ = st.store.set_cookies_status(plat.name(), &v);
    let m = if ok { "Teste: ok." } else { "Teste: falhou." };
    dl_reply(&st, StatusCode::OK, m)
}

pub(crate) async fn dl_remove(State(st): S, Path(name): Path<String>) -> Response {
    let Some((plat, ..)) = plat_of(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let _ = st.store.set_cookies_status(plat.name(), "");
    match cookies::remove(&cookies_dir(), plat) {
        Ok(()) => dl_reply(&st, StatusCode::OK, "Removido."),
        Err(_) => dl_reply(
            &st,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Não foi possível remover.",
        ),
    }
}
