//! Painel local (T7): HTML puro, sem JS. Só metadados; nunca conteúdo de mensagem nem hash (P4/P5).
use crate::auth::verify_password;
use crate::roles::Role;
use crate::state::AppState;
use axum::{
    Router,
    extract::{ConnectInfo, Form, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

type S = State<Arc<AppState>>;
type Fields = Form<HashMap<String, String>>;

/// Escapa texto dinâmico antes de entrar no HTML.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

pub fn is_loopback(addr: &str) -> bool {
    addr.parse::<SocketAddr>()
        .is_ok_and(|a| a.ip().is_loopback())
}

/// Sem senha configurada o painel só pode escutar em loopback (P5).
pub fn effective_bind(configured: &str, has_password: bool) -> String {
    if has_password {
        return configured.to_string();
    }
    let port = configured
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);
    format!("127.0.0.1:{port}")
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/login", get(login_get).post(login_post))
        .route("/logout", axum::routing::post(logout))
        .route("/", get(index))
        .route("/chats", get(chats_get).post(chats_post))
        .route("/settings", get(settings_get).post(settings_post))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

fn cookie_token(h: &HeaderMap) -> Option<String> {
    let raw = h.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == "silicon_session")
        .map(|(_, v)| v.to_string())
}

fn same_origin(h: &HeaderMap, uri: &axum::http::Uri) -> bool {
    // HTTP/2 não manda `Host`: o host vem em `:authority`, que aparece na URI.
    let host = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| uri.authority().map(|a| a.as_str()));
    match h.get(header::ORIGIN) {
        Some(o) => {
            let o = o.to_str().unwrap_or("");
            let o_host = o.split_once("://").map_or(o, |(_, r)| r);
            host == Some(o_host)
        }
        None => h.get("sec-fetch-site").is_none_or(|v| v != "cross-site"),
    }
}

/// CSRF em todo POST, depois auth em tudo menos /login, e cabeçalhos de resposta.
async fn guard(State(st): S, req: Request, next: Next) -> Response {
    let mut resp = if req.method() == Method::POST && !same_origin(req.headers(), req.uri()) {
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

fn redirect(to: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string())]).into_response()
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>Silicon - {t}</title></head><body>\
         <nav><a href=\"/\">Status</a> | <a href=\"/chats\">Chats</a> | <a href=\"/settings\">Ajustes</a> \
         <form method=\"post\" action=\"/logout\" style=\"display:inline\"><button>Sair</button></form></nav>\
         <h1>{t}</h1>{body}</body></html>",
        t = esc(title)
    )
}

fn login_page(msg: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>Silicon - entrar</title></head><body>\
         <h1>Silicon</h1><p>{}</p><form method=\"post\" action=\"/login\">\
         <input type=\"password\" name=\"password\" autofocus> <button>Entrar</button></form></body></html>",
        esc(msg)
    )
}

async fn login_get(State(st): S) -> Html<String> {
    Html(login_page(if has_password(&st) {
        ""
    } else {
        "Login indisponível: defina a senha com `silicon set-password`."
    }))
}

fn has_password(st: &AppState) -> bool {
    matches!(st.store.password_hash(), Ok(Some(h)) if !h.is_empty())
}

