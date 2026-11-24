// Config persistente em SQLite: papéis dos chats e ajustes do painel.
use crate::roles::Role;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Store(Mutex<Connection>);

const SCHEMA_V1: &str = "
CREATE TABLE roles(jid TEXT PRIMARY KEY,
  role TEXT NOT NULL CHECK(role IN('comando','monitorado','alerta')),
  name TEXT, updated_at INTEGER NOT NULL);
CREATE UNIQUE INDEX one_alert ON roles(role) WHERE role='alerta';
CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

// Spec 003: regras do monitor, lista de ignorar e histórico de alertas.
// A regra "bug" é semeada só aqui (uma vez, ao criar a tabela): se o dono apagar, não volta.
const SCHEMA_V2: &str = r"
CREATE TABLE rules(id INTEGER PRIMARY KEY, nome TEXT NOT NULL, padrao TEXT NOT NULL,
  ativa INTEGER NOT NULL DEFAULT 1);
INSERT INTO rules(nome,padrao) VALUES('bug','\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao|zinho|ue[im])?\b');
CREATE TABLE ignore(id INTEGER PRIMARY KEY, padrao TEXT NOT NULL);
CREATE TABLE alert_log(ts INTEGER NOT NULL, regra TEXT NOT NULL, resumo TEXT NOT NULL);
";

// "HH:MM" válido (00:00..23:59)
fn hhmm_valido(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else {
        return false;
    };
    h.len() == 2
        && m.len() == 2
        && h.parse::<u8>().is_ok_and(|h| h < 24)
        && m.parse::<u8>().is_ok_and(|m| m < 60)
}

