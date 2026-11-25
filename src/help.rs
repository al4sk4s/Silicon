//! Ajuda (`/help`, `/comandos`) e texto da bio do grupo (`/updatedc`), spec 008.
//! Fonte única: a tabela `CMDS` alimenta a ajuda e a bio (nada de texto à mão em dois lugares).
use crate::stickers::variante;

pub struct Cmd {
    pub name: &'static str,
    pub alias: &'static [&'static str],
    pub short: &'static str,
    pub long: &'static str,
    pub owner_only: bool,
    pub in_bio: bool,
}

pub const CMDS: &[Cmd] = &[
    Cmd {
        name: "s",
        alias: &["fig", "figurinha"],
        short: "cria figurinha de imagem, vídeo ou GIF",
        long: "Responda uma imagem, vídeo, GIF ou figurinha com */s*.\n\
               • Vídeo e GIF viram figurinha animada (até 10 s), com o som original quando tem.\n\
               • Exemplo: responda uma foto e mande _/s_.\n\
               • Apelidos: /fig, /figurinha.",
        owner_only: false,
        in_bio: true,
    },
    Cmd {
        name: "rename",
        alias: &[],
        short: "troca nome e autor de uma figurinha",
        long: "Responda uma figurinha com */rename nome / autor*.\n\
               • Exemplo: _/rename Gato triste / Fulano_.\n\
               • _/rename /_ apaga nome e autor.\n\
               • A figurinha renomeada sai sem o som.",
        owner_only: false,
        in_bio: true,
    },
    Cmd {
        name: "baixar",
        alias: &["dl", "download"],
        short: "baixa vídeos e fotos do TikTok, Instagram e Facebook",
        long: "Mande */baixar link* ou responda uma mensagem que tenha o link.\n\
               • Aceita TikTok, Instagram e Facebook.\n\
               • Vídeos e também fotos/carrosséis (chegam como álbum, até 10 itens por link).\n\
               • O vídeo vem na melhor qualidade; acima de 64 MB vai como arquivo.\n\
               • Exemplo: _/baixar https://www.tiktok.com/..._\n\
               • Apelidos: /dl, /download.",
        owner_only: false,
        in_bio: true,
    },
    Cmd {
        name: "help",
        alias: &["comandos"],
        short: "mostra esta ajuda",
        long: "*/help* lista os comandos; */help baixar* explica um comando.\n\
               • Apelido: /comandos.",
        owner_only: false,
        in_bio: true,
    },
    Cmd {
        name: "c",
        alias: &[],
        short: "papel deste chat (allow, deny, status)",
        long: "Só o dono. */c allow* libera o chat, */c deny* bloqueia, */c status* mostra o papel.",
        owner_only: true,
        in_bio: false,
    },
    Cmd {
        name: "updatedc",
        alias: &[],
        short: "atualiza a descrição do grupo",
        long: "Só o dono, dentro do grupo. Reescreve a descrição com os comandos e as novidades.\n\
               • Rodar de novo sem mudança não altera nada.",
        owner_only: true,
        in_bio: false,
    },
];

/// Seção "Novidades" da bio; atualizar a cada spec que lance função visível.
pub const NOVIDADES: &[&str] = &[
    "Figurinhas animadas com som",
    "Download de vídeos e fotos (TikTok, Instagram e Facebook)",
    "Álbum de fotos de uma vez, como no WhatsApp",
    "Reação de emoji enquanto processa",
];

const GITHUB: &str = "https://github.com/al4sk4s/Silicon";
const BIO_MAX: usize = 2048;

const CABECALHO: &[&str] = &[
    "*Comandos do Silicon*",
    "*O que eu sei fazer*",
    "*Cardápio de comandos*",
    "*Guia rápido do Silicon*",
    "*Comandos disponíveis*",
];

/// `/help` ou `/comandos` (sem caixa). Some(argumento) se for o comando.
pub fn parse_cmd(text: &str) -> Option<&str> {
    let t = text.trim_start();
    let end = t.find(char::is_whitespace).unwrap_or(t.len());
    let (first, rest) = t.split_at(end);
    ["/help", "/comandos"]
        .iter()
        .any(|c| first.eq_ignore_ascii_case(c))
        .then_some(rest.trim())
}

pub fn parse_updatedc(text: &str) -> bool {
    text.split_whitespace()
        .next()
        .is_some_and(|f| f.eq_ignore_ascii_case("/updatedc"))
}

