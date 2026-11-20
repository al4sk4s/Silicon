use crate::roles::{ChatRoles, Role};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Drop,
    Command { admin: bool },
    Monitor,
}

/// Tira o sufixo de dispositivo (`user:80@lid` -> `user@lid`) para comparar o dono.
fn bare(jid: &str) -> String {
    match jid.split_once('@') {
        Some((u, d)) => format!("{}@{d}", u.split(':').next().unwrap_or(u)),
        None => jid.to_string(),
    }
}

/// Decide o destino de uma mensagem só pelo papel do chat (P2/P3). Pura: sem IO.
pub fn decide(
    roles: &ChatRoles,
    chat: &str,
    sender: &str,
    sender_alt: Option<&str>,
    from_me: bool,
    owner: Option<&str>,
) -> Action {
    // Lock envenenado = sem papel = mudo.
    let role = match roles.read() {
        Ok(map) => map.get(chat).copied(),
        Err(_) => None,
    };
    match role {
        Some(Role::Monitorado) => Action::Monitor,
        Some(Role::Comando) => Action::Command {
            admin: from_me
                || owner.is_some_and(|o| {
                    let o = bare(o);
                    o == bare(sender) || sender_alt.is_some_and(|a| o == bare(a))
                }),
        },
        // Alerta é só destino; sem papel é ignorado.
        Some(Role::Alerta) | None => Action::Drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    fn roles(entries: &[(&str, Role)]) -> ChatRoles {
        let m: HashMap<String, Role> = entries.iter().map(|(k, r)| (k.to_string(), *r)).collect();
        Arc::new(RwLock::new(m))
    }

    #[test]
    fn no_role_or_empty_drops() {
        assert_eq!(
            decide(&roles(&[]), "c", "s", None, true, Some("s")),
            Action::Drop
        );
        let r = roles(&[("outro", Role::Comando)]);
        assert_eq!(decide(&r, "c", "s", None, true, Some("s")), Action::Drop);
    }

    #[test]
    fn monitored_never_commands() {
        let r = roles(&[("c", Role::Monitorado)]);
        assert_eq!(decide(&r, "c", "s", None, true, Some("s")), Action::Monitor);
    }

    #[test]
    fn alert_drops() {
        let r = roles(&[("c", Role::Alerta)]);
        assert_eq!(decide(&r, "c", "s", None, true, Some("s")), Action::Drop);
    }

    #[test]
    fn command_admin_rules() {
        let r = roles(&[("c", Role::Comando)]);
        let adm = |alt, me, owner| decide(&r, "c", "s", alt, me, owner);
        assert_eq!(adm(None, false, None), Action::Command { admin: false });
        assert_eq!(
            adm(None, false, Some("x")),
            Action::Command { admin: false }
        );
        assert_eq!(adm(None, true, None), Action::Command { admin: true });
        assert_eq!(adm(None, false, Some("s")), Action::Command { admin: true });
        assert_eq!(
            adm(Some("a"), false, Some("a")),
            Action::Command { admin: true }
        );
    }

    #[test]
    fn owner_matches_ignoring_device_suffix() {
        let r = roles(&[("c", Role::Comando)]);
        let d = |sender, owner| decide(&r, "c", sender, None, false, Some(owner));
        assert_eq!(d("1:80@lid", "1@lid"), Action::Command { admin: true });
        assert_eq!(d("1@lid", "1:82@lid"), Action::Command { admin: true });
        assert_eq!(d("2:80@lid", "1@lid"), Action::Command { admin: false });
    }
}
