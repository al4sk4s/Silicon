use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Papel de um chat na whitelist (P2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Comando,
    Monitorado,
    Alerta,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Comando => "comando",
            Role::Monitorado => "monitorado",
            Role::Alerta => "alerta",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "comando" => Some(Role::Comando),
            "monitorado" => Some(Role::Monitorado),
            "alerta" => Some(Role::Alerta),
            _ => None,
        }
    }
}

/// Cache da tabela `roles` (jid -> papel).
pub type ChatRoles = Arc<RwLock<HashMap<String, Role>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for r in [Role::Comando, Role::Monitorado, Role::Alerta] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(Role::parse("x"), None);
    }
}
