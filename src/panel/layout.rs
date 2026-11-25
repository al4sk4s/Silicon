//! Casca das páginas: sidebar + topbar + área de conteúdo (contrato em ui-contract.md).
use crate::panel::*;

/// (grupo, ícone, título/página, href). O título da página marca o item ativo.
const NAV: [(&str, &str, &str, &str); 8] = [
    ("Overview", "home", "Dashboard", "/"),
    ("Gerenciamento", "chat", "Chats", "/chats"),
    ("Gerenciamento", "users", "Contatos", "/contacts"),
    ("Automação", "rules", "Regras", "/rules"),
    ("Automação", "bell", "Alertas", "/alerts"),
    ("Sistema", "bug", "Debug", "/debug"),
    ("Sistema", "qr", "Pareamento", "/pair"),
    ("Sistema", "download", "Downloader", "/downloader"),
];

/// Itens do rodapé da sidebar.
const NAV_FOOT: [(&str, &str, &str); 2] = [
    ("settings", "Ajustes", "/settings"),
    ("palette", "Aparência", "/appearance"),
];

fn head(title: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-BR\" data-theme=\"dark\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"dark light\">\
         <title>Silicon - {}</title>\
         <link rel=\"stylesheet\" href=\"{}\"><link rel=\"stylesheet\" href=\"{}\">\
         <script src=\"{}\"></script><script src=\"{}\" defer></script></head>",
        esc(title),
        assets::url("tokens.css"),
        assets::url("components.css"),
        assets::url("theme.js"),
        assets::url("app.js"),
    )
}

const TAIL: &str = "<div class=\"toasts\" id=\"toasts\" role=\"region\" aria-label=\"Avisos\" aria-live=\"polite\"></div>\
<dialog class=\"modal\" id=\"confirm-dlg\" aria-labelledby=\"confirm-title\"><form method=\"dialog\">\
<h2 class=\"modal-title\" id=\"confirm-title\">Confirmar</h2><p class=\"modal-text\"></p>\
<div class=\"modal-actions\"><button value=\"cancel\" class=\"btn btn-ghost\">Cancelar</button>\
<button value=\"ok\" class=\"btn btn-danger\">Confirmar</button></div></form></dialog></body></html>";

fn nav_item(ic: &str, label: &str, href: &str, active: bool) -> String {
    format!(
        "<a class=\"nav-item{}\" href=\"{href}\" data-tip=\"{l}\"{}>{}<span class=\"label\">{l}</span></a>",
        if active { " active" } else { "" },
        if active { " aria-current=\"page\"" } else { "" },
        icon(ic),
        l = esc(label)
    )
}

/// Página completa. `title` também escolhe o item ativo da sidebar.
pub(crate) fn page(st: &AppState, title: &str, body: &str) -> String {
    let mut nav = String::new();
    let mut last = "";
    for (group, ic, name, href) in NAV {
        if group != last {
            if !last.is_empty() {
                nav += "</div>";
            }
            nav +=
                &format!("<div class=\"nav-group\"><div class=\"nav-title label\">{group}</div>");
            last = group;
        }
        nav += &nav_item(ic, name, href, name == title);
    }
    nav += "</div>";
    let mut foot = String::new();
    for (ic, name, href) in NAV_FOOT {
        foot += &nav_item(ic, name, href, name == title);
    }
    let connected = st
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connected;
    let status = if connected {
        status_dot("ok", "Conectado")
    } else {
        status_dot("bad", "Desconectado")
    };
    format!(
        "{head}<body><div class=\"app\">\
         <aside class=\"sidebar\" id=\"sidebar\"><div class=\"brand\">{logo}<span class=\"label\">Silicon</span></div>\
         <nav class=\"nav\" aria-label=\"Principal\">{nav}</nav>\
         <div class=\"sidebar-foot\">{foot}\
         <button type=\"button\" class=\"nav-item\" id=\"sidebar-toggle\" data-tip=\"Recolher\" aria-label=\"Recolher ou expandir a barra lateral\">{panel}<span class=\"label\">Recolher</span></button></div></aside>\
         <div class=\"backdrop\" id=\"backdrop\"></div>\
         <div class=\"main\"><header class=\"topbar\">\
         <button type=\"button\" class=\"icon-btn\" id=\"menu-btn\" aria-label=\"Abrir menu\">{menu}</button>\
         <h1 class=\"topbar-title\">{t}</h1>{status}<div class=\"spacer\"></div>\
         <form class=\"inline\" method=\"post\" action=\"/logout\"><button class=\"btn btn-ghost btn-sm\">{out}<span>Sair</span></button></form></header>\
         <main class=\"content\">{body}</main></div></div>{TAIL}",
        head = head(title),
        logo = icon("logo").replace("class=\"icon\"", "class=\"icon brand-logo\""),
        panel = icon("panel"),
        menu = icon("menu"),
        out = icon("logout"),
        t = esc(title),
    )
}

/// Telas sem sessão (login e primeira senha): cartão centralizado, sem sidebar.
pub(crate) fn bare_page(title: &str, inner: &str) -> String {
    format!(
        "{}<body><main class=\"auth\"><div class=\"auth-card\"><div class=\"brand\">{}<span>Silicon</span></div>{inner}</div></main>{TAIL}",
        head(title),
        icon("logo").replace("class=\"icon\"", "class=\"icon brand-logo\""),
    )
}

pub(crate) fn login_page(msg: &str) -> String {
    bare_page(
        "entrar",
        &format!(
            "{}<form method=\"post\" action=\"/login\" class=\"stack\">{}{}</form>",
            flash(msg),
            field(
                "Senha",
                "password",
                &input(
                    "password",
                    "",
                    "type=\"password\" autofocus autocomplete=\"current-password\" required"
                ),
                ""
            ),
            btn("Entrar", "primary", "")
        ),
    )
}
