# Plano 005 — Figurinhas animadas com som (o COMO)

Status: rascunho · Spec: `specs/005-figurinhas-som/spec.md` (aprovada 2026-11-24) · Depende de 001 e 002 ·
Constituição v1.0.0 · whatsapp-rust `eeb24da` (fixo, **sem atualizar o pin**: nada do que usamos mudou no upstream).

## Decisões a confirmar
1. **Onde vai o áudio: `StickerMessage.audioMessage` (campo 26).** O proto da lib tem
   `oneof audio { Message.AudioMessage audioMessage = 26 }` dentro do `StickerMessage`. O campo 134
   (`Message.audioStickerMessage`, `FutureProofMessage`) é só um envelope genérico. Nada na lib ou
   no upstream diz como o WhatsApp real recebe; **padrão: campo 26**, validado no T-WA. Se o som não
   tocar, plano B é embrulhar em 134 (um `match` de 3 linhas, isolado em `build_sticker`).
2. **`/rename` mantém o som** reaproveitando o `audio_message` que já vem no sticker citado (mesmos
   `url/media_key/sha`, sem baixar nem reenviar o áudio). Se o campo vier vazio, vira sticker mudo
   como hoje. Alternativa: aceitar a perda do som no `/rename`.
3. **`MediaType::Audio`** no upload (não `Ptt`), `ptt=false`. Chute; ajustar no T-WA.
4. **`seconds` do áudio = teto da tentativa vencedora** (não a duração real). Atalho com
   `shortcut:`; melhorar lendo a duração do ogg se o WhatsApp exibir errado.
5. **Aviso de falha do áudio (RF6)** é uma 2ª linha no chat. Uma só por comando, só se a mídia tinha
   som e falhou; mídia sem áudio nunca avisa.

## Fatos da lib (lidos no código, `eeb24da`)
- `Client::upload(Vec<u8>, MediaType, UploadOptions) -> Result<UploadResponse>` (`src/upload.rs:441`);
  `MediaType::Audio` em `wacore/src/download.rs:94`.
- `media::audio_message(upload, AudioOptions) -> wa::Message` (`src/media.rs:304`): preenche url,
  direct_path, media_key, shas, length, timestamp; `AudioOptions` (`media.rs:63`): `mimetype`
  (padrão `audio/ogg; codecs=opus`), `duration_seconds`, `ptt`, `waveform`, `context_info`.
- Não há helper de sticker com áudio e `send_message` não valida o conteúdo. O stanza do sticker
  é classificado só por `sticker_message.is_set()` (`wacore/src/send/classify.rs:405`): ok.
- O WhatsApp controla por AB prop de servidor (`SOUND_STICKERS_CREATION_ENABLED` etc.,
  `wacore/src/iq/abprops.rs:12026`): **a conta do bot pode não ter a função liberada** e o servidor
  ignorar o áudio sem erro. Risco real, só o T-WA mostra.
- Nome Rust do campo (oneof gerado no build) **não conferido**: ler no `cargo doc`/código gerado ao
  escrever T3; pode ser `audio_message` direto ou `Audio::AudioMessage(..)`.

## Módulos tocados
- `src/stickers/convert.rs`
  - `to_animated` passa a devolver `Anim { webp: Vec<u8>, secs: u32 }` (hoje só `Vec<u8>`), onde `secs`
    é o `-t` da rung vencedora (`ANIM_LADDER`, `convert.rs:24-31`). Chamadores atualizados.
  - `extract_audio(bytes, secs) -> Result<Option<Vec<u8>>, ConvErr>`: ffmpeg
    `-map 0:a:0? -vn -t <secs> -c:a libopus -b:a 32k -ac 1 -f ogg pipe:1`. Sem faixa de áudio o
    `?` gera saída vazia → `Ok(None)` (sem ffprobe). Mesmo timeout/semáforo do ffmpeg da 002.
