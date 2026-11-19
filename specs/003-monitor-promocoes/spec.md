# Spec 003 — Monitor de promoções (alerta de bug)

Status: **rascunho para alinhar** · Depende de: 001 · Constituição v0.2.0 (emendas P1/P2 aplicadas)

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

## Decisões a confirmar
1. **Detecção**: regras/regex no MVP (grátis, privado, editável). LLM fica fora.
2. O risco de ban é maior com número principal + monitor 24/7 + encaminhamento. Recomendo
   migrar para o chip secundário quando possível (troca de sessão, sem mudar código).

## Perguntas em aberto
- O alerta deve **encaminhar** (aparece "encaminhada") ou reenviar como texto novo com
  autor e regra? Recomendo reenviar (controla o formato e o `@todos`).
- Em grupo, `@todos` exige o bot saber os participantes; validar na API da lib.
