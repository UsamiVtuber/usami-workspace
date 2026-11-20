//! Módulo LoL Gamer Companion (`Usami.GG`).
//!
//! Toda a ingestão de dados ocorre de forma 100% passiva via Live Client Data API
//! (`https://127.0.0.1:2999/liveclientdata/allgamedata`) e APIs oficiais da Riot/DataDragon.

pub mod brain;
pub mod ddragon;
pub mod ipc;
pub mod live_client;
pub mod overlay_win32;
pub mod provider;
pub mod riot_cloud;
pub mod state_diff;
pub mod types;

use crate::api::UsamiApiClient;
use crate::lol::brain::{EventPriorityQueue, build_llm_prompt, parse_companion_reply};
use crate::lol::ddragon::DataDragonStore;
use crate::lol::ipc::{LolControlMsg, LolUiCommand, LolUiEndpoint, create_ipc_channels};
use crate::lol::live_client::{
    CircuitBreaker, LiveClientPoller, PollerState, ProbeResult, advance_state,
};
use crate::lol::provider::{GameProvider, NullProvider};
use crate::lol::riot_cloud::RiotCloudClient;
use crate::lol::state_diff::resolve_active_player;
use crate::lol::types::{LolEvent, LolExpression, PlayerRole};
use crate::ollama::OllamaClient;
use gpui_vrm::VrmController;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as TokioMutex, mpsc};

/// Restaura a trava de geração ao ser descartado, mesmo que a tarefa LLM
/// panique. Evita que o worker fique permanentemente travado e cesse de
/// disparar eventos de voz/expressão até reiniciar o app.
struct GeneratingGuard {
    flag: Arc<AtomicBool>,
}

impl Drop for GeneratingGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Relaxed);
    }
}

/// Canal global para comandos da UI vindos da thread Win32 (hotkey `Ctrl+Alt+L`).
///
/// Usa `std::sync::Mutex` (não `TokioMutex`) porque é acessado a partir de
/// `presence.rs::notify_hotkey_toggle_click_through()`, que roda na thread
/// Win32 síncrona (message loop da janela oculta). Essa thread **não** tem
/// runtime Tokio e não pode fazer `.await`.
///
/// O lock é mantido por tempo mínimo (só o `send` no canal unbounded) e não
/// há contenção real: a thread Win32 só envia esse comando esporadicamente,
/// enquanto o worker assíncrono só lê o canal na inicialização (`start()`).
static GLOBAL_UI_TX: OnceLock<Mutex<Option<mpsc::UnboundedSender<LolUiCommand>>>> = OnceLock::new();

pub struct LolCompanionHandle {
    pub ctrl_tx: mpsc::UnboundedSender<LolControlMsg>,
    shutdown: Arc<AtomicBool>,
}

impl LolCompanionHandle {
    pub fn send_control(&self, msg: LolControlMsg) {
        let _ = self.ctrl_tx.send(msg);
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// Chamado pela thread Win32 de `presence.rs` quando o usuário pressiona `Ctrl+Alt+L`.
pub fn notify_hotkey_toggle_click_through() {
    let slot = GLOBAL_UI_TX.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = slot.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(LolUiCommand::ToggleClickThrough);
        }
    }
}

/// Inicia o worker assíncrono do LoL Companion no runtime Tokio compartilhado
/// e devolve o endpoint de recebimento para a thread de UI do GPUI.
pub fn start(
    api_client: Arc<UsamiApiClient>,
    vrm_controller: VrmController,
) -> (LolCompanionHandle, LolUiEndpoint) {
    let (bridge, endpoint) = create_ipc_channels();
    let slot = GLOBAL_UI_TX.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(bridge.ui_tx.clone());
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let worker_shutdown = Arc::clone(&shutdown);
    let ctrl_tx = endpoint.ctrl_tx.clone();

    let provider: Arc<dyn GameProvider> = match LiveClientPoller::new() {
        Ok(poller) => Arc::new(poller),
        Err(err) => {
            eprintln!("[Usami.GG] Falha ao iniciar poller HTTP: {err}");
            Arc::new(NullProvider)
        }
    };

    crate::api::tokio_runtime().spawn(async move {
        run_worker(
            bridge,
            api_client,
            vrm_controller,
            worker_shutdown,
            provider,
        )
        .await;
    });

    (LolCompanionHandle { ctrl_tx, shutdown }, endpoint)
}

