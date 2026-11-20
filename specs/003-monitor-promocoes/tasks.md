# Tarefas 003 — Monitor de promoções

Legenda: **[WA]** só com WhatsApp real · **módulo** = tarefa isolada em arquivo próprio ·
**integração** = junta as peças. Build `-j1`. Pré-requisito: spec 001 pronta. Sem ligação, sem feature `voip`.

## Onda A — pura e paralela (arquivos diferentes, sem build entre si)
**T1. [x] Dependências** · módulo — `regex` 1.13.1 e `unicode-normalization` 0.1.26 no
`Cargo.toml`. Depende: 001/T1.

**T2. [x] Detector** · módulo — `src/detector.rs`: `normaliza` (NFD, sem marcas, minúsculas),
`avalia`, lista de ignorar, `RegexBuilder` com `size_limit`/`dfa_size_limit`, regra padrão
`\bbug(s|ad[oa]s?|ou|ar|ando|uei|zao|zinho|ue[im])?\b`. Testes: casa bug, bugs, bugado,
BUGADA, bugados, bugou, bugar, bugando, buguei, bugzão, bugzinho; não casa debug, debugar,
bugatti, ebuggy; ignorar vence; regra inválida recusada; desativada ignorada. Depende: T1.

**T3. [x] Alerta (puro)** · módulo — `src/alerta.rs`: `Dedupe` (hash de texto/1º link sem query,
janela 30 min), `Limite` (10/h), `em_silencio` (cruza meia-noite), `formata` (snapshot do
formato com `@`). Testes. Depende: T1.

**T4. [x] Config v2** · módulo — `src/config.rs`: tabelas `rules` (com a regra "bug"), `ignore`,
`alert_log` (resumo ≤ 80 chars, sem mídia), settings (`dedupe_min`, `alertas_hora`, silêncio,
`retencao_dias`=7), CRUD e purge (partida + a cada hora). Testes. Depende: 001/T5.

**Build 1** (integração): fmt + clippy + test `-j1`.

## Onda B — painel e integração (paralelas entre si, sem build)
**T5. [x] Painel `/rules` e `/alerts`** · módulo — `src/panel.rs` + campos novos em `src/state.rs` (`detector: RwLock<Arc<Detector>>`, `dedupe`, `limite`; **só T5 edita `state.rs`**, T6 só lê) e os 4 settings em `/settings`. CRUD de regras com ativar/desativar,
**testar texto contra as regras**, compilar e trocar `Arc<RwLock<Detector>>` sem reiniciar;
histórico. Testes `oneshot`. Depende: T2, T3, T4.

**T6. [x] `monitor::trata`** · integração — `src/monitor.rs` + gancho `Action::Monitor`: texto de
`text_content()`/`get_caption()`; dedupe → limite → silêncio; participantes do grupo `alerta`
via `fetch_metadata` (cache 10 min) para `@`; envio **só** por `client.send_message(jid_alerta)`
(texto ou `media_with_caption` reaproveitando o CDN); 2 retentativas (5 s, 30 s); atraso P8;
nada de read receipt/reação; sem chat `alerta` → registra e não envia. Teste: destino ≠ origem
e `monitorado` nunca é destino. Depende: T2, T3, T4 e os campos de `AppState` do T5 (T5 escreve `state.rs` primeiro; T6 integra depois).

**Build 2** (integração) após T5–T6.

## Onda C — validação real
**T7. [WA] Validação** · integração — alerta chega com `@todos` e **notifica**; menção sem `@`
no texto notifica?; `media_with_caption` dias depois (CDN expirado → plano B: baixar e
reenviar); LID vs PN dos participantes; latência < 5 s; nada é enviado ao grupo monitorado.
Depende: T6.

**T8. Plano B de mídia (se T7 falhar)** · módulo — `download` (limite 16 MiB) → `upload` →
nova mensagem; se falhar, só texto. Depende: T7.

## Paralelismo
Livres (sem build): T2, T3, T4 juntos; depois T5 e, quando `state.rs` estiver pronto, T6. Serial: Builds 1–2 (`-j1`).
Recomendação: migrar para o chip secundário **antes** de deixar o monitor rodando 24/7.

## Fechamento
**T9. Docs** — marcar tasks, anotar resultado do [WA] na spec 003, atualizar `docs/contexto.md`
se algo mudar, rodar `/gerar-prompt`. Depende: T7 (e T8 se houve).

Independente desta spec: **005/T5 (métricas RAM/CPU no Status)**, em `specs/005-painel-pareamento/tasks.md`.
Mexe em `panel.rs` e `state.rs` como o T5 daqui: fazer em onda separada (nunca em paralelo).

## Pendente (20/11/2026)
T7 [WA], T8 e T9 abertos: exigem o dono presente. **Antes de implantar no notebook**, conferir quais chats têm papel `monitorado`/`alerta` (o binário novo liga o monitor assim que os dois existirem) e decidir sobre o chip secundário.
