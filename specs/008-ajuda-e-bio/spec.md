# Spec 008 — Ajuda (`/help`) e bio do grupo (`/updatedc`)

Status: **rascunho** (2026-11-24) · Depende de: 001 (núcleo, papéis), 002 (figurinhas), 004-downloader (`/baixar`) ·
Constituição v1.1.0

## Por quê
Hoje quem entra no grupo só sabe dos comandos pela bio do grupo, escrita à mão e já defasada (cita
`/renomear`, o comando real é `/rename`; não fala de `/baixar` nem de som nas figurinhas). Falta um jeito
de o próprio bot explicar cada comando, e de o dono manter a bio em dia sem digitar tudo.

## Histórias
- **Membro do grupo:** mando `/help` e recebo, numa mensagem curta, a lista de comandos; mando
  `/help baixar` e recebo o passo a passo daquele comando, com exemplo.
- **Dono:** depois de lançar uma função nova, mando `/updatedc` no grupo e a bio é reescrita com o texto
  padrão + a seção "novidades", sem eu copiar e colar.

## Requisitos funcionais
- **RF1 `/help` e `/comandos`** (sem caixa, mesma coisa) respondem citando o comando. Sem argumento:
  **resumo** (uma linha por comando) em **uma única mensagem**, com seções e emojis, usando só a
  formatação nativa do WhatsApp (`*negrito*`, `_itálico_`), sem tabela nem ASCII pesado.
- **RF2 `/help <comando>`** (ex.: `/help s`, `/help rename`, `/help baixar`): ajuda **detalhada** daquele
  comando: o que faz, como usar (responder a quê), exemplo, limites e apelidos. Aceita o nome com ou sem `/`
  e qualquer apelido (`/help dl`). Nome desconhecido → uma linha curta com a lista de nomes válidos.
- **RF3 Quem usa:** qualquer chat com papel `comando` (como `/s` e `/baixar`). O resumo e o detalhe de
  **comandos do dono** (`/c`, `/updatedc`) só aparecem se quem pediu for o dono.
- **RF4 Fonte única:** os textos moram numa tabela no código (nome, apelidos, resumo, detalhe, "só dono",
  "entra na bio"). `/help` e `/updatedc` leem a mesma tabela, para não defasar. Texto de ajuda não inclui
  segredo, JID, telefone nem caminho do servidor.
- **RF5 Variações:** o cabeçalho do `/help` tem 5 variações sorteadas (como o resto do bot); o conteúdo dos
  comandos é fixo.
- **RF6 `/updatedc`** (só dono, **só em grupo**, só em chat com papel `comando`): reescreve a descrição **do
  grupo onde o comando foi mandado** com o texto montado abaixo. Responde com uma linha curta
  (reação ✅ ou mensagem de erro, como o `/c`).
- **RF7 Texto da bio** (montado, limpo, ≤ limite do WhatsApp para descrição, 2048 caracteres; truncar na
  seção "novidades" se estourar):
  1. link do GitHub (`https://github.com/al4sk4s/Silicon`);
  2. **Comandos** (uma linha cada, só os marcados "entra na bio": `/s`, `/rename`, `/baixar`, `/help`);
  3. **Novidades** (seção curta: som nas figurinhas animadas, download de TikTok/Instagram, reação ao
     processar); lista fixa no código, atualizada a cada spec que lance função visível;
  4. aviso: roda no servidor local dedicado 24/7; sugestões são bem-vindas.
  Os comandos da bio saem da tabela do RF4 (nunca digitados à mão).
- **RF8 Sem corrida:** a lib exige o token da descrição atual; o bot lê a descrição antes de gravar. Se o
  servidor disser que mudou (conflito), relê e tenta **uma** vez; falhou → uma linha de erro. Sem laço.
- **RF9 Idempotência:** rodar `/updatedc` duas vezes seguidas deixa a bio igual (e responde "já está
  atualizada" sem gravar de novo, quando o texto for idêntico).
- **RF10 Falhas** com uma linha, 5 variações cada: não sou dono; não é grupo; bot sem permissão de
  editar a descrição (grupo restrito a admins); erro do servidor.
- **RF11 Registro:** categorias na aba Debug (`ajuda_enviada`, `bio_atualizada`, `bio_erro_*`), sem o texto
  completo da bio nem JIDs de terceiros.

## Não funcionais
- **P1/P2/P3:** `/help` só responde a quem tem papel `comando`; `/updatedc` só do dono (P3) e só responde a
  ele. Atualizar a bio é **efeito** de um comando do dono, não mensagem espontânea. Chat `monitorado`/`alerta`
  e sem papel: ignorado, como hoje.
- **P8:** 1–2 s de atraso; uma mensagem por comando; sem rajada. A mudança de descrição gera um aviso
  automático do WhatsApp no grupo ("X mudou a descrição"); é inevitável e vem do dono.
- **P7:** zero crate novo; usa `Client::groups()` da lib já fixada (`eeb24da`: `set_description` com token).
- Ajuda e bio em PT-BR, sem emoji em excesso (no máximo um por seção).

## Fora do escopo
- Editar o texto da bio ou as novidades pelo painel (fica no código; pode virar spec depois).
- `/updatedc` em outro grupo que não o do comando, ou fora de grupo.
- Mudar nome, foto ou configurações do grupo.
- Ajuda interativa (botões/listas), tradução, `/help` por voz.
- Comando que mande mensagem sem ser chamado (anúncio de novidades).

## Critérios de aceite
1. `/help` num chat `comando` devolve **uma** mensagem com todos os comandos públicos; `/comandos` igual.
2. `/help baixar`, `/help s`, `/help rename` e `/help dl` mostram o detalhe correto; `/help xyz` lista os nomes.
3. Membro comum **não** vê `/c` nem `/updatedc`; o dono vê.
4. `/updatedc` do dono no grupo troca a descrição; a bio traz GitHub, comandos, novidades e o aviso 24/7;
   nenhum comando da bio está fora da tabela.
5. Rodar duas vezes seguidas: segunda vez diz "já atualizada" e não grava.
6. Membro comum ou chat sem papel mandando `/updatedc`: sem efeito (ignorado ou recusa curta, sem gravar).
7. Em grupo só de admins editarem, o bot responde a falha em uma linha, sem laço.
8. Testes (`cargo test`): tabela sem apelido duplicado, resumo ≤ limite, bio ≤ 2048 e idêntica quando
   repetida, `/help <nome>` com apelidos, ocultação dos comandos do dono. Validação com WhatsApp real
   (`[WA]`): ver a mensagem no celular e a bio no grupo.

## Conflito a resolver (apontado, não decidido)
`/comandos` **já existe** como apelido de `/c` (admin do dono: `allow`/`deny`/`status`, em `wa.rs:83`).
Proposta: `/comandos` sem argumento vira ajuda para todos; `/comandos allow|deny|status` continua
funcionando para o dono como hoje (compatível), e `/c` não muda.

## Perguntas em aberto
1. O bot é **admin** do grupo ou o grupo deixa qualquer membro editar a descrição? Se só admins editam e o
   bot não é, `/updatedc` falha (RF10); vale tornar o bot admin?
2. Texto das "novidades": quer revisar a redação antes de ir pro ar (proponho gerar um rascunho na
   implementação e você aprova no `[WA]`)?
3. `/help` deve listar também `/dl` e `/download` como apelidos, ou só o nome principal (`/baixar`)?
4. Corrigir a bio atual (`/renomear` → `/rename`) é o único erro conhecido; há outro comando que você queira
   esconder da ajuda?
