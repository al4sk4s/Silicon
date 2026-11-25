# Plano 008 — Ajuda (`/help`) e bio do grupo (`/updatedc`) (o COMO)

Status: rascunho · Spec: `specs/008-ajuda-e-bio/spec.md` · Constituição v1.1.0 · whatsapp-rust `eeb24da` (pin fixo, sem atualizar).

## Decisões a confirmar
1. **`/comandos` vira ajuda para todos** e `/comandos allow|deny|status` segue valendo para o dono. Hoje
   `parse_chat_cmd` (`wa.rs:80`) trata `/comandos` como `/c`. Plano: o ramo de ajuda entra **depois** do ramo
   admin e só pega `/comandos` sem argumento válido do `/c` (o `ChatCmd::Usage` do `/c` sem args não muda).
2. **`/updatedc` roda em chat com papel `comando`, só do dono, só em grupo (`@g.us`).** Em chat sem papel, o
   dono já passa pelo ramo `owner_only` (`wa.rs:206`), mas a spec limita a `comando`; ficam recusados
   (linha curta, sem gravar). O dono tem de ser admin do grupo? **Não: o *bot* (conta do dono) precisa poder
   editar.** Se o grupo restringe edição a admins e a conta não é admin, vira a falha de RF10.
3. **Aviso automático do WhatsApp** ("X mudou a descrição") é inevitável; aceito na spec.

## Fatos da lib (lidos no código, `eeb24da`)
- `client.groups().set_description(jid, Option<GroupDescription>, PreviousDescription) -> Result<(), GroupError>`
  (`src/features/groups.rs:1855`). `PreviousDescription::Resolve` (padrão) lê o token do servidor antes de gravar
  (1 round trip a mais); `GroupError::DescriptionConflict` = o token mudou (409).
- `GroupDescription::new(String) -> anyhow::Result<Self>` valida ≤ **2048** caracteres
  (`wacore/src/iq/groups.rs:22,208`). Exportado em `whatsapp_rust` (`lib.rs:623`).
- `client.groups().fetch_metadata(&jid) -> GroupMetadata` tem `description: Option<String>` e
  `description_id` (`groups.rs:642-644`); `GroupParticipant::is_admin()` (`:833`). Já usado em `monitor.rs:40`.
- Sem helper de "permissão": a falha de permissão vem como `GroupError::Iq` do servidor (`403`/`not-authorized`).
  Não há como saber antes; tentar e classificar o erro.

## Módulos tocados
- **`src/help.rs` (novo, ~150 linhas, sem dependência)**
  ```rust
  pub struct Cmd { name: &'static str, alias: &'static [&'static str], short: &'static str,
                   long: &'static str, owner_only: bool, in_bio: bool }
  pub const CMDS: &[Cmd]   // tabela única (RF4): /s, /rename, /baixar, /help, /c, /updatedc
  pub const NOVIDADES: &[&str]   // seção da bio, editada a cada spec
  pub fn help_text(arg: &str, is_owner: bool) -> String   // resumo ou detalhe; função pura
  pub fn bio_text() -> String                              // função pura, ≤ 2048, trunca só NOVIDADES
  pub fn parse_cmd(text) -> Option<&str>                   // /help, /comandos (arg = resto)
  pub fn parse_updatedc(text) -> bool
  ```
  Cabeçalho do resumo com 5 variações (`variante`, como o resto). Conteúdo dos comandos é fixo.
- **`src/wa.rs`**: dois ramos novos em `on_message` depois do `/baixar` (com o mesmo atraso 1–2 s, P8):
  `/help` e `/updatedc`; e o `/comandos` sem args do `/c` cai no `/help` (decisão 1). `/updatedc` em chat
  `owner_only` (sem papel) é recusado em silêncio, como qualquer outro comando que não é do dono em chat sem
  papel; em `Command { admin: false }` é recusado com uma linha. `owner_only` não ganha `/help`.
- **`src/main.rs`**: `mod help;`.
- **`src/panel.rs`** e **`config.rs`**: nada. Sem chave nova (a lista mora no código, decisão do dono).
- Sem tocar: `Cargo.toml` (zero crate novo, P7), `state.rs` (usa `st.dbg` já existente), `wa.rs` de sessão.

