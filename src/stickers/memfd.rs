//! T7: arquivo só em RAM (`memfd_create`) para o ffmpeg ler vídeo seekable sem tocar o disco.
use std::fs::File;
use std::io::{self, Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// Fd anônimo (sem caminho no disco). Fecha no `Drop`.
pub struct MemFile {
    file: File, // dono do fd; fecha no Drop
}

impl MemFile {
    /// Cria o memfd SEM `MFD_CLOEXEC` (o ffmpeg filho precisa herdar), grava `bytes` e
    /// volta o offset para 0.
    pub fn new(bytes: &[u8]) -> io::Result<MemFile> {
        // SAFETY: syscall sem ponteiros mutáveis; o nome é um C-string literal válido.
        let raw = unsafe { libc::memfd_create(c"silicon".as_ptr(), 0) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` é um fd novo e válido, que ninguém mais possui.
        let mut file = File::from(unsafe { OwnedFd::from_raw_fd(raw) });
        file.write_all(bytes)?;
        file.rewind()?;
        Ok(MemFile { file })
    }

    /// Caminho para passar ao ffmpeg como `-i`: `/proc/self/fd/N`.
    pub fn path(&self) -> String {
        format!("/proc/self/fd/{}", self.file.as_raw_fd())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nomes(dir: &str) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    #[test]
    fn le_de_volta_pelo_caminho() {
        let m = MemFile::new(b"ola silicon").unwrap();
        assert_eq!(std::fs::read(m.path()).unwrap(), b"ola silicon");
    }

    #[test]
    fn fd_fecha_no_drop() {
        let m = MemFile::new(b"x").unwrap();
        let p = m.path();
        drop(m);
        assert!(std::fs::metadata(p).is_err());
    }

    #[test]
    fn nao_toca_o_disco() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let antes = (nomes(&tmp), nomes("/dev/shm"));
        let m = MemFile::new(b"dados").unwrap();
        let _ = std::fs::read(m.path()).unwrap();
        drop(m);
        assert_eq!(antes, (nomes(&tmp), nomes("/dev/shm")));
    }
}
