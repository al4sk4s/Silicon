use crate::panel::*;

pub(crate) async fn login_get(State(st): S) -> Html<String> {
    if has_password(&st) {
        return Html(login_page(""));
    }
    Html(setup_page(""))
}

/// Primeiro acesso (sem senha gravada): o dono escolhe a senha aqui.
pub(crate) fn setup_page(msg: &str) -> String {
    bare_page(
        "primeira senha",
        &format!(
            "<p class=\"muted\">Primeiro acesso: escolha a senha do painel.</p>{}\
             <form method=\"post\" action=\"/login\" class=\"stack\">{}{}{}</form>",
            flash(msg),
            field(
                "Senha",
                "password",
                &input(
                    "password",
                    "",
                    "type=\"password\" autofocus autocomplete=\"new-password\" required"
                ),
                ""
            ),
            field(
                "Repita a senha",
                "password2",
                &input(
                    "password2",
                    "",
                    "type=\"password\" autocomplete=\"new-password\" required"
                ),
                ""
            ),
            btn("Definir senha", "primary", "")
        ),
    )
}

/// Cria a sessão do painel e devolve o redirect com o cookie.
pub(crate) fn start_session(st: &AppState) -> Response {
    let token = st.sessions.create();
    let secure = if st.https { "; Secure" } else { "" };
    let cookie = format!(
        "silicon_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{secure}"
    );
    let mut r = redirect("/");
    r.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("cookie ascii"),
    );
    r
}

pub(crate) fn has_password(st: &AppState) -> bool {
    matches!(st.store.password_hash(), Ok(Some(h)) if !h.is_empty())
}

pub(crate) async fn login_post(
    State(st): S,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(f): Fields,
) -> Response {
    let ip: IpAddr = addr.ip();
    let hash = match st.store.password_hash() {
        Ok(Some(h)) if !h.is_empty() => h,
        _ => return first_password(&st, ip, &f),
    };
    if st.limiter.blocked(ip, Instant::now()) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Html(login_page("Muitas tentativas. Tente mais tarde.")),
        )
            .into_response();
    }
    let pw = f.get("password").map_or("", String::as_str);
    if !verify_password(&hash, pw) {
        st.limiter.fail(ip, Instant::now());
        return (
            StatusCode::UNAUTHORIZED,
            Html(login_page("Senha incorreta.")),
        )
            .into_response();
    }
    st.limiter.success(ip);
    start_session(&st)
}

/// Sem senha gravada, só quem está na própria máquina (loopback) pode defini-la; o painel já
/// escuta só em loopback nesse estado (P5). Vale a primeira que gravar.
pub(crate) fn first_password(st: &AppState, ip: IpAddr, f: &HashMap<String, String>) -> Response {
    let fail = |code, m| (code, Html(setup_page(m))).into_response();
    if !ip.is_loopback() {
        return fail(
            StatusCode::FORBIDDEN,
            "Defina a senha pela própria máquina.",
        );
    }
    let pw = f.get("password").map_or("", String::as_str);
    if pw.is_empty() || pw != f.get("password2").map_or("", String::as_str) {
        return fail(
            StatusCode::BAD_REQUEST,
            "Senha vazia ou as duas não conferem.",
        );
    }
    let saved = crate::auth::hash_password(pw)
        .map_err(|e| e.to_string())
        .and_then(|h| st.store.set("password_hash", &h).map_err(|e| e.to_string()));
    match saved {
        Ok(()) => start_session(st),
        Err(_) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Não consegui salvar a senha.",
        ),
    }
}

pub(crate) async fn logout(State(st): S, headers: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&headers) {
        st.sessions.remove(&t);
    }
    let mut r = redirect("/login");
    r.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("silicon_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    );
    r
}
