use crate::panel::*;

pub(crate) fn settings_page(st: &AppState, msg: &str) -> String {
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    let owner = st.store.owner_jid().ok().flatten().unwrap_or_default();
    let per_min = st.store.sticker_per_min().unwrap_or(3);
    let pack = st.store.sticker_pack_name().unwrap_or_default();
    let publisher = st.store.sticker_publisher().unwrap_or_default();
    let sound_on = if st.store.sticker_sound().unwrap_or(true) {
        " checked"
    } else {
        ""
    };
    let sound_max = st.store.sticker_sound_max_s().unwrap_or(10);
    let dedupe = st.store.dedupe_min().unwrap_or(30);
    let por_hora = st.store.alertas_hora().unwrap_or(10);
    let retencao = st.store.retencao_dias().unwrap_or(7);
    let sil_ini = st.store.silencio_ini().unwrap_or_default();
    let sil_fim = st.store.silencio_fim().unwrap_or_default();
    let malvin = st.store.video_sticker_mode().is_ok_and(|m| m == "malvin");
    let (sw, sd) = if malvin {
        (" selected", "")
    } else {
        ("", " selected")
    };
    page(
        "Ajustes",
        &format!(
            "<p>{}</p><form method=\"post\" action=\"/settings\">\
             atraso mín (ms): <input name=\"delay_min_ms\" value=\"{min}\"><br>\
             atraso máx (ms): <input name=\"delay_max_ms\" value=\"{max}\"><br>\
             JID do dono (opcional): <input name=\"owner_jid\" value=\"{}\"><br>\
             figurinhas por minuto (1-30): <input type=\"number\" min=\"1\" max=\"30\" name=\"sticker_per_min\" value=\"{per_min}\"><br>\
             nome da figurinha (até 64): <input name=\"sticker_pack_name\" value=\"{}\"><br>\
             autor da figurinha (até 64): <input name=\"sticker_publisher\" value=\"{}\"><br>\
             figurinha com som (vídeo com áudio): <input type=\"checkbox\" name=\"sticker_sound\" value=\"1\"{sound_on}><br>\
             segundos máximos de som (1-10): <input type=\"number\" min=\"1\" max=\"10\" name=\"sticker_sound_max_s\" value=\"{sound_max}\"><br>\
             qualidade do vídeo: <select name=\"video_sticker_mode\"><option value=\"silicon\"{sd}>silicon (512 nítido, até 15–20 fps, mais pesado de converter)</option><option value=\"malvin\"{sw}>malvin (320 esticado, 15 fps, mais leve e mais borrado)</option></select><br>\
             <h2>Monitor</h2>\
             repetido ignorado por (min, 1-1440): <input type=\"number\" min=\"1\" max=\"1440\" name=\"dedupe_min\" value=\"{dedupe}\"><br>\
             alertas por hora (1-1000): <input type=\"number\" min=\"1\" max=\"1000\" name=\"alertas_hora\" value=\"{por_hora}\"><br>\
             guardar histórico por (dias, 1-365): <input type=\"number\" min=\"1\" max=\"365\" name=\"retencao_dias\" value=\"{retencao}\"><br>\
             silêncio de (HH:MM, vazio = desligado): <input name=\"silencio_ini\" value=\"{}\" placeholder=\"23:00\" size=\"6\"> \
             até <input name=\"silencio_fim\" value=\"{}\" placeholder=\"07:00\" size=\"6\"><br>\
             <button>Salvar</button></form>",
            esc(msg),
            esc(&owner),
            esc(&pack),
            esc(&publisher),
            esc(&sil_ini),
            esc(&sil_fim)
        ),
    )
}

pub(crate) async fn settings_get(State(st): S) -> Html<String> {
    Html(settings_page(&st, ""))
}

pub(crate) async fn settings_post(State(st): S, Form(f): Fields) -> Response {
    let num = |k: &str| f.get(k).and_then(|v| v.trim().parse::<u64>().ok());
    let (Some(min), Some(max)) = (num("delay_min_ms"), num("delay_max_ms")) else {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(&st, "Atrasos devem ser números inteiros.")),
        )
            .into_response();
    };
    if min > max {
        return (
            StatusCode::BAD_REQUEST,
            Html(settings_page(
                &st,
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
                &st,
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
                        &st,
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
                &st,
                "Nome e autor aceitam até 64 caracteres.",
            )),
        )
            .into_response();
    }
    // monitor: campo ausente mantém o valor atual; tudo é validado antes de gravar qualquer coisa
    let bad = |m: &str| (StatusCode::BAD_REQUEST, Html(settings_page(&st, m))).into_response();
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
        Ok(()) => Html(settings_page(&st, "Salvo.")).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(settings_page(&st, &format!("Erro ao salvar: {e}"))),
        )
            .into_response(),
    }
}