- `src/stickers/mod.rs`
  - Fluxo de vídeo/GIF: `to_animated` → `extract_audio(.., min(secs, sticker_sound_max_s))` (só se
    `sticker_sound` ligada) → `send_webp(.., audio: Option<SoundAudio>)`.
  - `send_webp` (`mod.rs:371-418`): se há áudio, `client.upload(ogg, MediaType::Audio, ..)`, monta com
    `media::audio_message(..)` e move o `AudioMessage` interno para o campo de áudio do
    `StickerMessage` (`mod.rs:395-409`). Falha no upload → envia sem som + aviso (decisão 5).
  - `/rename` (`mod.rs:335-368`): copia o `audio_message` do sticker citado para o novo (decisão 2).
  - Função pura `build_sticker(upload, exif.., audio: Option<AudioMessage>) -> StickerMessage` para
    testar sem rede.
- `src/config.rs`: `sticker_sound() -> bool` (padrão **true**; `get("sticker_sound") != "0"`) e
  `sticker_sound_max_s() -> u32` (1..10, padrão 10; modelo `sticker_per_min`, `config.rs:125-138`).
- `src/panel.rs`: em Ajustes (`settings_page`, `:318-342`) um checkbox e um `number`; em
  `settings_post` (`:348-411`) validar `sticker_sound_max_s` **antes** da cadeia de `set`
  (400 sem gravar nada); checkbox ausente do form = "0".
- `specs/002-figurinhas/spec.md`: nota de que RF "áudio descartado" (`-an`) foi substituído pela 005.
- Sem tocar: `Cargo.toml` (zero crate novo, P7), `state.rs`, `wa.rs`, `main.rs`.

## Tipos
```rust
struct Anim { webp: Vec<u8>, secs: u32 }
struct SoundAudio { ogg: Vec<u8>, secs: u32 }   // só em memória, nunca em disco
```

## Fluxo (vídeo/GIF citado com `/s`)
`decide` (papel `comando`) → delay 1–2 s → baixa mídia → `to_animated` → (flag ligada)
`extract_audio` → `Some` ? upload do áudio : nada → upload do webp → `send_webp` com `audioMessage`
→ envia citando o comando. Uma mensagem só (P8). Falha do áudio = sticker mudo + 1 linha.

## Config e padrões seguros
`sticker_sound=ligada` (o fallback torna isso seguro), `sticker_sound_max_s=10` (teto da escada da 002;
os "5 s" dos vazamentos são do beta, não confirmados). Desligar a flag = comportamento da 002.

## Riscos
- **Função não liberada para a conta / caminho errado (26 vs 134):** sticker chega mudo, sem erro.
  Mitigação: T-WA decide o caminho antes de dar por pronto; flag para desligar.
- **Ban:** baixo (1 envio, formato normal de mensagem); campo atípico pode chamar atenção. Sem rajada.
- **Privacidade:** o áudio é da mídia que o próprio usuário citou; fica só em memória (RF5 da 002, nada
  em disco), nunca em log.
- **RAM/tempo:** ogg de 10 s mono 32 kb/s ≈ 40 KB; um ffmpeg a mais (~1 s) no semáforo existente.
- **Mudança de proto na lib:** pin fixo; nome do campo conferido no build.

## Testes
- `cargo test`: `extract_audio` com vídeo gerado por `ffmpeg lavfi` (com e sem faixa de áudio →
  `Some`/`None`; `#[ignore]`/pulado sem ffmpeg, como na 002); `build_sticker` com áudio faz roundtrip
  prost e tem o campo 26; config (faixa 1..10, padrão); `settings_post` salva as duas chaves e
  rejeita fora da faixa sem gravar; função pura de fallback (áudio falhou → `None` + aviso).
- Só WhatsApp real (T-WA): som toca no Android/iOS, ícone de volume, vídeo mudo igual à 002,
  áudio de 10 s aceito (ou cortado?), `/rename` mantém som, flag desligada, falha forçada → mudo + aviso,
  conta sem a função (mudo sem erro).

## Passos (quem faz)
| # | Passo | Quem | Depende |
|---|---|---|---|
| A | `config.rs` (2 chaves) + teste | worker | — |
| B | `panel.rs` (checkbox, number, validação) + teste | worker | A |
| C | `convert.rs`: `Anim`, `extract_audio` + testes | worker | — |
| D | `mod.rs`: `build_sticker`, `send_webp`, `/rename`, fallback | eu | A, C |
| E | build `-j1` (fmt, clippy, test) e revisão P1/P7/P8 | eu | B, D |
| F | validação [WA] | dono + eu | E |
Paralelo sem build: A, C. Build um por vez (`-j1`).
