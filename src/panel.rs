//! Painel local (T7, spec 005): HTML server-side, CSS embutido e JS mínimo próprio. Só metadados; nunca conteúdo de mensagem nem hash (P4/P5).
use crate::auth::verify_password;
use crate::download::{Platform, cookies, cookies_dir, ytdlp};
use crate::roles::Role;
use crate::state::AppState;
use axum::{
    Router,
    extract::{ConnectInfo, DefaultBodyLimit, Form, Multipart, Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
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
        .unwrap_or(8787);
    format!("127.0.0.1:{port}")
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/login", get(login_get).post(login_post))
        .route("/logout", axum::routing::post(logout))
        .route("/", get(index))
        .route("/chats", get(chats_get).post(chats_post))
        .route("/contacts", get(contacts_get).post(contacts_post))
        .route("/settings", get(settings_get).post(settings_post))
        .route("/pair", get(pair_get).post(pair_post))
        .route("/pair/state", get(pair_state))
        .route("/metrics", get(metrics_get))
        .route("/rules", get(rules_get).post(rules_post))
        .route("/alerts", get(alerts_get))
        .route("/downloader", get(dl_get))
        .route("/downloader/limit", post(dl_limit))
        .route(
            "/downloader/{plat}/save",
            post(dl_save).layer(DefaultBodyLimit::max(1 << 20)),
        )
        .route("/downloader/{plat}/test", post(dl_test))
        .route("/downloader/{plat}/remove", post(dl_remove))
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

/// `trusted`: host público do proxy reverso (env `SILICON_PROXY_HOST`), aceito além do Host.
fn same_origin(h: &HeaderMap, uri: &axum::http::Uri, trusted: Option<&str>) -> bool {
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
async fn guard(State(st): S, req: Request, next: Next) -> Response {
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

fn redirect(to: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string())]).into_response()
}

const CSS: &str = "body{font:15px system-ui,sans-serif;background:#f4f4f5;color:#27272a;margin:0}\
main{max-width:780px;margin:0 auto;padding:16px}nav{background:#fff;border-bottom:1px solid #e4e4e7;padding:10px 16px;display:flex;gap:16px;align-items:center}\
nav a{color:#52525b;text-decoration:none}nav a:hover{color:#18181b}nav form{margin-left:auto}h1{font-size:20px}\
.card{background:#fff;border:1px solid #e4e4e7;border-radius:8px;padding:12px 14px;margin:8px 0}\
.row{display:flex;gap:10px;align-items:center;justify-content:space-between;flex-wrap:wrap}\
button{background:#3f3f46;color:#fff;border:0;border-radius:6px;padding:6px 12px;cursor:pointer}button:hover{background:#18181b}\
button.off{background:#e4e4e7;color:#27272a}input,select{padding:6px;border:1px solid #d4d4d8;border-radius:6px}\
code{color:#71717a;font-size:12px}.tag{background:#e4e4e7;border-radius:10px;padding:2px 8px;font-size:12px}\
.tag.on{background:#3f3f46;color:#fff}table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:4px;border-bottom:1px solid #e4e4e7}\
.code{font:700 32px monospace;letter-spacing:4px}";

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Silicon - {t}</title><style>{CSS}</style></head><body>\
         <nav><b>Silicon</b><a href=\"/\">Status</a><a href=\"/chats\">Chats</a><a href=\"/contacts\">Contatos</a><a href=\"/rules\">Regras</a><a href=\"/alerts\">Alertas</a><a href=\"/pair\">Pareamento</a><a href=\"/downloader\">Downloader</a><a href=\"/settings\">Ajustes</a>\
         <form method=\"post\" action=\"/logout\"><button class=\"off\">Sair</button></form></nav>\
         <main><h1>{t}</h1>{body}</main></body></html>",
        t = esc(title)
    )
}

fn login_page(msg: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Silicon - entrar</title><style>{CSS}</style></head><body><main><h1>Silicon</h1><p>{}</p>\
         <form class=\"card\" method=\"post\" action=\"/login\">\
         <input type=\"password\" name=\"password\" autofocus> <button>Entrar</button></form></main></body></html>",
        esc(msg)
    )
}

async fn login_get(State(st): S) -> Html<String> {
    if has_password(&st) {
        return Html(login_page(""));
    }
    Html(setup_page(""))
}

/// Primeiro acesso (sem senha gravada): o dono escolhe a senha aqui.
fn setup_page(msg: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Silicon - primeira senha</title><style>{CSS}</style></head><body><main><h1>Silicon</h1>\
         <p>Primeiro acesso: escolha a senha do painel.</p><p>{}</p>\
         <form class=\"card\" method=\"post\" action=\"/login\">\
         <input type=\"password\" name=\"password\" placeholder=\"Senha\" autofocus> \
         <input type=\"password\" name=\"password2\" placeholder=\"Repita\"> <button>Definir</button></form></main></body></html>",
        esc(msg)
    )
}

