use crate::panel::*;

pub(crate) fn metrics_fragment(st: &AppState) -> String {
    crate::metrics::texto(crate::metrics::atualizar(&st.metricas), st.boot.elapsed())
}

pub(crate) async fn metrics_get(State(st): S) -> Html<String> {
    Html(metrics_fragment(&st))
}

pub(crate) async fn index(State(st): S) -> Html<String> {
    let (connected, number) = {
        let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
        (s.connected, s.number.clone())
    };
    let mut rows = String::new();
    for e in st
        .events
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .rev()
        .take(50)
    {
        rows += &format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            crate::state::local_hms(e.at),
            esc(&e.kind),
            esc(&e.jid)
        );
    }
    Html(page(
        "Status",
        &format!(
            "<p>Conectado: {}</p><p>Número: {}</p><p id=\"m\">{}</p><h2>Últimos eventos</h2>\
             <table><tr><th>Hora</th><th>Tipo</th><th>JID</th></tr>{rows}</table>\
             <script>setInterval(async()=>{{try{{const r=await fetch('/metrics');if(r.redirected){{location.reload();return}}m.innerHTML=await r.text()}}catch(e){{}}}},5000)</script>",
            if connected { "sim" } else { "não" },
            esc(number.as_deref().unwrap_or("não pareado")),
            metrics_fragment(&st),
        ),
    ))
}
