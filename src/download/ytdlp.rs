//! Chamada ao yt-dlp externo (sem shell) + probe de boot.
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::process::Command;

use super::{DlError, Platform, cookies};

static YTDLP_OK: AtomicBool = AtomicBool::new(false);

/// Binário; `SILICON_YTDLP` existe só para testes com script falso.
fn bin() -> String {
    std::env::var("SILICON_YTDLP").unwrap_or_else(|_| "yt-dlp".into())
}

pub struct Fetched {
    pub path: PathBuf,
    pub size: u64,
}

/// Argumentos do yt-dlp. A URL vem sempre depois de `--` (nunca vira opção).
pub fn build_args(url: &str, out_dir: &Path, cookies: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = [
        "-f",
        // H.264+AAC preferidos (o único que o WhatsApp/celular toca sempre); sem eles, handle.rs recodifica
        "bv*[vcodec^=avc1]+ba[acodec^=mp4a]/b[vcodec^=avc1]/bv*+ba[acodec^=mp4a]/b[acodec^=mp4a]/bv*+ba/b",
        "--no-cache-dir",
        "--merge-output-format",
        "mp4",
        "--no-playlist",
        "--playlist-items",
        "1",
        "--max-filesize",
        "100M",
        "--no-progress",
        "--quiet",
        "--no-warnings",
        "-o",
    ]
    .map(String::from)
    .into();
    a.push(format!("{}/v.%(ext)s", out_dir.display()));
    if let Some(c) = cookies {
        a.push("--cookies".into());
        a.push(c.display().to_string());
    }
    a.push("--".into());
    a.push(url.into());
    a
}

/// Traduz o stderr do yt-dlp em erro. LoginRequired vem antes de NotFound.
pub fn classify(stderr: &str) -> DlError {
    let s = stderr.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| s.contains(k));
    if has(&["larger than max-filesize", "file is larger"]) {
        DlError::TooBig
    } else if has(&["login", "registered users", "rate-limit for accessing"]) {
        DlError::LoginRequired
    } else if has(&["429", "too many requests"]) {
        DlError::Other
    } else if has(&[
        "empty media response",
        "no video formats",
        "there is no video",
        "no video in this post",
        "unsupported url",
        "ip address is blocked",
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

pub async fn fetch(
    url: &str,
    plat: Platform,
    cookies_dir: &Path,
    tmp: &Path,
    timeout: Duration,
) -> Result<Fetched, DlError> {
    fetch_with(&bin(), url, plat, cookies_dir, tmp, timeout).await
}

async fn fetch_with(
    bin: &str,
    url: &str,
    plat: Platform,
    cookies_dir: &Path,
    tmp: &Path,
    timeout: Duration,
) -> Result<Fetched, DlError> {
    let res = run(bin, url, plat, cookies_dir, tmp, timeout).await;
    match &res {
        Ok(f) => tracing::info!(plat = plat.name(), bytes = f.size, "download ok"),
        Err(_) => tracing::warn!(plat = plat.name(), "download falhou"),
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
) -> Result<Fetched, DlError> {
    // Cópia 0600 em tmp: o yt-dlp pode reescrever o arquivo de cookies.
    let copy = match std::fs::read(cookies::path(cookies_dir, plat)) {
        Ok(data) => {
            use std::io::Write;
            let dst = tmp.join("cookies.txt");
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
        return Err(classify(&String::from_utf8_lossy(&out.stderr)));
    }
    let rd = std::fs::read_dir(tmp).map_err(|_| DlError::Other)?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("v.") && name.ends_with(".mp4") {
            let size = e.metadata().map_err(|_| DlError::Other)?.len();
            return Ok(Fetched {
                path: e.path(),
                size,
            });
        }
    }
    Err(DlError::Other)
}

/// Roda `yt-dlp --version` no boot; avisa se ausente ou velho (> 60 dias).
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
            tracing::warn!(status = ?o.status, "probe do yt-dlp");
            None
        }
        Ok(Err(e)) => {
            tracing::warn!("probe do yt-dlp: não executou: {e}");
            None
        }
        Err(_) => {
            tracing::warn!("probe do yt-dlp: timeout");
            None
        }
    };
    YTDLP_OK.store(ver.is_some(), Ordering::Relaxed);
    match &ver {
        Some(v) => {
            tracing::info!(version = %v, "yt-dlp ok");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if is_old(v, now) {
                tracing::warn!(version = %v, "yt-dlp com mais de 60 dias: atualize");
            }
        }
        None => tracing::warn!("yt-dlp ausente: /baixar vai responder 'yt-dlp ausente'"),
    }
}

