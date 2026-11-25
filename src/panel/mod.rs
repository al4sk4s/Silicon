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
        .route(
            "/settings/{cat}",
            get(settings_cat_get).post(settings_cat_post),
        )
        .route("/appearance", get(appearance_get))
        .route("/w/{name}", get(widget_get))
        .route("/pair", get(pair_get).post(pair_post))
        .route("/pair/state", get(pair_state))
        .route("/metrics", get(metrics_get))
        .route("/rules", get(rules_get).post(rules_post))
        .route("/alerts", get(alerts_get))
        .route("/debug", get(debug_get))
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
        // estáticos públicos (o login precisa deles): ficam fora do guard e não tocam no AppState
        .merge(Router::new().route("/assets/{v}/{name}", get(assets::asset)))
}

mod assets;
mod auth;
mod components;
mod guard;
mod layout;
mod pages;
#[cfg(test)]
mod tests;

use auth::*;
use components::*;
use guard::*;
use layout::*;
use pages::*;