/// Cria a sessão do painel e devolve o redirect com o cookie.
fn start_session(st: &AppState) -> Response {
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
fn first_password(st: &AppState, ip: IpAddr, f: &HashMap<String, String>) -> Response {
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

fn metrics_fragment(st: &AppState) -> String {
    crate::metrics::texto(crate::metrics::atualizar(&st.metricas), st.boot.elapsed())
}

async fn metrics_get(State(st): S) -> Html<String> {
    Html(metrics_fragment(&st))
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
        rows += &format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            crate::state::local_hms(e.at),
            esc(&e.kind),
            esc(&e.jid)
        );
    }
    Html(page(
        "Status",
        &format!(
            "<p>Conectado: {}</p><p>Número: {}</p><p id=\"m\">{}</p><h2>Últimos eventos</h2>\
             <table><tr><th>Hora</th><th>Tipo</th><th>JID</th></tr>{rows}</table>\
             <script>setInterval(async()=>{{try{{const r=await fetch('/metrics');if(r.redirected){{location.reload();return}}m.innerHTML=await r.text()}}catch(e){{}}}},5000)</script>",
            if connected { "sim" } else { "não" },
            esc(number.as_deref().unwrap_or("não pareado")),
            metrics_fragment(&st),
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
    let mut body = format!(
        "<p>{}</p><input id=\"q\" placeholder=\"Buscar grupo ou contato…\" style=\"width:100%;box-sizing:border-box\">\
         <script>q.oninput=()=>document.querySelectorAll('.chat').forEach(c=>c.hidden=!c.dataset.q.includes(q.value.toLowerCase()));\
         setInterval(()=>{{if(!q.value&&!document.activeElement.matches('input,select'))location.replace('/chats')}},5000)</script>",
        esc(msg)
    );
    for (jid, label) in &chats {
        body += &chat_form(jid, label, roles.get(jid).copied());
    }
    body += "<h2>Adicionar por JID</h2>";
    body += &chat_form("", "", None);
    page("Chats", &body)
}

fn chat_form(jid: &str, label: &str, cur: Option<Role>) -> String {
    if jid.is_empty() {
        // JID manual
        return "<form class=\"card row\" method=\"post\" action=\"/chats\">\
                <input name=\"jid\" placeholder=\"123@g.us\" required> \
                <input type=\"hidden\" name=\"role\" value=\"comando\">\
                <input name=\"name\" placeholder=\"nome (opcional)\"> <button>Liberar</button></form>"
            .to_string();
    }
    let on = cur == Some(Role::Comando);
    let mut opts = String::new();
    for (v, txt) in [
        ("comando", "comando"),
        ("monitorado", "monitorado"),
        ("alerta", "alerta"),
    ] {
        let sel = cur.is_some_and(|r| r.as_str() == v);
        opts += &format!(
            "<option value=\"{v}\"{}>{txt}</option>",
            if sel { " selected" } else { "" }
        );
    }
    let (next, btn, cls) = if on {
        ("none", "Bloquear", " class=\"off\"")
    } else {
        ("comando", "Liberar", "")
    };
    format!(
        "<div class=\"card row chat\" data-q=\"{q}\"><div><b>{name}</b><br><code>{j}</code></div><div class=\"row\">\
         <span class=\"tag{tc}\">{role}</span>\
         <form method=\"post\" action=\"/chats\"><input type=\"hidden\" name=\"jid\" value=\"{j}\">\
         <input type=\"hidden\" name=\"name\" value=\"{l}\"><input type=\"hidden\" name=\"role\" value=\"{next}\"><button{cls}>{btn}</button></form>\
         <form method=\"post\" action=\"/chats\"><input type=\"hidden\" name=\"jid\" value=\"{j}\"><input type=\"hidden\" name=\"name\" value=\"{l}\">\
         <select name=\"role\" onchange=\"this.form.submit()\"><option value=\"\" disabled{nosel}>outro papel…</option>{opts}</select></form></div></div>",
        q = esc(&format!("{label} {jid}").to_lowercase()),
        name = if label.is_empty() {
            "(sem nome)".into()
        } else {
            esc(label)
        },
        j = esc(jid),
        l = esc(label),
        tc = if on { " on" } else { "" },
        role = cur.map_or("sem papel", |r| r.as_str()),
        nosel = if cur.is_none() { " selected" } else { "" },
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

/// "11 99999-8888" -> "5511999998888". Até 11 dígitos assume Brasil (+55); precisa ter 10–15.
fn normalize_phone(s: &str) -> Option<String> {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    let d = if d.len() <= 11 { format!("55{d}") } else { d };
    (12..=15).contains(&d.len()).then_some(d)
}

fn contacts_page(st: &AppState, msg: &str) -> String {
    let mut body = format!(
        "<p>{}</p><form method=\"post\" action=\"/contacts\"><input type=\"hidden\" name=\"action\" value=\"add\">\
         número: <input name=\"phone\" placeholder=\"11 99999-8888\" required> \
         nome (opcional): <input name=\"name\"> <button>Adicionar</button></form>",
        esc(msg)
    );
    for (jid, name) in st.store.dm_commands().unwrap_or_default() {
        let label = name.unwrap_or_else(|| jid.split('@').next().unwrap_or("").to_string());
        body.push_str(&format!(
            "<div class=\"card row\"><span>{} <small>{}</small></span>\
             <form method=\"post\" action=\"/contacts\"><input type=\"hidden\" name=\"action\" value=\"del\">\
             <input type=\"hidden\" name=\"jid\" value=\"{j}\"><button class=\"off\">Remover</button></form></div>",
            esc(&label),
            esc(&jid),
            j = esc(&jid),
        ));
    }
    page("Contatos", &body)
}

async fn contacts_get(State(st): S) -> Html<String> {
    Html(contacts_page(&st, ""))
}

fn contacts_err(st: &AppState, code: StatusCode, msg: &str) -> Response {
    (code, Html(contacts_page(st, msg))).into_response()
}

async fn contacts_post(State(st): S, Form(f): Fields) -> Response {
    if f.get("action").map(String::as_str) == Some("del") {
        let jid = f.get("jid").map_or("", |s| s.trim());
        // só remove DM com papel `comando` (esta aba não mexe em grupos nem em outros papéis)
        let known = st
            .store
            .dm_commands()
            .is_ok_and(|v| v.iter().any(|(j, _)| j == jid));
        if !known {
            return contacts_err(&st, StatusCode::BAD_REQUEST, "Contato não encontrado.");
        }
        return match st.store.clear_role(jid).and_then(|_| st.reload_roles()) {
            Ok(()) => {
                st.push_event("contato_del", jid);
                Html(contacts_page(&st, "Removido.")).into_response()
            }
            Err(e) => contacts_err(
                &st,
                StatusCode::CONFLICT,
                &format!("Não foi possível remover: {e}"),
            ),
        };
    }
    let Some(num) = normalize_phone(f.get("phone").map_or("", String::as_str)) else {
        return contacts_err(&st, StatusCode::BAD_REQUEST, "Número inválido.");
    };
    let name = f.get("name").map(|s| s.trim()).filter(|s| !s.is_empty());
    let client = st.client.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(client) = client else {
        return contacts_err(&st, StatusCode::SERVICE_UNAVAILABLE, "Bot desconectado.");
    };
    let found = match client
        .contacts()
        .is_on_whatsapp(&[whatsapp_rust::Jid::pn(num.as_str())])
        .await
    {
        Ok(v) => v.into_iter().find(|r| r.is_registered),
        Err(e) => {
            return contacts_err(
                &st,
                StatusCode::BAD_GATEWAY,
                &format!("Consulta falhou: {e}"),
            );
        }
    };
    let Some(r) = found else {
        return contacts_err(
            &st,
            StatusCode::NOT_FOUND,
            "Esse número não está no WhatsApp.",
        );
    };
    // o dispatch compara a chave exata do chat: DM de conta migrada chega como @lid
    let jid = r.lid.as_ref().unwrap_or(&r.jid).to_non_ad_string();
    let cur = st
        .roles
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(&jid)
        .copied();
    if cur.is_some_and(|c| c != Role::Comando) {
        return contacts_err(
            &st,
            StatusCode::CONFLICT,
            "Esse chat tem outro papel; use Chats.",
        );
    }
    match st
        .store
        .set_role(&jid, Role::Comando, name)
        .and_then(|_| st.reload_roles())
    {
        Ok(()) => {
            st.push_event("contato_add", &jid);
            Html(contacts_page(&st, "Adicionado.")).into_response()
        }
        Err(e) => contacts_err(
            &st,
            StatusCode::CONFLICT,
            &format!("Não foi possível salvar: {e}"),
        ),
    }
}

fn settings_page(st: &AppState, msg: &str) -> String {
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    let owner = st.store.owner_jid().ok().flatten().unwrap_or_default();
    let per_min = st.store.sticker_per_min().unwrap_or(3);
    let pack = st.store.sticker_pack_name().unwrap_or_default();
    let publisher = st.store.sticker_publisher().unwrap_or_default();
    let sound_on = if st.store.sticker_sound().unwrap_or(true) {
        " checked"
    } else {
        ""
    };
    let sound_max = st.store.sticker_sound_max_s().unwrap_or(10);
    let dedupe = st.store.dedupe_min().unwrap_or(30);
    let por_hora = st.store.alertas_hora().unwrap_or(10);
    let retencao = st.store.retencao_dias().unwrap_or(7);
    let sil_ini = st.store.silencio_ini().unwrap_or_default();
    let sil_fim = st.store.silencio_fim().unwrap_or_default();
    let malvin = st.store.video_sticker_mode().is_ok_and(|m| m == "malvin");
    let (sw, sd) = if malvin {
        (" selected", "")
    } else {
        ("", " selected")
    };
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
             figurinha com som (vídeo com áudio): <input type=\"checkbox\" name=\"sticker_sound\" value=\"1\"{sound_on}><br>\
             segundos máximos de som (1-10): <input type=\"number\" min=\"1\" max=\"10\" name=\"sticker_sound_max_s\" value=\"{sound_max}\"><br>\
             qualidade do vídeo: <select name=\"video_sticker_mode\"><option value=\"silicon\"{sd}>silicon (512 nítido, até 15–20 fps, mais pesado de converter)</option><option value=\"malvin\"{sw}>malvin (320 esticado, 15 fps, mais leve e mais borrado)</option></select><br>\
             <h2>Monitor</h2>\
             repetido ignorado por (min, 1-1440): <input type=\"number\" min=\"1\" max=\"1440\" name=\"dedupe_min\" value=\"{dedupe}\"><br>\
             alertas por hora (1-1000): <input type=\"number\" min=\"1\" max=\"1000\" name=\"alertas_hora\" value=\"{por_hora}\"><br>\
             guardar histórico por (dias, 1-365): <input type=\"number\" min=\"1\" max=\"365\" name=\"retencao_dias\" value=\"{retencao}\"><br>\
             silêncio de (HH:MM, vazio = desligado): <input name=\"silencio_ini\" value=\"{}\" placeholder=\"23:00\" size=\"6\"> \
             até <input name=\"silencio_fim\" value=\"{}\" placeholder=\"07:00\" size=\"6\"><br>\
             <button>Salvar</button></form>",
            esc(msg),
            esc(&owner),
            esc(&pack),
            esc(&publisher),
            esc(&sil_ini),
            esc(&sil_fim)
        ),
    )
}

fn rules_page(st: &AppState, msg: &str) -> String {
    let mut body = format!(
        "<p>{}</p><p><small>Os padrões (regex) casam contra o texto já em minúsculas e sem acento: escreva o padrão assim. \
         \"Ignorar\" vence as regras.</small></p>",
        esc(msg)
    );
    for (id, nome, padrao, ativa) in st.store.rules().unwrap_or_default() {
        let (next, btn, cls) = if ativa {
            ("0", "Desativar", " class=\"off\"")
        } else {
            ("1", "Ativar", "")
        };
        body += &format!(
            "<div class=\"card\"><form class=\"row\" method=\"post\" action=\"/rules\">\
             <input type=\"hidden\" name=\"action\" value=\"edit\"><input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <input name=\"nome\" value=\"{n}\" required> <input name=\"padrao\" value=\"{p}\" required> \
             <span class=\"tag{tc}\">{st_}</span> <button>Salvar</button></form>\
             <form class=\"row\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <button{cls} name=\"action\" value=\"{act}\">{btn}</button> <button class=\"off\" name=\"action\" value=\"del\">Apagar</button></form></div>",
            n = esc(&nome),
            p = esc(&padrao),
            tc = if ativa { " on" } else { "" },
            st_ = if ativa { "ativa" } else { "inativa" },
            act = if next == "1" { "on" } else { "off" },
        );
    }
    body += "<h2>Nova regra</h2><form class=\"card row\" method=\"post\" action=\"/rules\">\
             <input type=\"hidden\" name=\"action\" value=\"add\"><input name=\"nome\" placeholder=\"nome\" required> \
             <input name=\"padrao\" placeholder=\"padrão (regex)\" required> <button>Adicionar</button></form>\
             <h2>Ignorar</h2>";
    for (id, padrao) in st.store.ignores().unwrap_or_default() {
        body += &format!(
            "<div class=\"card row\"><code>{}</code><form method=\"post\" action=\"/rules\">\
             <input type=\"hidden\" name=\"action\" value=\"ign_del\"><input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <button class=\"off\">Apagar</button></form></div>",
            esc(&padrao)
        );
    }
    body += "<form class=\"card row\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"action\" value=\"ign_add\">\
             <input name=\"padrao\" placeholder=\"padrão a ignorar (regex)\" required> <button>Adicionar</button></form>\
             <h2>Testar texto</h2><form class=\"card row\" method=\"post\" action=\"/rules\">\
             <input type=\"hidden\" name=\"action\" value=\"test\"><input name=\"texto\" placeholder=\"texto de exemplo\" style=\"flex:1\" required> \
             <button>Testar</button></form>";
    page("Regras", &body)
}

async fn rules_get(State(st): S) -> Html<String> {
    Html(rules_page(&st, ""))
}

async fn rules_post(State(st): S, Form(f): Fields) -> Response {
    let get = |k: &str| f.get(k).map_or("", |s| s.trim());
    let bad = |m: &str| (StatusCode::BAD_REQUEST, Html(rules_page(&st, m))).into_response();
    let id = get("id").parse::<i64>().unwrap_or(-1);
    let (nome, padrao) = (get("nome"), get("padrao"));
    let action = get("action");
    if action == "test" {
        let det = st
            .detector
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let m = match det.avalia(get("texto")) {
            Some(r) => format!("Casou a regra: {}", r.nome),
            None => "Nenhuma regra casou.".to_string(),
        };
        return Html(rules_page(&st, &m)).into_response();
    }
    if matches!(action, "add" | "edit" | "ign_add") {
        if padrao.is_empty() || (action != "ign_add" && nome.is_empty()) {
            return bad("Nome e padrão não podem ser vazios.");
        }
        if let Err(e) = crate::detector::valida(padrao) {
            return bad(&e);
        }
    }
    let res = match action {
        "add" => st.store.add_rule(nome, padrao).map(|_| ()),
        "edit" => st.store.update_rule(id, nome, padrao),
        "on" | "off" => st.store.set_rule_active(id, action == "on"),
        "del" => st.store.delete_rule(id),
        "ign_add" => st.store.add_ignore(padrao).map(|_| ()),
        "ign_del" => st.store.delete_ignore(id),
        _ => return bad("Ação inválida."),
    };
    match res
        .map_err(|e| e.to_string())
        .and_then(|_| st.recarrega_detector())
    {
        Ok(()) => Html(rules_page(&st, "Salvo.")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(rules_page(&st, &format!("Erro ao salvar: {e}"))),
        )
            .into_response(),
    }
}

async fn alerts_get(State(st): S) -> Html<String> {
    let mut rows = String::new();
    for (ts, regra, resumo) in st.store.alerts(100).unwrap_or_default() {
        rows += &format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            crate::state::local_hms(ts),
            esc(&regra),
            esc(&resumo)
        );
    }
    Html(page(
        "Alertas",
        &format!("<table><tr><th>Hora</th><th>Regra</th><th>Resumo</th></tr>{rows}</table>"),
    ))
}

fn qr_svg(code: &str) -> Option<String> {
    let q = qrcode::QrCode::new(code.as_bytes()).ok()?;
    Some(
        q.render::<qrcode::render::svg::Color>()
            .min_dimensions(260, 260)
            .build(),
    )
}

/// Fragmento do estado do pareamento (trocado a cada 2 s pelo JS da página).
fn pair_fragment(st: &AppState) -> String {
    let (connected, number) = {
        let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
        (s.connected, s.number.clone())
    };
    if connected {
        return format!(
            "<div class=\"card\">Conectado como <b>{}</b>.</div>",
            esc(number.as_deref().unwrap_or("?"))
        );
    }
    let p = st.lock_pairing();
    let mut o = String::new();
    if let Some(code) = &p.code {
        o += &format!(
            "<div class=\"card\">Código de pareamento (WhatsApp &gt; Aparelhos conectados &gt; Conectar com número):<div class=\"code\">{}</div></div>",
            esc(code)
        );
    }
    match p.qr.as_deref().and_then(qr_svg) {
        Some(svg) => {
            let left = (p.at + p.ttl).saturating_sub(crate::state::now());
            o += &format!(
                "<div class=\"card\">Escaneie no WhatsApp (Aparelhos conectados). Este QR vale ~{left}s e troca sozinho.<br>{svg}</div>"
            );
        }
        None if p.exhausted => {
            o += "<div class=\"card\">O QR expirou. Use <b>Tentar de novo</b>.</div>"
        }
        None if p.code.is_none() => {
            o += "<div class=\"card\">Aguardando o WhatsApp gerar o QR…</div>"
        }
        None => {}
    }
    o
}

async fn pair_state(State(st): S) -> Html<String> {
    Html(pair_fragment(&st))
}

fn pair_page(st: &AppState, msg: &str) -> String {
    let phone = st.lock_pairing().phone.clone().unwrap_or_default();
    page(
        "Pareamento",
        &format!(
            "<p>{}</p><div id=\"s\">{}</div>\
             <form class=\"card\" method=\"post\" action=\"/pair\">\
             Número (opcional, para parear por código): <input name=\"phone\" value=\"{}\" placeholder=\"5511999998888\" inputmode=\"numeric\"><br>\
             <small>Só dígitos: DDI 55 + DDD + número, sem +, espaço ou traço. Ex.: <code>5511999998888</code>. Vazio = QR.</small><br><br>\
             <button name=\"action\" value=\"save\">Salvar e reiniciar pareamento</button> \
             <button class=\"off\" name=\"action\" value=\"retry\">Tentar de novo</button></form>\
             <script>setInterval(async()=>{{try{{const r=await fetch('/pair/state');if(r.redirected){{location.reload();return}}s.innerHTML=await r.text()}}catch(e){{}}}},2000)</script>",
            esc(msg),
            pair_fragment(st),
            esc(&phone)
        ),
    )
}

async fn pair_get(State(st): S) -> Html<String> {
    Html(pair_page(&st, ""))
}

async fn pair_post(State(st): S, Form(f): Fields) -> Response {
    let connected = st
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connected;
    if connected {
        return (
            StatusCode::CONFLICT,
            Html(pair_page(&st, "Já está pareado.")),
        )
            .into_response();
    }
    if f.get("action").map(String::as_str) != Some("retry") {
        let phone = f.get("phone").map_or("", |s| s.trim());
        if !phone.is_empty() && !crate::valid_phone(phone) {
            return (
                StatusCode::BAD_REQUEST,
                Html(pair_page(
                    &st,
                    "Número inválido: só dígitos com DDI (8 a 15), ex.: 5511999998888.",
                )),
            )
                .into_response();
        }
        st.lock_pairing().phone = (!phone.is_empty()).then(|| phone.to_string());
    }
    st.restart.notify_one();
    Html(pair_page(&st, "Reiniciando o pareamento…")).into_response()
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
    // ausente = mantém o valor atual (formulários antigos); presente e inválido = 400 sem gravar
    let sound_max = match f.get("sticker_sound_max_s") {
        None => st.store.sticker_sound_max_s().unwrap_or(10),
        Some(v) => match v
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=10).contains(n))
        {
            Some(n) => n,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(settings_page(
                        &st,
                        "Segundos de som deve ser um inteiro de 1 a 10.",
                    )),
                )
                    .into_response();
            }
        },
    };
    let sound_on = f.contains_key("sticker_sound");
    let owner = f.get("owner_jid").map_or("", |s| s.trim());
    let field = |k: &str| f.get(k).map_or("", |s| s.trim()).to_string();
    let (pack, publisher) = (field("sticker_pack_name"), field("sticker_publisher"));
    if pack.chars().count() > crate::stickers::MAX_FIELD
        || publisher.chars().count() > crate::stickers::MAX_FIELD
    {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                &st,
                "Nome e autor aceitam até 64 caracteres.",
            )),
        )
            .into_response();
    }
    // monitor: campo ausente mantém o valor atual; tudo é validado antes de gravar qualquer coisa
    let bad = |m: &str| (StatusCode::BAD_REQUEST, Html(settings_page(&st, m))).into_response();
    let mut mon = [0u64; 3];
    for (i, (k, atual, max)) in [
        ("dedupe_min", st.store.dedupe_min().unwrap_or(30), 1440),
        ("alertas_hora", st.store.alertas_hora().unwrap_or(10), 1000),
        ("retencao_dias", st.store.retencao_dias().unwrap_or(7), 365),
    ]
    .into_iter()
    .enumerate()
    {
        mon[i] = match f.get(k) {
            None => atual,
            Some(v) => match v.trim().parse::<u64>() {
                Ok(n) if (1..=max).contains(&n) => n,
                _ => return bad(&format!("{k} deve ser um inteiro de 1 a {max}.")),
            },
        };
    }
    // `set_silencio` valida as duas pontas antes de gravar
    if (f.contains_key("silencio_ini") || f.contains_key("silencio_fim"))
        && let Err(e) = st.store.set_silencio(
            field("silencio_ini").as_str(),
            field("silencio_fim").as_str(),
        )
    {
        return bad(&e.to_string());
    }
    let res = st
        .store
        .set_dedupe_min(mon[0])
        .and_then(|_| st.store.set_alertas_hora(mon[1]))
        .and_then(|_| st.store.set_retencao_dias(mon[2]))
        .and_then(|_| st.store.set_sticker_meta(&pack, &publisher))
        .and_then(|_| st.store.set_sticker_per_min(per_min))
        .and_then(|_| st.store.set_sticker_sound(sound_on))
        .and_then(|_| st.store.set_sticker_sound_max_s(sound_max))
        .and_then(|_| match f.get("video_sticker_mode") {
            Some(m) => st.store.set_video_sticker_mode(m.trim()),
            None => Ok(()),
        })
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