pub fn ok() -> bool {
    YTDLP_OK.load(Ordering::Relaxed)
}

/// Dias desde 1970-01-01 para data civil (algoritmo de H. Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `AAAA.MM.DD[...]` com mais de 60 dias em relação a `today_unix_s`; ilegível = não velha.
pub fn is_old(version: &str, today_unix_s: u64) -> bool {
    let mut p = version.split('.').map(|x| x.trim().parse::<i64>());
    let (Some(Ok(y)), Some(Ok(m)), Some(Ok(d))) = (p.next(), p.next(), p.next()) else {
        return false;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return false;
    }
    days_from_civil(y, m, d) + 60 < (today_unix_s / 86400) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn args_put_url_after_separator() {
        let a = build_args("https://x/y", Path::new("/t"), None);
        assert_eq!(a[a.len() - 2], "--");
        assert_eq!(a[a.len() - 1], "https://x/y");
        assert!(!a.contains(&"--cookies".to_string()));
        let a = build_args("u", Path::new("/t"), Some(Path::new("/c.txt")));
        let i = a.iter().position(|x| x == "--cookies").unwrap();
        assert_eq!(a[i + 1], "/c.txt");
        assert!(i < a.iter().position(|x| x == "--").unwrap());
        assert!(a.contains(&"/t/v.%(ext)s".to_string()));
    }

    #[test]
    fn classifies_real_messages() {
        use DlError::*;
        let c = classify;
        assert_eq!(
            c(
                "ERROR: [Instagram] redirected to the login page. You have exceeded the rate-limit for accessing posts anonymously"
            ),
            LoginRequired
        );
        assert_eq!(
            c("only available for registered users who follow this account"),
            LoginRequired
        );
        assert_eq!(c("ERROR: Instagram sent an empty media response"), NotFound);
        assert_eq!(
            c("ERROR: Unsupported URL: https://www.tiktok.com/?_r=1"),
            NotFound
        );
        assert_eq!(
            c("Your IP address is blocked from accessing this post"),
            NotFound
        );
        assert_eq!(
            c("File is larger than max-filesize (200 bytes > 100 bytes)"),
            TooBig
        );
        assert_eq!(c("HTTP Error 429: Too Many Requests"), Other);
        assert_eq!(c("kaboom"), Other);
    }

    #[test]
    fn old_version() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        let today = days_from_civil(2026, 11, 24) as u64 * 86400;
        assert!(!is_old("2026.11.23", today));
        assert!(!is_old("2026.09.25", today)); // 60 dias exatos
        assert!(is_old("2026.09.24", today));
        assert!(is_old("2025.01.01.123456", today));
        assert!(!is_old("lixo", today));
    }

    fn fake(dir: &Path, body: &str) -> String {
        let p = dir.join("fake.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.display().to_string()
    }

    const OK: &str = r#"while [ $# -gt 0 ]; do [ "$1" = "-o" ] && o="$2"; shift; done
out=$(printf %s "$o" | sed 's/%(ext)s/mp4/'); echo data > "$out""#;

    async fn go(body: &str, ms: u64) -> (Result<Fetched, DlError>, Duration) {
        let sh = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let ck = tempfile::tempdir().unwrap();
        let bin = fake(sh.path(), body);
        let t = std::time::Instant::now();
        let d = Duration::from_millis(ms);
        let r = fetch_with(
            &bin,
            "https://x",
            Platform::TikTok,
            ck.path(),
            tmp.path(),
            d,
        )
        .await;
        (r, t.elapsed())
    }

    #[tokio::test]
    async fn fetch_ok() {
        let (r, _) = go(OK, 5000).await;
        let f = r.unwrap();
        assert_eq!(f.size, 5);
        assert!(f.path.ends_with("v.mp4"));
    }

    #[tokio::test]
    async fn fetch_timeout_returns_fast() {
        let (r, el) = go("sleep 30", 300).await;
        assert_eq!(r.err(), Some(DlError::Timeout));
        assert!(el < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn fetch_login_and_missing() {
        let (r, _) = go(
            "echo 'ERROR: redirected to the login page' >&2; exit 1",
            5000,
        )
        .await;
        assert_eq!(r.err(), Some(DlError::LoginRequired));
        let t = tempfile::tempdir().unwrap();
        let d = Duration::from_secs(1);
        let r = fetch_with(
            "/nao/existe/yt-dlp",
            "u",
            Platform::TikTok,
            t.path(),
            t.path(),
            d,
        )
        .await;
        assert_eq!(r.err(), Some(DlError::Missing));
    }
}
