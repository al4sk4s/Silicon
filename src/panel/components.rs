//! Componentes de HTML do painel (contrato em `specs/010-redesign-painel/ui-contract.md`).
//! Todo texto vindo de fora passa por `esc`; `attrs`/`html` são sempre literais do código.
use crate::panel::*;

/// Ícone SVG inline (traço único, 18 px via CSS). Nome desconhecido → círculo.
pub(crate) fn icon(name: &str) -> String {
    let d = match name {
        "home" => "<path d=\"M3 11l9-8 9 8\"/><path d=\"M5 10v10h14V10\"/>",
        "chat" => "<path d=\"M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z\"/>",
        "users" => {
            "<circle cx=\"9\" cy=\"8\" r=\"3.5\"/><path d=\"M2.5 20a6.5 6.5 0 0 1 13 0\"/><path d=\"M16 4.5a3.5 3.5 0 0 1 0 7\"/><path d=\"M18 14.5a6.5 6.5 0 0 1 3.5 5.5\"/>"
        }
        "user" => "<circle cx=\"12\" cy=\"8\" r=\"4\"/><path d=\"M4 21a8 8 0 0 1 16 0\"/>",
        "rules" => {
            "<path d=\"M4 6h9M19 6h1M4 12h3M13 12h7M4 18h11M21 18h-1\"/><circle cx=\"16\" cy=\"6\" r=\"2\"/><circle cx=\"10\" cy=\"12\" r=\"2\"/><circle cx=\"18\" cy=\"18\" r=\"2\"/>"
        }
        "bell" => {
            "<path d=\"M6 8a6 6 0 0 1 12 0c0 7 3 8 3 8H3s3-1 3-8\"/><path d=\"M10 20a2 2 0 0 0 4 0\"/>"
        }
        "bug" => {
            "<rect x=\"8\" y=\"7\" width=\"8\" height=\"13\" rx=\"4\"/><path d=\"M9 7a3 3 0 0 1 6 0M4 12h4M16 12h4M5 5l3 3M19 5l-3 3M5 20l3-3M19 20l-3-3\"/>"
        }
        "qr" => {
            "<rect x=\"3\" y=\"3\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"14\" y=\"3\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"3\" y=\"14\" width=\"7\" height=\"7\" rx=\"1\"/><path d=\"M14 14h3v3h-3zM17 20h4M20 17v3\"/>"
        }
        "download" => "<path d=\"M12 4v11M7 11l5 5 5-5M4 20h16\"/>",
        "settings" => {
            "<circle cx=\"12\" cy=\"12\" r=\"3\"/><path d=\"M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9L7 7M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1\"/>"
        }
        "palette" => {
            "<path d=\"M12 3a9 9 0 1 0 0 18c1.2 0 2-.8 2-1.8 0-1.2-1-1.5-1-2.7 0-1 .8-1.7 1.8-1.7H17a4 4 0 0 0 4-4C21 6.5 17 3 12 3z\"/><circle cx=\"7.5\" cy=\"11\" r=\"1\"/><circle cx=\"10\" cy=\"7\" r=\"1\"/><circle cx=\"15\" cy=\"7.5\" r=\"1\"/>"
        }
        "logout" => "<path d=\"M9 4H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h4M16 8l4 4-4 4M20 12H9\"/>",
        "menu" => "<path d=\"M4 6h16M4 12h16M4 18h16\"/>",
        "panel" => {
            "<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"/><path d=\"M9 4v16\"/>"
        }
        "search" => "<circle cx=\"11\" cy=\"11\" r=\"7\"/><path d=\"M20 20l-3.5-3.5\"/>",
        "plus" => "<path d=\"M12 5v14M5 12h14\"/>",
        "trash" => "<path d=\"M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3\"/>",
        "check" => "<path d=\"M5 12l5 5 9-10\"/>",
        "x" => "<path d=\"M6 6l12 12M18 6L6 18\"/>",
        "edit" => "<path d=\"M4 20h4L19 9l-4-4L4 16z\"/>",
        "alert" => "<path d=\"M12 3l10 18H2z\"/><path d=\"M12 10v5M12 18h0\"/>",
        "info" => "<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 11v6M12 7.5h0\"/>",
        "cpu" => {
            "<rect x=\"6\" y=\"6\" width=\"12\" height=\"12\" rx=\"2\"/><path d=\"M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4\"/>"
        }
        "clock" => "<circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 7v5l3 2\"/>",
        "refresh" => "<path d=\"M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7\"/>",
        "logo" => {
            "<path d=\"M12 2.5l8.5 5v9l-8.5 5-8.5-5v-9z\"/><path d=\"M12 8l4 2.3v3.4L12 16l-4-2.3v-3.4z\"/>"
        }
        _ => "<circle cx=\"12\" cy=\"12\" r=\"8\"/>",
    };
    format!(
        "<svg class=\"icon\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.75\" \
         stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\">{d}</svg>"
    )
}

