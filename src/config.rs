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

    pub fn bind(&self) -> Result<String> {
        Ok(self.get("bind")?.unwrap_or_else(|| "127.0.0.1:8080".into()))
    }

    pub fn owner_jid(&self) -> Result<Option<String>> {
        // O painel grava "" para limpar o dono; vazio = sem dono.
        Ok(self.get("owner_jid")?.filter(|s| !s.is_empty()))
    }

    pub fn password_hash(&self) -> Result<Option<String>> {
        self.get("password_hash")
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
        assert_eq!(s.bind().unwrap(), "127.0.0.1:8080");
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
