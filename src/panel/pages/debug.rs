use crate::panel::*;

/// Aba Debug: contagem por categoria e as últimas 300 linhas (mais nova primeiro).
pub(crate) async fn debug_get(State(st): S) -> Html<String> {
    let q = st.debug.lock().unwrap_or_else(|e| e.into_inner());
    let mut cats: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut rows = String::new();
    for d in q.iter() {
        *cats.entry(d.cat.as_str()).or_default() += 1;
    }
    for d in q.iter().rev() {
        let bad = d.cat.contains("erro") || d.cat.contains("recusa");
        rows += &format!(
            "<tr{}><td>{}</td><td>{}</td><td>{}</td></tr>",
            if bad { " style=\"color:#c33\"" } else { "" },
            crate::state::local_hms(d.at),
            esc(&d.cat),
            esc(&d.msg)
        );
    }
    let resumo: String = cats
        .iter()
        .map(|(c, n)| format!("<code>{}</code> × {n} &nbsp; ", esc(c)))
        .collect();
    Html(page(
        "Debug",
        &format!(
            "<div class=\"card\">{resumo}</div>\
             <table><tr><th>Hora</th><th>Categoria</th><th>Detalhe</th></tr>{rows}</table>"
        ),
    ))
}
