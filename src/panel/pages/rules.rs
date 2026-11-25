use crate::panel::*;

/// Resultado do teste de texto, com selo visual (casou / nenhuma).
fn test_result(msg: &str) -> Option<String> {
    let (tone, label) = if msg.starts_with("Casou a regra") {
        ("ok", "Casou")
    } else if msg.starts_with("Nenhuma regra") {
        ("info", "Sem correspondência")
    } else {
        return None;
    };
    Some(format!(
        "<div class=\"card row\">{}<span>{}</span></div>",
        badge(label, tone),
        esc(msg)
    ))
}

pub(crate) fn rules_page(st: &AppState, msg: &str) -> String {
    let result = test_result(msg);
    let flash_html = if result.is_some() {
        String::new()
    } else {
        flash(msg)
    };
    let mut list = String::new();
    for (id, nome, padrao, ativa) in st.store.rules().unwrap_or_default() {
        let (act, txt) = if ativa {
            ("off", "Desativar")
        } else {
            ("on", "Ativar")
        };
        list += &format!(
            "<div class=\"card\" data-q=\"{q}\"><form class=\"row\" method=\"post\" action=\"/rules\">\
             <input type=\"hidden\" name=\"action\" value=\"edit\"><input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <input class=\"input\" name=\"nome\" value=\"{n}\" aria-label=\"Nome\" required>\
             <input class=\"input mono grow\" name=\"padrao\" value=\"{p}\" aria-label=\"Padrão (regex)\" required>\
             {badge} <button class=\"btn btn-sm\">Salvar</button></form>\
             <div class=\"row\"><form class=\"inline\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"id\" value=\"{id}\">\
             <button class=\"btn btn-sm btn-ghost\" name=\"action\" value=\"{act}\">{txt}</button></form>{del}</div></div>",
            q = esc(&format!("{nome} {padrao}").to_lowercase()),
            n = esc(&nome),
            p = esc(&padrao),
            badge = badge(
                if ativa { "ativa" } else { "inativa" },
                if ativa { "ok" } else { "muted" }
            ),
            del = confirm_form(
                "/rules",
                &[("action", "del"), ("id", &id.to_string())],
                "Apagar",
                &format!("Apagar a regra {nome}?"),
                "btn-sm btn-danger"
            ),
        );
    }
    if list.is_empty() {
        list = empty("rules", "Nenhuma regra cadastrada.");
    }
    let mut ign = String::new();
    for (id, padrao) in st.store.ignores().unwrap_or_default() {
        ign += &format!(
            "<div class=\"list-row\"><code class=\"mono grow\">{}</code>{}</div>",
            esc(&padrao),
            confirm_form(
                "/rules",
                &[("action", "ign_del"), ("id", &id.to_string())],
                "Apagar",
                "Apagar este padrão ignorado?",
                "btn-sm btn-danger"
            ),
        );
    }
    if ign.is_empty() {
        ign = "<p class=\"muted\">Nenhum padrão ignorado.</p>".into();
    }
    let new_rule = card(
        "Nova regra",
        &format!(
            "<form class=\"row\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"action\" value=\"add\">{}{}{}</form>",
            input(
                "nome",
                "",
                "placeholder=\"nome\" aria-label=\"Nome\" required"
            ),
            input(
                "padrao",
                "",
                "placeholder=\"padrão (regex)\" aria-label=\"Padrão\" required"
            ),
            btn("Adicionar", "primary", "")
        ),
    );
    let ignore = card(
        "Ignorar",
        &format!(
            "<p class=\"muted\">Texto que casa com um destes padrões nunca gera alerta.</p>{ign}\
             <form class=\"row\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"action\" value=\"ign_add\">{}{}</form>",
            input(
                "padrao",
                "",
                "placeholder=\"padrão a ignorar (regex)\" aria-label=\"Padrão a ignorar\" required"
            ),
            btn("Adicionar", "", "")
        ),
    );
    let test = card(
        "Testar texto",
        &format!(
            "<form class=\"row\" method=\"post\" action=\"/rules\"><input type=\"hidden\" name=\"action\" value=\"test\">{}{}</form>{}",
            input(
                "texto",
                "",
                "placeholder=\"texto de exemplo\" aria-label=\"Texto de exemplo\" required"
            ),
            btn("Testar", "", ""),
            result.unwrap_or_default()
        ),
    );
    page(
        st,
        "Regras",
        &format!(
            "{flash_html}{}{}<div class=\"stack\" id=\"rule-list\">{list}</div>{new_rule}{ignore}{test}",
            page_header(
                "Regras",
                "Os padrões (regex) casam contra o texto já em minúsculas e sem acento. \"Ignorar\" vence as regras.",
                ""
            ),
            search("#rule-list", "Buscar regra…"),
        ),
    )
}

pub(crate) async fn rules_get(State(st): S) -> Html<String> {
    Html(rules_page(&st, ""))
}

pub(crate) async fn rules_post(State(st): S, Form(f): Fields) -> Response {
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
