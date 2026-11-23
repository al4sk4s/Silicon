# Spec 004 — Figurinhas animadas melhores (WebP fluido + bypass `.was`/Lottie)

Status: **aprovada pelo dono em 23/11/2026 (fases A, B e C; testes no WhatsApp real ficam para o fim)** · Depende de: 002

## Por quê
Hoje a animada (spec 002) sofre de dois problemas, relatados pelo dono:
1. **Picota** (pouco fluida).
2. **Às vezes sai menor que 512x512** (testado com um vídeo).
E existe uma ideia em `ideias/bot-figurinhas-main` (bypass do formato `.was`, Lottie) que
pode dar figurinhas animadas melhores. Esta spec trata os três pontos, em **fases**.

## Achados da análise (leitura do código + `ideias/`)

### A. Por que sai menor (bug real, `src/stickers/convert.rs`)
`ANIM_LADDER` tem o último degrau `(25, 8, 4, 384)`. `filter(side)` faz `scale=384:384` e
depois `pad=512:512` → o conteúdo fica **384 centralizado numa tela transparente 512**,
ou seja, a figurinha "encolhe" com bordas vazias. Vídeo pesado/longo cai nesse degrau.
Isso contraria o RF4 da spec 002 (512x512 sempre). Os degraus de `scale` não podem
existir: a tela e o conteúdo ficam **sempre 512x512**; a escada só mexe em
qualidade/fps/duração.

### B. Por que "picota" = o vídeo é CORTADO (confirmado pelo dono, 23/11/2026)
Relato: "não picota, ele corta o resto do vídeo"; `travar.mp4` (10 s) ficou só com 2–4 s
iniciais e com bordas transparentes. Causa: a escada reduz **duração** (10→8→6→4 s) e o
último degrau encolhe (achado A). Conclusão: **a duração do vídeo (até 10 s) nunca pode
ser cortada para caber no tamanho**; cede-se fps/qualidade. (Texto da análise inicial:) Primeiro degrau já é 15 fps; a escada cai para 12 → 10 → 8 fps e `q` 60→25, e
`-compression_level 3`. Vídeo de 6–10 s com movimento estoura 500 KB e acaba em
10 fps/q30, que é o que o dono vê. Isso é limite do **WebP animado ≤ 500 KB**, não bug.

### C. O que a ideia `.was` faz (`ideias/bot-figurinhas-main`)
- `.was` = **ZIP** (entradas em `deflate`) com a pasta
  `animation/`: `animation.json` (Lottie 512x512, 60 fps), `animation_secondary.json`
  (Lottie com a **imagem em base64** num `assets[].p`), `*.trust_token` (JWT ES256, tem o
  `sha256` do JSON e `trusted_origin: whatsapp`) e `animation.json.overridden_metadata`
  (JSON: `sticker-pack-id/name/publisher`, `emojis`, `accessibility-text`,
  `is-from-user-created-pack`).
- O exemplo **só troca a imagem** dentro de um Lottie pronto da Meta (animação "Chomp"
  em cima de uma foto). **Não converte vídeo.** Os `trust_token` ficam os do original
  (o `sha256` já não bate com o JSON alterado; não sabemos se o app confere).
- Envio (Baileys): `sticker` com `mimetype: application/was`, `isLottie: true`,
  `isAnimated: true`. A lib Rust **tem** o campo: `StickerMessage.isLottie = 21`
  (`waproto/src/whatsapp.proto:4805`). O upload usaria `MediaType::Sticker` (a conferir).
- Origem: repositório de terceiros **sem licença declarada** ("mantenha os créditos").
  Pela constituição **P9**: não copiar código; só reescrever a ideia. O Lottie-base com
  trust tokens vem da Meta: **não vai para o repositório** (só estudar).

### D. Limite honesto da ideia para vídeo
Lottie não tem faixa de vídeo. Para tocar um vídeo como `.was` teríamos de embutir **um
frame por asset base64** (e ligar/desligar camadas), o que provavelmente fica **mais
pesado** que WebP animado. **Não há evidência de que o limite de tamanho do `.was` seja
maior, nem de que o app aceita Lottie com dezenas de imagens.** Por isso é um **spike**
(Fase B), não uma promessa.

## Fases

### Fase A — WebP animado certo (não depende do bypass; vale já)
- RFA1 Saída **sempre 512x512 exatos**, sem degrau de escala e sem `pad` que encolha o
  conteúdo (mídia esticada para o quadrado, como decidido na spec 002). Teste: nenhum
  degrau da escada altera 512; saída medida = 512x512.
- RFA0 **Nunca cortar duração** abaixo do vídeo (teto 10 s do WhatsApp; só vídeo > 10 s é
  cortado). Teste: vídeo de 10 s sai com 10 s. Se nem no piso de q/fps couber em 500 KB,
  responde "mídia pesada demais" em vez de entregar um pedaço.
- RFA2 Escada nova só em `q` e `fps` (duração e escala ficam fixas), **começando mais alta em fluidez** (ex.
  20–24 fps) e **só baixando fps depois de esgotar `q`**; manter teto 500.000 B,
  ≤ 10 s, timeout por tentativa e `-compression_level` menor. Valores exatos saem de
  **medição** (vídeos de teste reais do dono), não de chute.
- RFA3 `to_animated` devolve o que ficou (fps/q finais) em log (`evento`, sem conteúdo)
  para ver quando degrada.
