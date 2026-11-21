//! Abstração de provedor de dados de jogo.
//!
//! Permite trocar a fonte (Live Client API do LoL, outro jogo ou um
//! provedor nulo) sem tocar no worker do LoL Companion. O state diff
//! continua no worker via `GameSnapshot`; este trait expõe apenas a sonda
//! de dados brutos.

use std::future::Future;
use std::pin::Pin;

use crate::lol::live_client::ProbeResult;

/// Fonte de dados de jogo. Implementado pelo Live Client API do LoL hoje;
/// pode ser estendido para Valorant, CS2, etc., ou um `NullProvider` para
/// desativar completamente o módulo LoL.
pub trait GameProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Consulta a fonte de dados e retorna o resultado da sonda.
    fn poll(&self) -> Pin<Box<dyn Future<Output = ProbeResult> + Send + '_>>;

    /// Se falso, o worker ignora este provedor.
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Estado de disponibilidade de um conector de jogo no catálogo No-Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameConnectorAvailability {
    Ready,
    Planned,
}

/// Metadados de um conector de jogo exibidos no painel No-Code de Integrações.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameConnectorDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub availability: GameConnectorAvailability,
}

/// Catálogo oficial de conectores de jogos suportados e planejados na arquitetura
/// `GameProvider` (inspirada em conectores plugáveis como o projeto AIRI).
pub fn default_game_connectors() -> &'static [GameConnectorDescriptor] {
    &[
        GameConnectorDescriptor {
            id: "lol",
            title: "League of Legends (Usami.GG)",
            subtitle: "Leitura passiva via Live Client Data API local, DataDragon e overlay reativo.",
            availability: GameConnectorAvailability::Ready,
        },
        GameConnectorDescriptor {
            id: "minecraft",
            title: "Minecraft",
            subtitle: "Ponte local de eventos e chat do servidor (arquitetura GameProvider).",
            availability: GameConnectorAvailability::Planned,
        },
        GameConnectorDescriptor {
            id: "factorio",
            title: "Factorio",
            subtitle: "Telemetria de fábrica e alertas reativos via RCON/mod local.",
            availability: GameConnectorAvailability::Planned,
        },
    ]
}

/// Provedor nulo: nunca retorna dados ativos. Usado para desligar o módulo
/// LoL sem desmontar o worker nem os canais IPC.
pub struct NullProvider;

impl GameProvider for NullProvider {
    fn name(&self) -> &'static str {
        "null"
    }

    fn poll(&self) -> Pin<Box<dyn Future<Output = ProbeResult> + Send>> {
        Box::pin(async { ProbeResult::Closed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_game_connectors_lists_lol_ready_and_planned_entries() {
        let catalog = default_game_connectors();
        assert!(catalog.len() >= 3);
        assert_eq!(catalog[0].id, "lol");
        assert_eq!(catalog[0].availability, GameConnectorAvailability::Ready);
        assert!(catalog.iter().any(|c| c.id == "minecraft"));
        assert!(catalog.iter().any(|c| c.id == "factorio"));
    }
}
