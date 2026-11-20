# Spec 006 — Release e binários (GitHub Actions)

Status: **aprovada pelo dono em 2026-11-20** · Depende de: 001

## Por quê
Facilitar o setup do daemon 24/7: baixar um binário pronto em vez de compilar no servidor.

## Requisitos
- RF1 Workflow `.github/workflows/release.yml`: ao empurrar tag `v*`, compila `--release` em
  Linux x86_64 e publica no GitHub Release: `silicon-linux-x86_64` (binário),
  `silicon-x86_64.AppImage` e `SHA256SUMS`.
- RF2 Workflow de CI em push/PR: `fmt`, `clippy -D warnings`, `test`.
- RF3 Windows `.exe`: **não suportado agora** — o código usa `memfd_create`, `localtime_r`, `stty` e
  `libc` (Unix). Portar é tarefa futura própria (fora desta spec).
- RF4 O binário não embute `ffmpeg` (dependência de sistema, P7/licença); o AppImage também não.
- RF5 Nunca segredos no workflow; só `GITHUB_TOKEN`.
