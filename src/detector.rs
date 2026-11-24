//! Detector de promoções/palavras (spec 003): puro, sem AppState.
//! ATENÇÃO: o texto é normalizado (minúsculas, sem acento) antes do casamento, mas o
//! PADRÃO não é. Quem escreve a regra deve usar minúsculas e sem acento.

use regex::{Regex, RegexBuilder};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[cfg(test)]
pub const REGRA_BUG: &str = r"\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao|zinho|ue[im])?\b";

const LIMITE_REGEX: usize = 64 * 1024;
const LIMITE_DFA: usize = 256 * 1024;

/// NFD, sem marcas combinantes (acentos), minúsculas.
pub fn normaliza(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn compila(padrao: &str) -> Result<Regex, String> {
    RegexBuilder::new(padrao)
        .size_limit(LIMITE_REGEX)
        .dfa_size_limit(LIMITE_DFA)
        .build()
        .map_err(|e| format!("padrão inválido ou grande demais: {e}"))
}

/// Valida um padrão (mesmos limites do detector) para o painel.
pub fn valida(padrao: &str) -> Result<(), String> {
    compila(padrao).map(|_| ())
}

pub struct Regra {
    pub id: i64,
    pub nome: String,
    pub padrao: String,
    pub ativa: bool,
}

#[allow(dead_code)] // id só é lido nos testes; o monitor usa o nome
pub struct RegraCasada {
    pub id: i64,
    pub nome: String,
}

#[derive(Default)]
pub struct Detector {
    regras: Vec<(i64, String, Regex)>,
    ignorar: Vec<Regex>,
}

impl Detector {
    /// `regras` devem vir ordenadas por id; as desativadas são ignoradas.
    pub fn novo(regras: &[Regra], ignorar: &[String]) -> Result<Detector, String> {
        let mut rs = Vec::new();
        for r in regras.iter().filter(|r| r.ativa) {
            let re = compila(&r.padrao).map_err(|e| format!("regra \"{}\": {e}", r.nome))?;
            rs.push((r.id, r.nome.clone(), re));
        }
        let mut ig = Vec::new();
        for p in ignorar {
            ig.push(compila(p).map_err(|e| format!("ignorar \"{p}\": {e}"))?);
        }
        Ok(Detector {
            regras: rs,
            ignorar: ig,
        })
    }

    /// Ignorar vence; senão a primeira regra ativa que casar.
    pub fn avalia(&self, texto: &str) -> Option<RegraCasada> {
        let t = normaliza(texto);
        if self.ignorar.iter().any(|re| re.is_match(&t)) {
            return None;
        }
        self.regras
            .iter()
            .find(|(_, _, re)| re.is_match(&t))
            .map(|(id, nome, _)| RegraCasada {
                id: *id,
                nome: nome.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regra(id: i64, padrao: &str, ativa: bool) -> Regra {
        Regra {
            id,
            nome: format!("r{id}"),
            padrao: padrao.into(),
            ativa,
        }
    }

    fn bug() -> Detector {
        Detector::novo(&[regra(1, REGRA_BUG, true)], &[]).unwrap()
    }

    #[test]
    fn bug_casa_e_nao_casa() {
        let d = bug();
        for t in [
            "bug", "bugs", "bugado", "BUGADA", "bugados", "bugou", "bugar", "bugando", "buguei",
            "bugzão", "bugzinho",
        ] {
            assert!(d.avalia(t).is_some(), "deveria casar: {t}");
        }
        for t in ["debug", "debugar", "bugatti", "ebuggy"] {
            assert!(d.avalia(t).is_none(), "não deveria casar: {t}");
        }
    }

    #[test]
    fn ignorar_vence() {
        let d = Detector::novo(&[regra(1, REGRA_BUG, true)], &["bug de teste".into()]).unwrap();
        assert!(d.avalia("bug de teste").is_none());
        assert!(d.avalia("outro bug").is_some());
    }

    #[test]
    fn invalida_e_desativada() {
        assert!(valida("(").is_err());
        let e = Detector::novo(&[regra(7, "(", true)], &[]).err().unwrap();
        assert!(e.contains("r7"));
        let d = Detector::novo(&[regra(1, "promo", false), regra(2, "oferta", true)], &[]).unwrap();
        assert!(d.avalia("promo").is_none());
        assert_eq!(d.avalia("OFERTA").unwrap().id, 2);
    }

    #[test]
    fn acentos() {
        assert_eq!(normaliza("PROMOÇÃO Café"), "promocao cafe");
        let d = Detector::novo(&[regra(1, "promocao", true)], &[]).unwrap();
        assert!(d.avalia("Grande PROMOÇÃO!").is_some());
    }
}
