//! Autenticação do painel (P5): hash da senha, sessões em memória e limite de tentativas.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use rand::Rng;

/// Gera o hash argon2id (string PHC) com salt aleatório.
pub fn hash_password(pw: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Confere a senha contra o hash PHC; hash inválido conta como falha.
pub fn verify_password(hash: &str, pw: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// Sessões em memória: token -> instante de expiração (somem ao reiniciar).
pub struct Sessions {
    ttl: Duration,
    map: Mutex<HashMap<String, Instant>>,
}

impl Sessions {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Cria um token aleatório (32 bytes em hex = 64 chars).
    pub fn create(&self) -> String {
        let mut buf = [0u8; 32];
        rand::rng().fill(&mut buf);
        let token: String = buf.iter().map(|b| format!("{b:02x}")).collect();
        self.map
            .lock()
            .unwrap()
            .insert(token.clone(), Instant::now() + self.ttl);
        token
    }

    /// Token existe e não expirou; aproveita para apagar os expirados.
    pub fn valid(&self, token: &str) -> bool {
        let now = Instant::now();
        let mut map = self.map.lock().unwrap();
        map.retain(|_, exp| *exp > now);
        map.contains_key(token)
    }

    pub fn remove(&self, token: &str) {
        self.map.lock().unwrap().remove(token);
    }
}

/// Limite de tentativas de login por IP: `max_fails` falhas seguidas bloqueiam por `block_for`.
pub struct RateLimiter {
    max_fails: u32,
    block_for: Duration,
    // IP -> (falhas seguidas, bloqueado até)
    state: Mutex<HashMap<IpAddr, (u32, Option<Instant>)>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            max_fails: 5,
            block_for: Duration::from_secs(300),
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn blocked(&self, ip: IpAddr, now: Instant) -> bool {
        let mut st = self.state.lock().unwrap();
        match st.get(&ip).and_then(|(_, until)| *until) {
            Some(until) if now < until => true,
            Some(_) => {
                // prazo vencido: começa do zero
                st.remove(&ip);
                false
            }
            None => false,
        }
    }

    pub fn fail(&self, ip: IpAddr, now: Instant) {
        let mut st = self.state.lock().unwrap();
        let e = st.entry(ip).or_insert((0, None));
        e.0 += 1;
        if e.0 >= self.max_fails {
            e.1 = Some(now + self.block_for);
        }
    }

    pub fn success(&self, ip: IpAddr) {
        self.state.lock().unwrap().remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_and_verify() {
        let h = hash_password("segredo").unwrap();
        assert!(verify_password(&h, "segredo"));
        assert!(!verify_password(&h, "errada"));
        assert!(!verify_password("lixo", "segredo"));
    }

    #[test]
    fn session_lifecycle() {
        let s = Sessions::new(Duration::from_secs(60));
        let (a, b) = (s.create(), s.create());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(s.valid(&a));
        s.remove(&a);
        assert!(!s.valid(&a));
        assert!(s.valid(&b));
    }

    #[test]
    fn session_expires() {
        let s = Sessions::new(Duration::ZERO);
        let t = s.create();
        assert!(!s.valid(&t));
    }

    #[test]
    fn rate_limiter() {
        let rl = RateLimiter::new();
        let ip: IpAddr = "192.168.0.2".parse().unwrap();
        let other: IpAddr = "192.168.0.3".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..4 {
            rl.fail(ip, t0);
        }
        assert!(!rl.blocked(ip, t0));
        rl.fail(ip, t0);
        assert!(rl.blocked(ip, t0));
        assert!(!rl.blocked(other, t0));
        assert!(rl.blocked(ip, t0 + Duration::from_secs(299)));
        assert!(!rl.blocked(ip, t0 + Duration::from_secs(300)));
        // success zera a contagem
        for _ in 0..4 {
            rl.fail(other, t0);
        }
        rl.success(other);
        rl.fail(other, t0);
        assert!(!rl.blocked(other, t0));
    }
}
