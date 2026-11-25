//! Chamada ao gallery-dl externo (fotos e carrosséis; sem shell) + probe de boot.
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::process::Command;

use super::{DlError, Platform, cookies};

static GALLERY_OK: AtomicBool = AtomicBool::new(false);

/// Teto de itens por comando (P8: uma rajada curta).
pub const MAX_ITEMS: usize = 10;
/// Soma máxima dos itens mantidos.
const MAX_TOTAL: u64 = 100 * 1024 * 1024;
/// Nome da cópia de cookies dentro do tmp; `collect` o ignora.
const COOKIES_NAME: &str = "cookies.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Image,
    Video,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub path: PathBuf,
    pub size: u64,
    pub kind: ItemKind,
}

/// Binário; `SILICON_GALLERYDL` existe só para testes com script falso.
fn bin() -> String {
    std::env::var("SILICON_GALLERYDL").unwrap_or_else(|_| "gallery-dl".into())
}

/// Argumentos do gallery-dl. A URL vem sempre depois de `--` (nunca vira opção).
pub fn build_args(url: &str, out_dir: &Path, cookies: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = ["-D"].map(String::from).into();
    a.push(out_dir.display().to_string());
    a.extend(
        [
            "--no-mtime",
            "--range",
            "1-10",
            "--filesize-max",
            "100M",
            "--quiet",
        ]
        .map(String::from),
    );
    if let Some(c) = cookies {
        a.push("--cookies".into());
        a.push(c.display().to_string());
    }
    a.push("--".into());
    a.push(url.into());
    a
}

/// Traduz o stderr do gallery-dl em erro. Mesma lógica do yt-dlp.
pub fn classify(stderr: &str) -> DlError {
    let s = stderr.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| s.contains(k));
    if has(&["larger than", "filesize"]) {
        DlError::TooBig
    } else if has(&[
        "login",
        "registered users",
        "authenticat",
        "401",
        "unauthorized",
    ]) {
        DlError::LoginRequired
    } else if has(&["429", "too many requests"]) {
        DlError::Other
    } else if has(&[
        "unsupported url",
        "404",
        "not found",
        "unavailable",
        "private",
        "removed",
    ]) {
        DlError::NotFound
    } else {
        DlError::Other
    }
}

fn kind_of(path: &Path) -> Option<ItemKind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => Some(ItemKind::Image),
        "mp4" | "mov" | "webm" | "m4v" => Some(ItemKind::Video),
        _ => None,
    }
}

/// Lê o diretório: só mídia conhecida, ordenada por nome, até `MAX_ITEMS`.
pub fn collect(dir: &Path) -> Result<Vec<Item>, DlError> {
    let mut items = Vec::new();
    for e in std::fs::read_dir(dir)
        .map_err(|_| DlError::Other)?
        .flatten()
    {
        let path = e.path();
        if e.file_name() == COOKIES_NAME {
            continue;
        }
        let Some(kind) = kind_of(&path) else { continue };
        let Ok(md) = e.metadata() else { continue };
        if md.is_file() {
            items.push(Item {
                path,
                size: md.len(),
                kind,
            });
        }
    }
    // gallery-dl numera os arquivos (nome = ordem do post)
    items.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    items.truncate(MAX_ITEMS);
    if items.is_empty() {
        return Err(DlError::NotFound);
    }
    if items.iter().map(|i| i.size).sum::<u64>() > MAX_TOTAL {
        return Err(DlError::TooBig);
    }
    Ok(items)
}

pub async fn fetch(
    url: &str,
    plat: Platform,
    cookies_dir: &Path,
    tmp: &Path,
    timeout: Duration,
) -> Result<Vec<Item>, DlError> {
    let res = run(&bin(), url, plat, cookies_dir, tmp, timeout).await;
    match &res {
        Ok(v) => tracing::info!(plat = plat.name(), itens = v.len(), "gallery ok"),
        Err(_) => tracing::warn!(plat = plat.name(), "gallery falhou"),
    }
    res
}

