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

pub(crate) fn chat_form(jid: &str, label: &str, cur: Option<Role>) -> String {
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
