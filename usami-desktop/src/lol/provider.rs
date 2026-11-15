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
