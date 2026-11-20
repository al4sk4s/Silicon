//! T5: checagem do ffmpeg no boot.
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::process::Command;

static FFMPEG_OK: AtomicBool = AtomicBool::new(false);

/// Roda `ffmpeg -encoders` e guarda se há `libwebp_anim`.
pub async fn probe() {
    let run = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let ok = match tokio::time::timeout(Duration::from_secs(5), run).await {
        Ok(Ok(o)) => {
            let found = has_encoder(&String::from_utf8_lossy(&o.stdout));
            if !(o.status.success() && found) {
                tracing::warn!(status = ?o.status, found, bytes = o.stdout.len(), "probe do ffmpeg");
            }
            o.status.success() && found
        }
        Ok(Err(e)) => {
            tracing::warn!("probe do ffmpeg: não executou: {e}");
            false
        }
        Err(_) => {
            tracing::warn!("probe do ffmpeg: timeout");
            false
        }
    };
    FFMPEG_OK.store(ok, Ordering::Relaxed);
    if ok {
        tracing::info!("ffmpeg com libwebp_anim ok");
    } else {
        tracing::warn!(
            "ffmpeg ausente ou sem libwebp_anim: /figurinha vai responder 'ffmpeg ausente'"
        );
    }
}

pub fn ffmpeg_ok() -> bool {
    FFMPEG_OK.load(Ordering::Relaxed)
}

fn has_encoder(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|l| l.split_whitespace().nth(1) == Some("libwebp_anim"))
}

#[cfg(test)]
mod tests {
    use super::has_encoder;

    #[test]
    fn detects_libwebp_anim() {
        let com = " V....D libwebp_anim         libwebp WebP image (codec webp)\n";
        let sem = " V....D libx264              libx264 H.264\n V....D libwebp              libwebp WebP image\n";
        assert!(has_encoder(com));
        assert!(!has_encoder(sem));
    }
}
