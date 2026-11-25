use crate::panel::*;

/// Categorias de Ajustes: (id na URL, título, campos do formulário).
const CATS: [(&str, &str, &[&str]); 4] = [
    ("geral", "Geral", &["owner_jid"]),
    ("mensagens", "Mensagens", &["delay_min_ms", "delay_max_ms"]),
    (
        "figurinhas",
        "Figurinhas",
        &[
            "sticker_per_min",
            "sticker_pack_name",
            "sticker_publisher",
            "sticker_sound",
            "sticker_sound_max_s",
            "video_sticker_mode",
        ],
    ),
    (
        "monitor",
        "Monitoramento",
        &[
            "dedupe_min",
            "alertas_hora",
            "retencao_dias",
            "silencio_ini",
            "silencio_fim",
        ],
    ),
];

fn cat_of(id: &str) -> Option<&'static (&'static str, &'static str, &'static [&'static str])> {
    CATS.iter().find(|c| c.0 == id)
}

/// Valores gravados hoje, no formato do formulário completo (base do POST por categoria).
fn current_fields(st: &AppState) -> HashMap<String, String> {
    let s = &st.store;
    let mut m = HashMap::new();
    let mut put = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };
    put("delay_min_ms", s.delay_min_ms().unwrap_or(1000).to_string());
    put("delay_max_ms", s.delay_max_ms().unwrap_or(2000).to_string());
    put(
        "owner_jid",
        s.owner_jid().ok().flatten().unwrap_or_default(),
    );
    put(
        "sticker_per_min",
        s.sticker_per_min().unwrap_or(3).to_string(),
    );
    put(
        "sticker_pack_name",
        s.sticker_pack_name().unwrap_or_default(),
    );
    put(
        "sticker_publisher",
        s.sticker_publisher().unwrap_or_default(),
    );
    if s.sticker_sound().unwrap_or(true) {
        put("sticker_sound", "1".into());
    }
    put(
        "sticker_sound_max_s",
        s.sticker_sound_max_s().unwrap_or(10).to_string(),
    );
    put(
        "video_sticker_mode",
        s.video_sticker_mode().unwrap_or_default(),
    );
    put("dedupe_min", s.dedupe_min().unwrap_or(30).to_string());
    put("alertas_hora", s.alertas_hora().unwrap_or(10).to_string());
    put("retencao_dias", s.retencao_dias().unwrap_or(7).to_string());
    put("silencio_ini", s.silencio_ini().unwrap_or_default());
    put("silencio_fim", s.silencio_fim().unwrap_or_default());
    m
}

fn tabs(cur: &str) -> String {
    let mut t = String::from("<nav class=\"tabs\" aria-label=\"Categorias de ajustes\">");
    for (id, name, _) in CATS {
        t += &format!(
            "<a class=\"tab{}\" href=\"/settings/{id}\"{}>{name}</a>",
            if id == cur { " active" } else { "" },
            if id == cur {
                " aria-current=\"page\""
            } else {
                ""
            },
        );
    }
    t + "</nav>"
}