fn find(name: &str) -> Option<&'static Cmd> {
    let n = name.trim().trim_start_matches('/').to_lowercase();
    CMDS.iter()
        .find(|c| c.name == n || c.alias.contains(&n.as_str()))
}

/// Resumo (arg vazio) ou detalhe de um comando. Comandos do dono só aparecem para o dono.
pub fn help_text(arg: &str, is_owner: bool) -> String {
    if !arg.is_empty() {
        return match find(arg).filter(|c| is_owner || !c.owner_only) {
            Some(c) => format!("*/{}*: {}\n\n{}", c.name, c.short, c.long),
            None => {
                let nomes: Vec<String> = CMDS
                    .iter()
                    .filter(|c| is_owner || !c.owner_only)
                    .map(|c| format!("/{}", c.name))
                    .collect();
                format!("Não conheço esse comando. Tenta: {}", nomes.join(", "))
            }
        };
    }
    let linha = |c: &Cmd| format!("• */{}*: {}\n", c.name, c.short);
    let mut o = format!("{}\n\n", variante(CABECALHO));
    for c in CMDS.iter().filter(|c| !c.owner_only) {
        o += &linha(c);
    }
    if is_owner {
        o += "\n_Dono_\n";
        for c in CMDS.iter().filter(|c| c.owner_only) {
            o += &linha(c);
        }
    }
    o + "\nDetalhes: _/help comando_ (ex.: _/help baixar_)."
}

/// Texto da descrição do grupo. Se passar do limite, corta só as novidades.
pub fn bio_text() -> String {
    bio_with(NOVIDADES)
}

fn bio_with(novidades: &[&str]) -> String {
    let mut head = format!("{GITHUB}\n\nComandos (responda a mensagem):\n");
    for c in CMDS.iter().filter(|c| c.in_bio) {
        head += &format!("• /{}: {}\n", c.name, c.short);
    }
    let foot = "\nRodando em servidor local dedicado 24/7. Sugestões são bem-vindas.";
    let mut nov = String::new();
    for n in novidades {
        let linha = format!("• {n}\n");
        if head.len() + nov.len() + linha.len() + 12 + foot.len() > BIO_MAX {
            break;
        }
        nov += &linha;
    }
    if nov.is_empty() {
        return head + foot;
    }
    format!("{head}\nNovidades:\n{nov}{foot}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_cmd("/help"), Some(""));
        assert_eq!(parse_cmd(" /HELP baixar "), Some("baixar"));
        assert_eq!(parse_cmd("/comandos"), Some(""));
        assert_eq!(parse_cmd("/helpme"), None);
        assert!(parse_updatedc("/UpdateDC") && !parse_updatedc("/updatedcx"));
    }

    #[test]
    fn tabela_sem_nome_repetido() {
        let mut v: Vec<&str> = CMDS
            .iter()
            .flat_map(|c| std::iter::once(c.name).chain(c.alias.iter().copied()))
            .collect();
        let n = v.len();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), n);
    }

    #[test]
    fn detalhe_por_nome_e_apelido() {
        for q in ["baixar", "/dl", "/BAIXAR", "download"] {
            assert!(help_text(q, false).contains("TikTok"), "{q}");
        }
        assert!(help_text("fig", false).contains("*/s*"));
        assert!(help_text("xyz", false).contains("/baixar"));
    }

    #[test]
    fn dono_so_aparece_para_o_dono() {
        for arg in ["", "c", "updatedc"] {
            let t = help_text(arg, false);
            assert!(!t.contains("/updatedc*") && !t.contains("*/c*"), "{arg}");
        }
        assert!(help_text("", true).contains("/updatedc"));
        assert!(help_text("updatedc", true).contains("descrição"));
        assert!(!bio_text().contains("/updatedc") && !bio_text().contains("/c:"));
    }

    #[test]
    fn bio_cabe_e_e_estavel() {
        let b = bio_text();
        assert!(b.len() <= BIO_MAX && b.contains(GITHUB) && b.contains("24/7"));
        assert!(b.contains("/rename") && !b.contains("/renomear"));
        assert_eq!(b, bio_text());
        let gigante: Vec<&str> = vec!["x".repeat(300).leak(); 20];
        let g = bio_with(&gigante);
        assert!(g.len() <= BIO_MAX && g.contains("24/7"));
    }

    #[test]
    fn cabecalhos_sao_5() {
        assert_eq!(CABECALHO.len(), 5);
    }
}
