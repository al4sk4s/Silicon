use crate::panel::*;

/// Grupo de opções exclusivas (tema, densidade, sidebar). Lido por `app.js` via `data-pref`.
fn seg(pref: &str, label: &str, opts: &[(&str, &str)]) -> String {
    let mut o = String::new();
    for (v, t) in opts {
        o += &format!(
            "<label class=\"seg\"><input type=\"radio\" name=\"pref-{pref}\" data-pref=\"{pref}\" value=\"{v}\"><span>{t}</span></label>"
        );
    }
    format!(
        "<div class=\"field\"><span class=\"field-label\" id=\"lbl-{pref}\">{label}</span>\
         <div class=\"segs\" role=\"radiogroup\" aria-labelledby=\"lbl-{pref}\">{o}</div></div>"
    )
}

/// Preferências visuais: ficam só no navegador (localStorage), nunca no servidor.
pub(crate) async fn appearance_get(State(st): S) -> Html<String> {
    let presets = [
        ("#7c8cff", "Índigo"),
        ("#3b9eff", "Azul"),
        ("#2dd4a6", "Verde"),
        ("#f5a524", "Âmbar"),
        ("#f0568a", "Rosa"),
        ("#a78bfa", "Violeta"),
    ];
    let sw: String = presets
        .iter()
        .map(|(c, n)| {
            format!("<button type=\"button\" class=\"swatch\" data-accent=\"{c}\" aria-label=\"{n}\" title=\"{n}\"></button>")
        })
        .collect();
    let body = format!(
        "{}{}",
        page_header(
            "Aparência",
            "Vale só para este navegador e é guardada nele.",
            &btn(
                "Restaurar padrões",
                "ghost",
                "type=\"button\" id=\"prefs-reset\""
            )
        ),
        card(
            "",
            &format!(
                "<div class=\"stack\">{}<div class=\"field\"><span class=\"field-label\">Cor de destaque</span>\
                 <div class=\"row\">{sw}<label class=\"row\"><span class=\"muted\">Outra</span>\
                 <input type=\"color\" data-pref=\"accent-custom\" aria-label=\"Cor de destaque personalizada\"></label></div></div>{}{}</div>",
                seg(
                    "theme",
                    "Tema",
                    &[
                        ("system", "Sistema"),
                        ("dark", "Escuro"),
                        ("light", "Claro")
                    ]
                ),
                seg(
                    "density",
                    "Densidade",
                    &[("comfortable", "Confortável"), ("compact", "Compacta")]
                ),
                seg(
                    "sidebar",
                    "Barra lateral",
                    &[("expanded", "Expandida"), ("collapsed", "Compacta")]
                ),
            )
        )
    );
    Html(page(&st, "Aparência", &body))
}