pub(crate) fn settings_page(st: &AppState, cat: &str, msg: &str) -> String {
    let Some((id, name, _)) = cat_of(cat) else {
        return settings_page(st, "geral", msg);
    };
    let s = &st.store;
    let (desc, fields) = match *id {
        "geral" => (
            "Quem administra o bot.",
            field(
                "JID do dono (opcional)",
                "owner_jid",
                &input(
                    "owner_jid",
                    &s.owner_jid().ok().flatten().unwrap_or_default(),
                    "placeholder=\"5511999998888@s.whatsapp.net\"",
                ),
                "Só o dono pode usar comandos de administração.",
            ),
        ),
        "mensagens" => (
            "Atraso antes de responder, para parecer humano.",
            format!(
                "<div class=\"form-grid\">{}{}</div>",
                field(
                    "Atraso mínimo (ms)",
                    "delay_min_ms",
                    &number_input(
                        "delay_min_ms",
                        &s.delay_min_ms().unwrap_or(1000).to_string(),
                        "0",
                        "600000"
                    ),
                    ""
                ),
                field(
                    "Atraso máximo (ms)",
                    "delay_max_ms",
                    &number_input(
                        "delay_max_ms",
                        &s.delay_max_ms().unwrap_or(2000).to_string(),
                        "0",
                        "600000"
                    ),
                    ""
                ),
            ),
        ),
        "figurinhas" => {
            let mode = s.video_sticker_mode().unwrap_or_default();
            (
                "Limites e aparência das figurinhas.",
                format!(
                    "<div class=\"form-grid\">{}{}{}{}</div>{}{}",
                    field(
                        "Figurinhas por minuto (1–30)",
                        "sticker_per_min",
                        &number_input(
                            "sticker_per_min",
                            &s.sticker_per_min().unwrap_or(3).to_string(),
                            "1",
                            "30"
                        ),
                        ""
                    ),
                    field(
                        "Segundos máximos de som (1–10)",
                        "sticker_sound_max_s",
                        &number_input(
                            "sticker_sound_max_s",
                            &s.sticker_sound_max_s().unwrap_or(10).to_string(),
                            "1",
                            "10"
                        ),
                        ""
                    ),
                    field(
                        "Nome do pacote (até 64)",
                        "sticker_pack_name",
                        &input(
                            "sticker_pack_name",
                            &s.sticker_pack_name().unwrap_or_default(),
                            "maxlength=\"64\""
                        ),
                        ""
                    ),
                    field(
                        "Autor (até 64)",
                        "sticker_publisher",
                        &input(
                            "sticker_publisher",
                            &s.sticker_publisher().unwrap_or_default(),
                            "maxlength=\"64\""
                        ),
                        ""
                    ),
                    switch(
                        "sticker_sound",
                        "Figurinha com som (vídeo com áudio)",
                        s.sticker_sound().unwrap_or(true)
                    ),
                    field(
                        "Qualidade do vídeo",
                        "video_sticker_mode",
                        &select(
                            "video_sticker_mode",
                            &[
                                (
                                    "silicon",
                                    "silicon: 512 nítido, até 15–20 fps, mais pesado de converter"
                                ),
                                ("malvin", "malvin: mais leve"),
                            ],
                            &mode,
                            "",
                        ),
                        "",
                    ),
                ),
            )
        }
        _ => (
            "Quando e quanto o bot avisa você.",
            format!(
                "<div class=\"form-grid\">{}{}{}</div><div class=\"form-grid\">{}{}</div>",
                field(
                    "Repetido ignorado por (min, 1–1440)",
                    "dedupe_min",
                    &number_input(
                        "dedupe_min",
                        &s.dedupe_min().unwrap_or(30).to_string(),
                        "1",
                        "1440"
                    ),
                    ""
                ),
                field(
                    "Alertas por hora (1–1000)",
                    "alertas_hora",
                    &number_input(
                        "alertas_hora",
                        &s.alertas_hora().unwrap_or(10).to_string(),
                        "1",
                        "1000"
                    ),
                    ""
                ),
                field(
                    "Guardar histórico por (dias, 1–365)",
                    "retencao_dias",
                    &number_input(
                        "retencao_dias",
                        &s.retencao_dias().unwrap_or(7).to_string(),
                        "1",
                        "365"
                    ),
                    ""
                ),
                field(
                    "Silêncio de",
                    "silencio_ini",
                    &input(
                        "silencio_ini",
                        &s.silencio_ini().unwrap_or_default(),
                        "type=\"time\""
                    ),
                    "Vazio nos dois = desligado."
                ),
                field(
                    "Silêncio até",
                    "silencio_fim",
                    &input(
                        "silencio_fim",
                        &s.silencio_fim().unwrap_or_default(),
                        "type=\"time\""
                    ),
                    ""
                ),
            ),
        ),
    };
    page(
        st,
        "Ajustes",
        &format!(
            "{}{}{}<form class=\"stack settings-form\" method=\"post\" action=\"/settings/{id}\" data-dirty>{fields}\
             <div class=\"row\">{}</div></form>",
            flash(msg),
            page_header(name, desc, ""),
            tabs(id),
            btn("Salvar", "primary", "data-save"),
        ),
    )
}

pub(crate) async fn settings_get(State(st): S) -> Html<String> {
    Html(settings_page(&st, "geral", ""))
}

