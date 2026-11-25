//! Ligação com o WhatsApp (T8): sessão, QR/pair code, handler de mensagens.
//! Nunca registra nem guarda texto de mensagem (P4); só tipo, JID e horário.
use crate::dispatch::{Action, decide};
use crate::roles::Role;
use crate::state::AppState;
use rand::Rng;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;
use whatsapp_rust::{GroupDescription, GroupError, PreviousDescription};

/// IDs de mensagens enviadas por nós (anti-loop: o dono comanda do mesmo número).
/// shortcut: zera ao passar de 500 IDs; basta para o ritmo de um grupo de amigos.
#[derive(Default)]
pub struct Sent(Mutex<HashSet<String>>);

impl Sent {
    pub fn add(&self, id: &str) {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.len() >= 500 {
            s.clear();
        }
        s.insert(id.to_string());
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }
}

/// Gancho dos comandos (spec 002 pluga aqui). Na 001 não há comando: não responde.
fn command_reply(_text: &str, _admin: bool) -> Option<String> {
    None
}

use crate::stickers::variante;
const SALVAR: &[&str] = &[
    "Não consegui salvar.",
    "Deu erro ao salvar, tenta de novo.",
    "Falhou ao gravar, tenta mais uma vez.",
    "Não rolou salvar isso agora.",
    "Travei na hora de salvar, repete o comando.",
];
const OUTRO_PAPEL: &[&str] = &[
    "Este chat tem outro papel; use o painel.",
    "Esse chat já tem outro papel, muda pelo painel.",
    "Aqui o papel é outro; ajusta pelo painel.",
    "Não mexo nesse chat por aqui, ele tem outro papel: use o painel.",
    "Chat com outro papel (alerta/monitorado): resolve no painel.",
];
const STATUS: &[&str] = &[
    "Papel deste chat: {}.",
    "Aqui o papel é: {}.",
    "Este chat está como: {}.",
    "O papel desse chat é {}.",
    "Situação deste chat: {}.",
];
const USO_C: &[&str] = &[
    "Uso: /c allow | deny | status",
    "Use assim: /c allow, /c deny ou /c status",
    "Faltou o argumento: /c allow | deny | status",
    "Não entendi. Tenta /c allow, /c deny ou /c status",
    "Comando certo: /c allow | deny | status",
];

#[derive(Debug, PartialEq)]
enum ChatCmd {
    Allow,
    Deny,
    Status,
    Usage,
}

/// `/c` ou `/comandos` + `allow|deny|status` (spec 004). Qualquer outro argumento vira `Usage`.
fn parse_chat_cmd(text: &str) -> Option<ChatCmd> {
    let mut w = text.split_whitespace();
    let cmd = w.next()?.to_lowercase();
    if cmd != "/c" && cmd != "/comandos" {
        return None;
    }
    let sub = w.next().map(str::to_lowercase);
    Some(match sub.as_deref() {
        Some("allow") => ChatCmd::Allow,
        Some("deny") => ChatCmd::Deny,
        Some("status") => ChatCmd::Status,
        // `/comandos` sem subcomando é a ajuda (spec 008); só `/c` mostra o uso do admin.
        _ if cmd == "/comandos" => return None,
        _ => ChatCmd::Usage,
    })
}

/// Executa `/c` do dono: muda o papel do chat atual e reage ✅ (ou responde uma linha).
async fn handle_chat_cmd(
    ctx: &MessageContext,
    st: &AppState,
    sent: &Sent,
    chat: &str,
    cmd: ChatCmd,
) {
    let role = st
        .roles
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(chat)
        .copied();
    let text = match (cmd, role) {
        (ChatCmd::Allow, None | Some(Role::Comando)) => {
            let ok = role.is_some()
                || st
                    .store
                    .set_role(chat, Role::Comando, None)
                    .and_then(|_| st.reload_roles())
                    .is_ok();
            if ok {
                st.push_event("comando_dono_allow", chat);
            }
            (!ok).then(|| variante(SALVAR).to_string())
        }
        (ChatCmd::Deny, Some(Role::Comando)) => {
            let ok = st
                .store
                .clear_role(chat)
                .and_then(|_| st.reload_roles())
                .is_ok();
            if ok {
                st.push_event("comando_dono_deny", chat);
            }
            (!ok).then(|| variante(SALVAR).to_string())
        }
        (ChatCmd::Deny, None) => None,
        (ChatCmd::Allow | ChatCmd::Deny, Some(_)) => Some(variante(OUTRO_PAPEL).into()),
        (ChatCmd::Status, r) => {
            Some(variante(STATUS).replace("{}", r.map_or("nenhum", |r| r.as_str())))
        }
        (ChatCmd::Usage, _) => Some(variante(USO_C).into()),
    };
    let res = match text {
        None => ctx.react("✅").await,
        Some(t) => ctx.reply(t).await,
    };
    match res {
        Ok(r) => sent.add(r.message_id.as_str()),
        Err(_) => st.push_event("erro_envio", chat),
    }
}

