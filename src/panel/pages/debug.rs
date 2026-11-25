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
        // erro/recusa: classe + rótulo, nunca só cor
        let tag = if bad {
            badge("erro/recusa", "bad")
        } else {
            String::new()
        };
        rows += &format!(
            "<tr{}><td data-label=\"Hora\" class=\"mono\">{}</td><td data-label=\"Categoria\" class=\"mono\">{}{tag}</td><td data-label=\"Detalhe\" class=\"mono\">{}</td></tr>",
            if bad { " class=\"bad-text\"" } else { "" },
            crate::state::local_hms(d.at),
            esc(&d.cat),
            esc(&d.msg)
        );
    }
    let resumo: String = cats
        .iter()
        .map(|(c, n)| {
            format!(
                "<span class=\"badge muted\"><span class=\"mono\">{}</span> × {n}</span> ",
                esc(c)
            )
        })
        .collect();
    let content = if rows.is_empty() {
        empty("bug", "Sem registros de debug.")
    } else {
        format!(
            "<div class=\"row\">{resumo}</div>\
             <table class=\"table\"><thead><tr><th>Hora</th><th>Categoria</th><th>Detalhe</th></tr></thead><tbody>{rows}</tbody></table>"
        )
    };
    Html(page(
        &st,
        "Debug",
        &format!(
            "{}{content}",
            page_header(
                "Debug",
                "Últimas 300 linhas, da mais nova para a mais antiga.",
                ""
            )
        ),
    ))
}
