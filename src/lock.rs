//! Lock de instância única: impede dois bots usando a mesma sessão.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

/// Abre/cria o arquivo e toma o lock exclusivo. O chamador deve manter o
/// `File` vivo enquanto o bot roda; o lock some quando ele é fechado.
pub fn acquire(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let file = opts.open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "outra instância do Silicon já está rodando",
        )),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_until_first_is_dropped() {
        let dir = std::env::temp_dir().join(format!("silicon-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("silicon.lock");

        let first = acquire(&path).unwrap();
        let err = acquire(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

        drop(first);
        assert!(acquire(&path).is_ok());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