/// Atraso configurado antes de responder (P8).
pub(crate) async fn pause(st: &AppState) {
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    tokio::time::sleep(pick_delay(min, max)).await;
}

async fn say(ctx: &MessageContext, sent: &Sent, t: &str) {
    if let Ok(r) = ctx.reply_quoting(t).await {
        sent.add(r.message_id.as_str());
    }
}

const BIO_NAO_DONO: &[&str] = &[
    "Esse comando é só do dono.",
    "Só o dono pode usar esse.",
    "Isso aqui é coisa do dono, desculpa.",
    "Esse eu só atendo se vier do dono.",
    "Comando restrito ao dono.",
];
const BIO_NAO_GRUPO: &[&str] = &[
    "Esse comando só funciona dentro de um grupo.",
    "Preciso estar num grupo pra isso.",
    "Aqui não é grupo, tenta lá.",
    "Só rola em grupo, esse comando.",
    "Manda isso dentro do grupo que eu atualizo.",
];
const BIO_SEM_PERM: &[&str] = &[
    "Não tenho permissão pra editar a descrição desse grupo.",
    "O grupo não me deixa mudar a descrição.",
    "Sem permissão pra mexer na descrição aqui.",
    "Não consegui: só admins podem editar a descrição e eu não sou.",
    "Descrição travada pra mim nesse grupo.",
];
const BIO_ERRO: &[&str] = &[
    "Não consegui atualizar a descrição agora.",
    "Deu erro ao mudar a descrição, tenta de novo daqui a pouco.",
    "Falhou na hora de gravar a descrição.",
    "A descrição não quis mudar, tenta mais tarde.",
    "Travei ao atualizar a descrição, repete depois.",
];

#[derive(Debug, PartialEq)]
enum BioErr {
    Conflito,
    SemPerm,
    Servidor,
}

fn classify_bio_err(e: &GroupError) -> BioErr {
    if matches!(e, GroupError::DescriptionConflict) {
        return BioErr::Conflito;
    }
    let s = e.to_string().to_lowercase();
    if ["403", "not-authorized", "forbidden"]
        .iter()
        .any(|k| s.contains(k))
    {
        BioErr::SemPerm
    } else {
        BioErr::Servidor
    }
}

/// `/updatedc`: só o dono, só em grupo. Grava a bio gerada por `help::bio_text` (spec 008).
async fn handle_updatedc(ctx: &MessageContext, st: &AppState, sent: &Sent, owner: bool) {
    if !owner {
        st.dbg("bio_recusa", "nao dono");
        return say(ctx, sent, variante(BIO_NAO_DONO)).await;
    }
    let jid = ctx.info.source.chat.clone();
    if !jid.to_string().ends_with("@g.us") {
        st.dbg("bio_recusa", "nao e grupo");
        return say(ctx, sent, variante(BIO_NAO_GRUPO)).await;
    }
    let bio = crate::help::bio_text();
    let Ok(desc) = GroupDescription::new(bio.clone()) else {
        st.dbg("bio_erro_servidor", "texto invalido");
        return say(ctx, sent, variante(BIO_ERRO)).await;
    };
    let groups = ctx.client.groups();
    let mut conflito = false;
    for _ in 0..2 {
        let meta = match groups.fetch_metadata(&jid).await {
            Ok(m) => m,
            Err(e) => {
                st.dbg(
                    "bio_erro_servidor",
                    &format!("leitura: {}", classify_name(&e)),
                );
                return say(ctx, sent, variante(BIO_ERRO)).await;
            }
        };
        if meta.description.as_deref() == Some(bio.as_str()) {
            st.dbg("bio_atualizada", "ja estava igual");
            return react_ok(ctx, sent).await;
        }
        let prev = PreviousDescription::from(meta.description_id.as_deref());
        match groups
            .set_description(jid.clone(), Some(desc.clone()), prev)
            .await
        {
            Ok(()) => {
                st.dbg(
                    "bio_atualizada",
                    &format!("{} caracteres", bio.chars().count()),
                );
                return react_ok(ctx, sent).await;
            }
            Err(e) => match classify_bio_err(&e) {
                BioErr::Conflito if !conflito => {
                    conflito = true;
                    st.dbg("bio_erro_conflito", "relendo");
                }
                BioErr::SemPerm => {
                    st.dbg("bio_erro_permissao", "sem permissao");
                    return say(ctx, sent, variante(BIO_SEM_PERM)).await;
                }
                k => {
                    st.dbg("bio_erro_servidor", &format!("{k:?}"));
                    return say(ctx, sent, variante(BIO_ERRO)).await;
                }
            },
        }
    }
}