async fn run_worker(
    mut bridge: ipc::LolIpcBridge,
    api_client: Arc<UsamiApiClient>,
    vrm_controller: VrmController,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn GameProvider>,
) {
    let _ = bridge.ui_tx.send(LolUiCommand::PollerStatus(
        format!("Provedor {} carregando DataDragon...", provider.name()).into(),
    ));

    let mut ddragon = match DataDragonStore::load_or_fetch().await {
        Ok(store) => {
            let _ = bridge.ui_tx.send(LolUiCommand::PollerStatus(format!(
                "DataDragon v{} pronto",
                store.patch
            )));
            store
        }
        Err(err) => {
            eprintln!("[Usami.GG] DataDragon offline ({err}); usando conhecimento embutido");
            let _ = bridge.ui_tx.send(LolUiCommand::PollerStatus(
                "DataDragon offline (modo local)".into(),
            ));
            DataDragonStore::default()
        }
    };

    if !provider.is_enabled() {
        let _ = bridge
            .ui_tx
            .send(LolUiCommand::PollerStatus("Provedor desativado".into()));
    }

    let mut riot_cloud = RiotCloudClient::from_env();
    let mut state = PollerState::default();
    let mut queue = EventPriorityQueue::default();
    let mut enabled = true;
    let mut was_loading = false;
    let mut was_in_match = false;
    let mut active_champion = String::new();
    let mut active_role = PlayerRole::Unknown;
    let mut active_enemies: Vec<String> = Vec::new();

    /// Histórico de falas recentes para evitar repetição (máx. 5).
    ///
    /// Usa `TokioMutex` porque é acessado **apenas** do worker assíncrono
    /// (`run_worker` e tasks `tokio::spawn` filhas). Nunca é acessado de
    /// contexto síncrono. O lock é sempre liberado antes de qualquer `.await`
    /// (clone ou push/pop rápido), evitando lock convoy.
    let recent_replies = Arc::new(TokioMutex::new(Vec::<String>::new()));
    let is_generating = Arc::new(AtomicBool::new(false));
    let mut last_kda = (0, 0, 0);
    let mut last_cs = 0;
    let mut last_game_time = 0.0;
    let mut circuit_breaker = CircuitBreaker::new();
    let mut circuit_was_open = false;
    // Circuit breaker dedicado para o backend 8787 (LLM core).
    let core_circuit = Arc::new(Mutex::new(CircuitBreaker::new()));
    let mut core_circuit_was_open = false;

    while !shutdown.load(Ordering::SeqCst) {
        let _now = Instant::now();
        let delay = if enabled {
            state.next_poll_delay()
        } else {
            Duration::from_secs(3)
        };

        tokio::select! {
            Some(ctrl) = bridge.ctrl_rx.recv() => {
                match ctrl {
                    LolControlMsg::SetEnabled(flag) => {
                        enabled = flag;
                        if !enabled && (was_in_match || was_loading) {
                            was_in_match = false;
                            was_loading = false;
                            queue.clear();
                            state = PollerState::default();
                            let _ = bridge.ui_tx.send(LolUiCommand::MatchStateChanged {
                                in_match: false,
                                loading: false,
                                champion: None,
                                role: PlayerRole::Unknown,
                                patch: non_empty(&ddragon.patch),
                                game_time: 0.0,
                                kda: (0, 0, 0),
                                cs: 0,
                            });
                        }
                    }
                    LolControlMsg::SetRiotApiKey(key) => {
                        riot_cloud.set_api_key(key);
                    }
                    LolControlMsg::RefreshDataDragon => {
                        if let Ok(updated) = DataDragonStore::load_or_fetch().await {
                            ddragon = updated;
                        }
                    }
                    LolControlMsg::UserVoice(query) => {
                        queue.push(
                            LolEvent::UserVoiceQuery {
                                query,
                                champion: active_champion.clone(),
                                role: active_role,
                                game_time: last_game_time,
                            },
                            Instant::now(),
                        );
                    }
                    LolControlMsg::ManualHint(hint) => {
                        let prompt = format!(
                            "[MODO USAMI.GG]\nPedido manual do jogador: {hint}\n\
                            RESPONDA EM JSON: {{\"fala\": \"...\", \"expressao\": \"smug|angry|disappointed|surprised|neutral\", \"prioridade\": 3}}"
                        );
                        let client = Arc::clone(&api_client);
                        let ui = bridge.ui_tx.clone();
                        let replies = Arc::clone(&recent_replies);
                        let core_cb = Arc::clone(&core_circuit);
                        tokio::spawn(async move {
                            dispatch_to_llm(&client, &ui, &prompt, 3, LolExpression::Smug, "manual", &replies, core_cb).await;
                        });
                    }
                }
            }
            _ = tokio::time::sleep(delay) => {
                if !enabled {
                    continue;
                }

                if !provider.is_enabled() {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }

                let now = Instant::now();
                let circuit_open = !circuit_breaker.should_poll(now);
                if circuit_open && !circuit_was_open {
                    // Modo offline reativo: avisa o usuario uma unica vez e
                    // segue reagendo a eventos locais (PTT, teclas de atalho)
                    // sem tocar na rede ate o circuito fechar.
                    let _ = bridge.ui_tx.send(LolUiCommand::PollerStatus(
                        "LoL indisponivel (modo offline reativo)".into(),
                    ));
                    let _ = bridge.ui_tx.send(LolUiCommand::CompanionSpeak {
                        fala: "Hmph, o League nao esta respondendo. Fico por aqui, mas sem falar com o jogo por enquanto.".into(),
                        expressao: LolExpression::Disappointed,
                        prioridade: 4,
                        event_tag: "lol_offline".into(),
                    });
                    circuit_was_open = true;
                } else if !circuit_open {
                    circuit_was_open = false;
                }
                if circuit_open {
                    // Circuito aberto: pula o poll deste ciclo e espera o retry.
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                // Verifica circuit breaker do backend 8787 (LLM core).
                let core_circuit_open = {
                    let cb = core_circuit.lock().unwrap_or_else(|e| e.into_inner());
                    !cb.should_poll(now)
                };
                if core_circuit_open && !core_circuit_was_open {
                    let _ = bridge.ui_tx.send(LolUiCommand::PollerStatus(
                        "LLM core indisponivel (circuit breaker)".into(),
                    ));
                    let _ = bridge.ui_tx.send(LolUiCommand::CompanionSpeak {
                        fala: "O servico de fala nao esta respondendo. Vou continuar monitorando o jogo, mas sem comentar por enquanto.".into(),
                        expressao: LolExpression::Disappointed,
                        prioridade: 4,
                        event_tag: "core_offline".into(),
                    });
                    core_circuit_was_open = true;
                } else if !core_circuit_open {
                    core_circuit_was_open = false;
                }

                let probe = provider.poll().await;
                match &probe {
                    ProbeResult::Closed => circuit_breaker.record_failure(now),
                    _ => circuit_breaker.record_success(),
                }
                let (next_state, maybe_data) = advance_state(state, probe);
                state = next_state;

                let now = Instant::now();

                match &mut state {
                    PollerState::LoadingScreen { .. } => {
                        if !was_loading {
                            was_loading = true;
                            let _ = bridge.ui_tx.send(LolUiCommand::MatchStateChanged {
                                in_match: false,
                                loading: true,
                                champion: None,
                                role: PlayerRole::Unknown,
                                patch: non_empty(&ddragon.patch),
                                game_time: 0.0,
                                kda: (0, 0, 0),
                                cs: 0,
                            });
                            queue.push(LolEvent::MatchLoading { champion: None }, now);
                        }
                    }
                    PollerState::InMatch { snapshot, .. } => {
                        was_loading = false;
                        was_in_match = true;

                        if let Some(ref data) = maybe_data {
                            let events = snapshot.diff(data);
                            active_role = snapshot.role;
                            if let Some(me) = resolve_active_player(data) {
                                active_champion = me.champion_name.clone();
                                last_kda = (me.scores.kills, me.scores.deaths, me.scores.assists);
                                last_cs = me.scores.creep_score;
                                let new_enemies: Vec<String> = data
                                    .all_players
                                    .iter()
                                    .filter(|p| !p.team.is_empty() && p.team != me.team)
                                    .map(|p| p.champion_name.clone())
                                    .filter(|n| !n.is_empty())
                                    .collect();
                                // Só realoca o vetor de inimigos quando a composição do time muda;
                                // na maioria dos ticks o vetor anterior é reutilizado.
                                if new_enemies != active_enemies {
                                    active_enemies = new_enemies;
                                }
                            }
                            last_game_time = data.game_data.game_time;

                            let _ = bridge.ui_tx.send(LolUiCommand::MatchStateChanged {
                                in_match: true,
                                loading: false,
                                champion: non_empty(&active_champion),
                                role: active_role,
                                patch: non_empty(&ddragon.patch),
                                game_time: last_game_time,
                                kda: last_kda,
                                cs: last_cs,
                            });

                            for ev in events {
                                queue.push(ev, now);
                            }
                        }
                    }
                    PollerState::MatchEnded => {
                        if was_in_match {
                            let end_ev = LolEvent::MatchEnded {
                                champion: active_champion.clone(),
                                role: active_role,
                                duration_secs: last_game_time,
                                kills: last_kda.0,
                                deaths: last_kda.1,
                                assists: last_kda.2,
                            };
                            let recent_snapshot = recent_replies.lock().await.clone();
                            let prompt = build_llm_prompt(&end_ev, &ddragon, &active_champion, active_role, &active_enemies, &recent_snapshot);
                            let client = Arc::clone(&api_client);
                            let ui = bridge.ui_tx.clone();
                            let replies = Arc::clone(&recent_replies);
                            let core_cb = Arc::clone(&core_circuit);
                            tokio::spawn(async move {
                                dispatch_to_llm(&client, &ui, &prompt, 4, LolExpression::Smug, "match_ended", &replies, core_cb).await;
                            });
                        }
                        was_in_match = false;
                        was_loading = false;
                        queue.clear();
                        active_champion.clear();
                        active_enemies.clear();
                        recent_replies.lock().await.clear();
                        active_role = PlayerRole::Unknown;
                        let _ = bridge.ui_tx.send(LolUiCommand::MatchStateChanged {
                            in_match: false,
                            loading: false,
                            champion: None,
                            role: PlayerRole::Unknown,
                            patch: non_empty(&ddragon.patch),
                            game_time: 0.0,
                            kda: (0, 0, 0),
                            cs: 0,
                        });
                    }
                    PollerState::GameClosed { .. } => {
                        if was_loading || was_in_match {
                            was_loading = false;
                            was_in_match = false;
                            queue.clear();
                            recent_replies.lock().await.clear();
                            active_role = PlayerRole::Unknown;
                            let _ = bridge.ui_tx.send(LolUiCommand::MatchStateChanged {
                                in_match: false,
                                loading: false,
                                champion: None,
                                role: PlayerRole::Unknown,
                                patch: non_empty(&ddragon.patch),
                                game_time: 0.0,
                                kda: (0, 0, 0),
                                cs: 0,
                            });
                        }
                    }
                }

                // Processa no máximo 1 evento por ciclo respeitando voz ativa, cooldown e geração
                if !is_generating.load(Ordering::Relaxed) {
                    if let Some(queued) = queue.pop_ready(now, vrm_controller.is_speaking()) {
                        let tag = queued.event.cooldown_bucket().to_string();
                        let fallback_expr = match &queued.event {
                            LolEvent::UserVoiceQuery { .. } => LolExpression::Neutral,
                            LolEvent::PlayerDied { .. } => LolExpression::Disappointed,
                            LolEvent::ChampionKill { .. } | LolEvent::BaronKilled { .. } => LolExpression::Smug,
                            LolEvent::FirstBlood { .. } => LolExpression::Surprised,
                            _ => LolExpression::Neutral,
                        };
                        let recent_snapshot = recent_replies.lock().await.clone();
                        let prompt = build_llm_prompt(&queued.event, &ddragon, &active_champion, active_role, &active_enemies, &recent_snapshot);
                        let client = Arc::clone(&api_client);
                        let ui = bridge.ui_tx.clone();
                        let replies = Arc::clone(&recent_replies);
                        let core_cb = Arc::clone(&core_circuit);
                        is_generating.store(true, Ordering::Relaxed);
                        let guard = GeneratingGuard { flag: Arc::clone(&is_generating) };
                        tokio::spawn(async move {
                            let _guard = guard;
                            dispatch_to_llm(&client, &ui, &prompt, queued.priority, fallback_expr, &tag, &replies, core_cb).await;
                        });
                    }
                }
            }
        }
    }
}

async fn dispatch_to_llm(
    api_client: &UsamiApiClient,
    ui_tx: &mpsc::UnboundedSender<LolUiCommand>,
    prompt: &str,
    fallback_priority: u8,
    fallback_expr: LolExpression,
    event_tag: &str,
    recent_replies: &Arc<TokioMutex<Vec<String>>>,
    core_circuit: Arc<Mutex<CircuitBreaker>>,
) {
    // Tenta o Ollama local primeiro quando USAMI_LLM_SOURCE=ollama, para
    // poupar as APIs pagas (OpenRouter/NVIDIA) do backend 8787.
    let ollama_text = if std::env::var("USAMI_LLM_SOURCE").ok().as_deref() == Some("ollama") {
        let client = OllamaClient::default();
        match tokio::time::timeout(Duration::from_secs(30), client.chat(prompt)).await {
            Ok(Ok(text)) if !text.trim().is_empty() => Some(text),
            Ok(Ok(_)) => None,
            Ok(Err(err)) => {
                eprintln!("[Usami.GG] Ollama falhou ({err}); tentando backend 8787.");
                None
            }
            Err(_) => {
                eprintln!("[Usami.GG] Timeout (30s) aguardando resposta do Ollama.");
                None
            }
        }
    } else {
        None
    };

    if let Some(text) = ollama_text {
        let reply = parse_companion_reply(&text, fallback_priority, fallback_expr);
        if reply.fala.trim().is_empty() {
            return;
        }
        let mut lock = recent_replies.lock().await;
        if !lock.contains(&reply.fala) {
            lock.push(reply.fala.clone());
            if lock.len() > 5 {
                lock.remove(0);
            }
        }
        let _ = ui_tx.send(LolUiCommand::CompanionSpeak {
            fala: reply.fala,
            expressao: reply.expressao,
            prioridade: reply.prioridade,
            event_tag: event_tag.to_string(),
        });
        return;
    }

    // Verifica o circuit breaker do backend 8787 antes de chamar.
    let now = Instant::now();
    let should_call = {
        let cb = core_circuit.lock().unwrap_or_else(|e| e.into_inner());
        cb.should_poll(now)
    };
    if !should_call {
        eprintln!(
            "[Usami.GG] Circuit breaker 8787 aberto; pulando chamada LLM para {}",
            event_tag
        );
        return;
    }

    // Fallback para o backend 8787 (OpenRouter/NVIDIA).
    let call_fut = api_client.send_chat_with_options(prompt, false, true);
    match tokio::time::timeout(Duration::from_secs(8), call_fut).await {
        Ok(Ok(res)) => {
            // Sucesso: fecha o circuito.
            {
                let mut cb = core_circuit.lock().unwrap_or_else(|e| e.into_inner());
                cb.record_success();
            }
            let reply = parse_companion_reply(&res.reply, fallback_priority, fallback_expr);
            if reply.fala.trim().is_empty() {
                // Silenciado (erro operacional ou resposta vazia filtrada para economizar TTS)
                return;
            }
            let mut lock = recent_replies.lock().await;
            if !lock.contains(&reply.fala) {
                lock.push(reply.fala.clone());
                if lock.len() > 5 {
                    lock.remove(0);
                }
            }
            let _ = ui_tx.send(LolUiCommand::CompanionSpeak {
                fala: reply.fala,
                expressao: reply.expressao,
                prioridade: reply.prioridade,
                event_tag: event_tag.to_string(),
            });
        }
        Ok(Err(err)) => {
            // Erro da API: registra falha no circuit breaker.
            {
                let mut cb = core_circuit.lock().unwrap_or_else(|e| e.into_inner());
                cb.record_failure(now);
            }
            eprintln!(
                "[Usami.GG] Falha ao consultar core 8787 para {}: {}",
                event_tag, err
            );
        }
        Err(_) => {
            // Timeout: registra falha no circuit breaker.
            {
                let mut cb = core_circuit.lock().unwrap_or_else(|e| e.into_inner());
                cb.record_failure(now);
            }
            eprintln!(
                "[Usami.GG] Timeout (8s) aguardando resposta da LLM para {}",
                event_tag
            );
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
