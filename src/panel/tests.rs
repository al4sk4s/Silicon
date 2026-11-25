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
            &format!("{base}&sticker_per_min=7&sticker_pack_name=Meu+pacote&sticker_publisher=Eu"),
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

#[tokio::test]
async fn debug_page_lists_and_escapes() {
    let (app, st) = app();
    st.dbg("baixar_erro_x", "<b>oi</b>");
    let tok = st.sessions.create();
    let r = app.oneshot(get("/debug", Some(&tok))).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    let s = String::from_utf8_lossy(&b);
    assert!(s.contains("baixar_erro_x") && s.contains("&lt;b&gt;oi"));
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
