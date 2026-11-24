//! Cookies do usuário: converte export JSON (Cookie-Editor/EditThisCookie) para
//! Netscape e guarda em arquivo 0600. Nunca loga/retorna valores de cookie em erros.

use super::Platform;
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// Valor JSON escalar (cookies não têm valores aninhados).
#[derive(Debug, Clone)]
enum Val {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

// shortcut: parser só aceita array de objetos com valores escalares; trocar por serde_json se precisar de mais.
struct Parser {
    c: Vec<char>,
    i: usize,
}

impl Parser {
    fn ws(&mut self) {
        while self.c.get(self.i).is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, ch: char) -> Result<(), ()> {
        self.ws();
        if self.c.get(self.i) == Some(&ch) {
            self.i += 1;
            Ok(())
        } else {
            Err(())
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.ws();
        self.c.get(self.i).copied()
    }
    fn hex4(&mut self) -> Result<u32, ()> {
        let s: String = self.c.get(self.i..self.i + 4).ok_or(())?.iter().collect();
        self.i += 4;
        u32::from_str_radix(&s, 16).map_err(|_| ())
    }
    fn string(&mut self) -> Result<String, ()> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            let ch = *self.c.get(self.i).ok_or(())?;
            self.i += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let e = *self.c.get(self.i).ok_or(())?;
                    self.i += 1;
                    match e {
                        '"' | '\\' | '/' => out.push(e),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let mut n = self.hex4()?;
                            if (0xD800..0xDC00).contains(&n)
                                && self.c.get(self.i) == Some(&'\\')
                                && self.c.get(self.i + 1) == Some(&'u')
                            {
                                self.i += 2;
                                let lo = self.hex4()?;
                                n = 0x10000 + ((n - 0xD800) << 10) + lo.wrapping_sub(0xDC00);
                            }
                            out.push(char::from_u32(n).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(()),
                    }
                }
                _ => out.push(ch),
            }
        }
    }
    fn value(&mut self) -> Result<Val, ()> {
        match self.peek().ok_or(())? {
            '"' => Ok(Val::Str(self.string()?)),
            _ => {
                let start = self.i;
                while self
                    .c
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || "+-.".contains(*c))
                {
                    self.i += 1;
                }
                let t: String = self.c[start..self.i].iter().collect();
                match t.as_str() {
                    "true" => Ok(Val::Bool(true)),
                    "false" => Ok(Val::Bool(false)),
                    "null" => Ok(Val::Null),
                    _ => t.parse::<f64>().map(Val::Num).map_err(|_| ()),
                }
            }
        }
    }
    fn object(&mut self) -> Result<HashMap<String, Val>, ()> {
        self.eat('{')?;
        let mut m = HashMap::new();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(m);
        }
        loop {
            let k = self.string()?;
            self.eat(':')?;
            m.insert(k, self.value()?);
            match self.peek() {
                Some(',') => self.i += 1,
                Some('}') => {
                    self.i += 1;
                    return Ok(m);
                }
                _ => return Err(()),
            }
        }
    }
    fn array(&mut self) -> Result<Vec<HashMap<String, Val>>, ()> {
        self.eat('[')?;
        let mut v = Vec::new();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(v);
        }
        loop {
            v.push(self.object()?);
            match self.peek() {
                Some(',') => self.i += 1,
                Some(']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(()),
            }
        }
        if self.peek().is_some() {
            return Err(());
        }
        Ok(v)
    }
}

fn str_of<'a>(o: &'a HashMap<String, Val>, k: &str) -> Option<&'a str> {
    match o.get(k) {
        Some(Val::Str(s)) => Some(s),
        _ => None,
    }
}

fn bad(s: &str) -> bool {
    s.contains(['\t', '\n', '\r'])
}

