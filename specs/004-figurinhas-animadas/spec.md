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

### Fases B e C — `.was`/Lottie: **REPROVADAS e removidas do código (23/11/2026)**
Spike no número principal: o `.was` gerado por nós (Lottie com N frames, sem `trust_token`)
chegou com ~3 MB mas **não abriu** (celular: "não é possível exibir os dados da figurinha";
desktop: nada). Na referência (`ideias/`) o `animation.json` é uma animação da Meta com
`trust_token` válido e só o `animation_secondary.json` é trocado; reproduzir isso exigiria
embutir arquivo da Meta (contra P9/esta spec), e o `.was` do vídeo de teste (3 MB) já era
~6x o WebP (482 KB). Decisão do dono: reprovar e tirar tudo. `was.rs`, `to_frames`,
`send_sticker(lottie)` e a opção `was` foram apagados.

### Fase C' — modo de qualidade do vídeo: `silicon` | `malvin` (implementada)
Toggle em Ajustes (`video_sticker_mode`, padrão `silicon`; valor antigo `was` cai em `silicon`).
- `silicon`: 512x512, escada fps 20→8 x q 50→0 (ver Fase A).
- `malvin` (inspirado no MALVIN-XD, que usa 320 px e 15 fps): reduz para 320 e volta a 512
  (canvas 512x512 mantido), 15 fps, q 75→0. Mais leve, mais borrado.
- Os dois: 10 s inteiros, nunca encolhem a tela, mesmo teto de 500 KB, mesmo log `fps q`.

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

## Implementado (23/11/2026; falta [WA] da escolha do modo)
- Fase A: `convert.rs` 512 fixo, `-t 10`, escada medida com vídeo real (13,7 s, 464x832, 60 fps): fecha em 15 fps q0 (482 KB). Modo `malvin`: 458 KB. `hqdn3d` não ajudou.
- Fase C': ver acima. Porta padrão do painel 8787.

## Resultado do spike `.was`
Reprovado (ver Fases B e C).

## Resposta rápida e variações (23/11/2026)
- RF-R1: ao começar a trabalhar num `/s` (fonte válida, antes do download), o bot reage ao comando com um emoji sorteado de 🚰 ⤵️ ⤴️ ↪️ ↩️ 🔄 🔃 🔜. Vem depois do atraso de 1–2 s (P8); é resposta ao comando, não iniciativa (P1). Falha na reação é ignorada.
- RF-R2: toda resposta de texto do bot tem pelo menos 5 variações de mesmo sentido (inclui "Formato não suportado" e as respostas de `/c`).
- RF-R3: imagem tem fila de ffmpeg própria (2 por vez) e não espera vídeo em conversão; o log traz `dl_ms`, `conv_ms` e `up_ms` (sem conteúdo) para achar onde está a demora.