fn classify_name(e: &GroupError) -> String {
    format!("{:?}", classify_bio_err(e))
}

async fn react_ok(ctx: &MessageContext, sent: &Sent) {
    if let Ok(r) = ctx.react("✅").await {
        sent.add(r.message_id.as_str());
    }
}

/// Atraso aleatório configurado (P8). Mínimo maior que máximo vira o mínimo.
pub fn pick_delay(min: u64, max: u64) -> Duration {
    Duration::from_millis(rand::rng().random_range(min..=max.max(min)))
}

async fn on_message(st: Arc<AppState>, sent: Arc<Sent>, ctx: MessageContext) {
    let src = &ctx.info.source;
    let chat = src.chat.to_string();
    let sender = src.sender.to_string();
    let alt = src.sender_alt.as_ref().map(|j| j.to_string());
    // Só o JID e o horário, para a lista do painel (não é parse).
    st.note_recent(&chat);
    let owner = st.store.owner_jid().ok().flatten();
    let action = decide(
        &st.roles,
        &chat,
        &sender,
        alt.as_deref(),
        src.is_from_me,
        owner.as_deref(),
    );
    // `owner_only`: dono em chat sem papel (spec 004) só dispara figurinha e `/c`.
    let (admin, owner_only) = match action {
        Action::Drop => return,
        Action::Monitor => {
            // Spec 003: lê em silêncio; só o monitor envia, e só ao chat `alerta`.
            // Mensagem nossa (is_from_me/eco) nunca vira alerta.
            if !src.is_from_me && !sent.contains(ctx.info.id.as_str()) {
                tokio::spawn(crate::monitor::trata(st, sent, ctx));
            }
            return;
        }
        Action::Command { admin } => (admin, false),
        Action::Owner => (true, true),
    };
    // Eco das nossas próprias respostas.
    if src.is_from_me && sent.contains(ctx.info.id.as_str()) {
        return;
    }
    // Legenda de imagem/vídeo também vale (RF2: comando na própria mídia).
    let Some(text) = ctx
        .message
        .text_content()
        .or_else(|| ctx.message.get_caption())
    else {
        return;
    };
    if !text.starts_with('/') {
        return;
    }
    if admin && let Some(c) = parse_chat_cmd(text) {
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        handle_chat_cmd(&ctx, &st, &sent, &chat, c).await;
        return;
    }
    if owner_only
        && !crate::stickers::parse_cmd(text)
        && crate::stickers::parse_rename(text).is_none()
        && crate::download::handle::parse_cmd(text).is_none()
    {
        return;
    }
    st.push_event("comando", &chat);
    if crate::stickers::parse_cmd(text) {
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        crate::stickers::handle(&ctx, &st, &sent).await;
        return;
    }
    if let Some(arg) = crate::download::handle::parse_cmd(text) {
        st.dbg("baixar_comando", "chegou ao despacho");
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        crate::download::handle::handle(&ctx, &st, &sent, arg).await;
        return;
    }
    if let Some(arg) = crate::help::parse_cmd(text) {
        pause(&st).await;
        let txt = crate::help::help_text(arg, admin);
        say(&ctx, &sent, &txt).await;
        st.dbg(
            "ajuda_enviada",
            if arg.is_empty() { "resumo" } else { "detalhe" },
        );
        return;
    }
    if crate::help::parse_updatedc(text) {
        pause(&st).await;
        handle_updatedc(&ctx, &st, &sent, admin).await;
        return;
    }
    if let Some(args) = crate::stickers::parse_rename(text) {
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        crate::stickers::handle_rename(&ctx, &st, &sent, args).await;
        return;
    }
    let Some(reply) = command_reply(text, admin) else {
        return;
    };
    // Resposta que começa com `/` seria lida como comando pelo próprio bot.
    if reply.starts_with('/') {
        return;
    }
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    tokio::time::sleep(pick_delay(min, max)).await;
    match ctx.reply(reply).await {
        Ok(r) => sent.add(r.message_id.as_str()),
        Err(_) => st.push_event("erro_envio", &chat),
    }
}

