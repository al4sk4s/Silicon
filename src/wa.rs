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
    Some(match w.next().map(str::to_lowercase).as_deref() {
        Some("allow") => ChatCmd::Allow,
        Some("deny") => ChatCmd::Deny,
        Some("status") => ChatCmd::Status,
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
            (!ok).then(|| "Não consegui salvar.".to_string())
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
            (!ok).then(|| "Não consegui salvar.".to_string())
        }
        (ChatCmd::Deny, None) => None,
        (ChatCmd::Allow | ChatCmd::Deny, Some(_)) => {
            Some("Este chat tem outro papel; use o painel.".into())
        }
        (ChatCmd::Status, r) => Some(format!(
            "Papel deste chat: {}.",
            r.map_or("nenhum", |r| r.as_str())
        )),
        (ChatCmd::Usage, _) => Some("Uso: /c allow | deny | status".into()),
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

/// Atraso aleatório configurado (P8). Mínimo maior que máximo vira o mínimo.
fn pick_delay(min: u64, max: u64) -> Duration {
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
        Action::Drop | Action::Monitor => return, // Monitor: gancho da spec 003
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
