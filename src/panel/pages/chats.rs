use crate::panel::*;

pub(crate) fn chats_page(st: &AppState, msg: &str) -> String {
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
    let mut list = String::new();
    for (jid, label) in &chats {
        list += &chat_form(jid, label, roles.get(jid).copied());
    }
    if list.is_empty() {
        list = empty(
            "chat",
            "Nenhum chat conhecido ainda. Adicione por JID abaixo.",
        );
    }
    let add = card(
        "Adicionar por JID",
        &format!(
            "<form class=\"row\" method=\"post\" action=\"/chats\">{}<input type=\"hidden\" name=\"role\" value=\"comando\">{}{}</form>",
            input(
                "jid",
                "",
                "placeholder=\"123@g.us\" aria-label=\"JID\" required"
            ),
            input(
                "name",
                "",
                "placeholder=\"nome (opcional)\" aria-label=\"Nome\""
            ),
            btn("Liberar", "primary", "")
        ),
    );
    let body = format!(
        "{}{}{}<div class=\"list\" id=\"chat-list\">{list}</div>{add}",
        flash(msg),
        page_header("Chats", "Quem pode usar o bot e com qual papel.", ""),
        search("#chat-list", "Buscar grupo ou contato…"),
    );
    page(st, "Chats", &body)
}

/// Troca de papel: envia sozinho ao mudar (JS); sem JS aparece o botão "Ok".
fn role_select(cur: Option<Role>) -> String {
    let mut o = format!(
        "<option value=\"\" disabled{}>outro papel…</option>",
        if cur.is_none() { " selected" } else { "" }
    );
    for v in ["comando", "monitorado", "alerta"] {
        o += &format!(
            "<option value=\"{v}\"{}>{v}</option>",
            if cur.is_some_and(|r| r.as_str() == v) {
                " selected"
            } else {
                ""
            }
        );
    }
    format!(
        "<select class=\"select\" name=\"role\" data-autosubmit aria-label=\"Mudar papel\">{o}</select>"
    )
}

pub(crate) fn chat_form(jid: &str, label: &str, cur: Option<Role>) -> String {
    let on = cur == Some(Role::Comando);
    let hidden = format!(
        "<input type=\"hidden\" name=\"jid\" value=\"{j}\"><input type=\"hidden\" name=\"name\" value=\"{l}\">",
        j = esc(jid),
        l = esc(label)
    );
    let (next, txt, kind) = if on {
        ("none", "Bloquear", "")
    } else {
        ("comando", "Liberar", "btn-primary")
    };
    let is_group = jid.ends_with("@g.us");
    let (tone, role_txt) = match cur {
        Some(r) => ("info", r.as_str()),
        None => ("muted", "sem papel"),
    };
    format!(
        "<div class=\"list-row chat\" data-q=\"{q}\">{av}<div class=\"grow\"><b>{name}</b><div class=\"meta mono\">{j}</div></div>\
         {kind_badge}{role_badge}\
         <div class=\"actions\"><form class=\"inline\" method=\"post\" action=\"/chats\">{hidden}<input type=\"hidden\" name=\"role\" value=\"{next}\"><button class=\"btn btn-sm {kind}\">{txt}</button></form>\
         <form class=\"inline\" method=\"post\" action=\"/chats\">{hidden}{sel}<noscript><button class=\"btn btn-sm\">Ok</button></noscript></form></div></div>",
        q = esc(&format!("{label} {jid}").to_lowercase()),
        av = avatar(if label.is_empty() { jid } else { label }),
        name = if label.is_empty() {
            "(sem nome)".into()
        } else {
            esc(label)
        },
        j = esc(jid),
        kind_badge = badge(if is_group { "grupo" } else { "contato" }, "muted"),
        role_badge = badge(role_txt, if on { "ok" } else { tone }),
        sel = role_select(cur),
    )
}

pub(crate) async fn chats_get(State(st): S) -> Html<String> {
    Html(chats_page(&st, ""))
}

pub(crate) async fn chats_post(State(st): S, Form(f): Fields) -> Response {
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
