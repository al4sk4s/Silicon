//! Estado compartilhado entre o painel e o bot (só metadados: nunca conteúdo, P4).
use crate::auth::{RateLimiter, Sessions};
use crate::config::Store;
use crate::roles::ChatRoles;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_EVENTS: usize = 200;
const MAX_RECENT: usize = 100;

/// Evento do log do painel: horário (unix s), tipo e JID. Sem texto de mensagem.
pub struct Event {
    pub at: u64,
    pub kind: String,
    pub jid: String,
}

#[derive(Default)]
pub struct Status {
    pub connected: bool,
    /// Número pareado, `None` antes do pareamento.
    pub number: Option<String>,
}

pub struct AppState {
    pub store: Store,
    pub roles: ChatRoles,
    pub sessions: Sessions,
    pub limiter: RateLimiter,
    /// Limite de figurinhas por chat (spec 002).
    pub sticker_limit: crate::stickers::limit::Limiter,
    pub status: Mutex<Status>,
    pub events: Mutex<VecDeque<Event>>,
    /// Grupos (jid, assunto) vindos de `list_participating`; só rótulo (RF12).
    pub groups: Mutex<Vec<(String, String)>>,
    /// JIDs que mandaram mensagem recentemente (jid -> unix s). Só em memória.
    pub recent: Mutex<HashMap<String, u64>>,
    /// Painel servido por HTTPS: cookie ganha `Secure`.
    pub https: bool,
}

fn local_tm(secs: u64) -> libc::tm {
    let t = secs as libc::time_t;
    // SAFETY: `tm` é zerado e só preenchido por localtime_r (thread-safe) a partir de `t`.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    }
}

/// Horário local "HH:MM:SS" de um instante unix (usa o fuso do sistema).
pub fn local_hms(secs: u64) -> String {
    let tm = local_tm(secs);
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// Relógio local nos logs do `tracing` (o feature `local-time` exigiria o crate `time`).
pub struct LocalClock;

impl tracing_subscriber::fmt::time::FormatTime for LocalClock {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let tm = local_tm(now());
        write!(
            w,
            "{:04}-{:02}-{:02} {}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            local_hms(now())
        )
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl AppState {
    pub fn new(store: Store, roles: ChatRoles, https: bool) -> Self {
        AppState {
            store,
            roles,
            sessions: Sessions::new(std::time::Duration::from_secs(12 * 3600)),
            limiter: RateLimiter::new(),
            sticker_limit: Default::default(),
            status: Mutex::default(),
            events: Mutex::default(),
            groups: Mutex::default(),
            recent: Mutex::default(),
            https,
        }
    }

    pub fn push_event(&self, kind: &str, jid: &str) {
        let mut q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() == MAX_EVENTS {
            q.pop_front();
        }
        q.push_back(Event {
            at: now(),
            kind: kind.into(),
            jid: jid.into(),
        });
    }

    /// Guarda só o JID e o horário de quem falou (alimenta a lista do painel).
    pub fn note_recent(&self, jid: &str) {
        let mut m = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        if m.len() >= MAX_RECENT
            && !m.contains_key(jid)
            && let Some(old) = m.iter().min_by_key(|(_, t)| **t).map(|(k, _)| k.clone())
        {
            m.remove(&old);
        }
        m.insert(jid.into(), now());
    }

    /// Relê a tabela `roles` para o cache. Chamar após toda escrita de papel.
    pub fn reload_roles(&self) -> crate::config::Result<()> {
        let fresh = self.store.roles()?;
        *self.roles.write().unwrap_or_else(|e| e.into_inner()) = fresh;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hms_format() {
        let h = local_hms(1_700_000_000);
        assert_eq!((h.len(), &h[2..3], &h[5..6]), (8, ":", ":"));
    }
}
