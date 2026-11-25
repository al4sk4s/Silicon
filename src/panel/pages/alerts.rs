use crate::panel::*;

pub(crate) async fn alerts_get(State(st): S) -> Html<String> {
    let mut rows = String::new();
    for (ts, regra, resumo) in st.store.alerts(100).unwrap_or_default() {
        rows += &format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            crate::state::local_hms(ts),
            esc(&regra),
            esc(&resumo)
        );
    }
    Html(page(
        "Alertas",
        &format!("<table><tr><th>Hora</th><th>Regra</th><th>Resumo</th></tr>{rows}</table>"),
    ))
}