/// Sobe o bot e bloqueia até o fim da sessão. O número do pair code vem de `st.pairing.phone`;
/// o painel pode pedir `st.restart` (tentar de novo / trocar número) e a sessão sobe de novo.
pub async fn run(st: Arc<AppState>, session_db: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sent = Arc::new(Sent::default());
    loop {
        let phone = st.lock_pairing().phone.clone();
        let bot = build(&st, &sent, session_db, phone).await?;
        let client = bot.client();
        *st.client.lock().unwrap_or_else(|e| e.into_inner()) = Some(client.clone());
        let ended = tokio::select! {
            reason = bot.run() => Some(reason),
            _ = st.restart.notified() => {
                let _ = client.shutdown().await;
                None
            }
        };
        if let Some(reason) = ended {
            // QRs esgotados: a lib encerra a sessão sozinha (ShutdownRequested) e o evento pode chegar
            // depois do fim do run, então a flag sozinha não basta. Nada no processo chama shutdown
            // fora do "tentar de novo", logo esse motivo também espera o botão do painel.
            let shutdown = matches!(
                reason,
                whatsapp_rust::RunCompletionReason::ShutdownRequested
            );
            if shutdown || st.lock_pairing().exhausted {
                st.restart.notified().await;
            } else {
                // Fim sem pedido de shutdown (ex.: reconexão esgotada): erro, para o systemd reiniciar.
                // Logout já saiu com código 2 dentro de `on_logged_out`.
                return Err(format!("bot terminou sem pedido de shutdown: {reason:?}").into());
            }
        }
        *st.client.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let mut p = st.lock_pairing();
        p.qr = None;
        p.code = None;
        p.exhausted = false;
    }
}

