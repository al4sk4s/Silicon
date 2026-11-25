//! Peças puras do alerta (spec 003): dedupe, limite por hora, silêncio e formato.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

const MAX_TEXTO: usize = 1000;

fn primeiro_link(texto: &str) -> Option<&str> {
    texto.split_whitespace().find(|p| p.starts_with("http"))
}

/// Chave de dedupe: 1º link sem query/fragmento, senão texto normalizado.
fn chave(texto: &str) -> u64 {
    let base = match primeiro_link(texto) {
        Some(l) => l.split(['?', '#']).next().unwrap_or(l).to_string(),
        None => texto
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    };
    let mut h = DefaultHasher::new();
    base.hash(&mut h);
    h.finish()
}

#[derive(Default)]
pub struct Dedupe {
    vistos: HashMap<u64, Instant>,
}

impl Dedupe {
    pub fn novo() -> Self {
        Self::default()
    }

    /// true se o mesmo conteúdo já foi visto dentro da `janela`; senão registra.
    /// `janela` zero = nunca repete.
    pub fn repetido(&mut self, texto: &str, janela: Duration, agora: Instant) -> bool {
        self.vistos
            .retain(|_, t| agora.saturating_duration_since(*t) < janela);
        if janela.is_zero() {
            return false;
        }
        let k = chave(texto);
        if self.vistos.contains_key(&k) {
            return true;
        }
        self.vistos.insert(k, agora);
        false
    }
}

#[derive(Default)]
pub struct Limite {
    envios: VecDeque<Instant>,
}

impl Limite {
    /// `max_por_hora` 0 = sem limite. Janela deslizante de 1 h; só registra se permitir.
    pub fn permite(&mut self, max_por_hora: usize, agora: Instant) -> bool {
        let hora = Duration::from_secs(3600);
        while let Some(&t) = self.envios.front() {
            if agora.saturating_duration_since(t) >= hora {
                self.envios.pop_front();
            } else {
                break;
            }
        }
        if max_por_hora == 0 {
            return true;
        }
        if self.envios.len() >= max_por_hora {
            return false;
        }
        self.envios.push_back(agora);
        true
    }
}

fn minutos(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// "HH:MM". Vazio ou inválido = desligado. Aceita intervalo que cruza a meia-noite.
pub fn em_silencio(hora_atual: &str, ini: &str, fim: &str) -> bool {
    let (Some(a), Some(i), Some(f)) = (minutos(hora_atual), minutos(ini), minutos(fim)) else {
        return false;
    };
    match i.cmp(&f) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => a >= i && a < f,
        std::cmp::Ordering::Greater => a >= i || a < f,
    }
}

pub fn formata(
    regra: &str,
    grupo: &str,
    autor: &str,
    hora: &str,
    texto: &str,
    mencoes: &[String],
) -> String {
    let mut s = format!("🔔 Regra: {regra}\nGrupo: {grupo}\nDe: {autor}\nHora: {hora}\n");
    if let Some(l) = primeiro_link(texto) {
        s.push_str(l);
        s.push('\n');
    }
    s.push('\n');
    if texto.chars().count() > MAX_TEXTO {
        s.extend(texto.chars().take(MAX_TEXTO));
        s.push('…');
    } else {
        s.push_str(texto);
    }
    if !mencoes.is_empty() {
        let m: Vec<String> = mencoes.iter().map(|u| format!("@{u}")).collect();
        s.push_str("\n\n");
        s.push_str(&m.join(" "));
    }
    s
}

/// Resumo de 1 linha, até 80 chars (histórico).
pub fn resumo(texto: &str) -> String {
    let linha = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if linha.chars().count() > 80 {
        let mut s: String = linha.chars().take(80).collect();
        s.push('…');
        s
    } else {
        linha
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn dedupe_janela() {
        let t0 = Instant::now();
        let mut d = Dedupe::novo();
        assert!(!d.repetido("Oferta X", 30 * MIN, t0));
        assert!(d.repetido("  oferta   x ", 30 * MIN, t0 + MIN));
        assert!(!d.repetido("oferta x", 30 * MIN, t0 + 31 * MIN));
        assert!(!d.repetido("oferta x", Duration::ZERO, t0 + 31 * MIN));
    }

    #[test]
    fn dedupe_mesmo_link() {
        let t0 = Instant::now();
        let mut d = Dedupe::novo();
        assert!(!d.repetido("bug! https://l.com/p?ref=a", 30 * MIN, t0));
        assert!(d.repetido("corre https://l.com/p#x agora", 30 * MIN, t0));
        assert!(!d.repetido("https://l.com/outro", 30 * MIN, t0));
    }

    #[test]
    fn limite_desliza() {
        let t0 = Instant::now();
        let mut l = Limite::default();
        for i in 0..10 {
            assert!(l.permite(10, t0 + Duration::from_secs(i)));
        }
        assert!(!l.permite(10, t0 + 30 * MIN));
        assert!(l.permite(10, t0 + 61 * MIN));
        assert!(l.permite(0, t0 + 61 * MIN));
    }

    #[test]
    fn silencio() {
        assert!(em_silencio("23:00", "22:00", "07:00"));
        assert!(em_silencio("06:59", "22:00", "07:00"));
        assert!(!em_silencio("07:00", "22:00", "07:00"));
        assert!(!em_silencio("12:00", "22:00", "07:00"));
        assert!(em_silencio("12:00", "09:00", "18:00"));
        assert!(!em_silencio("23:00", "", "07:00"));
        assert!(!em_silencio("23:00", "22:00", ""));
        assert!(!em_silencio("23:00", "xx", "07:00"));
        assert!(!em_silencio("23:00", "25:00", "07:00"));
    }

    #[test]
    fn formata_snapshot() {
        let m = formata("bug", "G", "Ana (55)", "14:32", "veja https://a.b/x", &[]);
        assert_eq!(
            m,
            "🔔 Regra: bug\nGrupo: G\nDe: Ana (55)\nHora: 14:32\nhttps://a.b/x\n\nveja https://a.b/x"
        );
        let m = formata("r", "G", "A", "1:00", "oi", &["1".into(), "2".into()]);
        assert_eq!(m, "🔔 Regra: r\nGrupo: G\nDe: A\nHora: 1:00\n\noi\n\n@1 @2");
    }

    #[test]
    fn formata_trunca() {
        let m = formata("r", "G", "A", "h", &"é".repeat(1500), &[]);
        assert_eq!(m.chars().filter(|&c| c == 'é').count(), 1000);
        assert!(m.ends_with('…'));
    }

    #[test]
    fn resumo_80() {
        assert_eq!(resumo("a\n b  c"), "a b c");
        let r = resumo(&"x".repeat(200));
        assert_eq!(r.chars().count(), 81);
    }
}
