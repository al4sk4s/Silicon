use crate::panel::*;

pub(crate) fn cookie_token(h: &HeaderMap) -> Option<String> {
    let raw = h.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == "silicon_session")
        .map(|(_, v)| v.to_string())
}

/// `trusted`: host público do proxy reverso (env `SILICON_PROXY_HOST`), aceito além do Host.
pub(crate) fn same_origin(h: &HeaderMap, uri: &axum::http::Uri, trusted: Option<&str>) -> bool {
    // HTTP/2 não manda `Host`: o host vem em `:authority`, que aparece na URI.
    let host = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| uri.authority().map(|a| a.as_str()));
    match h.get(header::ORIGIN) {
        Some(o) => {
            let o = o.to_str().unwrap_or("");
            let o_host = o.split_once("://").map_or(o, |(_, r)| r);
            host == Some(o_host) || trusted.is_some_and(|t| t == o_host)
        }
        None => h.get("sec-fetch-site").is_none_or(|v| v != "cross-site"),
    }
}

/// CSRF em todo POST, depois auth em tudo menos /login, e cabeçalhos de resposta.
pub(crate) async fn guard(State(st): S, req: Request, next: Next) -> Response {
    let mut resp = if req.method() == Method::POST
        && !same_origin(
            req.headers(),
            req.uri(),
            std::env::var("SILICON_PROXY_HOST").ok().as_deref(),
        ) {
        (StatusCode::FORBIDDEN, "origem não permitida").into_response()
    } else if req.uri().path() != "/login"
        && !cookie_token(req.headers()).is_some_and(|t| st.sessions.valid(&t))
    {
        redirect("/login")
    } else {
        next.run(req).await
    };
    let h = resp.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    resp
}

pub(crate) fn redirect(to: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string())]).into_response()
}
