use crate::panel::*;

pub(crate) fn qr_svg(code: &str) -> Option<String> {
    let q = qrcode::QrCode::new(code.as_bytes()).ok()?;
    Some(
        q.render::<qrcode::render::svg::Color>()
            .min_dimensions(260, 260)
            .build(),
    )
}

/// Fragmento do estado do pareamento (trocado a cada 2 s pelo JS da página).
pub(crate) fn pair_fragment(st: &AppState) -> String {
    let (connected, number) = {
        let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
        (s.connected, s.number.clone())
    };
    if connected {
        return format!(
            "<div class=\"card\">Conectado como <b>{}</b>.</div>",
            esc(number.as_deref().unwrap_or("?"))
        );
    }
    let p = st.lock_pairing();
    let mut o = String::new();
    if let Some(code) = &p.code {
        o += &format!(
            "<div class=\"card\">Código de pareamento (WhatsApp &gt; Aparelhos conectados &gt; Conectar com número):<div class=\"code\">{}</div></div>",
            esc(code)
        );
    }
    match p.qr.as_deref().and_then(qr_svg) {
        Some(svg) => {
            let left = (p.at + p.ttl).saturating_sub(crate::state::now());
            o += &format!(
                "<div class=\"card\">Escaneie no WhatsApp (Aparelhos conectados). Este QR vale ~{left}s e troca sozinho.<br>{svg}</div>"
            );
        }
        None if p.exhausted => {
            o += "<div class=\"card\">O QR expirou. Use <b>Tentar de novo</b>.</div>"
        }
        None if p.code.is_none() => {
            o += "<div class=\"card\">Aguardando o WhatsApp gerar o QR…</div>"
        }
        None => {}
    }
    o
}

pub(crate) async fn pair_state(State(st): S) -> Html<String> {
    Html(pair_fragment(&st))
}

pub(crate) fn pair_page(st: &AppState, msg: &str) -> String {
    let phone = st.lock_pairing().phone.clone().unwrap_or_default();
    page(
        "Pareamento",
        &format!(
            "<p>{}</p><div id=\"s\">{}</div>\
             <form class=\"card\" method=\"post\" action=\"/pair\">\
             Número (opcional, para parear por código): <input name=\"phone\" value=\"{}\" placeholder=\"5511999998888\" inputmode=\"numeric\"><br>\
             <small>Só dígitos: DDI 55 + DDD + número, sem +, espaço ou traço. Ex.: <code>5511999998888</code>. Vazio = QR.</small><br><br>\
             <button name=\"action\" value=\"save\">Salvar e reiniciar pareamento</button> \
             <button class=\"off\" name=\"action\" value=\"retry\">Tentar de novo</button></form>\
             <script>setInterval(async()=>{{try{{const r=await fetch('/pair/state');if(r.redirected){{location.reload();return}}s.innerHTML=await r.text()}}catch(e){{}}}},2000)</script>",
            esc(msg),
            pair_fragment(st),
            esc(&phone)
        ),
    )
}

pub(crate) async fn pair_get(State(st): S) -> Html<String> {
    Html(pair_page(&st, ""))
}

pub(crate) async fn pair_post(State(st): S, Form(f): Fields) -> Response {
    let connected = st
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connected;
    if connected {
        return (
            StatusCode::CONFLICT,
            Html(pair_page(&st, "Já está pareado.")),
        )
            .into_response();
    }
    if f.get("action").map(String::as_str) != Some("retry") {
        let phone = f.get("phone").map_or("", |s| s.trim());
        if !phone.is_empty() && !crate::valid_phone(phone) {
            return (
                StatusCode::BAD_REQUEST,
                Html(pair_page(
                    &st,
                    "Número inválido: só dígitos com DDI (8 a 15), ex.: 5511999998888.",
                )),
            )
                .into_response();
        }
        st.lock_pairing().phone = (!phone.is_empty()).then(|| phone.to_string());
    }
    st.restart.notify_one();
    Html(pair_page(&st, "Reiniciando o pareamento…")).into_response()
}