async fn run(
    bin: &str,
    url: &str,
    plat: Platform,
    cookies_dir: &Path,
    tmp: &Path,
    timeout: Duration,
) -> Result<Vec<Item>, DlError> {
    // Cópia 0600 em tmp: o gallery-dl pode reescrever o arquivo de cookies.
    let copy = match std::fs::read(cookies::path(cookies_dir, plat)) {
        Ok(data) => {
            use std::io::Write;
            let dst = tmp.join(COOKIES_NAME);
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&dst)
                .and_then(|mut f| f.write_all(&data))
                .map_err(|_| DlError::Other)?;
            Some(dst)
        }
        Err(_) => None,
    };
    let args = build_args(url, tmp, copy.as_deref());
    let run = Command::new(bin)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let out = match tokio::time::timeout(timeout, run).await {
        Err(_) => return Err(DlError::Timeout),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => return Err(DlError::Missing),
        Ok(Err(_)) => return Err(DlError::Other),
        Ok(Ok(o)) => o,
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // Só a classe do erro no journal (corta antes da URL).
        let linha = err
            .lines()
            .rev()
            .find(|l| l.contains("error"))
            .unwrap_or("");
        tracing::warn!(erro = %linha.split(" for '").next().unwrap_or(""), "gallery-dl falhou");
        return Err(classify(&err));
    }
    collect(tmp)
}

/// Roda `gallery-dl --version` no boot; sem ele só o caminho yt-dlp funciona.
pub async fn probe() {
    let run = Command::new(bin())
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let ver = match tokio::time::timeout(Duration::from_secs(30), run).await {
        Ok(Ok(o)) if o.status.success() => {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        Ok(Ok(o)) => {
            tracing::warn!(status = ?o.status, "probe do gallery-dl");
            None
        }
        Ok(Err(e)) => {
            tracing::warn!("probe do gallery-dl: não executou: {e}");
            None
        }
        Err(_) => {
            tracing::warn!("probe do gallery-dl: timeout");
            None
        }
    };
    GALLERY_OK.store(ver.is_some(), Ordering::Relaxed);
    match &ver {
        Some(v) => tracing::info!(version = %v, "gallery-dl ok"),
        None => tracing::warn!("gallery-dl ausente: fotos vão responder 'indisponível'"),
    }
}

pub fn ok() -> bool {
    GALLERY_OK.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_put_url_after_separator() {
        let a = build_args("https://x/y", Path::new("/t"), None);
        assert_eq!(a[a.len() - 2], "--");
        assert_eq!(a[a.len() - 1], "https://x/y");
        assert!(!a.contains(&"--cookies".to_string()));
        let i = a.iter().position(|x| x == "-D").unwrap();
        assert_eq!(a[i + 1], "/t");
        let i = a.iter().position(|x| x == "--range").unwrap();
        assert_eq!(a[i + 1], "1-10");
        let a = build_args("u", Path::new("/t"), Some(Path::new("/c.txt")));
        let i = a.iter().position(|x| x == "--cookies").unwrap();
        assert_eq!(a[i + 1], "/c.txt");
        assert!(i < a.iter().position(|x| x == "--").unwrap());
    }

    fn touch(d: &Path, name: &str, n: usize) {
        std::fs::write(d.join(name), vec![0u8; n]).unwrap();
    }

    #[test]
    fn collect_orders_and_filters() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "b_02.mp4", 3);
        touch(d.path(), "a_01.JPG", 2);
        touch(d.path(), "c_03.webp", 1);
        touch(d.path(), "cookies.txt", 9);
        touch(d.path(), "a_01.json", 9);
        let v = collect(d.path()).unwrap();
        let names: Vec<_> = v
            .iter()
            .map(|i| i.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["a_01.JPG", "b_02.mp4", "c_03.webp"]);
        assert_eq!(v[0].kind, ItemKind::Image);
        assert_eq!(v[1].kind, ItemKind::Video);
        assert_eq!(v[1].size, 3);
    }

    #[test]
    fn collect_caps_and_empty() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(collect(d.path()).err(), Some(DlError::NotFound));
        touch(d.path(), "cookies.txt", 1);
        assert_eq!(collect(d.path()).err(), Some(DlError::NotFound));
        for i in 0..12 {
            touch(d.path(), &format!("img_{i:02}.png"), 1);
        }
        let v = collect(d.path()).unwrap();
        assert_eq!(v.len(), MAX_ITEMS);
        assert!(v[9].path.ends_with("img_09.png"));
    }

    #[test]
    fn classifies_messages() {
        use DlError::*;
        assert_eq!(classify("[instagram][error] Login required"), LoginRequired);
        assert_eq!(classify("HttpError: '404 Not Found'"), NotFound);
        assert_eq!(classify("Unsupported URL 'https://x'"), NotFound);
        assert_eq!(classify("HTTP 429 Too Many Requests"), Other);
        assert_eq!(classify("file larger than filesize-max"), TooBig);
        assert_eq!(
            classify("HttpError: '401 Unauthorized' for 'x'"),
            LoginRequired
        );
        assert_eq!(classify("kaboom"), Other);
    }
}
