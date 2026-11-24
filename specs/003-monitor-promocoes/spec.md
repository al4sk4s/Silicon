# Spec 003 — Monitor de promoções (alerta de bug)

Status: **implementada (T1–T6, 2026-11-20); falta validar no WhatsApp real (T7 [WA])** · Depende de: 001, 004 · Constituição 1.1.0 (sem emenda nova)

## Por quê
No grupo de promoções saem "bugs de preço". O dono quer ser avisado na hora, sem ficar
olhando o grupo.

## Histórias
- O bot lê o grupo de promoções (papel `monitorado`) em silêncio.
- Quando uma mensagem parece um **bug**, o bot a **encaminha** para o grupo privado do dono
  (papel `alerta`) marcando **@todos** (o dono recebe notificação).
- (Ligação: **removida** do escopo por decisão do dono; proibida pela P1.)

## Requisitos funcionais
- RF1 Em chats `monitorado`: **só lê**; nunca responde, reage ou marca como lido além do
  que a lib faz sozinha.
- RF2 Detecção por **regras editáveis no painel** (regex sobre texto normalizado: minúsculas,
  sem acento) e lista de ignorar. Cada regra tem nome; o alerta diz **qual regra** casou.
  **Regra padrão "bug"** (o MVP começa só com ela): palavra inteira `bug` e derivações —
  `bug`, `bugs`, `bugado/a/os/as`, `bugou`, `bugar`, `bugando`, `buguei`, `bugzao` etc.
  Ex.: `\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao)?\b`. Não casa `debug`/`debugar`.
  Também olha a **legenda** de imagens/vídeos; o texto dentro da imagem (OCR) fica fora.
- RF3 Ao casar, encaminha para o chat `alerta`: o texto/mídia original + autor + horário +
  regra casada, com `@todos`. Mantém mídia (print/foto) quando houver.
- RF4 **Deduplicação**: mesma mensagem/link não alerta duas vezes numa janela configurável.
- RF5 Limite de alertas por hora (anti-rajada) e **modo silêncio** por horário.
- RF6 Histórico mínimo no painel: hora, regra e resumo de 1 linha; **sem guardar** mídia.
  Retenção curta e configurável (padrão 7 dias).
- RF7 O alerta é **só mensagem** (sem ligação, sem feature `voip`).
- RF8 Detector em módulo isolado com interface simples (`fn avalia(msg) -> Option<Regra>`),
  para trocar por outro método depois sem tocar no resto.

## Não funcionais
- Latência do alerta < 5 s do recebimento.
- Sem envio do conteúdo das promoções a serviços externos no MVP.

## Fora do escopo
LLM/serviço externo, scraping de lojas, comparação de preço, resposta no grupo monitorado.

## Critérios de aceite
- Mensagem que casa regra chega no grupo de alerta com @todos e regra citada.
- Mensagem repetida não gera segundo alerta dentro da janela.
- Nada é enviado para o grupo monitorado.
- Painel permite criar/editar/desativar regras sem reiniciar.

## Constituição
Esta spec usa a **exceção da P1** (alerta ao dono, só para o chat `alerta`) e os papéis da
P2 (`monitorado`, `alerta`). Respeita a P8 com dedupe e limite de alertas (RF4/RF5).

## Decisões do dono (2026-11-20)
1. **O que é "bug":** só a palavra `bug` e derivações (regra padrão, RF2). Mais regras depois,
   criadas pelo painel. Sem regra de "preço absurdo" no MVP.
2. **Formato:** **mensagem nova** (não "encaminhada") com `@todos`, como no RF3.
3. **Limites:** padrões do rascunho: dedupe 30 min, 10 alertas/h, sem silêncio por padrão
   (configurável), histórico de 7 dias só com resumo (RF4–RF6).
4. Risco de ban maior com número principal + monitor 24/7: recomendado chip secundário (troca
   de sessão, sem mudar código).

## Convivência com o estado atual do código
- Hoje `decide` devolve `Action::Monitor` para `monitorado` e `wa.rs` descarta (gancho vazio).
  A 003 só preenche esse gancho: o bot continua **mudo** no chat monitorado (nunca `reply`,
  reação nem read receipt); o envio vai só ao chat `alerta`, lido na hora do envio.
- `alerta` continua `Action::Drop` (só destino). Comando do dono em `monitorado`/`alerta` segue
  ignorado (004 RF2); o papel desses chats só muda pelo painel.
- Sem chat `alerta` configurado: nada é enviado; o painel registra o evento.

## Perguntas em aberto (resolver no plano/[WA])
- `@todos` exige participantes via `fetch_metadata` e menção em `mentioned_jid`; se a menção
  "oculta" não notificar, o texto leva `@<user>` por pessoa (grupo pequeno).
- Mídia reaproveitada do CDN pode expirar; plano B: baixar e reenviar, senão só texto.
