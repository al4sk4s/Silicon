use crate::panel::*;

/// "11 99999-8888" -> "5511999998888". Até 11 dígitos assume Brasil (+55); precisa ter 10–15.
pub(crate) fn normalize_phone(s: &str) -> Option<String> {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    let d = if d.len() <= 11 { format!("55{d}") } else { d };
    (12..=15).contains(&d.len()).then_some(d)
}

pub(crate) fn contacts_page(st: &AppState, msg: &str) -> String {
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

pub(crate) async fn contacts_get(State(st): S) -> Html<String> {
    Html(contacts_page(&st, ""))
}

pub(crate) fn contacts_err(st: &AppState, code: StatusCode, msg: &str) -> Response {
    (code, Html(contacts_page(st, msg))).into_response()
}

pub(crate) async fn contacts_post(State(st): S, Form(f): Fields) -> Response {
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