- RFA4 Spec 002 RF4 atualizada com a escada nova no mesmo passo.

### Fase B — Spike do `.was` (decide se vale a Fase C)
Experimento **manual no WhatsApp real**, sem produção:
- RFB1 Gerar `.was` do jeito do exemplo (reescrito, sem copiar código) com um Lottie
  mínimo **escrito por nós** (uma camada de imagem), enviar com `isLottie=true`,
  `isAnimated=true`, `mimetype application/was`, e ver se Android **e** iOS tocam.
- RFB2 Testar **sem** `trust_token` e com token inválido: o app aceita? (decide se
  precisamos do JWT, que não conseguimos gerar: é assinado pela Meta.)
- RFB3 Testar Lottie com **N frames como assets** (ex. 30 e 90 frames JPEG/WebP): qual
  tamanho máximo o WhatsApp aceita? Fluidez vs. WebP animado do mesmo vídeo?
- RFB4 Registrar resultados em "Resultado do spike" nesta spec. **Critério de corte:** se
  exigir `trust_token` válido, ou só aceitar com Lottie da Meta, ou o frame-a-frame ficar
  maior/mais lento que o WebP → **Fase C é cancelada** e fica só a Fase A.

### Fase C — modo de conversão do vídeo: `webp` ou `was` (só se o spike passar)
Decisão do dono (23/11/2026): em vez de comando separado, **toggle de modo de conversão
do vídeo→figurinha** com duas opções, `webp` (padrão) e `was`.
- RFC1 Config `video_sticker_mode` (`webp`|`was`), no painel em **Ajustes**, valor padrão
  `webp`; vale para o `/s` de vídeo/GIF. Imagem estática não muda.
- RFC2 Modo `was`: fallback automático para WebP (Fase A) se a conversão falhar ou passar
  do limite do `.was`; o usuário não vê erro por isso.
- RFC3 Mesmo limite por chat, atraso 1–2 s (P8), só chat `comando` (P1/P2).
- RFC4 Se o spike reprovar, o painel **não** mostra a opção `was`.

## Constituição — revisão
- P1/P2/P8: igual à 002 (só responde, só chat `comando`, atraso, rajada).
- P7: Fase A não adiciona dependência. Fase B/C: zip com `zip` crate (ver licença) ou
  escrever o ZIP à mão; decidir no plano.
- P9: nenhum código de `ideias/` é copiado; Lottie-base da Meta não entra no repo.
- **Risco de banimento:** mandar `.was` fabricado é **fora do uso normal do cliente**
  (burla um bloqueio do app). Risco **maior** que o de WebP comum, e o dono
  decidiu testar no **número principal** (contrário à recomendação de `docs/contexto.md`).
  Mitigação: spike com poucas mensagens, espaçadas, 1 só envio por teste; modo `was`
  desligado por padrão.

## Fora do escopo
Pacotes/packs, edição, animação "Chomp" em foto (o exemplo do `ideias/`), gerar JWT, Lottie
vetorial a partir de vídeo.

## Perguntas em aberto (para o dono)
Respostas do dono (23/11/2026): (1) aprovou A, B e C em sequência, planejar+implementar,
testar no fim; (2) spike no **número principal** do dono (risco de ban assumido por ele; ver
Risco); (3) vídeo `travar.mp4`: bordas transparentes e só 2–4 s iniciais; (4) o problema é o
corte da duração, não fluidez; (5) toggle `webp`/`was` no painel (RFC1).

Perguntas originais:
1. Aprova começar pela **Fase A** (corrige "menor" e "picota" sem o bypass) e só depois o spike?
2. O spike pode rodar no número de teste (risco de ban maior que WebP)? Qual número?
3. Qual vídeo você usou quando "ficou menor"? (duração/tamanho) — vira vídeo de teste da Fase A.
4. O que "picota" para você: poucos frames, travadas no início/fim do loop, ou qualidade
   borrada? (muda o ajuste da escada)
5. Na Fase C, o comando é separado (`/sl`) ou o bot escolhe sozinho?

## Critérios de aceite
- Fase A: vídeo pesado de 10 s sai **512x512** e ≤ 500 KB; nenhum degrau encolhe o
  conteúdo; fps final ≥ 15 nos vídeos de teste do dono, ou o log diz por quê.
- Fase B: tabela "aceita? Android/iOS" para cada teste RFB1–RFB3 preenchida.
- Fase C: só existe se a B passar.

## Implementado em código (23/11/2026; falta [WA])
- Fase A: `convert.rs` com 512 fixo, `-t 10`, escada só fps (24/20/15/12) x q (60..25); log `figurinha animada fps q`.
- Fases B/C juntas: `src/stickers/was.rs` (ZIP stored + Lottie, um JPEG por frame, sem trust_token, validado com `zipfile` do Python), `convert::to_frames` (10 fps, até 10 s), toggle `video_sticker_mode` em Ajustes (padrão `webp`), `send_sticker` com `application/was` + `isLottie`. Falha no `.was` cai no WebP sem aviso. Teto provisório `WAS_MAX` 3 MB (shortcut).
- RFC4 (esconder a opção se o spike reprovar): ainda não; hoje a opção aparece sempre, marcada "experimental". Tirar depois do B5.

## Resultado do spike
(vazio; testes [WA] A6, B5, C5 pendentes: usar `/s` em `travar.mp4` com modo webp e depois modo was; anotar Android/iOS, tamanho, se anima)