pub(crate) async fn settings_cat_get(State(st): S, Path(cat): Path<String>) -> Response {
    match cat_of(&cat) {
        Some(_) => Html(settings_page(&st, &cat, "")).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// POST por categoria: só os campos dela vêm do formulário; o resto fica como está gravado.
/// A validação e a gravação são as mesmas do formulário completo (`apply_settings`).
pub(crate) async fn settings_cat_post(
    State(st): S,
    Path(cat): Path<String>,
    Form(f): Fields,
) -> Response {
    let Some((_, _, keys)) = cat_of(&cat) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut full = current_fields(&st);
    for k in *keys {
        full.remove(*k);
        if let Some(v) = f.get(*k) {
            full.insert((*k).to_string(), v.clone());
        }
    }
    apply_settings(&st, &full, &cat)
}

/// Formulário completo (legado, sem JS): mesmo comportamento de antes.
pub(crate) async fn settings_post(State(st): S, Form(f): Fields) -> Response {
    apply_settings(&st, &f, "geral")
}

fn apply_settings(st: &AppState, f: &HashMap<String, String>, cat: &str) -> Response {
    let num = |k: &str| f.get(k).and_then(|v| v.trim().parse::<u64>().ok());
    let (Some(min), Some(max)) = (num("delay_min_ms"), num("delay_max_ms")) else {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                st,
                cat,
                "Atrasos devem ser números inteiros.",
            )),
        )
            .into_response();
    };
    if min > max {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                st,
                cat,
                "O mínimo não pode ser maior que o máximo.",
            )),
        )
            .into_response();
    }
    let Some(per_min) = f
        .get("sticker_per_min")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| (1..=30).contains(n))
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                st,
                cat,
                "Figurinhas por minuto deve ser um inteiro de 1 a 30.",
            )),
        )
            .into_response();
    };
    // ausente = mantém o valor atual (formulários antigos); presente e inválido = 400 sem gravar
    let sound_max = match f.get("sticker_sound_max_s") {
        None => st.store.sticker_sound_max_s().unwrap_or(10),
        Some(v) => match v
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=10).contains(n))
        {
            Some(n) => n,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(settings_page(
                        st,
                        cat,
                        "Segundos de som deve ser um inteiro de 1 a 10.",
                    )),
                )
                    .into_response();
            }
        },
    };
    let sound_on = f.contains_key("sticker_sound");
    let owner = f.get("owner_jid").map_or("", |s| s.trim());
    let field = |k: &str| f.get(k).map_or("", |s| s.trim()).to_string();
    let (pack, publisher) = (field("sticker_pack_name"), field("sticker_publisher"));
    if pack.chars().count() > crate::stickers::MAX_FIELD
        || publisher.chars().count() > crate::stickers::MAX_FIELD
    {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                st,
                cat,
                "Nome e autor aceitam até 64 caracteres.",
            )),
        )
            .into_response();
    }
    // monitor: campo ausente mantém o valor atual; tudo é validado antes de gravar qualquer coisa
    let bad = |m: &str| (StatusCode::BAD_REQUEST, Html(settings_page(st, cat, m))).into_response();
    let mut mon = [0u64; 3];
    for (i, (k, atual, max)) in [
        ("dedupe_min", st.store.dedupe_min().unwrap_or(30), 1440),
        ("alertas_hora", st.store.alertas_hora().unwrap_or(10), 1000),
        ("retencao_dias", st.store.retencao_dias().unwrap_or(7), 365),
    ]
    .into_iter()
    .enumerate()
    {
        mon[i] = match f.get(k) {
            None => atual,
            Some(v) => match v.trim().parse::<u64>() {
                Ok(n) if (1..=max).contains(&n) => n,
                _ => return bad(&format!("{k} deve ser um inteiro de 1 a {max}.")),
            },
        };
    }
    // `set_silencio` valida as duas pontas antes de gravar
    if (f.contains_key("silencio_ini") || f.contains_key("silencio_fim"))
        && let Err(e) = st.store.set_silencio(
            field("silencio_ini").as_str(),
            field("silencio_fim").as_str(),
        )
    {
        return bad(&e.to_string());
    }
    let res = st
        .store
        .set_dedupe_min(mon[0])
        .and_then(|_| st.store.set_alertas_hora(mon[1]))
        .and_then(|_| st.store.set_retencao_dias(mon[2]))
        .and_then(|_| st.store.set_sticker_meta(&pack, &publisher))
        .and_then(|_| st.store.set_sticker_per_min(per_min))
        .and_then(|_| st.store.set_sticker_sound(sound_on))
        .and_then(|_| st.store.set_sticker_sound_max_s(sound_max))
        .and_then(|_| match f.get("video_sticker_mode") {
            Some(m) => st.store.set_video_sticker_mode(m.trim()),
            None => Ok(()),
        })
        .and_then(|_| st.store.set("delay_min_ms", &min.to_string()))
        .and_then(|_| st.store.set("delay_max_ms", &max.to_string()))
        .and_then(|_| st.store.set("owner_jid", owner));
    match res {
        Ok(()) => Html(settings_page(st, cat, "Salvo.")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(settings_page(st, cat, &format!("Erro ao salvar: {e}"))),
        )
            .into_response(),
    }
}