/// `kind`: primary | ghost | danger | "" (padrão). `attrs`: literal do código (ex.: `type="submit"`).
pub(crate) fn btn(label: &str, kind: &str, attrs: &str) -> String {
    let k = if kind.is_empty() {
        String::new()
    } else {
        format!(" btn-{kind}")
    };
    format!("<button class=\"btn{k}\" {attrs}>{}</button>", esc(label))
}

/// `tone`: ok | warn | bad | info | muted. O texto sempre diz o estado (nunca só cor).
pub(crate) fn badge(text: &str, tone: &str) -> String {
    format!("<span class=\"badge {tone}\">{}</span>", esc(text))
}

/// `state`: ok | warn | bad.
pub(crate) fn status_dot(state: &str, label: &str) -> String {
    format!(
        "<span class=\"status\" data-state=\"{state}\"><i class=\"dot\"></i><span>{}</span></span>",
        esc(label)
    )
}

pub(crate) fn empty(icon_name: &str, msg: &str) -> String {
    format!(
        "<div class=\"empty\">{}<p>{}</p></div>",
        icon(icon_name),
        esc(msg)
    )
}

/// `actions`: HTML já pronto (botões/forms). Descrição e ações são opcionais.
pub(crate) fn page_header(title: &str, desc: &str, actions: &str) -> String {
    let d = if desc.is_empty() {
        String::new()
    } else {
        format!("<p class=\"desc\">{}</p>", esc(desc))
    };
    let a = if actions.is_empty() {
        String::new()
    } else {
        format!("<div class=\"actions\">{actions}</div>")
    };
    format!(
        "<div class=\"page-header\"><div><h2 class=\"title\">{}</h2>{d}</div>{a}</div>",
        esc(title)
    )
}

pub(crate) fn section_header(title: &str) -> String {
    format!("<h3 class=\"section-header\">{}</h3>", esc(title))
}

pub(crate) fn card(title: &str, body: &str) -> String {
    let t = if title.is_empty() {
        String::new()
    } else {
        format!("<h3 class=\"card-title\">{}</h3>", esc(title))
    };
    format!("<section class=\"card\">{t}{body}</section>")
}

/// Mensagem de retorno de POST: inline sem JS, vira toast com JS. Sem mensagem → vazio.
/// shortcut: o tipo (ok/erro) é deduzido do texto, pois os handlers só passam a string;
/// melhorar se os handlers passarem a devolver o tipo.
pub(crate) fn flash(msg: &str) -> String {
    if msg.is_empty() {
        return String::new();
    }
    let l = msg.to_lowercase();
    let err = [
        "não",
        "nao",
        "inválid",
        "invalid",
        "falhou",
        "erro",
        "desconectado",
        "incorret",
        "muitas",
        "precisa",
        "obrigat",
        "proibid",
        "deve",
        "exige",
        "vazio",
        "grande",
        "ausente",
        "negad",
    ]
    .iter()
    .any(|w| l.contains(w));
    format!(
        "<p class=\"flash\" data-kind=\"{}\" role=\"status\">{}</p>",
        if err { "err" } else { "ok" },
        esc(msg)
    )
}