const PLATS: [(Platform, &str, &str); 2] = [
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

fn plat_of(name: &str) -> Option<(Platform, &'static str, &'static str)> {
    PLATS.iter().copied().find(|p| p.0.name() == name)
}

fn dl_status(st: &AppState, dir: &std::path::Path, plat: Platform) -> String {
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
fn dl_page(st: &AppState, dir: &std::path::Path, msg: &str) -> String {
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

fn dl_reply(st: &AppState, code: StatusCode, msg: &str) -> Response {
    (code, Html(dl_page(st, &cookies_dir(), msg))).into_response()
}

async fn dl_get(State(st): S) -> Html<String> {
    Html(dl_page(&st, &cookies_dir(), ""))
}

async fn dl_limit(State(st): S, Form(f): Fields) -> Response {
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
fn apply_upload(dir: &std::path::Path, plat: Platform, bytes: &[u8]) -> Result<(), &'static str> {
    let json = std::str::from_utf8(bytes).map_err(|_| "Arquivo inválido.")?;
    let txt = cookies::json_to_netscape(json, plat)
        .map_err(|_| "JSON inválido ou sem cookies desta plataforma.")?;
    cookies::save(dir, plat, &txt).map_err(|_| "Não foi possível gravar.")?;
    Ok(())
}

async fn dl_save(State(st): S, Path(name): Path<String>, mut mp: Multipart) -> Response {
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

async fn dl_test(State(st): S, Path(name): Path<String>) -> Response {
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

async fn dl_remove(State(st): S, Path(name): Path<String>) -> Response {
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
                Request::post("https://box:8787/login")
                    .header("origin", "https://box:8787")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("password=x"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(r.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn proxy_host_is_opt_in() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "127.0.0.1:8787".parse().unwrap());
        h.insert(header::ORIGIN, "https://bot.casa".parse().unwrap());
        let uri = axum::http::Uri::from_static("/login");
        assert!(!same_origin(&h, &uri, None));
        assert!(!same_origin(&h, &uri, Some("outro.casa")));
        assert!(same_origin(&h, &uri, Some("bot.casa")));
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
    async fn first_access_sets_password() {
        let (app, st) = app();
        st.store.set("password_hash", "").unwrap();
        let r = app
            .clone()
            .oneshot(post("/login", "password=a&password2=b", None, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let r = app
            .oneshot(post("/login", "password=novo&password2=novo", None, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        assert!(verify_password(
            &st.store.password_hash().unwrap().unwrap(),
            "novo"
        ));
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

    #[test]
    fn normalize_phone_cases() {
        assert_eq!(
            normalize_phone("(11) 99999-8888").as_deref(),
            Some("5511999998888")
        );
        assert_eq!(
            normalize_phone("+55 11 99999-8888").as_deref(),
            Some("5511999998888")
        );
        assert_eq!(normalize_phone("1199").as_deref(), None);
        assert_eq!(normalize_phone("abc"), None);
    }

    #[tokio::test]
    async fn contacts_validation_and_remove() {
        let (app, st) = app();
        let tok = st.sessions.create();
        let r = app
            .clone()
            .oneshot(
                Request::get("/contacts")
                    .header("host", "localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        let r = app
            .clone()
            .oneshot(post("/contacts", "action=add&phone=abc", None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        // número válido, mas bot desconectado: nada gravado
        let r = app
            .clone()
            .oneshot(post(
                "/contacts",
                "action=add&phone=11999998888",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(st.store.dm_commands().unwrap().is_empty());
        // remover só vale para DM com papel comando
        st.store
            .set_role("1@lid", Role::Comando, Some("Ana"))
            .unwrap();
        st.store.set_role("g@g.us", Role::Comando, None).unwrap();
        let r = app
            .clone()
            .oneshot(post(
                "/contacts",
                "action=del&jid=g%40g.us",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let r = app
            .clone()
            .oneshot(post(
                "/contacts",
                "action=del&jid=1%40lid",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(st.store.dm_commands().unwrap().is_empty());
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
    async fn settings_post_saves_sticker_sound() {
        let (app, st) = app();
        let tok = st.sessions.create();
        let base = "delay_min_ms=1000&delay_max_ms=2000&owner_jid=&sticker_per_min=3";
        let r = app
            .clone()
            .oneshot(post(
                "/settings",
                &format!("{base}&sticker_sound=1&sticker_sound_max_s=6"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(st.store.sticker_sound().unwrap());
        assert_eq!(st.store.sticker_sound_max_s().unwrap(), 6);
        // checkbox ausente desliga; segundos fora da faixa = 400 sem gravar nada
        let r = app
            .clone()
            .oneshot(post(
                "/settings",
                &format!("{base}&sticker_sound_max_s=11"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert!(st.store.sticker_sound().unwrap());
        let r = app
            .clone()
            .oneshot(post("/settings", base, None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(!st.store.sticker_sound().unwrap());
        assert_eq!(st.store.sticker_sound_max_s().unwrap(), 6);
    }

    #[tokio::test]
    async fn metrics_exige_login_e_mostra_ram() {
        let (app, st) = app();
        let get = |c: Option<&str>| {
            let mut b = Request::get("/metrics").header("host", "localhost");
            if let Some(c) = c {
                b = b.header("cookie", format!("silicon_session={c}"));
            }
            b.body(Body::empty()).unwrap()
        };
        let r = app.clone().oneshot(get(None)).await.unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        let tok = st.sessions.create();
        let r = app.oneshot(get(Some(&tok))).await.unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
        let b = axum::body::to_bytes(r.into_body(), 4096).await.unwrap();
        assert!(String::from_utf8_lossy(&b).contains("RAM"));
    }

    #[tokio::test]
    async fn pair_requires_login_validates_phone_and_csrf() {
        let (app, st) = app();
        let r = app
            .clone()
            .oneshot(
                Request::get("/pair/state")
                    .header("host", "localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        // o JS da página detecta este redirect (fetch.redirected) e recarrega
        assert_eq!(r.headers()[header::LOCATION], "/login");
        let tok = st.sessions.create();
        let r = app
            .clone()
            .oneshot(post("/pair", "phone=abc", None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let r = app
            .clone()
            .oneshot(post(
                "/pair",
                "phone=5511999998888",
                Some("http://evil.com"),
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = app
            .oneshot(post(
                "/pair",
                "phone=5511999998888&action=save",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(st.lock_pairing().phone.as_deref(), Some("5511999998888"));
    }

    #[test]
    fn pair_fragment_shows_qr_then_expired() {
        let (_, st) = app();
        {
            let mut p = st.lock_pairing();
            p.qr = Some("2@abc,def,ghi".into());
            p.ttl = 20;
            p.at = crate::state::now();
        }
        assert!(pair_fragment(&st).contains("<svg"));
        {
            let mut p = st.lock_pairing();
            p.qr = None;
            p.exhausted = true;
        }
        assert!(pair_fragment(&st).contains("expirou"));
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
        let longo = format!(
            "{base}&sticker_per_min=7&sticker_pack_name={}",
            "a".repeat(65)
        );
        let r = app
            .clone()
            .oneshot(post("/settings", &longo, None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
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

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut b = Request::get(uri).header("host", "localhost");
        if let Some(c) = cookie {
            b = b.header("cookie", format!("silicon_session={c}"));
        }
        b.body(Body::empty()).unwrap()
    }

    async fn body_of(r: Response) -> String {
        let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        String::from_utf8_lossy(&b).into_owned()
    }

    #[tokio::test]
    async fn rules_alerts_auth_csrf_e_fluxo() {
        let (app, st) = app();
        for u in ["/rules", "/alerts"] {
            let r = app.clone().oneshot(get(u, None)).await.unwrap();
            assert_eq!(r.status(), StatusCode::SEE_OTHER);
        }
        let tok = st.sessions.create();
        let r = app
            .clone()
            .oneshot(post(
                "/rules",
                "action=add&nome=x&padrao=a",
                Some("http://evil.com"),
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        // regex inválida e campos vazios: 400 sem gravar (a regra "bug" já vem semeada)
        let antes = st.store.rules().unwrap().len();
        for b in [
            "action=add&nome=x&padrao=%28",
            "action=add&nome=&padrao=a",
            "action=add&nome=x&padrao=",
        ] {
            let r = app
                .clone()
                .oneshot(post("/rules", b, None, Some(&tok)))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        }
        assert_eq!(st.store.rules().unwrap().len(), antes);
        // texto do usuário volta escapado (nome e mensagem de erro da regex)
        let r = app
            .clone()
            .oneshot(post(
                "/rules",
                "action=add&nome=%3Cb%3E&padrao=%28%3Cb%3E",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        let h = body_of(r).await;
        assert!(!h.contains("(<b>") && h.contains("&lt;b&gt;"));
        // regra válida passa a casar no "testar texto"
        let r = app
            .clone()
            .oneshot(post(
                "/rules",
                "action=add&nome=Promo&padrao=promocao",
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let testa = |app: Router| {
            let req = post(
                "/rules",
                "action=test&texto=Super+PROMO%C3%87%C3%83O",
                None,
                Some(&tok),
            );
            async move { body_of(app.oneshot(req).await.unwrap()).await }
        };
        assert!(testa(app.clone()).await.contains("Casou a regra: Promo"));
        // desativar para de casar
        let id = st
            .store
            .rules()
            .unwrap()
            .iter()
            .find(|r| r.1 == "Promo")
            .unwrap()
            .0;
        app.clone()
            .oneshot(post(
                "/rules",
                &format!("action=off&id={id}"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert!(testa(app.clone()).await.contains("Nenhuma regra casou"));
        // alerta gravado aparece escapado
        st.store
            .log_alert(1_700_000_000, "<b>r</b>", "<script>x</script>")
            .unwrap();
        let h = body_of(app.oneshot(get("/alerts", Some(&tok))).await.unwrap()).await;
        assert!(h.contains("&lt;script&gt;") && !h.contains("<script>x"));
    }

    #[tokio::test]
    async fn settings_monitor_valida_e_grava() {
        let (app, st) = app();
        let tok = st.sessions.create();
        let base = "delay_min_ms=1000&delay_max_ms=2000&sticker_per_min=3&dedupe_min=15&alertas_hora=5&retencao_dias=3";
        let r = app
            .clone()
            .oneshot(post(
                "/settings",
                &format!("{base}&silencio_ini=23%3A00&silencio_fim=07%3A00"),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            (
                st.store.dedupe_min().unwrap(),
                st.store.silencio_ini().unwrap()
            ),
            (15, "23:00".into())
        );
        let r = app
            .oneshot(post(
                "/settings",
                &format!("{base}&silencio_ini=25%3A00&silencio_fim="),
                None,
                Some(&tok),
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert_eq!(st.store.silencio_ini().unwrap(), "23:00");
    }

    #[tokio::test]
    async fn dl_limit_validates_and_saves() {
        let (app, st) = app();
        let tok = st.sessions.create();
        for bad in ["0", "11", "x"] {
            let r = app
                .clone()
                .oneshot(post(
                    "/downloader/limit",
                    &format!("dl_per_min={bad}"),
                    None,
                    Some(&tok),
                ))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        }
        let r = app
            .oneshot(post("/downloader/limit", "dl_per_min=5", None, Some(&tok)))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(st.store.dl_per_min().unwrap(), 5);
    }

    #[test]
    fn dl_page_without_cookie_and_bad_upload() {
        let (_, st) = app();
        let dir = tempfile::tempdir().unwrap();
        assert!(dl_page(&st, dir.path(), "").contains("sem cookie"));
        assert!(apply_upload(dir.path(), Platform::TikTok, b"nao e json").is_err());
        assert!(apply_upload(dir.path(), Platform::TikTok, &[0xff, 0xfe]).is_err());
        assert!(!cookies::path(dir.path(), Platform::TikTok).exists());
    }

    #[test]
    fn esc_and_bind() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(effective_bind("0.0.0.0:9000", false), "127.0.0.1:9000");
        assert_eq!(effective_bind("0.0.0.0:9000", true), "0.0.0.0:9000");
        assert_eq!(effective_bind("lixo", false), "127.0.0.1:8787");
        assert!(is_loopback("127.0.0.1:80") && !is_loopback("0.0.0.0:80"));
    }
}
