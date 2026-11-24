//! Ligação com o WhatsApp (T8): sessão, QR/pair code, handler de mensagens.
//! Nunca registra nem guarda texto de mensagem (P4); só tipo, JID e horário.
use crate::dispatch::{Action, decide};
use crate::state::AppState;
use rand::Rng;
use std::collections::HashSet;
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
    let admin = match action {
        Action::Drop | Action::Monitor => return, // Monitor: gancho da spec 003
        Action::Command { admin } => admin,
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
    st.push_event("comando", &chat);
    if crate::stickers::parse_cmd(text) {
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        crate::stickers::handle(&ctx, &st, &sent).await;
        return;
    }
    if let Some(arg) = crate::download::handle::parse_cmd(text) {
        let min = st.store.delay_min_ms().unwrap_or(1000);
        let max = st.store.delay_max_ms().unwrap_or(2000);
        tokio::time::sleep(pick_delay(min, max)).await;
        crate::download::handle::handle(&ctx, &st, &sent, arg).await;
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

/// Sobe o bot e bloqueia até o fim da sessão. `pair_phone` (só dígitos) pede pair code.
pub async fn run(
    st: Arc<AppState>,
    session_db: &str,
    pair_phone: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = SqliteStore::open(session_db).await?;
    // A sessão é segredo: só o dono do arquivo lê (P4).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(session_db, std::fs::Permissions::from_mode(0o600))?;
    }
    let sent = Arc::new(Sent::default());
    let (s_msg, s_con, s_evt) = (st.clone(), st.clone(), st.clone());

    let mut b = Bot::builder()
        .with_backend(backend)
        // Nome que aparece em "Aparelhos conectados"; vale só no 1º pareamento.
        .with_device_props(
            whatsapp_rust::wacore::store::DevicePropsOverride::new().with_os("Silicon™"),
        )
        .on_qr_code(|code, _ttl| async move {
            match qrcode::QrCode::new(code.as_bytes()) {
                Ok(q) => println!(
                    "Escaneie o QR no WhatsApp (Aparelhos conectados):\n{}",
                    q.render::<qrcode::render::unicode::Dense1x2>().build()
                ),
                Err(_) => eprintln!("Não consegui desenhar o QR."),
            }
        })
        .on_pair_code(|code, _ttl| async move {
            println!("Código de pareamento: {code}");
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
    let bot = b.build().await?;
    let reason = bot.run().await;
    // Fim sem pedido de shutdown (ex.: reconexão esgotada): erro, para o systemd reiniciar.
    // Logout já saiu com código 2 dentro de `on_logged_out`.
    match reason {
        whatsapp_rust::RunCompletionReason::ShutdownRequested => Ok(()),
        other => Err(format!("bot terminou sem pedido de shutdown: {other:?}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
