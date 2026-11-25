use crate::panel::*;

pub(crate) async fn alerts_get(State(st): S) -> Html<String> {
    let mut rows = String::new();
    for (ts, regra, resumo) in st.store.alerts(100).unwrap_or_default() {
        rows += &format!(
            "<tr><td data-label=\"Hora\">{}</td><td data-label=\"Regra\">{}</td><td data-label=\"Resumo\">{}</td></tr>",
            crate::state::local_hms(ts),
            esc(&regra),
            esc(&resumo)
        );
    }
    let content = if rows.is_empty() {
        empty("bell", "Nenhum alerta ainda.")
    } else {
        format!(
            "<table class=\"table\"><thead><tr><th>Hora</th><th>Regra</th><th>Resumo</th></tr></thead><tbody>{rows}</tbody></table>"
        )
    };
    Html(page(
        &st,
        "Alertas",
        &format!(
            "{}{content}",
            page_header(
                "Alertas",
                "Do mais recente para o mais antigo (até 100).",
                ""
            )
        ),
    ))
}
