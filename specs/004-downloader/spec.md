# Spec 004 — Downloader (Instagram e TikTok)

Status: **aprovada pelo dono em 2026-11-24** · Depende de: 001 (núcleo, painel, papéis)

## Por quê
Evitar que o grupo precise de apps/sites cheios de anúncio para salvar vídeo de Instagram
e TikTok. Um comando devolve o vídeo no chat, na melhor qualidade disponível e sem a marca
d'água do TikTok.

## Histórias
- Mando `/baixar <link>` com link de TikTok ou Instagram → o bot responde com o vídeo.
- **Respondo** uma mensagem que contém o link com `/baixar` → mesmo resultado.
- O vídeo passa do limite de vídeo do WhatsApp → o bot manda como **documento**, com uma
  linha avisando.
- O link não é de TikTok/Instagram, está quebrado, é privado ou exige login → o bot responde
  uma linha curta com o motivo.
- Dono (painel): anexo o arquivo de **cookies** de cada plataforma, clico em **Testar** e vejo
  um status (ok / falhou / sem cookie).

## Requisitos funcionais
- RF1 Comando `/baixar` (aliases `/dl`, `/download`). Só em chats com papel `comando`.
- RF2 Fonte do link: argumento do comando **ou** primeira URL da mensagem **citada**.
  Sem link → uma linha de uso.
- RF3 Plataformas aceitas: TikTok (`tiktok.com`, `vm.tiktok.com`, `vt.tiktok.com`) e
  Instagram (`instagram.com`: `/reel/`, `/p/`, `/tv/`; `instagr.am`). Qualquer outro domínio
  é recusado **antes** de chamar o motor (valida host; nada de passar URL arbitrária).
- RF4 Motor: **`yt-dlp` como processo externo**, nunca embutido. Selecionar a **melhor
  qualidade** de vídeo+áudio disponível; **sem reencodar** (só juntar/remux se necessário).
- RF5 TikTok **sem marca d'água**: escolher o formato sem watermark. Se só existir a versão
  com marca, o bot entrega essa e avisa numa linha (não falha em silêncio).
- RF6 Envio: até o limite de vídeo do WhatsApp → `videoMessage`; acima → `documentMessage`
  (mimetype `video/mp4`, nome de arquivo legível). Limite exato a medir (ver perguntas).
  Nunca reencoda para caber.
- RF7 Instagram com cookies: o dono configura cookies **por plataforma** (TikTok e Instagram
  separados). Sem cookie, tenta sem login; se a plataforma exigir login, responde
  "esse conteúdo exige login".
- RF8 Painel (rede local, com senha — P5): por plataforma, **upload do arquivo de cookies**
  (`.json` exportado do navegador), botão **Testar** e **status** (sem cookie / válido /
  falhou + horário do último teste), botão remover. O painel também edita o **limite de downloads por minuto** por chat
  (RF11), validando inteiro ≥ 1. O teste roda o motor contra uma URL
  pública fixa da plataforma e só diz ok/falhou.
- RF9 Formato dos cookies: o painel aceita o `.json` exportado e o guarda no formato que o
  `yt-dlp` lê (Netscape `cookies.txt`), convertendo no upload. Formato inválido → erro
  claro no painel, nada gravado.
- RF10 Falhas (timeout, link morto, yt-dlp ausente, conteúdo removido, rate limit da
  plataforma) respondem **uma linha** e nunca travam o daemon. Timeout por download; o
  processo filho é morto no timeout.
- RF11 Limite de uso por chat (N downloads/minuto, **configurável no painel**, reaproveitando
  o `Limiter` da 002; valor aplicado sem reiniciar o bot) e **no máximo 1 download simultâneo** no bot inteiro (P8, e protege o servidor).
- RF12 Temporários: o vídeo baixado vive em diretório temporário exclusivo, apagado ao fim
  **sempre** (sucesso, erro ou timeout). Nada de cache de mídia.
- RF13 Verificação no boot: `yt-dlp` presente e versão no log; ausente → o comando responde
  "yt-dlp ausente". Aviso se a versão tiver mais de ~60 dias (extratores quebram rápido).

