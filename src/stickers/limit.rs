//! T2: limite de figurinhas por chat.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Primeiro bloqueio da janela: o bot responde "Calma" uma vez.
    DenyFirst,
    /// Bloqueios seguintes: mudo até a janela liberar.
    DenySilent,
}

/// Por chat: instantes dos pedidos aceitos na janela + se o aviso "Calma" já foi dado.
#[derive(Default)]
pub struct Limiter(Mutex<HashMap<String, (VecDeque<Instant>, bool)>>);

impl Limiter {
    pub fn check(&self, chat: &str, now: Instant, max_per_min: usize) -> Verdict {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        // Descarta chats inativos (janela vencida e sem aviso pendente) para o mapa não crescer.
        map.retain(|k, (q, warned)| {
            k == chat
                || *warned
                || q.back()
                    .is_some_and(|t| now.saturating_duration_since(*t) < WINDOW)
        });
        let (queue, warned) = map.entry(chat.to_string()).or_default();
        queue.retain(|t| now.saturating_duration_since(*t) < WINDOW);
        let verdict = if queue.len() < max_per_min {
            queue.push_back(now);
            *warned = false;
            Verdict::Allow
        } else if *warned {
            Verdict::DenySilent
        } else {
            *warned = true;
            Verdict::DenyFirst
        };
        // fila vazia e já avisado só ocorre com max 0; mantém o aviso, descarta o resto
        if queue.is_empty() && !*warned {
            map.remove(chat);
        }
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_n_then_silent_then_frees() {
        let l = Limiter::default();
        let t = Instant::now();
        assert_eq!(l.check("a", t, 2), Verdict::Allow);
        assert_eq!(l.check("a", t, 2), Verdict::Allow);
        assert_eq!(l.check("a", t, 2), Verdict::DenyFirst);
        assert_eq!(l.check("a", t, 2), Verdict::DenySilent);
        let later = t + Duration::from_secs(60);
        assert_eq!(l.check("a", later, 2), Verdict::Allow);
        // estado "já avisei" resetou
        assert_eq!(l.check("a", later, 2), Verdict::Allow);
        assert_eq!(l.check("a", later, 2), Verdict::DenyFirst);
    }

    #[test]
    fn idle_chats_are_dropped() {
        let l = Limiter::default();
        let t = Instant::now();
        l.check("a", t, 2);
        l.check("b", t + Duration::from_secs(60), 2);
        let map = l.0.lock().unwrap();
        assert!(!map.contains_key("a") && map.contains_key("b"));
    }

    #[test]
    fn chats_are_independent_and_zero_blocks_all() {
        let l = Limiter::default();
        let t = Instant::now();
        assert_eq!(l.check("a", t, 1), Verdict::Allow);
        assert_eq!(l.check("b", t, 1), Verdict::Allow);
        assert_eq!(l.check("c", t, 0), Verdict::DenyFirst);
        assert_eq!(l.check("c", t, 0), Verdict::DenySilent);
    }
}
