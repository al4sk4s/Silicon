mod auth;
mod config;
mod dispatch;
mod lock;
mod metrics;
mod panel;
mod roles;
mod state;
mod stickers;
mod wa;

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use sha2::{Digest, Sha256};
use state::AppState;
use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

type BoxErr = Box<dyn Error>;

#[derive(Debug, PartialEq)]
enum Cmd {
    Daemon,
    SetPassword,
}

/// Argumento inválido sai com 64 (nunca 2: o 2 é "logout", o systemd não reinicia).
fn parse_args(args: &[String]) -> Result<Cmd, String> {
    match args {
        [] => Ok(Cmd::Daemon),
        [a] if a == "set-password" => Ok(Cmd::SetPassword),
        _ => Err("uso: silicon [set-password]".into()),
    }
}

/// Telefone do pair code: só dígitos (com DDI), 8 a 15.
fn valid_phone(s: &str) -> bool {
    (8..=15).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

fn data_dir() -> Result<PathBuf, BoxErr> {
    let dir = PathBuf::from(
        std::env::var("SILICON_DATA_DIR").unwrap_or_else(|_| "/var/lib/silicon".into()),
    );
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(&dir)?;
    Ok(dir)
}

/// Gera (uma vez) o certificado autoassinado; a chave nasce com 0600.
fn ensure_cert(dir: &Path) -> Result<(PathBuf, PathBuf), BoxErr> {
    use std::io::Write;
    let (cert_path, key_path) = (dir.join("panel-cert.pem"), dir.join("panel-key.pem"));
    if !(cert_path.exists() && key_path.exists()) {
        let ck =
            rcgen::generate_simple_self_signed(vec!["silicon.local".into(), "localhost".into()])?;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        opts.open(&key_path)?
            .write_all(ck.signing_key.serialize_pem().as_bytes())?;
        std::fs::write(&cert_path, ck.cert.pem())?;
    }
    Ok((cert_path, key_path))
}

/// SHA-256 do certificado salvo, para o dono conferir no aviso do navegador.
fn fingerprint(cert_path: &Path) -> Result<String, BoxErr> {
    let der = CertificateDer::from_pem_file(cert_path)?;
    let hex: Vec<String> = Sha256::digest(der.as_ref())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect();
    Ok(hex.join(":"))
}

/// Lê uma linha do stdin sem eco (se for terminal); restaura o eco em qualquer saída.
fn read_secret(prompt: &str) -> Result<String, BoxErr> {
    use std::io::{IsTerminal, Write};
    struct Echo(bool);
    impl Drop for Echo {
        fn drop(&mut self) {
            if self.0 {
                let _ = std::process::Command::new("stty").arg("echo").status();
            }
        }
    }
    let tty = std::io::stdin().is_terminal();
    // shortcut: usa `stty` (sem crate); só Unix, trocar por rpassword se for portar.
    if tty {
        std::process::Command::new("stty").arg("-echo").status()?;
    }
    let _guard = Echo(tty);
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    if tty {
        eprintln!();
    }
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

fn set_password(dir: &Path) -> Result<(), BoxErr> {
    let store = config::Store::open(&dir.join("silicon.db"))?;
    let pw = read_secret("Nova senha do painel: ")?;
    if pw.is_empty() {
        return Err("senha vazia não vale".into());
    }
    if pw != read_secret("Repita a senha: ")? {
        return Err("as senhas não conferem".into());
    }
    store.set("password_hash", &auth::hash_password(&pw)?)?;
    eprintln!("Senha gravada. Reinicie o serviço: o bind do painel só é decidido na partida.");
    Ok(())
}

async fn daemon(dir: &Path) -> Result<(), BoxErr> {
    // Duas bibliotecas de cripto no processo dariam panic no TLS; fixa a ring.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _lock = lock::acquire(&dir.join("silicon.lock"))?;
    let store = config::Store::open(&dir.join("silicon.db"))?;
    let has_pw = store.password_hash()?.is_some();
    let configured = store.bind()?;
    let bind = panel::effective_bind(&configured, has_pw);
    if bind != configured {
        warn!("sem senha do painel: forçando loopback ({bind}); use `silicon set-password`");
    }
    let https = !panel::is_loopback(&bind);
    let addr: SocketAddr = bind.parse()?;

    let roles = Arc::new(RwLock::new(Default::default()));
    let st = Arc::new(AppState::new(store, roles, https));
    st.reload_roles()?;
    let app: IntoMakeServiceWithConnectInfo<_, SocketAddr> =
        panel::router(st.clone()).into_make_service_with_connect_info::<SocketAddr>();

    let serve = async {
        if https {
            let (cert, key) = ensure_cert(dir)?;
            info!(
                "painel em https://{addr} — SHA-256 do certificado: {}",
                fingerprint(&cert)?
            );
            let cfg = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
            axum_server::bind_rustls(addr, cfg).serve(app).await?;
        } else {
            info!("painel em http://{addr}");
            let l = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(l, app).await?;
        }
        Ok::<(), BoxErr>(())
    };

    let pair = match std::env::var("SILICON_PAIR_PHONE") {
        Ok(p) if valid_phone(&p) => Some(p),
        Ok(_) => return Err("SILICON_PAIR_PHONE inválido: só dígitos, com DDI (8 a 15)".into()),
        Err(_) => None,
    };
    st.lock_pairing().phone = pair;
    let session = dir.join("session.db");
    let session = session.to_str().ok_or("caminho da sessão não é UTF-8")?;

    stickers::check::probe().await;

    // Qualquer lado que terminar encerra o processo; wa::run Ok = shutdown pedido.
    tokio::select! {
        r = serve => r,
        r = wa::run(st.clone(), session) => r,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(64);
        }
    };
    // INFO no máximo: debug/trace da lib poderia expor conteúdo (P4).
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_timer(state::LocalClock)
        .init();
    let res = match data_dir() {
        Ok(dir) => match cmd {
            Cmd::Daemon => daemon(&dir).await,
            Cmd::SetPassword => set_password(&dir),
        },
        Err(e) => Err(e),
    };
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_and_phone() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(parse_args(&s(&[])), Ok(Cmd::Daemon));
        assert_eq!(parse_args(&s(&["set-password"])), Ok(Cmd::SetPassword));
        assert!(parse_args(&s(&["x"])).is_err());
        assert!(valid_phone("5511999998888"));
        assert!(
            !valid_phone("+5511999998888") && !valid_phone("123") && !valid_phone("55a1199999")
        );
    }

    #[tokio::test]
    async fn cert_roundtrip_and_key_mode() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = std::env::temp_dir().join(format!("silicon-tls-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (c, k) = ensure_cert(&dir).unwrap();
        let fp = fingerprint(&c).unwrap();
        assert_eq!(fp.len(), 32 * 3 - 1);
        assert_eq!(fingerprint(&ensure_cert(&dir).unwrap().0).unwrap(), fp); // reuso
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&k).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        axum_server::tls_rustls::RustlsConfig::from_pem_file(c, k)
            .await
            .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