## Fluxo `/updatedc`
`decide` → papel `comando` + é o dono (`admin`) → é grupo? → delay 1–2 s → `fetch_metadata(chat)` (lê
`description` e `description_id`) → `bio_text()`; igual à atual → reage ✅ + "já está atualizada" sem gravar (RF9)
→ `set_description(jid, Some(GroupDescription::new(bio)?), PreviousDescription::from(id))` → ok: ✅.
`DescriptionConflict`: relê **uma** vez e tenta de novo; falhou de novo → erro (RF8, sem laço). Outro erro →
classificar: contém `not-authorized`/`forbidden`/`403` → "sem permissão"; senão "erro do servidor"; cada um com
5 variações de resposta (RF10). Log/Debug só com categoria (`bio_atualizada`, `bio_erro_permissao`,
`bio_erro_conflito`, `bio_erro_servidor`, `ajuda_enviada`), nunca o texto nem JIDs de terceiros (RF11).

## Texto (rascunho para o dono aprovar no `[WA]`)
Ajuda: seções "📌 Figurinhas", "⬇️ Downloads", "ℹ️ Ajuda" (+ "🔧 Dono" só para o dono), formatação
`*negrito*`/`_itálico_`, 1 emoji por seção. Bio: link do GitHub, bloco "Comandos" gerado da tabela, bloco
"Novidades" (som nas figurinhas animadas; `/baixar` TikTok/Instagram; reação ao processar), rodapé "Roda em
servidor local dedicado 24/7; sugestões são bem-vindas". Estimativa < 900 caracteres.

## Riscos
- **Ban/incômodo:** baixo. Uma mensagem por comando, atraso 1–2 s, só a quem chamou (P1/P8). A mudança de
  descrição é ação do dono, feita no próprio grupo; gera 1 aviso do WhatsApp. Sem rajada: sem laço, no máximo
  1 nova tentativa por conflito.
- **Permissão:** bot não é admin e o grupo trava edição → falha limpa em 1 linha (RF10). Sem tentar contornar.
- **Privacidade:** bio e ajuda são texto fixo público (já é a bio de hoje); nada de JID, telefone ou caminho.
- **Texto defasado:** mitigado pela tabela única; teste obriga todo comando público do dispatcher a existir em
  `CMDS` (lista conferida à mão no teste, `shortcut:` limite conhecido: não detecta comando novo esquecido).
- **Colisão `/comandos`:** resolvida na decisão 1; teste garante que `/comandos allow` do dono continua `Allow`.
- **Limite de 2048:** truncar só `NOVIDADES`; teste com lista gigante.

## Config e padrões seguros
Nenhuma chave nova. `/updatedc` só do dono e só em grupo com papel `comando`; fora disso não grava.

## Testes
- `cargo test`: `parse_cmd` (`/help`, `/HELP x`, `/comandos`, não pega `/helpme`); `help_text` resumo cabe numa
  mensagem e esconde `/c` e `/updatedc` para não-dono; `/help baixar`, `/help dl`, `/help /s` → detalhe certo;
  nome inválido lista os nomes; tabela sem nome/apelido duplicado; `bio_text` ≤ 2048, sem os comandos de dono,
  idêntica em duas chamadas, trunca novidades gigantes; `/comandos allow` ainda é `ChatCmd::Allow`;
  classificação de erro (`DescriptionConflict`, permissão) como função pura.
- Só WhatsApp real `[WA]`: ver a ajuda no celular (formatação), `/updatedc` troca a bio de verdade, repetir = "já
  atualizada", membro comum sem efeito, grupo restrito a admins com o bot sem poder → falha de 1 linha.

## Passos (quem faz)
| # | Passo | Quem | Depende |
|---|---|---|---|
| A | `help.rs`: tabela, `help_text`, `bio_text`, parsers + testes | worker | — |
| B | `wa.rs`: ramos `/help`, `/updatedc`, colisão `/comandos`; `main.rs` | eu | A |
| C | `handle_updatedc` (fetch, set, conflito, erros 5×) + teste de classificação | eu | A |
| D | build `-j1` (fmt, clippy, test) + revisão P1/P3/P8 | eu | B, C |
| E | validação `[WA]` + aprovar texto + atualizar bio | dono + eu | D |