async fn build(
    st: &Arc<AppState>,
    sent: &Arc<Sent>,
    session_db: &str,
    pair_phone: Option<String>,
) -> Result<Bot, Box<dyn std::error::Error>> {
    let backend = SqliteStore::open(session_db).await?;
    // A sessão é segredo: só o dono do arquivo lê (P4).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(session_db, std::fs::Permissions::from_mode(0o600))?;
    }
    let (s_msg, s_con, s_evt, s_qr, s_pc, s_ex) = (
        st.clone(),
        st.clone(),
        st.clone(),
        st.clone(),
        st.clone(),
        st.clone(),
    );
    let sent = sent.clone();

    let mut b = Bot::builder()
        .with_backend(backend)
        // Nome que aparece em "Aparelhos conectados"; vale só no 1º pareamento.
        .with_device_props(
            whatsapp_rust::wacore::store::DevicePropsOverride::new().with_os("Silicon™"),
        )
        .on_qr_code(move |code, ttl| {
            let st = s_qr.clone();
            async move {
                // Só no terminal: sob o systemd o stdout vai para o journald (P4).
                if std::io::stdout().is_terminal() {
                    match qrcode::QrCode::new(code.as_bytes()) {
                        Ok(q) => println!(
                            "Escaneie o QR no WhatsApp (Aparelhos conectados):\n{}",
                            q.render::<qrcode::render::unicode::Dense1x2>().build()
                        ),
                        Err(_) => eprintln!("Não consegui desenhar o QR."),
                    }
                }
                // QR/código são credenciais temporárias: só em memória, nunca no log (P4).
                let mut p = st.lock_pairing();
                p.qr = Some(code);
                p.ttl = ttl.as_secs();
                p.at = crate::state::now();
                p.exhausted = false;
            }
        })
        .on_pair_code(move |code, _ttl| {
            let st = s_pc.clone();
            async move {
                if std::io::stdout().is_terminal() {
                    println!("Código de pareamento: {code}");
                }
                st.lock_pairing().code = Some(code);
            }
        })
        .on_event_for(&[EventKind::PairingQrCodesExhausted], move |_ev, _c| {
            let st = s_ex.clone();
            async move {
                let mut p = st.lock_pairing();
                p.qr = None;
                p.exhausted = true;
            }
        })
        .on_message(move |ctx| on_message(s_msg.clone(), sent.clone(), ctx))
        .on_connected(move |client| {
            let st = s_con.clone();
            async move {
                {
                    let mut s = st.status.lock().unwrap_or_else(|e| e.into_inner());
                    s.connected = true;
                    s.number = client.pn().map(|j| j.to_string());
                }
                *st.lock_pairing() = Default::default();
                st.push_event("conectado", "");
                match client.groups().list_participating().await {
                    Ok(list) => {
                        *st.groups.lock().unwrap_or_else(|e| e.into_inner()) = list
                            .into_iter()
                            .map(|g| (g.id.to_string(), g.subject.unwrap_or_default()))
                            .collect();
                    }
                    Err(_) => st.push_event("erro_grupos", ""),
                }
            }
        })
        .on_event_for(&[EventKind::Disconnected], move |_ev, _c| {
            let st = s_evt.clone();
            async move {
                st.status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .connected = false;
                st.push_event("desconectado", "");
            }
        })
        // Logout pelo celular: precisa de QR novo; sai com 2 para o systemd não reiniciar (P8).
        .on_logged_out(|_| async move {
            eprintln!("Sessão encerrada pelo WhatsApp. Pareie de novo no terminal.");
            std::process::exit(2);
        });
    if let Some(phone) = pair_phone {
        b = b.with_pair_code(PairCodeOptions {
            phone_number: phone,
            ..Default::default()
        });
    }
    Ok(b.build().await?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn comandos_e_ajuda_ou_c() {
        assert_eq!(parse_chat_cmd("/comandos allow"), Some(ChatCmd::Allow));
        assert_eq!(parse_chat_cmd("/comandos status"), Some(ChatCmd::Status));
        assert_eq!(parse_chat_cmd("/comandos"), None);
        assert_eq!(parse_chat_cmd("/c"), Some(ChatCmd::Usage));
    }

    #[test]
    fn classifica_erro_da_bio() {
        assert_eq!(
            classify_bio_err(&GroupError::DescriptionConflict),
            BioErr::Conflito
        );
        let perm = GroupError::InvalidRequest("403 forbidden".into());
        assert_eq!(classify_bio_err(&perm), BioErr::SemPerm);
        let outro = GroupError::InvalidRequest("timeout".into());
        assert_eq!(classify_bio_err(&outro), BioErr::Servidor);
    }

    #[test]
    fn mensagens_do_c_tem_5_variantes() {
        for g in [
            BIO_NAO_DONO,
            BIO_NAO_GRUPO,
            BIO_SEM_PERM,
            BIO_ERRO,
            SALVAR,
            OUTRO_PAPEL,
            STATUS,
            USO_C,
        ] {
            assert!(g.len() >= 5);
        }
        assert!(STATUS.iter().all(|t| t.contains("{}")));
    }

    use super::*;

    #[test]
    fn chat_cmd_parser() {
        assert_eq!(parse_chat_cmd("/c allow"), Some(ChatCmd::Allow));
        assert_eq!(parse_chat_cmd("/COMANDOS Deny"), Some(ChatCmd::Deny));
        assert_eq!(parse_chat_cmd("/c status x"), Some(ChatCmd::Status));
        assert_eq!(parse_chat_cmd("/c"), Some(ChatCmd::Usage));
        assert_eq!(parse_chat_cmd("/c foo"), Some(ChatCmd::Usage));
        assert_eq!(parse_chat_cmd("/cx allow"), None);
        assert_eq!(parse_chat_cmd("/s"), None);
    }

    #[test]
    fn sent_set_and_delay() {
        let s = Sent::default();
        assert!(!s.contains("A"));
        s.add("A");
        assert!(s.contains("A"));
        for i in 0..600 {
            s.add(&i.to_string());
        }
        assert!(s.0.lock().unwrap().len() <= 500);
        for _ in 0..50 {
            let d = pick_delay(1000, 2000);
            assert!((1000..=2000).contains(&(d.as_millis() as u64)));
        }
        assert_eq!(pick_delay(3000, 1000), Duration::from_millis(3000));
    }
}