## Não funcionais
- Atraso de 1–2 s antes de responder (P8); o aviso "baixando…" **não** é enviado (seria
  mensagem extra; só a resposta final).
- Cookies são credencial (P4): fora do git (`.gitignore`), permissão `0600`, **nunca** em
  log, painel (só status), nem em argumento de linha de comando visível quando evitável.
- Link e conteúdo baixado não vão para log (só plataforma + resultado ok/erro + tamanho).
- Dependência de sistema `yt-dlp` (Unlicense; é invocado como processo, sem copiar código —
  P9). Nenhum crate novo, salvo conversor de cookies se não der para escrever em poucas linhas.
- Download típico (< 50 MB) em segundos; teto de duração do processo configurável
  (padrão sugerido: 120 s).

## Fora do escopo
- Stories, lives, perfis inteiros, playlists, hashtags e download em lote.
- Só áudio / extrair MP3; só foto; carrossel (ver perguntas); legenda/comentários.
- Reencodar, cortar, converter em figurinha (a 002 já converte mídia citada, pode ser usado
  depois sobre o vídeo enviado).
- Baixar automaticamente ao ver um link (violaria P1/P6: o bot só responde a comando).
- Login pelo bot (usuário/senha); só cookies já exportados pelo dono.
- Outras plataformas (YouTube etc.): nova spec.

## Critérios de aceite
- Link público de TikTok → vídeo sem marca d'água, resolução igual à maior oferecida.
- Link público de Reel → vídeo na maior qualidade oferecida.
- Citando mensagem com o link e `/baixar` funciona igual.
- Vídeo maior que o limite chega como documento e abre/toca.
- Domínio fora da lista, link sem vídeo e conteúdo privado sem cookie → uma linha de erro,
  daemon segue vivo.
- Chat sem papel `comando` é ignorado (P2); dois `/baixar` simultâneos → o segundo espera
  ou recebe "ocupado" (decidir no plano), nunca rodam juntos.
- Após o envio (e após erro/timeout) o diretório temporário não existe mais.
- Painel: anexar cookies válidos → Testar mostra "válido"; arquivo inválido → erro e nada
  gravado; cookies nunca aparecem em log nem no HTML do painel; sem senha no painel, o
  upload não é acessível (P5).

## Riscos
- **Quebra frequente**: TikTok/Instagram mudam e o `yt-dlp` precisa atualizar. Tratar como
  manutenção rotineira (RF13 ajuda a perceber).
- **Ban da conta dos cookies**: usar conta **secundária**; raspar com cookies viola termos
  do Instagram. Risco aceito pelo dono, só para o grupo.
- **Termos de uso / direitos autorais**: baixar conteúdo de terceiros; uso pessoal no grupo,
  sem redistribuição pública.
- **WhatsApp**: vídeos grandes enviados em sequência aumentam risco; por isso RF11.

## Conferência com a constituição
P1 ok (só responde a `/baixar`) · P2 ok (papel `comando`) · P3 ok (cookies só pelo painel do
dono) · P4 ok (RF8/Não funcionais) · P5 ok (painel LAN + senha, upload atrás dela) · P6 ok
(sem leitura de mídia privada do grupo) · P7 ok (um binário externo, zero crate) · P8 ok
(RF11) · P9 ok (yt-dlp só como processo). Sem conflito.

## Perguntas em aberto
1. Limite exato de vídeo vs documento no WhatsApp que a `whatsapp-rust` aceita (medir com
   vídeo real; o plano decide o teto).
2. Fotos e **carrossel** do Instagram: ficam fora do MVP (padrão desta spec) ou o bot manda
   até N itens em sequência (conflita com P8)?
3. Segundo `/baixar` durante um download: esperar na fila ou responder "ocupado"?
4. O `.json` exportado pelo navegador: qual extensão/formato o dono usa (EditThisCookie,
   Cookie-Editor)? Define o conversor do RF9.
5. `yt-dlp` está instalado no servidor? (hoje não está no notebook de dev.)
6. Teto de tamanho absoluto para recusar (ex.: > 200 MB mesmo como documento)?
7. Cookies expiram: o painel só mostra o status do último teste ou testa periodicamente
   (periódico = tráfego não pedido à plataforma)?