async fn login_post(
    State(st): S,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(f): Fields,
) -> Response {
    let ip: IpAddr = addr.ip();
    let hash = match st.store.password_hash() {
        Ok(Some(h)) if !h.is_empty() => h,
        _ => {
            let m = "Login indisponível: defina a senha com `silicon set-password`.";
            return (StatusCode::SERVICE_UNAVAILABLE, Html(login_page(m))).into_response();
        }
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

async fn logout(State(st): S, headers: HeaderMap) -> Response {
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

async fn index(State(st): S) -> Html<String> {
    let (connected, number) = {
        let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
        (s.connected, s.number.clone())
    };
    let mut rows = String::new();
    for e in st
        .events
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .rev()
        .take(50)
    {
        let t = e.at % 86400;
        rows += &format!(
            "<tr><td>{:02}:{:02}:{:02} UTC</td><td>{}</td><td>{}</td></tr>",
            t / 3600,
            t % 3600 / 60,
            t % 60,
            esc(&e.kind),
            esc(&e.jid)
        );
    }
    Html(page(
        "Status",
        &format!(
            "<p>Conectado: {}</p><p>Número: {}</p><h2>Últimos eventos</h2>\
             <table><tr><th>Hora</th><th>Tipo</th><th>JID</th></tr>{rows}</table>",
            if connected { "sim" } else { "não" },
            esc(number.as_deref().unwrap_or("não pareado")),
        ),
    ))
}

fn chats_page(st: &AppState, msg: &str) -> String {
    // jid -> rótulo (assunto do grupo); BTreeMap dá ordem estável
    let mut chats: BTreeMap<String, String> = BTreeMap::new();
    for (j, subject) in st.groups.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        chats.insert(j.clone(), subject.clone());
    }
    for j in st.recent.lock().unwrap_or_else(|e| e.into_inner()).keys() {
        chats.entry(j.clone()).or_default();
    }
    let roles = st.roles.read().unwrap_or_else(|e| e.into_inner()).clone();
    for j in roles.keys() {
        chats.entry(j.clone()).or_default();
    }
    let mut body = format!("<p>{}</p>", esc(msg));
    for (jid, label) in &chats {
        body += &chat_form(jid, label, roles.get(jid).copied());
    }
    body += "<h2>JID manual</h2>";
    body += &chat_form("", "", None);
    page("Chats", &body)
}

fn chat_form(jid: &str, label: &str, cur: Option<Role>) -> String {
    let jid_field = if jid.is_empty() {
        "<input name=\"jid\" placeholder=\"123@g.us\" required>".to_string()
    } else {
        format!(
            "<code>{j}</code> <input type=\"hidden\" name=\"jid\" value=\"{j}\">",
            j = esc(jid)
        )
    };
    let mut opts = String::new();
    for (v, txt) in [
        ("none", "sem papel"),
        ("comando", "comando"),
        ("monitorado", "monitorado"),
        ("alerta", "alerta"),
    ] {
        let sel = cur.map_or("none", |r| r.as_str()) == v;
        opts += &format!(
            "<option value=\"{v}\"{}>{txt}</option>",
            if sel { " selected" } else { "" }
        );
    }
    format!(
        "<form method=\"post\" action=\"/chats\">{jid_field} <select name=\"role\">{opts}</select> \
         nome: <input name=\"name\" value=\"{}\"> <button>Salvar</button></form>",
        esc(label)
    )
}

async fn chats_get(State(st): S) -> Html<String> {
    Html(chats_page(&st, ""))
}

async fn chats_post(State(st): S, Form(f): Fields) -> Response {
    let jid = f.get("jid").map_or("", |s| s.trim());
    let role = f.get("role").map_or("", String::as_str);
    let name = f.get("name").map(|s| s.trim()).filter(|s| !s.is_empty());
    let parsed = Role::parse(role);
    if jid.is_empty() || (role != "none" && parsed.is_none()) {
        return (
            StatusCode::BAD_REQUEST,
            Html(chats_page(&st, "JID ou papel inválido.")),
        )
            .into_response();
    }
    let res = match parsed {
        Some(r) => st.store.set_role(jid, r, name),
        None => st.store.clear_role(jid),
    };
    let res = res.and_then(|_| st.reload_roles());
    match res {
        Ok(()) => Html(chats_page(&st, "Salvo.")).into_response(),
        // ex.: segundo `alerta` viola o índice único
        Err(e) => (
            StatusCode::CONFLICT,
            Html(chats_page(&st, &format!("Não foi possível salvar: {e}"))),
        )
            .into_response(),
    }
}

fn settings_page(st: &AppState, msg: &str) -> String {
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    let owner = st.store.owner_jid().ok().flatten().unwrap_or_default();
    let per_min = st.store.sticker_per_min().unwrap_or(3);
    let pack = st.store.sticker_pack_name().unwrap_or_default();
    let publisher = st.store.sticker_publisher().unwrap_or_default();
    page(
        "Ajustes",
        &format!(
            "<p>{}</p><form method=\"post\" action=\"/settings\">\
             atraso mín (ms): <input name=\"delay_min_ms\" value=\"{min}\"><br>\
             atraso máx (ms): <input name=\"delay_max_ms\" value=\"{max}\"><br>\
             JID do dono (opcional): <input name=\"owner_jid\" value=\"{}\"><br>\
             figurinhas por minuto (1-30): <input type=\"number\" min=\"1\" max=\"30\" name=\"sticker_per_min\" value=\"{per_min}\"><br>\
             nome da figurinha (até 64): <input name=\"sticker_pack_name\" value=\"{}\"><br>\
             autor da figurinha (até 64): <input name=\"sticker_publisher\" value=\"{}\"><br>\
             <button>Salvar</button></form>",
            esc(msg),
            esc(&owner),
            esc(&pack),
            esc(&publisher)
        ),
    )
}

async fn settings_get(State(st): S) -> Html<String> {
    Html(settings_page(&st, ""))
}

async fn settings_post(State(st): S, Form(f): Fields) -> Response {
    let num = |k: &str| f.get(k).and_then(|v| v.trim().parse::<u64>().ok());
    let (Some(min), Some(max)) = (num("delay_min_ms"), num("delay_max_ms")) else {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(&st, "Atrasos devem ser números inteiros.")),
        )
            .into_response();
    };
    if min > max {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                &st,
                "O mínimo não pode ser maior que o máximo.",
            )),
        )
            .into_response();
    }
    let Some(per_min) = f
        .get("sticker_per_min")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| (1..=30).contains(n))
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                &st,
                "Figurinhas por minuto deve ser um inteiro de 1 a 30.",
            )),
        )
            .into_response();
    };
    let owner = f.get("owner_jid").map_or("", |s| s.trim());
    let field = |k: &str| f.get(k).map_or("", |s| s.trim()).to_string();
    let (pack, publisher) = (field("sticker_pack_name"), field("sticker_publisher"));
    let res = st
        .store
        .set_sticker_meta(&pack, &publisher)
        .and_then(|_| st.store.set_sticker_per_min(per_min))
        .and_then(|_| st.store.set("delay_min_ms", &min.to_string()))
        .and_then(|_| st.store.set("delay_max_ms", &max.to_string()))
        .and_then(|_| st.store.set("owner_jid", owner));
    match res {
        Ok(()) => Html(settings_page(&st, "Salvo.")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(settings_page(&st, &format!("Erro ao salvar: {e}"))),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::hash_password;
    use crate::config::Store;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::Request;
    use std::sync::RwLock;
    use tower::ServiceExt;

    fn app() -> (Router, Arc<AppState>) {
        let store = Store::memory().unwrap();
        store
            .set("password_hash", &hash_password("segredo").unwrap())
            .unwrap();
        let st = Arc::new(AppState::new(
            store,
            Arc::new(RwLock::new(HashMap::new())),
            false,
        ));
        let addr: SocketAddr = "127.0.0.1:5555".parse().unwrap();
        (router(st.clone()).layer(MockConnectInfo(addr)), st)
    }

    fn post(uri: &str, body: &str, origin: Option<&str>, cookie: Option<&str>) -> Request<Body> {
        let mut b = Request::post(uri)
            .header("host", "localhost")
            .header("content-type", "application/x-www-form-urlencoded");
        if let Some(o) = origin {
            b = b.header("origin", o);
        }
        if let Some(c) = cookie {
            b = b.header("cookie", format!("silicon_session={c}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn h2_login_uses_authority_not_host() {
        let (app, _) = app();
        // HTTP/2: sem header Host, URI absoluta com authority
        let r = app
            .oneshot(
                Request::post("https://box:8080/login")
                    .header("origin", "https://box:8080")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("password=x"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn no_cookie_redirects_to_login() {
        let (app, _) = app();
        let r = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        assert_eq!(r.headers()[header::LOCATION], "/login");
    }

    #[tokio::test]
    async fn login_ok_sets_cookie() {
        let (app, _) = app();
        let r = app
            .oneshot(post("/login", "password=segredo", None, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict"));
    }

    #[tokio::test]
    async fn wrong_password_then_blocked() {
        let (app, _) = app();
        for _ in 0..5 {
            let r = app
                .clone()
                .oneshot(post("/login", "password=x", None, None))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
        let r = app
            .oneshot(post("/login", "password=segredo", None, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn chats_post_csrf_and_save() {
        let (app, st) = app();
        let tok = st.sessions.create();
        let body = "jid=a%40g.us&role=comando&name=Grupo";
        let r = app
            .clone()
            .oneshot(post("/chats", body, Some("http://evil.com"), Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = app
            .clone()
            .oneshot(post("/chats", body, Some("http://localhost"), Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(st.roles.read().unwrap()["a@g.us"], Role::Comando);
        // segundo alerta em outro JID: erro na página, sem panic
        app.clone()
            .oneshot(post("/chats", "jid=b%40g.us&role=alerta", None, Some(&tok)))
            .await
            .unwrap();
        let r = app
            .oneshot(post("/chats", "jid=c%40g.us&role=alerta", None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn settings_post_saves_sticker_options() {
        let (app, st) = app();
        let tok = st.sessions.create();
        let base = "delay_min_ms=1000&delay_max_ms=2000&owner_jid=";
        let r = app
            .clone()
            .oneshot(post(
                "/settings",
                &format!(
                    "{base}&sticker_per_min=7&sticker_pack_name=Meu+pacote&sticker_publisher=Eu"
                ),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(st.store.sticker_per_min().unwrap(), 7);
        assert_eq!(st.store.sticker_pack_name().unwrap(), "Meu pacote");
        assert_eq!(st.store.sticker_publisher().unwrap(), "Eu");
        // limite fora de 1..=30 é recusado sem alterar
        let r = app
            .clone()
            .oneshot(post(
                "/settings",
                &format!("{base}&sticker_per_min=31"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let r = app
            .oneshot(post(
                "/settings",
                &format!("{base}&sticker_per_min=7"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
    }

    #[test]
    fn esc_and_bind() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(effective_bind("0.0.0.0:9000", false), "127.0.0.1:9000");
        assert_eq!(effective_bind("0.0.0.0:9000", true), "0.0.0.0:9000");
        assert_eq!(effective_bind("lixo", false), "127.0.0.1:8080");
        assert!(is_loopback("127.0.0.1:80") && !is_loopback("0.0.0.0:80"));
    }
}
