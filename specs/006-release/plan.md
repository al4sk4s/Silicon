# Plano 006 — Release e binários (GitHub Actions)

Status: implementado (T1–T3 [x]; T4 fora de escopo) · Spec: `specs/006-release/spec.md` · Depende de: 001.

## Decisões (já no código)
1. **`.github/workflows/ci.yml`**: push (todas as branches) e PR, `contents: read`, `ubuntu-latest`, toolchain stable com rustfmt e clippy, `Swatinem/rust-cache`. Passos: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Testes que precisam de ffmpeg são `#[ignore]`.
2. **`.github/workflows/release.yml`**: tag `v*`, `contents: write`, `ubuntu-22.04` (glibc mais antiga), `cargo build --release --locked`. Gera em `dist/` `silicon-linux-x86_64`, `silicon-x86_64.AppImage` (AppDir mínima: `AppRun` symlink para o binário, `.desktop` com `Terminal=true`, ícone PNG 1x1; `appimagetool` *continuous* com `--appimage-extract-and-run`) e `SHA256SUMS`. Publica com `softprops/action-gh-release@v2`.
3. **Segredos e dependências**: só o `GITHUB_TOKEN` implícito (RF5). ffmpeg não vai embutido (RF4).

## Arquivos tocados
`.github/workflows/ci.yml`, `.github/workflows/release.yml`.

## Validação
v0.1.0 (20/11/2026): checksums conferem; binário e AppImage rodam `--help` no notebook Arch (FUSE presente).

## Riscos / fora
- `appimagetool` baixado da tag `continuous`, sem versão nem checksum fixos; actions fixadas por tag, não por SHA. Mudança externa pode quebrar o release.
- Fora: Windows `.exe` (código depende de `memfd_create`, `localtime_r`, `stty`, `libc`).
