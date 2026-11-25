use crate::panel::*;

pub(crate) fn qr_svg(code: &str) -> Option<String> {
    let q = qrcode::QrCode::new(code.as_bytes()).ok()?;
    Some(
        q.render::<qrcode::render::svg::Color>()
            .min_dimensions(260, 260)
            .build(),
    )
}

/// Fragmento do estado do pareamento (atualizado por polling na página).
pub(crate) fn pair_fragment(st: &AppState) -> String {
    let (connected, number) = {
        let s = st.status.lock().unwrap_or_else(|e| e.into_inner());
        (s.connected, s.number.clone())
    };
    if connected {
        return format!(
            "<div class=\"card row\">{}<span>Conectado como <b>{}</b>.</span></div>",
            status_dot("ok", "Conectado"),
            esc(number.as_deref().unwrap_or("?"))
        );
    }
    let p = st.lock_pairing();
    let mut o = String::new();
    if let Some(code) = &p.code {
        o += &format!(
            "<div class=\"card\"><p class=\"muted\">Código de pareamento (WhatsApp &gt; Aparelhos conectados &gt; Conectar com número):</p><div class=\"code-big mono\">{}</div></div>",
            esc(code)
        );
    }
    match p.qr.as_deref().and_then(qr_svg) {
        Some(svg) => {
            let left = (p.at + p.ttl).saturating_sub(crate::state::now());
            o += &format!(
                "<div class=\"card\"><p class=\"muted\">Escaneie no WhatsApp (Aparelhos conectados). Este QR vale ~{left}s e troca sozinho.</p><div class=\"qr\">{svg}</div></div>"
            );
        }
        None if p.exhausted => {
            o += &format!(
                "<div class=\"card row\">{}<span>O QR expirou. Use <b>Tentar de novo</b>.</span></div>",
                badge("expirou", "warn")
            )
        }
        None if p.code.is_none() => {
            o += &format!(
                "<div class=\"card row\">{}<span>Aguardando o WhatsApp gerar o QR…</span></div>",
                badge("aguardando", "info")
            )
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
    let connected = st
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .connected;
    // conectado: só o estado; os controles de pareamento saem da tela
    let form = if connected {
        String::new()
    } else {
        card(
            "Parear",
            &format!(
                "<form class=\"stack\" method=\"post\" action=\"/pair\">{}\
                 <div class=\"row\"><button class=\"btn btn-primary\" name=\"action\" value=\"save\">Salvar e reiniciar pareamento</button>\
                 <button class=\"btn\" name=\"action\" value=\"retry\">Tentar de novo</button></div></form>",
                field(
                    "Número (opcional, para parear por código)",
                    "phone",
                    &input(
                        "phone",
                        &phone,
                        "placeholder=\"5511999998888\" inputmode=\"numeric\""
                    ),
                    "Só dígitos: DDI 55 + DDD + número, sem +, espaço ou traço. Vazio = QR."
                )
            ),
        )
    };
    page(
        st,
        "Pareamento",
        &format!(
            "{}{}<div id=\"s\" data-poll=\"/pair/state\" data-interval=\"5000\">{}</div>{form}",
            flash(msg),
            page_header("Pareamento", "Conecte o bot ao WhatsApp.", ""),
            pair_fragment(st),
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
