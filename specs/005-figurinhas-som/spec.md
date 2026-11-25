# Spec 005 — Figurinhas animadas com som

Status: **aprovada pelo dono em 2026-11-24** · Depende de: 001 (núcleo), 002 (figurinhas)

## Por quê
O WhatsApp está introduzindo "sound stickers": figurinha com ícone de volume que toca um
áudio curto ao toque. Ao converter um vídeo em figurinha, o app anexa até ~5 s do áudio
original. Hoje o `/figurinha` (002) descarta o som. Queremos que o bot gere a versão com som.

## Situação da tecnologia (verificada em 2026-11-24)
- **WhatsApp:** só vista em beta Android (v2.26.41.x, teardown/leaks); sem lançamento oficial
  confirmado e sem iOS reportado. O formato final pode mudar. *(Fontes: Android Authority,
  WABetaInfo.)*
- **Lib `whatsapp-rust` pinada (`eeb24da`)**: o proto já tem `StickerMessage.audioMessage = 26`
  (oneof `audio`) e `Message.audioStickerMessage = 134` (wrapper `FutureProofMessage`). Ou
  seja, **dá para montar e enviar via proto cru**, mas **não há helper** e o formato de fio
  real é não confirmado. O upstream (82 commits à frente) só ajustou o *recebimento*
  (#2110: desembrulha `audioStickerMessage`); nada de envio.
- Conclusão: **viável em tese, não validado**. Exige experimento no WhatsApp real (T-WA).

## Histórias
- Respondo um vídeo/GIF **com áudio** com `/s`, `/fig` ou `/figurinha` → o bot devolve figurinha
  animada **com som**, sem modificador: é o comportamento padrão da 002, só que agora o áudio vai junto.
- Respondo um vídeo **sem áudio** → figurinha animada normal (como na 002), sem aviso extra.
- O destinatário não tem a função → vê a figurinha animada sem som (degradação esperada).

## Requisitos funcionais
- RF1 **Sem comando novo**: `/s`, `/fig`, `/figurinha` (e `/rename`, que reenvia figurinha) passam a
  anexar áudio sempre que a mídia de origem tiver áudio. Mídia estática não muda.
- RF2 Duração do som = duração final da figurinha (a escada da 002 já limita a **≤ 10 s**; o áudio
  usa o mesmo `-t` da tentativa vencedora). **Teto configurável** `sticker_sound_max_s` no painel
  (padrão **10**). Os "5 s" dos vazamentos são do beta e não estão confirmados; se o WhatsApp real
  recusar áudio longo, baixar o padrão (validar no T-WA).
- RF3 Extrair o áudio com `ffmpeg` (opus em ogg, mono, mesmo padrão de voice note); sem faixa de
  áudio na origem → segue sem som.
- RF4 Subir o áudio via `Client::upload` (`MediaType::Audio`) e o webp animado como na 002; montar
  `StickerMessage` com `audioMessage` preenchido (proto cru; sem helper na lib).
- RF5 Mesmo papel `comando`, atraso 1–2 s, limiter, semáforo do ffmpeg e tamanhos da 002.
- RF6 Falha no áudio (ffmpeg, upload) → **cai para figurinha sem som** e avisa numa linha; nunca
  deixa de responder.
- RF7 Chave `sticker_sound` no painel (Ajustes), **ligada por padrão**; desligada = 002 idêntica.

## Fora do escopo
- Escolher/editar áudio, trocar trilha, pacote de figurinhas com som, receber/baixar sound
  stickers de terceiros (pode virar spec própria).
- Reencodar o vídeo; áudio > 5 s.

## Critérios de aceite
- Vídeo com áudio respondido com `/s` → figurinha com ícone de volume que toca no WhatsApp real (Android com a
  função); vídeo mudo → igual à 002.
- Falha forçada no áudio → figurinha sem som + 1 linha de aviso.
- `sticker_sound` desligada → comportamento idêntico ao da 002.
- Dispositivo sem a função não quebra a conversa (apenas sem som).

## Riscos
- **Formato não oficial**: pode ser rejeitado ou ignorado pelo servidor; sem som no destino.
  Mitigação: fallback para figurinha sem som (RF6) + flag `sticker_sound` para desligar.
- **Mudança de proto**: lib pré-1.0; atualizar o pin só com revisão (CLAUDE.md).
- Banimento por comportamento atípico: baixo (mensagem normal), mas campo "futuro" pode
  chamar atenção; manter 1 envio por comando (P8).

## Conferência com a constituição
P1 ok (só responde a comando) · P2 ok · P7 ok (sem crate novo) · P8 ok (1 envio) · P9 ok.
Sem conflito.

## Decisões do dono (2026-11-24)
Funciona junto do `/s` sem modificador; áudio acompanha a duração da figurinha (≤ 10 s, visto pelo dono).

## Perguntas em aberto
1. O WhatsApp real aceita áudio de 10 s no sticker ou corta em 5 s? (medir no T-WA)
2. Atualizar o pin da lib (inclui #2110, recebimento) antes? Pede revisão da API.