/// Campo com rótulo associado. `control`: HTML do controle (use `input`, `select`...), com `id` = `for`.
pub(crate) fn field(label: &str, id: &str, control: &str, hint: &str) -> String {
    let h = if hint.is_empty() {
        String::new()
    } else {
        format!("<p class=\"hint\">{}</p>", esc(hint))
    };
    format!(
        "<div class=\"field\"><label for=\"{id}\">{}</label>{control}{h}</div>",
        esc(label)
    )
}

/// `attrs`: literal do código (type, placeholder, required...).
pub(crate) fn input(name: &str, value: &str, attrs: &str) -> String {
    format!(
        "<input class=\"input\" id=\"{name}\" name=\"{name}\" value=\"{}\" {attrs}>",
        esc(value)
    )
}

pub(crate) fn number_input(name: &str, value: &str, min: &str, max: &str) -> String {
    format!(
        "<input class=\"input\" id=\"{name}\" name=\"{name}\" type=\"number\" inputmode=\"numeric\" \
         min=\"{min}\" max=\"{max}\" value=\"{}\" required>",
        esc(value)
    )
}

pub(crate) fn select(name: &str, options: &[(&str, &str)], cur: &str, attrs: &str) -> String {
    let mut o = String::new();
    for (v, t) in options {
        o += &format!(
            "<option value=\"{}\"{}>{}</option>",
            esc(v),
            if *v == cur { " selected" } else { "" },
            esc(t)
        );
    }
    format!("<select class=\"select\" id=\"{name}\" name=\"{name}\" {attrs}>{o}</select>")
}

pub(crate) fn switch(name: &str, label: &str, checked: bool) -> String {
    format!(
        "<label class=\"switch\"><input type=\"checkbox\" role=\"switch\" name=\"{name}\"{}>\
         <span class=\"track\"></span><span class=\"switch-label\">{}</span></label>",
        if checked { " checked" } else { "" },
        esc(label)
    )
}

/// Caixa de busca; `target` = seletor CSS da lista cujos filhos têm `data-q`.
pub(crate) fn search(target: &str, placeholder: &str) -> String {
    format!(
        "<div class=\"search\">{}<input type=\"search\" data-filter data-filter-target=\"{target}\" \
         placeholder=\"{}\" aria-label=\"{}\"></div>",
        icon("search"),
        esc(placeholder),
        esc(placeholder)
    )
}

/// Form de POST com confirmação (modal via JS; sem JS envia direto, como antes).
pub(crate) fn confirm_form(
    action: &str,
    hidden: &[(&str, &str)],
    label: &str,
    msg: &str,
    class: &str,
) -> String {
    let h: String = hidden
        .iter()
        .map(|(k, v)| format!("<input type=\"hidden\" name=\"{k}\" value=\"{}\">", esc(v)))
        .collect();
    format!(
        "<form class=\"inline\" method=\"post\" action=\"{action}\" data-confirm=\"{}\">{h}\
         <button class=\"btn {class}\">{}</button></form>",
        esc(msg),
        esc(label)
    )
}

/// Iniciais (até 2) para o avatar.
pub(crate) fn avatar(name: &str) -> String {
    let ini: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .collect::<String>()
        .to_uppercase();
    format!(
        "<span class=\"avatar\" aria-hidden=\"true\">{}</span>",
        esc(if ini.is_empty() { "?" } else { &ini })
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_texto_de_fora() {
        let x = "<script>alert(1)</script>";
        for h in [
            btn(x, "primary", ""),
            badge(x, "ok"),
            status_dot("ok", x),
            empty("info", x),
            page_header(x, x, ""),
            section_header(x),
            card(x, ""),
            flash(x),
            field(x, "a", "", x),
            input("a", x, ""),
            switch("a", x, true),
            search("#l", x),
            confirm_form("/x", &[("k", x)], x, x, ""),
            avatar(x),
        ] {
            assert!(!h.contains("<script>"), "{h}");
        }
    }

    #[test]
    fn flash_tipo_e_vazio() {
        assert_eq!(flash(""), "");
        assert!(flash("Salvo.").contains("data-kind=\"ok\""));
        assert!(flash("Número inválido.").contains("data-kind=\"err\""));
    }

    #[test]
    fn avatar_iniciais() {
        assert!(avatar("ana maria souza").contains(">AM<"));
        assert!(avatar("").contains(">?<"));
    }
}
