use crate::panel::*;

pub(crate) fn rules_page(st: &AppState, msg: &str) -> String {
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
