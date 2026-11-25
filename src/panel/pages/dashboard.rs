use crate::panel::*;

pub(crate) fn metrics_fragment(st: &AppState) -> String {
    crate::metrics::texto(crate::metrics::atualizar(&st.metricas), st.boot.elapsed())
}

pub(crate) async fn metrics_get(State(st): S) -> Html<String> {
    Html(metrics_fragment(&st))
}

/// (id, título, tamanho, intervalo de atualização em ms). Ordem = layout padrão.
const WIDGETS: [(&str, &str, &str, u32); 7] = [
    ("conexao", "Conexão", "w-md", 10_000),
    ("sistema", "Sistema", "w-md", 5_000),
    ("pareamento", "Pareamento", "w-md", 5_000),
    ("chats", "Chats por papel", "w-md", 15_000),
    ("downloader", "Downloader", "w-md", 15_000),
    ("eventos", "Eventos recentes", "w-lg", 15_000),
    ("alertas", "Alertas recentes", "w-lg", 15_000),
];

fn kv(k: &str, v: &str) -> String {
    format!(
        "<div class=\"list-row\"><span class=\"muted grow\">{}</span><span>{}</span></div>",
        esc(k),
        esc(v)
    )
}

/// Corpo de um widget (HTML já escapado). `None` = widget desconhecido.
pub(crate) fn widget_body(st: &AppState, name: &str) -> Option<String> {
    Some(match name {
        "conexao" => {
            let (connected, number) = {
                let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
                (s.connected, s.number.clone())
            };
            let s = st.boot.elapsed().as_secs();
            let estado = if connected {
                status_dot("ok", "Conectado")
            } else {
                status_dot("bad", "Desconectado")
            };
            format!(
                "<div class=\"list\"><div class=\"list-row\"><span class=\"muted grow\">Estado</span>{estado}</div>{}{}</div>",
                kv("Número", number.as_deref().unwrap_or("não pareado")),
                kv("Ativo há", &format!("{}h{:02}", s / 3600, s % 3600 / 60)),
            )
        }
        "sistema" => {
            let t = metrics_fragment(st);
            let rows: String = t
                .split(" · ")
                .map(|p| format!("<div class=\"list-row\">{}</div>", esc(p)))
                .collect();
            format!("<div class=\"list\">{rows}</div>")
        }
        "pareamento" => {
            if st
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .connected
            {
                "<p class=\"muted\">Bot pareado.</p><a class=\"btn btn-sm\" href=\"/pair\">Abrir</a>"
                    .to_string()
            } else {
                format!(
                    "{}<a class=\"btn btn-sm\" href=\"/pair\">Abrir pareamento</a>",
                    pair_fragment(st)
                )
            }
        }
        "chats" => {
            let roles = st.roles.read().unwrap_or_else(|e| e.into_inner());
            let mut rows = String::new();
            for r in ["comando", "monitorado", "alerta"] {
                let (mut g, mut d) = (0, 0);
                for (j, role) in roles.iter() {
                    if role.as_str() == r {
                        if j.ends_with("@g.us") { g += 1 } else { d += 1 }
                    }
                }
                rows += &kv(r, &format!("{g} grupos · {d} conversas diretas"));
            }
            format!("<div class=\"list\">{rows}</div>")
        }
        "downloader" => {
            let dir = cookies_dir();
            let rows: String = PLATS
                .iter()
                .map(|(p, label, _)| kv(label, &format!("cookies: {}", dl_status(st, &dir, *p))))
                .collect();
            format!("<div class=\"list\">{rows}</div>")
        }
        "eventos" => {
            let ev = st.events.lock().unwrap_or_else(|e| e.into_inner());
            if ev.is_empty() {
                return Some(empty("clock", "Nenhum evento ainda."));
            }
            let rows: String = ev
                .iter()
                .rev()
                .take(10)
                .map(|e| {
                    format!(
                        "<tr><td data-label=\"Hora\">{}</td><td data-label=\"Tipo\">{}</td><td data-label=\"JID\" class=\"mono\">{}</td></tr>",
                        crate::state::local_hms(e.at),
                        esc(&e.kind),
                        esc(&e.jid)
                    )
                })
                .collect();
            format!(
                "<table class=\"table\"><thead><tr><th>Hora</th><th>Tipo</th><th>JID</th></tr></thead><tbody>{rows}</tbody></table>"
            )
        }
        "alertas" => {
            let al = st.store.alerts(5).unwrap_or_default();
            if al.is_empty() {
                return Some(empty("bell", "Nenhum alerta."));
            }
            let rows: String = al
                .iter()
                .map(|(ts, regra, resumo)| {
                    format!(
                        "<tr><td data-label=\"Hora\">{}</td><td data-label=\"Regra\">{}</td><td data-label=\"Resumo\">{}</td></tr>",
                        crate::state::local_hms(*ts),
                        esc(regra),
                        esc(resumo)
                    )
                })
                .collect();
            format!(
                "<table class=\"table\"><thead><tr><th>Hora</th><th>Regra</th><th>Resumo</th></tr></thead><tbody>{rows}</tbody></table>"
            )
        }
        _ => return None,
    })
}

/// Fragmento de um widget (usado pelo polling do dashboard).
pub(crate) async fn widget_get(State(st): S, Path(name): Path<String>) -> Response {
    match widget_body(&st, &name) {
        Some(h) => Html(h).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(crate) async fn index(State(st): S) -> Html<String> {
    let connected = st
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connected;
    let mut grid = String::new();
    for (id, title, size, every) in WIDGETS {
        grid += &format!(
            "<article class=\"widget {size}\" data-widget=\"{id}\" data-title=\"{t}\">\
             <header class=\"widget-head\"><h3>{t}</h3><div class=\"widget-tools\"></div></header>\
             <div class=\"widget-body\" data-poll=\"/w/{id}\" data-interval=\"{every}\">{}</div></article>",
            widget_body(&st, id).unwrap_or_default(),
            t = esc(title)
        );
    }
    let actions = format!(
        "{}{}",
        btn("Personalizar", "ghost", "type=\"button\" id=\"dash-edit\""),
        btn(
            "Restaurar padrão",
            "ghost",
            "type=\"button\" id=\"dash-reset\" hidden"
        )
    );
    Html(page(
        &st,
        "Dashboard",
        &format!(
            "{}<div id=\"dash-hidden\" class=\"row\" hidden></div>\
             <section id=\"dash\" class=\"dash-grid\" data-editable>{grid}</section>",
            page_header(
                "Visão geral",
                if connected {
                    "O bot está conectado e respondendo."
                } else {
                    "O bot está desconectado."
                },
                &actions
            )
        ),
    ))
}
