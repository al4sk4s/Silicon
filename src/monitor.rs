//! Monitor de promoções (spec 003): lê o chat `monitorado` em silêncio e, se uma regra casar,
//! manda UMA mensagem nova ao chat `alerta`. Nunca responde, reage nem envia ao chat de origem (P1).
//! Logs/eventos só com JID e tipo, nunca o texto (P4).
use crate::alerta::{em_silencio, formata, resumo};
use crate::roles::Role;
use crate::state::{AppState, local_hms, now};
use crate::wa::{Sent, pick_delay};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use whatsapp_rust::prelude::*;

/// Único destino possível: o chat `alerta`, e nunca a própria origem.
pub fn destino(roles: &HashMap<String, Role>, origem: &str) -> Option<String> {
    roles
        .iter()
        .find(|(j, r)| **r == Role::Alerta && j.as_str() != origem)
        .map(|(j, _)| j.clone())
}

/// Participantes do grupo `alerta` (user, jid completo), cache de 10 min.
/// shortcut: cache global de um só grupo (só existe um chat `alerta`), sem invalidar ao mudar o papel; vence em 10 min.
type Cache = Option<(String, Instant, Vec<(String, String)>)>;
static MENCOES: Mutex<Cache> = Mutex::new(None);

async fn participantes(client: &Client, grupo: &str) -> Vec<(String, String)> {
    {
        let g = MENCOES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((j, t, v)) = &*g
            && j == grupo
            && t.elapsed() < Duration::from_secs(600)
        {
            return v.clone();
        }
    }
    let Ok(jid) = grupo.parse::<Jid>() else {
        return vec![];
    };
    // Falha no metadata: alerta sai sem menções (não perde o alerta).
    let Ok(meta) = client.groups().fetch_metadata(&jid).await else {
        return vec![];
    };
    let eu: Vec<String> = [client.pn(), client.lid()]
        .into_iter()
        .flatten()
        .map(|j| j.user.to_string())
        .collect();
    let v: Vec<(String, String)> = meta
        .participants
        .iter()
        .filter(|p| !eu.contains(&p.jid.user.to_string()))
        .map(|p| (p.jid.user.to_string(), p.jid.to_string()))
        .collect();
    *MENCOES.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((grupo.into(), Instant::now(), v.clone()));
    v
}

fn monta(texto: &str, midia: Option<&wa::Message>, jids: Vec<String>) -> wa::Message {
    let mut ctx = wa::ContextInfo::default();
    ctx.mentioned_jid = jids;
    if let Some(mut m) = midia.and_then(|m| m.media_with_caption(texto))
        && m.set_context_info(ctx.clone())
    {
        return m;
    }
    let mut e = wa::message::ExtendedTextMessage::default();
    e.text = Some(texto.to_string());
    e.context_info = MessageField::some(ctx);
    let mut m = wa::Message::default();
    m.extended_text_message = MessageField::some(e);
    m
}

/// Chamado num `tokio::spawn` pelo `on_message` para `Action::Monitor`.
pub async fn trata(st: Arc<AppState>, sent: Arc<Sent>, ctx: MessageContext) {
    let origem = ctx.info.source.chat.to_string();
    let Some(texto) = ctx
        .message
        .text_content()
        .or_else(|| ctx.message.get_caption())
        .map(str::to_string)
    else {
        return;
    };
    let detector = st
        .detector
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let Some(regra) = detector.avalia(&texto) else {
        return; // nada é gravado
    };
    let agora = Instant::now();
    let (dedupe_min, por_hora, ini, fim) = (
        st.store.dedupe_min().unwrap_or(30),
        st.store.alertas_hora().unwrap_or(10),
        st.store.silencio_ini().unwrap_or_default(),
        st.store.silencio_fim().unwrap_or_default(),
    );
    let hhmm = local_hms(now());
    let barrado = em_silencio(&hhmm[..5], &ini, &fim)
        || st
            .dedupe
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .repetido(&texto, Duration::from_secs(dedupe_min * 60), agora)
        || !st
            .limite
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .permite(por_hora as usize, agora);
    if barrado {
        st.push_event("alerta_barrado", &origem);
        return;
    }
    let Some(dest) = destino(&st.roles.read().unwrap_or_else(|e| e.into_inner()), &origem) else {
        st.push_event("alerta_sem_destino", &origem);
        return;
    };
    let Ok(dest_jid) = dest.parse::<Jid>() else {
        st.push_event("alerta_falhou", &origem);
        return;
    };
    let min = st.store.delay_min_ms().unwrap_or(1000);
    let max = st.store.delay_max_ms().unwrap_or(2000);
    tokio::time::sleep(pick_delay(min, max)).await;

    let pessoas = if dest.ends_with("@g.us") {
        participantes(&ctx.client, &dest).await
    } else {
        vec![] // DM: o alerta já notifica sozinho
    };
    let grupo = st
        .groups
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(j, _)| *j == origem)
        .map_or(origem.clone(), |(_, s)| s.clone());
    let src = &ctx.info.source;
    let autor = format!("{} ({})", ctx.info.push_name, src.sender.user);
    let hora = local_hms(ctx.info.timestamp.as_secs() as u64)[..5].to_string();
    let mencoes: Vec<String> = pessoas.iter().map(|(u, _)| format!("@{u}")).collect();
    let corpo = formata(&regra.nome, &grupo, &autor, &hora, &texto, &mencoes);
    let msg = monta(
        &corpo,
        Some(&ctx.message),
        pessoas.into_iter().map(|(_, j)| j).collect(),
    );

    // 2 retentativas (5 s e 30 s); erro nunca derruba o daemon.
    for espera in [Some(5), Some(30), None] {
        match ctx.client.send_message(dest_jid.clone(), msg.clone()).await {
            Ok(r) => {
                sent.add(r.message_id.as_str());
                let _ = st.store.log_alert(now(), &regra.nome, &resumo(&texto));
                st.push_event("alerta_enviado", &dest);
                return;
            }
            Err(_) => match espera {
                Some(s) => tokio::time::sleep(Duration::from_secs(s)).await,
                None => st.push_event("alerta_falhou", &dest),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destino_so_alerta_e_nunca_origem() {
        let mut r = HashMap::new();
        r.insert("promo@g.us".to_string(), Role::Monitorado);
        r.insert("cmd@g.us".to_string(), Role::Comando);
        assert_eq!(destino(&r, "promo@g.us"), None); // sem alerta: nada enviado
        r.insert("dono@g.us".to_string(), Role::Alerta);
        assert_eq!(destino(&r, "promo@g.us").as_deref(), Some("dono@g.us"));
        assert_eq!(destino(&r, "dono@g.us"), None); // origem ≠ destino
    }

    #[test]
    fn monta_texto_leva_mencoes() {
        let m = monta("oi @1", None, vec!["1@s.whatsapp.net".into()]);
        let e = m.extended_text_message.as_option().unwrap();
        assert_eq!(e.text.as_deref(), Some("oi @1"));
        assert_eq!(e.context_info.as_option().unwrap().mentioned_jid.len(), 1);
    }
}