/// Converte o export JSON para texto Netscape, mantendo só cookies da plataforma.
pub fn json_to_netscape(json: &str, plat: Platform) -> Result<String, String> {
    let first = json.trim_start().chars().next();
    if first.is_some() && first != Some('[') {
        // objeto/valor solto: distingue de JSON quebrado só pela mensagem
        let mut p = Parser {
            c: json.chars().collect(),
            i: 0,
        };
        if p.peek() == Some('{') && p.object().is_ok() {
            return Err("O JSON precisa ser uma lista (array) de cookies.".into());
        }
    }
    let mut p = Parser {
        c: json.chars().collect(),
        i: 0,
    };
    let items = p.array().map_err(|_| "JSON inválido.".to_string())?;
    if items.is_empty() {
        return Err("A lista de cookies está vazia.".into());
    }
    let base = match plat {
        Platform::TikTok => "tiktok.com",
        Platform::Instagram => "instagram.com",
    };
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    let mut kept = 0;
    for o in &items {
        let (Some(name), Some(domain)) = (str_of(o, "name"), str_of(o, "domain")) else {
            continue;
        };
        let host = domain.trim_start_matches('.').to_ascii_lowercase();
        if host != base && !host.ends_with(&format!(".{base}")) {
            continue;
        }
        let value = str_of(o, "value").unwrap_or("");
        let path = str_of(o, "path").unwrap_or("/");
        if [name, value, domain, path].iter().any(|s| bad(s)) {
            return Err(
                "Cookie com caractere de controle (tab/quebra de linha); rejeitado.".into(),
            );
        }
        let secure = matches!(o.get("secure"), Some(Val::Bool(true)));
        let expiry = match o.get("expirationDate") {
            Some(Val::Num(n)) if *n > 0.0 => *n as i64,
            _ => 0,
        };
        let sub = if domain.starts_with('.') {
            "TRUE"
        } else {
            "FALSE"
        };
        let sec = if secure { "TRUE" } else { "FALSE" };
        out.push_str(&format!(
            "{domain}\t{sub}\t{path}\t{sec}\t{expiry}\t{name}\t{value}\n"
        ));
        kept += 1;
    }
    if kept == 0 {
        return Err(format!("Nenhum cookie de {base} encontrado."));
    }
    Ok(out)
}

pub fn path(dir: &Path, plat: Platform) -> PathBuf {
    dir.join(format!("{}.txt", plat.name()))
}

/// Grava `dir/<plataforma>.txt` com 0600 desde a criação (tmp + rename).
pub fn save(dir: &Path, plat: Platform, netscape: &str) -> std::io::Result<PathBuf> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let dest = path(dir, plat);
    let tmp = dir.join(format!("{}.txt.tmp", plat.name()));
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(netscape.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

/// Remove os cookies da plataforma; ausente = ok.
pub fn remove(dir: &Path, plat: Platform) -> std::io::Result<()> {
    match std::fs::remove_file(path(dir, plat)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const J: &str = r#"[
      {"name":"sid","value":"a\"b","domain":".tiktok.com","path":"/","secure":true,"expirationDate":1900000000.5},
      {"name":"s","value":"x","domain":"www.tiktok.com","path":"/a"},
      {"name":"o","value":"y","domain":".example.com","path":"/"},
      {"name":"fake","value":"z","domain":"nottiktok.com","path":"/"}
    ]"#;

    #[test]
    fn converts_and_filters() {
        let s = json_to_netscape(J, Platform::TikTok).unwrap();
        let l: Vec<&str> = s.lines().collect();
        assert_eq!(l[0], "# Netscape HTTP Cookie File");
        assert_eq!(l[1], ".tiktok.com\tTRUE\t/\tTRUE\t1900000000\tsid\ta\"b");
        assert_eq!(l[2], "www.tiktok.com\tFALSE\t/a\tFALSE\t0\ts\tx");
        assert_eq!(l.len(), 3);
        assert!(json_to_netscape(J, Platform::Instagram).is_err());
    }

    #[test]
    fn errors() {
        assert!(json_to_netscape("[]", Platform::TikTok).is_err());
        assert!(json_to_netscape("[{", Platform::TikTok).is_err());
        assert!(
            json_to_netscape("{\"a\":1}", Platform::TikTok)
                .unwrap_err()
                .contains("lista")
        );
        let inj = r#"[{"name":"a","value":"x\ty","domain":".tiktok.com","path":"/"}]"#;
        let e = json_to_netscape(inj, Platform::TikTok).unwrap_err();
        assert!(!e.contains('x'));
    }

    #[test]
    fn saves_0600_and_removes() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("silicon-ck-{}-{n}", std::process::id()));
        let p = save(&dir, Platform::TikTok, "x\n").unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        remove(&dir, Platform::TikTok).unwrap();
        remove(&dir, Platform::TikTok).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
