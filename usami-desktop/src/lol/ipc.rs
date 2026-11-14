//! Canais IPC assíncronos entre o backend Tokio (LoL Companion) e a thread UI do GPUI.

use crate::lol::types::{LolExpression, PlayerRole};
use tokio::sync::mpsc;

/// Comandos emitidos pelo backend Rust (Tokio) para o frontend GPUI (`UsamiApp`).
#[derive(Debug, Clone)]
pub enum LolUiCommand {
    /// Atualiza estado de partida e dispara troca automática de modo overlay.
    MatchStateChanged {
        in_match: bool,
        loading: bool,
        champion: Option<String>,
        role: PlayerRole,
        patch: Option<String>,
        game_time: f64,
        kda: (i32, i32, i32),
        cs: i32,
    },
    /// Entrega uma fala estruturada da Usami pronta para legenda, VRM e TTS.
    CompanionSpeak {
        fala: String,
        expressao: LolExpression,
        prioridade: u8,
        event_tag: String,
    },
    /// Atualiza o texto de status técnico do poller/DataDragon no overlay.
    PollerStatus(String),
    /// Solicita alternância do modo click-through (ex.: via atalho global Ctrl+Alt+L).
    ToggleClickThrough,
}

/// Mensagens enviadas da UI do GPUI para o worker Tokio.
#[derive(Debug, Clone)]
pub enum LolControlMsg {
    SetEnabled(bool),
    SetRiotApiKey(Option<String>),
    RefreshDataDragon,
    ManualHint(String),
    UserVoice(String),
}

pub struct LolIpcBridge {
    pub ui_tx: mpsc::UnboundedSender<LolUiCommand>,
    pub ctrl_rx: mpsc::UnboundedReceiver<LolControlMsg>,
}

pub struct LolUiEndpoint {
    pub ui_rx: mpsc::UnboundedReceiver<LolUiCommand>,
    pub ctrl_tx: mpsc::UnboundedSender<LolControlMsg>,
}

pub fn create_ipc_channels() -> (LolIpcBridge, LolUiEndpoint) {
    let (ui_tx, ui_rx) = mpsc::unbounded_channel();
    let (ctrl_tx, ctrl_rx) = mpsc::unbounded_channel();
    (
        LolIpcBridge { ui_tx, ctrl_rx },
        LolUiEndpoint { ui_rx, ctrl_tx },
    )
}