impl Store {
    /// Abre (ou cria) o banco em `path`, com permissão 0600 em Unix, e migra.
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Self::migrate(conn)
    }

    /// Banco em memória, para testes.
    #[cfg(test)]
    pub fn memory() -> Result<Store> {
        Self::migrate(Connection::open_in_memory()?)
    }

    fn migrate(mut conn: Connection) -> Result<Store> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            let tx = conn.transaction()?;
            tx.execute_batch(SCHEMA_V1)?;
            tx.execute_batch("PRAGMA user_version = 1")?;
            tx.commit()?;
        }
        if version < 2 {
            let tx = conn.transaction()?;
            tx.execute_batch(SCHEMA_V2)?;
            tx.execute_batch("PRAGMA user_version = 2")?;
            tx.commit()?;
        }
        Ok(Store(Mutex::new(conn)))
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Upsert do papel. Um segundo `alerta` em outro JID falha (índice `one_alert`).
    pub fn set_role(&self, jid: &str, role: Role, name: Option<&str>) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn().execute(
            "INSERT INTO roles(jid, role, name, updated_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(jid) DO UPDATE SET role = excluded.role,
               name = excluded.name, updated_at = excluded.updated_at",
            params![jid, role.as_str(), name, now],
        )?;
        Ok(())
    }

    pub fn clear_role(&self, jid: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM roles WHERE jid = ?1", params![jid])?;
        Ok(())
    }

    /// Todos os papéis, para recarregar o cache `ChatRoles`.
    pub fn roles(&self) -> Result<HashMap<String, Role>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT jid, role FROM roles")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut map = HashMap::new();
        for row in rows {
            let (jid, role) = row?;
            // papel desconhecido (não deveria ocorrer pelo CHECK) é ignorado
            if let Some(role) = Role::parse(&role) {
                map.insert(jid, role);
            }
        }
        Ok(map)
    }

    /// Chats privados com papel `comando` (jid, nome), para a aba Contatos (spec 007).
    pub fn dm_commands(&self) -> Result<Vec<(String, Option<String>)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT jid, name FROM roles WHERE role = 'comando'
             AND (jid LIKE '%@lid' OR jid LIKE '%@s.whatsapp.net') ORDER BY name, jid",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // valor ausente ou inválido cai no padrão
    fn get_u64(&self, key: &str, default: u64) -> Result<u64> {
        Ok(self
            .get(key)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(default))
    }

    pub fn delay_min_ms(&self) -> Result<u64> {
        self.get_u64("delay_min_ms", 1000)
    }

    pub fn delay_max_ms(&self) -> Result<u64> {
        self.get_u64("delay_max_ms", 2000)
    }

    // fora de 1..=30 ou lixo no banco cai no padrão 3
    pub fn sticker_per_min(&self) -> Result<usize> {
        Ok(self
            .get("sticker_per_min")?
            .and_then(|v| v.parse().ok())
            .filter(|n| (1..=30).contains(n))
            .unwrap_or(3))
    }

    pub fn set_sticker_per_min(&self, n: usize) -> Result<()> {
        if !(1..=30).contains(&n) {
            return Err("figurinhas por minuto deve estar entre 1 e 30".into());
        }
        self.set("sticker_per_min", &n.to_string())
    }

    // som das figurinhas animadas: ligado, a menos que gravado "0"
    pub fn sticker_sound(&self) -> Result<bool> {
        Ok(self.get("sticker_sound")?.as_deref() != Some("0"))
    }

    pub fn set_sticker_sound(&self, on: bool) -> Result<()> {
        self.set("sticker_sound", if on { "1" } else { "0" })
    }

    // fora de 1..=10 ou lixo no banco cai no padrão 10
    pub fn sticker_sound_max_s(&self) -> Result<u32> {
        Ok(self
            .get("sticker_sound_max_s")?
            .and_then(|v| v.parse().ok())
            .filter(|n| (1..=10).contains(n))
            .unwrap_or(10))
    }

    pub fn set_sticker_sound_max_s(&self, n: u32) -> Result<()> {
        if !(1..=10).contains(&n) {
            return Err("duração máxima do som deve estar entre 1 e 10 segundos".into());
        }
        self.set("sticker_sound_max_s", &n.to_string())
    }

    // fora de 1..=10 ou lixo no banco cai no padrão 2
    pub fn dl_per_min(&self) -> Result<usize> {
        Ok(self
            .get("dl_per_min")?
            .and_then(|v| v.parse().ok())
            .filter(|n| (1..=10).contains(n))
            .unwrap_or(2))
    }

    pub fn set_dl_per_min(&self, n: usize) -> Result<()> {
        if !(1..=10).contains(&n) {
            return Err("downloads por minuto deve estar entre 1 e 10".into());
        }
        self.set("dl_per_min", &n.to_string())
    }

    // só leitura; fora de 10..=600 ou lixo cai no padrão 120 s
    pub fn dl_timeout_s(&self) -> Result<u64> {
        Ok(self
            .get("dl_timeout_s")?
            .and_then(|v| v.parse().ok())
            .filter(|n| (10..=600).contains(n))
            .unwrap_or(120))
    }

    fn cookies_key(plat: &str) -> Result<String> {
        match plat {
            "tiktok" | "instagram" => Ok(format!("cookies_status_{plat}")),
            _ => Err("plataforma deve ser tiktok ou instagram".into()),
        }
    }

    /// Estado dos cookies da plataforma, valor livre ("ok|<unix s>" ou "falhou|<unix s>").
    pub fn cookies_status(&self, plat: &str) -> Result<Option<String>> {
        self.get(&Self::cookies_key(plat)?)
    }

    pub fn set_cookies_status(&self, plat: &str, v: &str) -> Result<()> {
        self.set(&Self::cookies_key(plat)?, v)
    }

    /// Nome e autor gravados no EXIF das figurinhas novas (RF10); vazio é válido.
    pub fn sticker_pack_name(&self) -> Result<String> {
        Ok(self
            .get("sticker_pack_name")?
            .unwrap_or_else(|| "Silicon™".into()))
    }

    pub fn sticker_publisher(&self) -> Result<String> {
        Ok(self
            .get("sticker_publisher")?
            .unwrap_or_else(|| "@al4sk4s".into()))
    }

    pub fn set_sticker_meta(&self, name: &str, publisher: &str) -> Result<()> {
        if name.chars().count() > crate::stickers::MAX_FIELD
            || publisher.chars().count() > crate::stickers::MAX_FIELD
        {
            return Err(format!(
                "nome e autor aceitam até {} caracteres",
                crate::stickers::MAX_FIELD
            )
            .into());
        }
        self.set("sticker_pack_name", name)?;
        self.set("sticker_publisher", publisher)
    }

    /// Modo de conversão do vídeo (spec 004): "silicon" (padrão) ou "malvin". Lixo cai em "silicon".
    pub fn video_sticker_mode(&self) -> Result<String> {
        Ok(self
            .get("video_sticker_mode")?
            .filter(|v| v == "malvin")
            .unwrap_or_else(|| "silicon".into()))
    }

    pub fn set_video_sticker_mode(&self, m: &str) -> Result<()> {
        if m != "silicon" && m != "malvin" {
            return Err("modo de vídeo deve ser silicon ou malvin".into());
        }
        self.set("video_sticker_mode", m)
    }

    pub fn bind(&self) -> Result<String> {
        Ok(self.get("bind")?.unwrap_or_else(|| "127.0.0.1:8787".into()))
    }

    pub fn owner_jid(&self) -> Result<Option<String>> {
        // O painel grava "" para limpar o dono; vazio = sem dono.
        Ok(self.get("owner_jid")?.filter(|s| !s.is_empty()))
    }

    pub fn password_hash(&self) -> Result<Option<String>> {
        self.get("password_hash")
    }

    // ---- Monitor (spec 003) ----

    /// Regras (id, nome, padrão, ativa). O padrão NÃO é validado aqui (o painel usa `detector::valida`).
    pub fn rules(&self) -> Result<Vec<(i64, String, String, bool)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, nome, padrao, ativa FROM rules ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_rule(&self, nome: &str, padrao: &str) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO rules(nome, padrao) VALUES(?1, ?2)",
            params![nome, padrao],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_rule(&self, id: i64, nome: &str, padrao: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE rules SET nome = ?2, padrao = ?3 WHERE id = ?1",
            params![id, nome, padrao],
        )?;
        Ok(())
    }

    pub fn set_rule_active(&self, id: i64, ativa: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE rules SET ativa = ?2 WHERE id = ?1",
            params![id, ativa],
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn ignores(&self) -> Result<Vec<(i64, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, padrao FROM ignore ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_ignore(&self, padrao: &str) -> Result<i64> {
        let conn = self.conn();
        conn.execute("INSERT INTO ignore(padrao) VALUES(?1)", params![padrao])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_ignore(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM ignore WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Registra um alerta enviado; o resumo é cortado em 80 chars (nunca guarda mídia).
    pub fn log_alert(&self, ts: u64, regra: &str, resumo: &str) -> Result<()> {
        let resumo: String = resumo.chars().take(80).collect();
        self.conn().execute(
            "INSERT INTO alert_log(ts, regra, resumo) VALUES(?1, ?2, ?3)",
            params![ts as i64, regra, resumo],
        )?;
        Ok(())
    }

    /// Alertas mais recentes primeiro: (ts, regra, resumo).
    pub fn alerts(&self, limit: usize) -> Result<Vec<(u64, String, String)>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT ts, regra, resumo FROM alert_log ORDER BY ts DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get(1)?, r.get(2)?))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Apaga alertas com `ts` anterior a `agora - retencao_dias`; devolve quantos.
    pub fn purge_alerts(&self, retencao_dias: u64, agora: u64) -> Result<usize> {
        let corte = agora.saturating_sub(retencao_dias.saturating_mul(86_400));
        Ok(self
            .conn()
            .execute("DELETE FROM alert_log WHERE ts < ?1", params![corte as i64])?)
    }

    // valor ausente, inválido ou fora da faixa cai no padrão
    fn get_range(&self, key: &str, default: u64, max: u64) -> Result<u64> {
        Ok(self
            .get(key)?
            .and_then(|v| v.parse().ok())
            .filter(|n| (1..=max).contains(n))
            .unwrap_or(default))
    }

    fn set_range(&self, key: &str, nome: &str, n: u64, max: u64) -> Result<()> {
        if !(1..=max).contains(&n) {
            return Err(format!("{nome} deve estar entre 1 e {max}").into());
        }
        self.set(key, &n.to_string())
    }

    /// Janela do dedupe em minutos (padrão 30).
    pub fn dedupe_min(&self) -> Result<u64> {
        self.get_range("dedupe_min", 30, 1440)
    }

    pub fn set_dedupe_min(&self, n: u64) -> Result<()> {
        self.set_range("dedupe_min", "dedupe (min)", n, 1440)
    }

    /// Máximo de alertas por hora (padrão 10).
    pub fn alertas_hora(&self) -> Result<u64> {
        self.get_range("alertas_hora", 10, 1000)
    }

    pub fn set_alertas_hora(&self, n: u64) -> Result<()> {
        self.set_range("alertas_hora", "alertas por hora", n, 1000)
    }

    /// Retenção do histórico em dias (padrão 7).
    pub fn retencao_dias(&self) -> Result<u64> {
        self.get_range("retencao_dias", 7, 365)
    }

    pub fn set_retencao_dias(&self, n: u64) -> Result<()> {
        self.set_range("retencao_dias", "retenção (dias)", n, 365)
    }

    // "" = desligado; HH:MM inválido no banco também vira ""
    fn get_hhmm(&self, key: &str) -> Result<String> {
        Ok(self
            .get(key)?
            .filter(|v| hhmm_valido(v))
            .unwrap_or_default())
    }

    /// Início do silêncio ("HH:MM" ou "" = desligado).
    pub fn silencio_ini(&self) -> Result<String> {
        self.get_hhmm("silencio_ini")
    }

    pub fn silencio_fim(&self) -> Result<String> {
        self.get_hhmm("silencio_fim")
    }

    /// Grava o intervalo de silêncio; cada ponta é vazia ou "HH:MM" válido.
    pub fn set_silencio(&self, ini: &str, fim: &str) -> Result<()> {
        for v in [ini, fim] {
            if !v.is_empty() && !hhmm_valido(v) {
                return Err("silêncio deve ser vazio ou HH:MM (00:00 a 23:59)".into());
            }
        }
        self.set("silencio_ini", ini)?;
        self.set("silencio_fim", fim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dm_commands_lists_only_private_command_chats() {
        let s = Store::memory().unwrap();
        s.set_role("1@lid", Role::Comando, Some("Ana")).unwrap();
        s.set_role("2@s.whatsapp.net", Role::Monitorado, None)
            .unwrap();
        s.set_role("g@g.us", Role::Comando, None).unwrap();
        assert_eq!(
            s.dm_commands().unwrap(),
            vec![("1@lid".into(), Some("Ana".into()))]
        );
    }

    #[test]
    fn role_crud_and_cache() {
        let s = Store::memory().unwrap();
        s.set_role("a@g.us", Role::Comando, Some("Grupo")).unwrap();
        s.set_role("a@g.us", Role::Monitorado, None).unwrap(); // upsert
        s.set_role("b@g.us", Role::Comando, None).unwrap();
        let m = s.roles().unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m["a@g.us"], Role::Monitorado);
        s.clear_role("a@g.us").unwrap();
        assert_eq!(s.roles().unwrap().len(), 1);
    }

    #[test]
    fn second_alert_fails() {
        let s = Store::memory().unwrap();
        s.set_role("a@g.us", Role::Alerta, None).unwrap();
        s.set_role("a@g.us", Role::Alerta, None).unwrap(); // mesmo JID ok
        assert!(s.set_role("b@g.us", Role::Alerta, None).is_err());
        assert_eq!(s.roles().unwrap()["a@g.us"], Role::Alerta);
    }

    #[test]
    fn sticker_meta_defaults_and_limit() {
        let s = Store::memory().unwrap();
        assert_eq!(s.sticker_pack_name().unwrap(), "Silicon™");
        assert_eq!(s.sticker_publisher().unwrap(), "@al4sk4s");
        s.set_sticker_meta("", "x").unwrap();
        assert_eq!(s.sticker_pack_name().unwrap(), "");
        assert!(s.set_sticker_meta(&"a".repeat(65), "x").is_err());
        assert_eq!(s.sticker_publisher().unwrap(), "x");
    }

    #[test]
    fn settings_and_defaults() {
        let s = Store::memory().unwrap();
        assert_eq!(s.delay_min_ms().unwrap(), 1000);
        assert_eq!(s.delay_max_ms().unwrap(), 2000);
        assert_eq!(s.bind().unwrap(), "127.0.0.1:8787");
        assert_eq!(s.owner_jid().unwrap(), None);
        assert_eq!(s.password_hash().unwrap(), None);
        s.set("delay_min_ms", "500").unwrap();
        s.set("delay_min_ms", "700").unwrap();
        s.set("owner_jid", "x@s.whatsapp.net").unwrap();
        assert_eq!(s.get("delay_min_ms").unwrap().as_deref(), Some("700"));
        assert_eq!(s.delay_min_ms().unwrap(), 700);
        assert_eq!(s.owner_jid().unwrap().as_deref(), Some("x@s.whatsapp.net"));
        // Dono vazio (limpo pelo painel) = sem dono.
        s.set("owner_jid", "").unwrap();
        assert_eq!(s.owner_jid().unwrap(), None);
    }

    #[test]
    fn sticker_settings() {
        let s = Store::memory().unwrap();
        assert_eq!(s.sticker_per_min().unwrap(), 3);
        assert_eq!(s.video_sticker_mode().unwrap(), "silicon");
        s.set_video_sticker_mode("malvin").unwrap();
        assert_eq!(s.video_sticker_mode().unwrap(), "malvin");
        assert!(s.set_video_sticker_mode("x").is_err());
        s.set_sticker_per_min(30).unwrap();
        assert_eq!(s.sticker_per_min().unwrap(), 30);
        assert!(s.set_sticker_per_min(0).is_err());
        assert!(s.set_sticker_per_min(31).is_err());
        assert_eq!(s.sticker_per_min().unwrap(), 30);
        s.set("sticker_per_min", "999").unwrap();
        assert_eq!(s.sticker_per_min().unwrap(), 3);
        s.set("sticker_per_min", "abc").unwrap();
        assert_eq!(s.sticker_per_min().unwrap(), 3);
    }

    #[test]
    fn sticker_sound_settings() {
        let s = Store::memory().unwrap();
        assert!(s.sticker_sound().unwrap());
        assert_eq!(s.sticker_sound_max_s().unwrap(), 10);
        s.set_sticker_sound(false).unwrap();
        assert!(!s.sticker_sound().unwrap());
        s.set_sticker_sound(true).unwrap();
        assert!(s.sticker_sound().unwrap());
        s.set_sticker_sound_max_s(1).unwrap();
        assert_eq!(s.sticker_sound_max_s().unwrap(), 1);
        assert!(s.set_sticker_sound_max_s(0).is_err());
        assert!(s.set_sticker_sound_max_s(11).is_err());
        assert_eq!(s.sticker_sound_max_s().unwrap(), 1);
        s.set("sticker_sound_max_s", "99").unwrap();
        assert_eq!(s.sticker_sound_max_s().unwrap(), 10);
    }

    #[test]
    fn purge_usa_retencao_das_settings() {
        let s = Store::memory().unwrap();
        s.log_alert(1_000, "bug", "velho").unwrap();
        s.log_alert(900_000, "bug", "novo").unwrap();
        let n = s
            .purge_alerts(s.retencao_dias().unwrap(), 1_000_000)
            .unwrap(); // 7 dias = 604800 s
        assert_eq!((n, s.alerts(10).unwrap().len()), (1, 1));
    }

    #[test]
    fn rules_crud_and_seed_once() {
        let s = Store::memory().unwrap();
        let r = s.rules().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, "bug");
        assert_eq!(r[0].2, crate::detector::REGRA_BUG); // semente do SQL = constante testada
        assert!(r[0].3);
        let id = s.add_rule("promo", "promo").unwrap();
        s.update_rule(id, "promo2", "promo\\d").unwrap();
        s.set_rule_active(id, false).unwrap();
        let r = s.rules().unwrap();
        assert_eq!(r[1], (id, "promo2".into(), "promo\\d".into(), false));
        s.delete_rule(r[0].0).unwrap();
        s.delete_rule(id).unwrap();
        assert!(s.rules().unwrap().is_empty());
        let i = s.add_ignore("debug").unwrap();
        assert_eq!(s.ignores().unwrap(), vec![(i, "debug".into())]);
        s.delete_ignore(i).unwrap();
        assert!(s.ignores().unwrap().is_empty());
    }

    #[test]
    fn migrates_existing_v1_db_once() {
        let path = std::env::temp_dir().join(format!("silicon-mig-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(SCHEMA_V1).unwrap();
            c.execute_batch("PRAGMA user_version = 1").unwrap();
            c.execute("INSERT INTO roles VALUES('a@g.us','comando',NULL,1)", [])
                .unwrap();
            c.execute("INSERT INTO settings VALUES('delay_min_ms','700')", [])
                .unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.roles().unwrap().len(), 1);
        assert_eq!(s.delay_min_ms().unwrap(), 700);
        let r = s.rules().unwrap();
        assert_eq!(r.len(), 1);
        s.delete_rule(r[0].0).unwrap();
        drop(s);
        // reabrir não ressemeia
        let s = Store::open(&path).unwrap();
        assert!(s.rules().unwrap().is_empty());
        drop(s);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn alert_log_list_and_purge() {
        let s = Store::memory().unwrap();
        let dia = 86_400;
        let agora = 100 * dia;
        s.log_alert(agora - 8 * dia, "bug", "velho").unwrap();
        s.log_alert(agora - dia, "bug", &"x".repeat(200)).unwrap();
        let l = s.alerts(10).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].0, agora - dia); // mais recente primeiro
        assert_eq!(l[0].2.chars().count(), 80);
        assert_eq!(s.alerts(1).unwrap().len(), 1);
        assert_eq!(s.purge_alerts(7, agora).unwrap(), 1);
        let l = s.alerts(10).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].0, agora - dia);
    }

    #[test]
    fn monitor_settings_defaults_and_invalid() {
        let s = Store::memory().unwrap();
        assert_eq!(s.dedupe_min().unwrap(), 30);
        assert_eq!(s.alertas_hora().unwrap(), 10);
        assert_eq!(s.retencao_dias().unwrap(), 7);
        assert_eq!(s.silencio_ini().unwrap(), "");
        assert_eq!(s.silencio_fim().unwrap(), "");
        s.set_dedupe_min(5).unwrap();
        s.set_alertas_hora(20).unwrap();
        s.set_retencao_dias(14).unwrap();
        s.set_silencio("22:00", "07:00").unwrap();
        assert_eq!(s.dedupe_min().unwrap(), 5);
        assert_eq!(s.alertas_hora().unwrap(), 20);
        assert_eq!(s.retencao_dias().unwrap(), 14);
        assert_eq!(s.silencio_ini().unwrap(), "22:00");
        assert_eq!(s.silencio_fim().unwrap(), "07:00");
        assert!(s.set_dedupe_min(0).is_err());
        assert!(s.set_alertas_hora(1001).is_err());
        assert!(s.set_retencao_dias(0).is_err());
        assert!(s.set_silencio("25:00", "").is_err());
        assert!(s.set_silencio("7:00", "").is_err());
        s.set_silencio("", "").unwrap();
        assert_eq!(s.silencio_ini().unwrap(), "");
        // lixo no banco cai no padrão
        s.set("dedupe_min", "abc").unwrap();
        s.set("alertas_hora", "0").unwrap();
        s.set("retencao_dias", "-3").unwrap();
        s.set("silencio_ini", "lixo").unwrap();
        assert_eq!(s.dedupe_min().unwrap(), 30);
        assert_eq!(s.alertas_hora().unwrap(), 10);
        assert_eq!(s.retencao_dias().unwrap(), 7);
        assert_eq!(s.silencio_ini().unwrap(), "");
    }

    #[test]
    fn download_settings() {
        let s = Store::memory().unwrap();
        assert_eq!(s.dl_per_min().unwrap(), 2);
        s.set_dl_per_min(10).unwrap();
        assert_eq!(s.dl_per_min().unwrap(), 10);
        assert!(s.set_dl_per_min(0).is_err());
        assert!(s.set_dl_per_min(11).is_err());
        s.set("dl_per_min", "abc").unwrap();
        assert_eq!(s.dl_per_min().unwrap(), 2);
        assert_eq!(s.dl_timeout_s().unwrap(), 120);
        s.set("dl_timeout_s", "5").unwrap();
        assert_eq!(s.dl_timeout_s().unwrap(), 120);
        s.set("dl_timeout_s", "600").unwrap();
        assert_eq!(s.dl_timeout_s().unwrap(), 600);
    }

    #[test]
    fn cookies_status_roundtrip() {
        let s = Store::memory().unwrap();
        assert_eq!(s.cookies_status("tiktok").unwrap(), None);
        s.set_cookies_status("tiktok", "ok|123").unwrap();
        assert_eq!(
            s.cookies_status("tiktok").unwrap().as_deref(),
            Some("ok|123")
        );
        assert_eq!(s.cookies_status("instagram").unwrap(), None);
        assert!(s.cookies_status("youtube").is_err());
        assert!(s.set_cookies_status("youtube", "ok").is_err());
    }

    #[test]
    fn open_twice_is_idempotent_and_private() {
        let path = std::env::temp_dir().join(format!("silicon-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Store::open(&path)
            .unwrap()
            .set_role("a@g.us", Role::Comando, None)
            .unwrap();
        let s = Store::open(&path).unwrap();
        assert_eq!(s.roles().unwrap().len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        drop(s);
        std::fs::remove_file(&path).unwrap();
    }
}
