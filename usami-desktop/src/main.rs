pub mod agent_center_model;
pub mod api;
mod assets;
pub mod audio;
mod media;
mod model;
#[cfg(target_os = "windows")]
mod presence;
pub mod recorder;
mod storage;
mod speech;
mod theme;
mod ui;
mod text_input;

use crate::agent_center_model::{
    AgentCenterTab, ExecFilter, ExecutionTone, SkillFilter, classify_execution_status,
    collect_skill_reasons, format_duration_ms, format_execution_time, format_relevance_pct,
    group_skills_by_state, group_tools_by_namespace, is_local_quiet_hours_now, origin_label,
    pending_confirmations_count, pending_suggestions_count, permission_level_label, state_label,
    suggestion_kind_label, suggestion_status_label, summarize_executions,
};
use crate::assets::{asset_base_dir, asset_path, env_path, first_existing};
use crate::api::UsamiApiClient;
use crate::audio::AudioPlayer;
use crate::media::AttachedMedia;
use crate::model::{chat_message_from_stored, current_phone_time, format_tool_badge, stored_turns, ActiveDrawer, BondTab, ChatMessage, Sender, SettingsTab};
use crate::ui::widgets::{icon, settings_code, settings_danger_button, settings_dl_row, settings_h3, settings_hint, settings_list_item, settings_p};
use crate::speech::{is_duplicate_utterance, sanitize_speech_text};
use crate::text_input::{resolve_key_char_with_dead_key, tokenize_text_spans};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_vrm::{VrmController, VrmView};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn window_minimized_or_hidden() -> bool {
    #[cfg(target_os = "windows")]
    {
        presence::app_is_minimized_or_hidden()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

const CLOSE_FAREWELL_MIN: Duration = Duration::from_millis(1200);
const CLOSE_FAREWELL_MAX: Duration = Duration::from_secs(10);

/// A janela só sai depois da despedida: mínimo legível, áudio terminado e avatar
/// sem falar — ou o teto rígido, para nunca travar o fechamento.
fn farewell_can_close(elapsed: Duration, audio_done: bool, speaking: bool) -> bool {
    (elapsed >= CLOSE_FAREWELL_MIN && audio_done && !speaking) || elapsed >= CLOSE_FAREWELL_MAX
}

struct UsamiApp {
    focus_handle: FocusHandle,
    input_text: String,
    cursor_char_idx: usize,
    selection_anchor: Option<usize>,
    is_mouse_selecting: bool,
    drag_start_idx: usize,
    pending_dead_key: Option<char>,
    input_active: bool,
    caret_on: bool,
    status_text: SharedString,
    last_reply: SharedString,
    last_emotion: SharedString,
    show_bubble: bool,
    is_busy: bool,
    /// Atividade ao vivo do turno em andamento (`GET /api/chat/activity`):
    /// o que a Usami está fazendo enquanto a resposta não chegou.
    /// `thinking` = modelo raciocinando, `tool` = ferramenta em execução
    /// (`active_tool` traz o nome), `responding` = compondo a fala.
    chat_activity: Option<crate::api::ChatActivity>,
    active_tool: Option<String>,
    /// Trava de polling da atividade: uma leitura por vez, sem empilhar.
    activity_loading: bool,
    is_calling: bool,
    call_started_at: Option<Instant>,
    call_seconds: u64,
    is_listening: bool,
    listener: Option<recorder::VoiceListener>,
    mic_gate: Arc<recorder::MicGate>,
    is_companion_mode: bool,
    sleep_companion_started_at: Option<Instant>,
    sleep_companion_minutes_left: u32,
    is_muted: bool,
    is_ui_idle: bool,
    last_interaction: Instant,
    is_away: bool,
    away_since: Option<Instant>,
    window_focused: bool,
    last_user_activity: Instant,
    idle_prods: u8,
    last_return_greeting: Instant,
    was_maximized: bool,
    last_maximize_line: Instant,
    /// Despedida ao fechar: a primeira tentativa segura a janela, a segunda
    /// libera; o tick fecha sozinho quando o áudio acaba (ou estoura o teto).
    farewell_started_at: Option<Instant>,
    farewell_audio_done: Arc<AtomicBool>,
    close_allowed: bool,
    prefs: storage::UiPrefs,
    session: storage::SessionFile,
    last_prefs_save: Instant,
    window_subscriptions: Vec<Subscription>,
    close_subscription: Option<Subscription>,
    active_drawer: ActiveDrawer,
    /// Rolagem do chat do smartphone; abrir o painel salta para a última mensagem.
    phone_scroll: ScrollHandle,
    settings_advanced_open: bool,
    bond_tab: BondTab,
    chat_history: Vec<ChatMessage>,
    /// Conquistas avaliadas pelo core (`GET /api/achievements`); lista vazia
    /// até a primeira resposta.
    achievements: Vec<api::Achievement>,

    /// Estado do vínculo vindo do core (`GET /api/bond`); None até a primeira
    /// resposta. `bond_level`/`bond_exp_pct` acompanham o summary para o pill.
    bond: Option<api::BondData>,
    /// Monitor de humor contínuo vindo do core (`GET /api/mood` / `/api/chat`).
    mood: Option<api::MoodData>,
    mood_loading: bool,
    prev_portrait_image: Arc<Mutex<Option<Arc<RenderImage>>>>,
    bond_level: u32,
    bond_exp_pct: u32,
    bg_asset_path: Option<PathBuf>,
    api_client: Arc<UsamiApiClient>,
    audio_player: Option<Arc<AudioPlayer>>,
    vrm_controller: VrmController,
    vrm_view: Option<Entity<VrmView>>,
    last_source: Option<SharedString>,
    last_timing: Option<api::ChatTiming>,
    server_affinity: Option<i32>,
    server_stage: Option<SharedString>,
    last_agent_turn: Option<api::AgentTurnSummary>,
    memory_data: Option<api::MemoryData>,
    memory_loading: bool,
    memory_error: Option<SharedString>,
    memory_confirm_active: bool,
    memory_status: Option<SharedString>,
    personality_data: Option<api::PersonalityData>,
    personality_loading: bool,
    personality_error: Option<SharedString>,
    personality_status: Option<SharedString>,
    proactive_enabled: bool,
    proactive_seen: std::collections::HashSet<String>,
    proactive_queue: Vec<api::ProactiveEvent>,
    last_proactive_speech: Instant,
    last_voice_ended: Instant,
    last_voice_commit: Option<(String, Instant)>,
    /// Prévia do STT enquanto a pessoa ainda fala (fatias parciais).
    live_transcript: SharedString,
    /// Identifica a sessão de escuta atual; prévias atrasadas de uma sessão
    /// anterior não podem reaparecer depois de parar ou reiniciar o microfone.
    listen_session: u64,
    /// Último turno cujo texto final já foi aceito; prévias desse turno ou de
    /// turnos anteriores são descartadas ao chegar atrasadas.
    voice_turn_closed: u64,
    attached_media: Option<AttachedMedia>,
    agent_tab: AgentCenterTab,
    agent_skill_filter: SkillFilter,
    agent_exec_filter: ExecFilter,
    agent_skills: Vec<api::AgentSkill>,
    agent_skills_loaded: bool,
    agent_skills_loading: bool,
    agent_skills_error: Option<SharedString>,
    agent_executions: Vec<api::AgentExecution>,
    agent_executions_loaded: bool,
    agent_executions_loading: bool,
    agent_executions_error: Option<SharedString>,
    agent_skill_busy_id: Option<String>,
    agent_action_notice: Option<SharedString>,
    agent_action_error: Option<SharedString>,
    agent_tools: Vec<api::SelfToolSummary>,
    agent_permissions: Vec<api::PermissionRevocation>,
    agent_permissions_loaded: bool,
    agent_permissions_loading: bool,
    agent_permissions_error: Option<SharedString>,
    agent_permission_busy: Option<String>,
    agent_proposals: Vec<api::SkillProposal>,
    agent_plans: Vec<api::AgentPlan>,
    agent_confirm_armed: bool,
    agent_confirm_busy: Option<String>,
    agent_suggestions: Vec<api::AgentSuggestion>,
    agent_suggestions_unanswered: usize,
    agent_suggestions_loaded: bool,
    agent_suggestions_loading: bool,
    agent_suggestions_error: Option<SharedString>,
    agent_suggestion_busy: Option<String>,
    agent_briefing: Option<api::DailyBriefingData>,
    agent_briefing_loading: bool,
    agent_briefing_error: Option<SharedString>,
    #[cfg(target_os = "windows")]
    presence_subscription: Option<Subscription>,
}

impl Drop for UsamiApp {
    fn drop(&mut self) {
        // Mantém as assinaturas de janela vivas até o app encerrar.
        self.close_subscription.take();
        #[cfg(target_os = "windows")]
        self.presence_subscription.take();
        // A gravação no fechamento captura a geometria final (o observador de
        // bounds economiza escrita enquanto a janela é arrastada).
        if self.prefs.window.is_some() {
            self.prefs.muted = self.is_muted;
            storage::save_prefs(&self.prefs);
        }
    }
}

impl UsamiApp {
    fn new(cx: &mut Context<Self>, prefs: storage::UiPrefs) -> Self {
        let player = match AudioPlayer::try_new() {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                eprintln!("[Usami Audio] Dispositivo de som indisponível: {:?}", e);
                None
            }
        };

        let bg_asset_path = first_existing([
            env_path("USAMI_BACKGROUND"),
            Some(asset_path("assets/background.png")),
            Some(asset_path("../usami/public/background.png")),
        ]);

        let vrm_path = first_existing([
            env_path("USAMI_VRM_PATH"),
            Some(asset_path("../usami/public/models/shiro.vrm")),
            Some(asset_path("assets/shiro.vrm")),
        ]);

        let vrma_dir = first_existing([
            env_path("USAMI_VRMA_DIR"),
            Some(asset_path("../usami/public/animations/vrma")),
            Some(asset_path("assets/vrma")),
        ]);

        let vrm_controller = VrmController::new();
        vrm_controller.set_emotion_with_options("tsundere", 4.5, false);

        // A escuta segue a boca do avatar: 100 ms entre o TTS começar e o
        // microfone fechar — bem abaixo dos 120 ms que abrem uma fala na VAD,
        // então a própria voz da Usami nunca vira transcrição.
        let mic_gate = Arc::new(recorder::MicGate::new());
        let gate = Arc::clone(&mic_gate);
        let speaking_controller = vrm_controller.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let mut was_speaking = false;
                loop {
                    let speaking = speaking_controller.is_speaking();
                    gate.set_agent(speaking);
                    if was_speaking && !speaking {
                        let _ = cx.update(|cx: &mut App| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |view, _cx| {
                                    // O relógio de silêncio começa quando a VOZ termina (OpenAmika 11c7c4e).
                                    view.last_voice_ended = Instant::now();
                                });
                            }
                        });
                    }
                    was_speaking = speaking;
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                }
            }
        })
        .detach();

        let vrm_view =
            vrm_path.and_then(
                |path| match VrmView::new(&path, vrma_dir, vrm_controller.clone()) {
                    Ok(v) => Some(cx.new(|_cx| v)),
                    Err(e) => {
                        eprintln!("[gpui-vrm] Falha ao inicializar VrmView: {:?}", e);
                        None
                    }
                },
            );

        // O marcador de sessão precisa ser lido antes de gravar o novo: só assim
        // a abertura seguinte sabe se o processo anterior morreu sem fechar.
        let previous_session = storage::load_session();
        let session = storage::SessionFile::start();
        storage::save_session(&session);
        let abrupt_previous = previous_session.as_ref().is_some_and(|previous| {
            !previous.clean_shutdown
                && storage::now_ms().saturating_sub(previous.last_seen_ms) < 24 * 60 * 60 * 1000
        });

        let mut chat_history: Vec<ChatMessage> = storage::load_conversation()
            .into_iter()
            .filter_map(chat_message_from_stored)
            .collect();
        if chat_history.is_empty() {
            chat_history.push(ChatMessage {
                sender: Sender::Usami,
                text:
                    "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!"
                        .into(),
                emotion: Some("tsundere".into()),
                tool_badge: None,
                time_str: Some(current_phone_time()),
                media: None,
            });
        }
        if abrupt_previous {
            chat_history.push(ChatMessage {
                sender: Sender::Usami,
                text: "Você fechou o app sem se despedir da última vez... eu fiquei aqui esperando, sabia?".into(),
                emotion: Some("tsundere".into()),
                tool_badge: None,
                time_str: Some(current_phone_time()),
                media: None,
            });
            storage::save_conversation(&stored_turns(&chat_history));
        }
        let last_assistant = chat_history
            .iter()
            .rev()
            .find(|message| matches!(message.sender, Sender::Usami))
            .cloned();
        let last_reply: SharedString = last_assistant
            .as_ref()
            .map(|message| message.text.clone())
            .unwrap_or_else(|| {
                "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!"
                    .into()
            })
            .into();
        let last_emotion: SharedString = last_assistant
            .and_then(|message| message.emotion)
            .unwrap_or_else(|| "tsundere".into())
            .into();
        let close_subscription = Some(cx.on_window_closed(|_| storage::mark_session_clean()));

        // Loop em background para detectar inatividade e suavizar a UI (ui-idle)
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let mut tick_counter: u64 = 0;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    tick_counter = tick_counter.wrapping_add(1);
                    let should_poll = tick_counter % 30 == 1; // a cada ~15s (e no primeiro tick)
                    let should_present = tick_counter % 5 == 0; // a cada ~2.5s
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                let mut changed = false;
                                if let Some(started) = view.call_started_at {
                                    let seconds = started.elapsed().as_secs();
                                    if view.call_seconds != seconds {
                                        view.call_seconds = seconds;
                                        changed = true;
                                    }
                                }
                                if let Some(started) = view.sleep_companion_started_at {
                                    let elapsed = started.elapsed();
                                    let remaining =
                                        30_u32.saturating_sub((elapsed.as_secs() / 60) as u32);
                                    if view.sleep_companion_minutes_left != remaining {
                                        view.sleep_companion_minutes_left = remaining;
                                        changed = true;
                                    }
                                    if elapsed >= Duration::from_secs(30 * 60) {
                                        view.stop_sleep_companion();
                                        changed = true;
                                    }
                                }
                                let has_drawer = view.active_drawer != ActiveDrawer::None;
                                let is_idle = !has_drawer
                                    && view.last_interaction.elapsed()
                                        > Duration::from_millis(4500);
                                if view.is_ui_idle != is_idle {
                                    view.is_ui_idle = is_idle;
                                    view.vrm_controller.set_portrait_enabled(!is_idle);
                                    changed = true;
                                }
                                // Caret pisca enquanto o composer está ativo (o mesmo
                                // timer já redesenha a UI, então não há custo extra).
                                if view.input_active {
                                    view.caret_on = !view.caret_on;
                                    changed = true;
                                }
                                if changed {
                                    cx.notify();
                                }

                                // Minimizar/esconder não gera evento de foco em
                                // todo caso, então o estado ausente é reavaliado
                                // no pulso do timer.
                                view.refresh_away_state(cx);
                                view.tick_close_request(cx);
                                view.maybe_run_idle_prods(cx);
                                if view.session.touch() {
                                    storage::save_session(&view.session);
                                }

                                if should_poll {
                                    view.poll_proactive_events(cx);
                                }
                                if should_present {
                                    view.present_next_proactive(cx);
                                }
                                // Atividade do turno: enquanto há resposta pendente,
                                // a cada ~1 s lemos o que a Usami está fazendo
                                // (pensando / executando qual ferramenta).
                                if view.is_busy && tick_counter % 2 == 0 {
                                    view.fetch_chat_activity(cx);
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();

        vrm_controller.set_portrait_enabled(true);

        Self {
            focus_handle: cx.focus_handle(),
            input_text: String::new(),
            cursor_char_idx: 0,
            selection_anchor: None,
            is_mouse_selecting: false,
            drag_start_idx: 0,
            pending_dead_key: None,
            input_active: false,
            caret_on: true,
            status_text: "COM VOCÊ".into(),
            last_reply,
            last_emotion,
            show_bubble: true,
            is_busy: false,
            chat_activity: None,
            active_tool: None,
            activity_loading: false,
            is_calling: false,
            call_started_at: None,
            call_seconds: 0,
            is_listening: false,
            listener: None,
            mic_gate,
            is_companion_mode: false,
            sleep_companion_started_at: None,
            sleep_companion_minutes_left: 30,
            is_muted: prefs.muted,
            is_ui_idle: false,
            last_interaction: Instant::now(),
            is_away: false,
            away_since: None,
            window_focused: true,
            last_user_activity: Instant::now(),
            idle_prods: 0,
            last_return_greeting: Instant::now() - Duration::from_secs(300),
            was_maximized: false,
            last_maximize_line: Instant::now() - Duration::from_secs(900),
            farewell_started_at: None,
            farewell_audio_done: Arc::new(AtomicBool::new(false)),
            close_allowed: false,
            prefs,
            session,
            last_prefs_save: Instant::now() - Duration::from_secs(10),
            window_subscriptions: Vec::new(),
            close_subscription,
            active_drawer: ActiveDrawer::None,
            phone_scroll: ScrollHandle::new(),
            settings_advanced_open: false,
            bond_tab: BondTab::Home,
            chat_history,
            bond: None,
            mood: None,
            mood_loading: false,
            prev_portrait_image: Arc::new(Mutex::new(None)),
            achievements: Vec::new(),
            bond_level: 1,
            bond_exp_pct: 0,
            bg_asset_path,
            api_client: Arc::new(UsamiApiClient::from_env()),
            audio_player: player,
            vrm_controller,
            vrm_view,
            last_source: None,
            last_timing: None,
            server_affinity: None,
            server_stage: None,
            last_agent_turn: None,
            memory_data: None,
            memory_loading: false,
            memory_error: None,
            memory_confirm_active: false,
            memory_status: None,
            personality_data: None,
            personality_loading: false,
            personality_error: None,
            personality_status: None,
            proactive_enabled: storage::load_proactive_config(),
            proactive_seen: storage::load_proactive_seen(),
            proactive_queue: Vec::new(),
            last_proactive_speech: Instant::now() - Duration::from_secs(300),
            last_voice_ended: Instant::now() - Duration::from_secs(300),
            last_voice_commit: None,
            live_transcript: SharedString::default(),
            listen_session: 0,
            voice_turn_closed: 0,
            attached_media: None,
            agent_tab: AgentCenterTab::Skills,
            agent_skill_filter: SkillFilter::All,
            agent_exec_filter: ExecFilter::All,
            agent_skills: Vec::new(),
            agent_skills_loaded: false,
            agent_skills_loading: false,
            agent_skills_error: None,
            agent_executions: Vec::new(),
            agent_executions_loaded: false,
            agent_executions_loading: false,
            agent_executions_error: None,
            agent_skill_busy_id: None,
            agent_action_notice: None,
            agent_action_error: None,
            agent_tools: Vec::new(),
            agent_permissions: Vec::new(),
            agent_permissions_loaded: false,
            agent_permissions_loading: false,
            agent_permissions_error: None,
            agent_permission_busy: None,
            agent_proposals: Vec::new(),
            agent_plans: Vec::new(),
            agent_confirm_armed: false,
            agent_confirm_busy: None,
            agent_suggestions: Vec::new(),
            agent_suggestions_unanswered: 0,
            agent_suggestions_loaded: false,
            agent_suggestions_loading: false,
            agent_suggestions_error: None,
            agent_suggestion_busy: None,
            agent_briefing: None,
            agent_briefing_loading: false,
            agent_briefing_error: None,
            #[cfg(target_os = "windows")]
            presence_subscription: None,
        }
    }

    /// Sobe bandeja, atalho global e toggles. O HWND só existe depois de
    /// `open_window`, então isto roda uma vez na abertura da janela.
    #[cfg(target_os = "windows")]
    fn attach_presence(&mut self, window: &Window, cx: &mut Context<Self>) {
        presence::start(window);
        self.presence_subscription = Some(cx.on_window_closed(|_| presence::shutdown()));
    }

    fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_interaction = Instant::now();
        if self.is_ui_idle {
            self.is_ui_idle = false;
            self.vrm_controller.set_portrait_enabled(true);
            cx.notify();
        }
    }

    /// Atividade causada pelo usuário: além do relógio da UI, reinicia a
    /// contagem de provocações por inatividade.
    fn touch_ui(&mut self, cx: &mut Context<Self>) {
        self.last_user_activity = Instant::now();
        self.idle_prods = 0;
        self.poke(cx);
    }

    fn push_history(&mut self, mut message: ChatMessage) {
        if message.time_str.is_none() {
            message.time_str = Some(current_phone_time());
        }
        self.chat_history.push(message);
        if self.chat_history.len() > storage::MAX_STORED_TURNS {
            let overflow = self.chat_history.len() - storage::MAX_STORED_TURNS;
            self.chat_history.drain(0..overflow);
        }
        self.persist_history();
        // Segue a última mensagem como no WhatsApp, mas sem arrancar o painel
        // de quem está lendo o histórico acima.
        if self.active_drawer == ActiveDrawer::Chat {
            let max = self.phone_scroll.max_offset().height;
            let at_bottom = -self.phone_scroll.offset().y >= max - px(32.0);
            if at_bottom {
                self.phone_scroll.scroll_to_bottom();
            }
        }
    }

    fn cursor_idx(&self) -> usize {
        self.cursor_char_idx.min(self.input_text.chars().count())
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let cursor = self.cursor_idx();
        if anchor == cursor {
            None
        } else {
            Some((anchor.min(cursor), anchor.max(cursor)))
        }
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let chars: Vec<char> = self.input_text.chars().collect();
            let mut new_chars = Vec::with_capacity(chars.len());
            new_chars.extend_from_slice(&chars[..start]);
            new_chars.extend_from_slice(&chars[end..]);
            self.input_text = new_chars.into_iter().collect();
            self.cursor_char_idx = start;
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    fn insert_str_at_cursor(&mut self, text: &str) {
        self.delete_selection();
        let chars: Vec<char> = self.input_text.chars().collect();
        let idx = self.cursor_idx();
        let mut new_chars = Vec::with_capacity(chars.len() + text.chars().count());
        new_chars.extend_from_slice(&chars[..idx]);
        new_chars.extend(text.chars());
        let count = text.chars().count();
        new_chars.extend_from_slice(&chars[idx..]);
        self.input_text = new_chars.into_iter().collect();
        self.cursor_char_idx = idx + count;
        self.selection_anchor = None;
    }

    fn backspace_at_cursor(&mut self) {
        if !self.delete_selection() {
            let cursor = self.cursor_idx();
            if cursor > 0 {
                let chars: Vec<char> = self.input_text.chars().collect();
                let mut new_chars = Vec::with_capacity(chars.len().saturating_sub(1));
                new_chars.extend_from_slice(&chars[..cursor - 1]);
                new_chars.extend_from_slice(&chars[cursor..]);
                self.input_text = new_chars.into_iter().collect();
                self.cursor_char_idx = cursor - 1;
            }
        }
    }

    fn delete_forward_at_cursor(&mut self) {
        if !self.delete_selection() {
            let cursor = self.cursor_idx();
            let chars: Vec<char> = self.input_text.chars().collect();
            if cursor < chars.len() {
                let mut new_chars = Vec::with_capacity(chars.len().saturating_sub(1));
                new_chars.extend_from_slice(&chars[..cursor]);
                new_chars.extend_from_slice(&chars[cursor + 1..]);
                self.input_text = new_chars.into_iter().collect();
            }
        }
    }

    fn move_cursor_left(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        }
        let chars: Vec<char> = self.input_text.chars().collect();
        let current = self.cursor_idx();
        if ctrl {
            let mut target = current;
            while target > 0 && chars.get(target - 1).map_or(false, |c| c.is_whitespace()) {
                target -= 1;
            }
            while target > 0 && chars.get(target - 1).map_or(false, |c| !c.is_whitespace()) {
                target -= 1;
            }
            self.cursor_char_idx = target;
        } else if !shift {
            if let Some((start, _)) = self.selection_range() {
                self.cursor_char_idx = start;
            } else if current > 0 {
                self.cursor_char_idx = current - 1;
            }
        } else if current > 0 {
            self.cursor_char_idx = current - 1;
        }
        if !shift {
            self.selection_anchor = None;
        }
    }

    fn move_cursor_right(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        }
        let chars: Vec<char> = self.input_text.chars().collect();
        let current = self.cursor_idx();
        if ctrl {
            let mut target = current;
            while target < chars.len() && chars.get(target).map_or(false, |c| !c.is_whitespace()) {
                target += 1;
            }
            while target < chars.len() && chars.get(target).map_or(false, |c| c.is_whitespace()) {
                target += 1;
            }
            self.cursor_char_idx = target;
        } else if !shift {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_char_idx = end;
            } else if current < chars.len() {
                self.cursor_char_idx = current + 1;
            }
        } else if current < chars.len() {
            self.cursor_char_idx = current + 1;
        }
        if !shift {
            self.selection_anchor = None;
        }
    }

    fn move_cursor_home(&mut self, shift: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        } else if !shift {
            self.selection_anchor = None;
        }
        self.cursor_char_idx = 0;
    }

    fn move_cursor_end(&mut self, shift: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        } else if !shift {
            self.selection_anchor = None;
        }
        self.cursor_char_idx = self.input_text.chars().count();
    }

    fn select_word_at(&mut self, char_idx: usize) {
        let chars: Vec<char> = self.input_text.chars().collect();
        if chars.is_empty() {
            return;
        }
        let idx = char_idx.min(chars.len().saturating_sub(1));
        let mut start = idx;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let mut end = idx;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        self.selection_anchor = Some(start);
        self.cursor_char_idx = end;
    }

    fn persist_history(&self) {
        storage::save_conversation(&stored_turns(&self.chat_history));
    }

    fn persist_prefs(&mut self) {
        self.prefs.muted = self.is_muted;
        storage::save_prefs(&self.prefs);
        self.last_prefs_save = Instant::now();
    }

    /// Observa foco e geometria da janela: o foco alimenta o estado ausente e a
    /// geometria é lembrada entre sessões (a gravação é adiada para não escrever
    /// um JSON a cada pixel arrastado).
    fn attach_window_observers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds_subscription = cx.observe_window_bounds(window, |view, window, cx| {
            let bounds = window.bounds();
            view.prefs.window = Some(storage::WindowPrefs {
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y),
                width: f32::from(bounds.size.width),
                height: f32::from(bounds.size.height),
            });
            if view.last_prefs_save.elapsed() >= Duration::from_secs(2) {
                view.persist_prefs();
            }
            let maximized = window.is_maximized();
            if maximized != view.was_maximized {
                view.was_maximized = maximized;
                if maximized
                    && !view.is_away
                    && view.last_maximize_line.elapsed() >= Duration::from_secs(900)
                {
                    view.last_maximize_line = Instant::now();
                    view.show_local_line(
                        "Tela grande, hein? Vou fingir que não percebi que você quis me ver maior.",
                        "tsundere",
                        cx,
                    );
                }
            }
        });
        let activation_subscription = cx.observe_window_activation(window, |view, window, cx| {
            view.window_focused = window.is_window_active();
            view.refresh_away_state(cx);
        });
        self.window_subscriptions.push(bounds_subscription);
        self.window_subscriptions.push(activation_subscription);

        // Fechar (X, Alt+F4 ou "Sair" da bandeja) pede a despedida antes de
        // sair; o próprio app conclui o fechamento quando ela termina.
        let close_view = cx.weak_entity();
        window.on_window_should_close(cx, move |_window, cx| {
            close_view
                .update(cx, |this, cx| this.request_close(cx))
                .unwrap_or(true)
        });
        // Sem um resize inicial o observador não dispara; guardar a geometria
        // atual já garante que o fechamento salve a janela do primeiro uso.
        let bounds = window.bounds();
        self.prefs.window = Some(storage::WindowPrefs {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        });
        self.window_focused = window.is_window_active();
        self.was_maximized = window.is_maximized();
    }

    fn default_status(&self) -> SharedString {
        if self.is_calling {
            "SUA VEZ".into()
        } else if self.is_companion_mode {
            "MODO COMPANION".into()
        } else {
            "COM VOCÊ".into()
        }
    }

    /// Ausência é imediata: minimizar, esconder ou perder o foco já marca
    /// AUSENTE. Chamada e modo companhia são exceções — nessas horas o app pode
    /// ficar em segundo plano sem que ela "sinta" a ausência.
    fn refresh_away_state(&mut self, cx: &mut Context<Self>) {
        let exempt = self.is_calling || self.sleep_companion_started_at.is_some();
        let away = !exempt && (window_minimized_or_hidden() || !self.window_focused);
        if away == self.is_away {
            return;
        }
        self.is_away = away;
        if away {
            self.away_since = Some(Instant::now());
            self.status_text = "AUSENTE".into();
            // Eventos já buscados não devem falar quando ela voltar: o próximo
            // poll, com activity=away, consome e marca como pulados.
            self.proactive_queue.clear();
        } else {
            let away_for = self
                .away_since
                .take()
                .map(|since| since.elapsed())
                .unwrap_or_default();
            self.status_text = self.default_status();
            self.show_return_greeting(away_for, cx);
        }
        cx.notify();
    }

    fn show_return_greeting(&mut self, away_for: Duration, cx: &mut Context<Self>) {
        if away_for < Duration::from_secs(60)
            || self.last_return_greeting.elapsed() < Duration::from_secs(300)
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        let line = if away_for >= Duration::from_secs(30 * 60) {
            "Você ficou fora bastante tempo... eu fiquei aqui, sabia? Fico feliz que voltou."
        } else if away_for >= Duration::from_secs(5 * 60) {
            "Onde você foi? Não é como se eu estivesse esperando... mas pronto, você voltou."
        } else {
            "Você voltou rapidinho. Eu nem tive tempo de sentir sua falta."
        };
        self.last_return_greeting = Instant::now();
        self.show_local_line(line, "tsundere", cx);
    }

    /// Fala curta da própria Usami (sem backend): entrada no histórico, balão e
    /// emoção. Usada pelas despedidas/boas-vindas e pelas provocações de inatividade.
    fn show_local_line(
        &mut self,
        text: &'static str,
        emotion: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.last_reply = text.into();
        self.last_emotion = emotion.into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(emotion, 4.0, false);
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: text.to_string(),
            emotion: Some(emotion.to_string()),
            tool_badge: None,
            time_str: Some(current_phone_time()),
            media: None,
        });
        self.poke(cx);
        cx.notify();
    }

    /// Primeira tentativa de fechar segura a janela para a despedida; a segunda
    /// (ou depois de liberado) deixa sair na hora — quem insiste consegue.
    fn request_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.close_allowed || self.farewell_started_at.is_some() {
            self.close_allowed = true;
            return true;
        }
        self.begin_farewell(cx);
        false
    }

    fn begin_farewell(&mut self, cx: &mut Context<Self>) {
        const LINE: &str = "Hmph... já vai embora? T-tá bom, vai... mas não demora pra voltar, tá?! Ainda tenho umas coisas pra te contar.";
        self.farewell_started_at = Some(Instant::now());
        self.show_local_line(LINE, "tsundere", cx);

        if self.is_muted {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        }
        let Some(player) = self.audio_player.clone() else {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        };

        let api = Arc::clone(&self.api_client);
        let vrm_ctrl = self.vrm_controller.clone();
        let done = Arc::clone(&self.farewell_audio_done);
        crate::api::tokio_runtime().spawn(async move {
            match api.stream_tts(LINE, "tsundere").await {
                Ok(crate::api::TtsStream::Pcm {
                    sample_rate,
                    mut response,
                }) => {
                    let mouth = vrm_ctrl.clone();
                    let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                        mouth.set_speaking(true, level);
                    });
                    let mut started = false;
                    loop {
                        match response.chunk().await {
                            Ok(Some(chunk)) => {
                                if sink.push_i16_le(&chunk) && !started {
                                    started = true;
                                    vrm_ctrl.begin_speech_stream();
                                }
                            }
                            Ok(None) => break,
                            Err(error) => {
                                eprintln!("[Despedida TTS] stream interrompido: {error}");
                                break;
                            }
                        }
                    }
                    let tail = sink.finish();
                    if started {
                        let deadline = Instant::now() + Duration::from_secs(15);
                        while !tail.is_drained() && Instant::now() < deadline {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        vrm_ctrl.end_speech_stream();
                    }
                }
                Ok(crate::api::TtsStream::Audio(bytes)) => {
                    vrm_ctrl.speak(LINE, Some(bytes.clone()));
                    if let Err(error) = player.play_wav(bytes) {
                        eprintln!("[Despedida TTS] falha ao tocar áudio: {error}");
                    }
                }
                Err(error) => eprintln!("[Despedida TTS] falha: {error:?}"),
            }
            done.store(true, Ordering::SeqCst);
        });
    }

    fn tick_close_request(&mut self, cx: &mut Context<Self>) {
        let Some(started) = self.farewell_started_at else {
            return;
        };
        let audio_done = self.farewell_audio_done.load(Ordering::SeqCst);
        if !farewell_can_close(
            started.elapsed(),
            audio_done,
            self.vrm_controller.is_speaking(),
        ) {
            return;
        }
        self.close_allowed = true;
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    /// Sem interação e sem fala há alguns minutos, ela chama atenção em vez de
    /// ficar em silêncio para sempre. Só roda com a janela em primeiro plano:
    /// com o app ausente quem fala é a despedida/boas-vindas.
    fn maybe_run_idle_prods(&mut self, cx: &mut Context<Self>) {
        if self.is_away
            || self.is_calling
            || self.sleep_companion_started_at.is_some()
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        const PRODS: [(u64, &str); 3] = [
            (
                210,
                "Ei... você tá aí? Pode ficar quieto, mas eu prefiro quando você fala comigo.",
            ),
            (
                420,
                "Hmph. Se você não tem nada pra dizer, eu também não vou ficar puxando assunto.",
            ),
            (600, "Já faz um tempinho... tá tudo bem aí?"),
        ];
        let idle = self.last_user_activity.elapsed().as_secs();
        let Some((_, line)) = PRODS
            .get(self.idle_prods as usize)
            .copied()
            .filter(|(threshold, _)| idle >= *threshold)
        else {
            return;
        };
        self.idle_prods += 1;
        self.show_local_line(line, "tsundere", cx);
    }

    fn start_call(&mut self, cx: &mut Context<Self>) {
        self.is_calling = true;
        self.call_started_at = Some(Instant::now());
        self.call_seconds = 0;
        self.status_text = "SUA VEZ · 00:00".into();
        self.start_listening(cx);
    }

    fn end_call(&mut self, cx: &mut Context<Self>) {
        let minutes = self
            .call_started_at
            .map(|started| (started.elapsed().as_secs() / 60) as u32)
            .unwrap_or(0);
        self.stop_listening();
        self.is_calling = false;
        self.call_started_at = None;
        self.call_seconds = 0;
        if minutes > 0 {
            self.award_bond_call(minutes, cx);
        }
        self.status_text = if self.is_companion_mode {
            "MODO COMPANION"
        } else {
            "COM VOCÊ"
        }
        .into();
    }

    fn toggle_sleep_companion(&mut self) {
        if self.sleep_companion_started_at.is_some() {
            self.stop_sleep_companion();
        } else {
            self.is_companion_mode = true;
            self.sleep_companion_started_at = Some(Instant::now());
            self.sleep_companion_minutes_left = 30;
            self.last_emotion = "sleepy".into();
            self.status_text = "MODO COMPANION".into();
            self.vrm_controller.set_emotion("sleepy");
        }
    }

    fn stop_sleep_companion(&mut self) {
        self.sleep_companion_started_at = None;
        self.sleep_companion_minutes_left = 30;
        self.is_companion_mode = false;
        self.last_emotion = "tsundere".into();
        self.status_text = if self.is_calling {
            "SUA VEZ"
        } else {
            "COM VOCÊ"
        }
        .into();
        self.vrm_controller
            .set_emotion_with_options("tsundere", 4.0, false);
        self.vrm_controller.play_idle();
    }

    /// Fecha a escuta enquanto a Usami pensa/fala e quando o usuário muta.
    fn sync_mic_gate(&self) {
        self.mic_gate
            .set_user(self.is_muted || self.is_busy || !self.is_listening);
    }

    fn start_listening(&mut self, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.is_listening {
            return;
        }
        match recorder::VoiceListener::start(Arc::clone(&self.mic_gate)) {
            Ok(mut listener) => {
                let Some(segments) = listener.take_segments() else {
                    listener.stop();
                    self.is_listening = false;
                    self.sync_mic_gate();
                    self.status_text = "MIC: canal de áudio indisponível.".into();
                    cx.notify();
                    return;
                };
                let partials = listener.take_partials();
                self.listener = Some(listener);
                self.is_listening = true;
                self.listen_session = self.listen_session.wrapping_add(1);
                let session = self.listen_session;
                self.last_voice_commit = None;
                self.live_transcript = SharedString::default();
                self.voice_turn_closed = 0;
                self.sync_mic_gate();
                self.status_text = "OUVINDO...".into();

                // Prévia ao vivo: fatias do áudio em andamento vão para o STT em
                // paralelo ao turno final. A latência só atrasa a legenda; parar
                // de falar continua encerrando o turno pelo caminho normal.
                if let Some(mut partials) = partials {
                    let api = Arc::clone(&self.api_client);
                    cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let cx = cx.clone();
                        async move {
                            while let Some(mut slice) = partials.recv().await {
                                // O STT local pode demorar mais que o intervalo das
                                // fatias; transcrever a mais recente evita fila parada.
                                while let Ok(newer) = partials.try_recv() {
                                    slice = newer;
                                }
                                let Ok(text) = api.transcribe(slice.wav).await else {
                                    continue;
                                };
                                let trimmed = text.trim().to_string();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let active = cx
                                    .update(|cx: &mut App| {
                                        if let Some(this) = this.upgrade() {
                                            this.update(cx, |view, cx| {
                                                if !view.is_listening
                                                    || !view.is_calling
                                                    || view.listen_session != session
                                                {
                                                    return false;
                                                }
                                                // Chegou depois do texto final: é só um
                                                // eco atrasado do turno já respondido.
                                                if slice.turn <= view.voice_turn_closed {
                                                    return true;
                                                }
                                                if view.live_transcript.as_ref() != trimmed {
                                                    view.live_transcript = trimmed.clone().into();
                                                    cx.notify();
                                                }
                                                true
                                            })
                                        } else {
                                            false
                                        }
                                    })
                                    .unwrap_or(false);
                                if !active {
                                    break;
                                }
                            }
                        }
                    })
                    .detach();
                }

                // Consome as falas detectadas pela VAD e manda para o STT. O loop
                // vive enquanto o listener existir; trocar de chamada recria tudo.
                let api = Arc::clone(&self.api_client);
                cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        let mut segments = segments;
                        while let Some(slice) = segments.recv().await {
                            // Fecha o turno antes de transcrever: prévias dele que
                            // ainda estejam na fila não podem reaparecer na legenda.
                            let accepted = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || !view.is_calling {
                                                return false;
                                            }
                                            view.voice_turn_closed =
                                                view.voice_turn_closed.max(slice.turn);
                                            view.live_transcript = SharedString::default();
                                            cx.notify();
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !accepted {
                                break;
                            }

                            let transcript = api.transcribe(slice.wav).await;
                            let active = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || !view.is_calling {
                                                return false;
                                            }
                                            match transcript {
                                                Ok(text) if !text.trim().is_empty() => {
                                                    let trimmed = text.trim().to_string();
                                                    if is_duplicate_utterance(
                                                        &mut view.last_voice_commit,
                                                        &trimmed,
                                                        Instant::now(),
                                                    ) {
                                                        return true;
                                                    }
                                                    view.cursor_char_idx = trimmed.chars().count();
                                                    view.selection_anchor = None;
                                                    view.input_text = trimmed;
                                                    if view.is_busy {
                                                        // Não fala por cima: guarda no campo.
                                                        view.status_text = "TEXTO PRONTO".into();
                                                        cx.notify();
                                                    } else {
                                                        view.submit(cx);
                                                    }
                                                }
                                                Ok(_) => {
                                                    view.status_text = "NÃO ENTENDI".into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                                Err(error) => {
                                                    view.status_text =
                                                        format!("ERRO STT: {error}").into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                            }
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !active {
                                break;
                            }
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                self.is_listening = false;
                self.live_transcript = SharedString::default();
                self.sync_mic_gate();
                self.status_text = format!("MIC: {error}").into();
            }
        }
        cx.notify();
    }

    fn stop_listening(&mut self) {
        self.is_listening = false;
        self.listen_session = self.listen_session.wrapping_add(1);
        self.live_transcript = SharedString::default();
        self.sync_mic_gate();
        if let Some(listener) = self.listener.take() {
            listener.stop();
        }
    }

    fn toggle_listening(&mut self, cx: &mut Context<Self>) {
        if self.is_listening {
            self.stop_listening();
            self.status_text = "MICROFONE OFF".into();
            cx.notify();
        } else {
            self.start_listening(cx);
        }
    }

    fn attach_file_prompt(&mut self, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Selecione uma imagem ou arquivo para a Usami".into()),
        });

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(path) = paths.into_iter().next() {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("imagem.png")
                                .to_string();
                            let mime = match path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_ascii_lowercase())
                                .as_deref()
                            {
                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                Some("png") => "image/png",
                                Some("webp") => "image/webp",
                                Some("gif") => "image/gif",
                                Some("bmp") => "image/bmp",
                                Some("pdf") => "application/pdf",
                                Some("txt") => "text/plain",
                                Some("doc") => "application/msword",
                                Some("docx") => {
                                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                                }
                                Some("mp4") => "video/mp4",
                                Some("mp3") => "audio/mpeg",
                                Some("zip") => "application/zip",
                                _ => "application/octet-stream",
                            };
                            let media = AttachedMedia::from_bytes(filename, mime, bytes);
                            let _ = cx.update(|cx: &mut App| {
                                if let Some(this) = this.upgrade() {
                                    this.update(cx, |view, cx| {
                                        view.attached_media = Some(media);
                                        view.input_active = true;
                                        cx.notify();
                                    });
                                }
                            });
                        }
                    }
                }
            }
        })
        .detach();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        let text = self.input_text.trim().to_string();
        let mut attached = self.attached_media.take();
        if (text.is_empty() && attached.is_none()) || self.is_busy {
            self.attached_media = attached;
            return;
        }

        self.input_text.clear();
        self.cursor_char_idx = 0;
        self.selection_anchor = None;
        self.pending_dead_key = None;
        self.input_active = false;
        self.is_busy = true;
        self.sync_mic_gate();
        self.show_bubble = true;

        // Reação imediata de escuta e reflexão enquanto o LLM processa
        self.vrm_controller.play_vrma("thinking", false);
        // Limpa as expressões do turno anterior: sem isso elas se acumulam na cabeça.
        self.vrm_controller.clear_expressions();
        self.vrm_controller.set_expression("19-prefab_doubt", 0.45);

        // Persiste o anexo em disco (apenas na primeira gravação) para que
        // sobreviva a reinicializações do app.
        if let Some(media) = attached.as_mut() {
            if media.stored_path.is_none() {
                if let Some(bytes) = api::base64_decode(&media.data_base64) {
                    media.stored_path = storage::save_media_file(
                        &media.filename,
                        &media.mime_type,
                        &bytes,
                    );
                }
            }
        }

        if let Some(media) = attached {
            self.status_text = "ANALISANDO MÍDIA".into();
            let user_msg = if text.is_empty() {
                format!("[Mídia: {}]", media.filename)
            } else {
                format!("[Mídia: {}] {}", media.filename, text)
            };
            self.push_history(ChatMessage {
                sender: Sender::User,
                text: user_msg,
                emotion: None,
                tool_badge: None,
                time_str: Some(current_phone_time()),
                media: Some(media.clone()),
            });
            cx.notify();

            let api = Arc::clone(&self.api_client);
            let audio = self.audio_player.clone();
            let prompt = if text.is_empty() { None } else { Some(text) };

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api
                        .analyze_media(
                            &media.mime_type,
                            &media.data_base64,
                            Some(&media.filename),
                            prompt.as_deref(),
                            None,
                        )
                        .await;

                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.is_busy = false;
                                view.sync_mic_gate();
                                view.poke(cx);
                                match res {
                                    Ok(response) => {
                                        let emotion = response.gesture.clone().unwrap_or_else(|| "tsundere".into());
                                        view.last_reply = sanitize_speech_text(&response.reply).into();
                                        view.last_emotion = emotion.clone().into();
                                        view.show_bubble = true;
                                        view.status_text = if view.is_calling {
                                            "EM CHAMADA".into()
                                        } else if view.is_companion_mode {
                                            "MODO COMPANION".into()
                                        } else {
                                            "COM VOCÊ".into()
                                        };

                                        view.vrm_controller.set_emotion_with_options(&emotion, 5.0, false);
                                        view.vrm_controller.clear_expressions();
                                        for action in &response.actions {
                                            if matches!(
                                                action.action_type.as_str(),
                                                "set_expression" | "expression" | "emote"
                                            ) && let Some(name) = &action.name
                                            {
                                                let weight = action.weight.or(action.value).unwrap_or(0.8);
                                                view.vrm_controller.set_expression(name, weight);
                                            }
                                        }

                                        let gesture = response
                                            .actions
                                            .iter()
                                            .find_map(|action| match action.action_type.as_str() {
                                                "gesture" | "play_clip" | "play_vrma" => action.name.clone(),
                                                _ => None,
                                            })
                                            .or_else(|| {
                                                response
                                                    .gesture
                                                    .clone()
                                                    .filter(|gesture| gesture != "neutral" && gesture != "idle")
                                            });
                                        if let Some(gesture) = gesture {
                                            view.vrm_controller.play_vrma(&gesture, false);
                                        }

                                        view.push_history(ChatMessage {
                                            sender: Sender::Usami,
                                            text: response.reply.clone(),
                                            emotion: Some(emotion.clone()),
                                            tool_badge: None,
                                            time_str: Some(current_phone_time()),
                                            media: None,
                                        });

                                        cx.notify();

                                        if !view.is_muted
                                            && let Some(player) = audio
                                        {
                                            let api_tts = Arc::clone(&view.api_client);
                                            let tts_text = response.reply;
                                            let tts_style = emotion;
                                            let player = player.clone();
                                            let vrm_ctrl = view.vrm_controller.clone();
                                            crate::api::tokio_runtime().spawn(async move {
                                                match api_tts.stream_tts(&tts_text, &tts_style).await {
                                                    Ok(crate::api::TtsStream::Pcm {
                                                        sample_rate,
                                                        mut response,
                                                    }) => {
                                                        let mouth = vrm_ctrl.clone();
                                                        let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                                                            mouth.set_speaking(true, level);
                                                        });
                                                        let mut started = false;
                                                        loop {
                                                            match response.chunk().await {
                                                                Ok(Some(chunk)) => {
                                                                    if sink.push_i16_le(&chunk) && !started {
                                                                        started = true;
                                                                        vrm_ctrl.begin_speech_stream();
                                                                    }
                                                                }
                                                                Ok(None) => break,
                                                                Err(error) => {
                                                                    eprintln!("[TTS] stream interrompido: {error}");
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        let tail = sink.finish();
                                                        if started {
                                                            let deadline = Instant::now() + Duration::from_secs(180);
                                                            while !tail.is_drained() && Instant::now() < deadline {
                                                                tokio::time::sleep(Duration::from_millis(50)).await;
                                                            }
                                                            vrm_ctrl.end_speech_stream();
                                                        } else {
                                                            vrm_ctrl.speak(&tts_text, None);
                                                        }
                                                    }
                                                    Ok(crate::api::TtsStream::Audio(bytes)) => {
                                                        vrm_ctrl.speak(&tts_text, Some(bytes.clone()));
                                                        if let Err(error) = player.play_wav(bytes) {
                                                            eprintln!("[TTS] falha ao tocar áudio: {error}");
                                                        }
                                                    }
                                                    Err(error) => {
                                                        eprintln!("[TTS] streaming indisponível ({error}); fala só na legenda");
                                                        vrm_ctrl.speak(&tts_text, None);
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    Err(err) => {
                                        view.status_text = format!("ERRO: {}", err).into();
                                        cx.notify();
                                    }
                                }
                            });
                        }
                    });
                }
            })
            .detach();
            return;
        }

        self.push_history(ChatMessage {
            sender: Sender::User,
            text: text.clone(),
            emotion: None,
            tool_badge: None,
            time_str: Some(current_phone_time()),
            media: None,
        });

        cx.notify();

        // Em chamada a mensagem veio pela voz; fora dela é texto digitado.
        self.award_bond_chat(text.clone(), self.is_calling, cx);

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();
        let sleep_companion_active = self.sleep_companion_started_at.is_some();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.send_chat(&text, sleep_companion_active).await;

                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.is_busy = false;
                            view.chat_activity = None;
                            view.active_tool = None;
                            view.sync_mic_gate();
                            view.poke(cx);
                            match res {
                                Ok(response) => {
                                    let emotion = response.gesture.clone().unwrap_or_else(|| "tsundere".into());
                                    view.last_reply = sanitize_speech_text(&response.reply).into();
                                    view.last_emotion = emotion.clone().into();
                                    view.show_bubble = true;
                                    view.status_text = if view.is_calling {
                                        "EM CHAMADA".into()
                                    } else if view.is_companion_mode {
                                        "MODO COMPANION".into()
                                    } else {
                                        "COM VOCÊ".into()
                                    };

                                    // Emoção não dispara clipe: o gesto é escolhido uma vez, abaixo.
                                    view.vrm_controller.set_emotion_with_options(&emotion, 5.0, false);

                                    // Substitui (em vez de somar) as expressões do turno.
                                    view.vrm_controller.clear_expressions();
                                    for action in &response.actions {
                                        if matches!(
                                            action.action_type.as_str(),
                                            "set_expression" | "expression" | "emote"
                                        ) && let Some(name) = &action.name
                                        {
                                            let weight = action.weight.or(action.value).unwrap_or(0.8);
                                            view.vrm_controller.set_expression(name, weight);
                                        }
                                    }

                                    // Um único gesto por turno: a primeira ação de clipe; sem ação,
                                    // cai para o gesto do backend (nunca quando é só a emoção padrão).
                                    let gesture = response
                                        .actions
                                        .iter()
                                        .find_map(|action| match action.action_type.as_str() {
                                            "gesture" | "play_clip" | "play_vrma" => action.name.clone(),
                                            _ => None,
                                        })
                                        .or_else(|| {
                                            response
                                                .gesture
                                                .clone()
                                                .filter(|gesture| gesture != "neutral" && gesture != "idle")
                                        });
                                    if let Some(gesture) = gesture {
                                        view.vrm_controller.play_vrma(gesture, false);
                                    }

                                    view.push_history(ChatMessage {
                                        sender: Sender::Usami,
                                        text: response.reply.clone(),
                                        emotion: Some(emotion.clone()),
                                        tool_badge: format_tool_badge(response.agent_turn.as_ref()),
                                        time_str: Some(current_phone_time()),
                                        media: None,
                                    });

                                    view.last_source = response.source.clone().map(Into::into);
                                    view.last_timing = response.timing.clone();
                                    view.last_agent_turn = response.agent_turn.clone();
                                    // Nível/EXP do VN não saem mais da afinidade do
                                    // chat: quem manda é o /api/bond (abaixo, award).
                                    if let Some(aff) = response.affinity {
                                        view.server_affinity = Some(aff);
                                    }
                                    if let Some(stage) = response.stage.clone() {
                                        view.server_stage = Some(stage.into());
                                    }
                                    if let Some(mood) = response.mood.clone() {
                                        view.mood = Some(mood);
                                    }

                                    cx.notify();

                                    // Sem áudio não há boca animada: mudo mostra só as ações.
                                    if !view.is_muted
                                        && let Some(player) = audio
                                    {
                                        let api_tts = Arc::clone(&view.api_client);
                                        let tts_text = response.reply;
                                        let tts_style = emotion;
                                        let player = player.clone();
                                        let vrm_ctrl = view.vrm_controller.clone();
                                        crate::api::tokio_runtime().spawn(async move {
                                            match api_tts.stream_tts(&tts_text, &tts_style).await {
                                                Ok(crate::api::TtsStream::Pcm {
                                                    sample_rate,
                                                    mut response,
                                                }) => {
                                                    let mouth = vrm_ctrl.clone();
                                                    let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                                                        mouth.set_speaking(true, level);
                                                    });
                                                    let mut started = false;
                                                    loop {
                                                        match response.chunk().await {
                                                            Ok(Some(chunk)) => {
                                                                if sink.push_i16_le(&chunk) && !started {
                                                                    started = true;
                                                                    vrm_ctrl.begin_speech_stream();
                                                                }
                                                            }
                                                            Ok(None) => break,
                                                            Err(error) => {
                                                                eprintln!("[TTS] stream interrompido: {error}");
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    let tail = sink.finish();
                                                    if started {
                                                        // A boca acompanha o áudio até o buffer esvaziar,
                                                        // não até a rede fechar.
                                                        let deadline = Instant::now() + Duration::from_secs(180);
                                                        while !tail.is_drained() && Instant::now() < deadline {
                                                            tokio::time::sleep(Duration::from_millis(50)).await;
                                                        }
                                                        vrm_ctrl.end_speech_stream();
                                                    } else {
                                                        eprintln!("[TTS] stream sem áudio; fala só na legenda");
                                                        vrm_ctrl.speak(&tts_text, None);
                                                    }
                                                }
                                                Ok(crate::api::TtsStream::Audio(bytes)) => {
                                                    vrm_ctrl.speak(&tts_text, Some(bytes.clone()));
                                                    if let Err(error) = player.play_wav(bytes) {
                                                        eprintln!("[TTS] falha ao tocar áudio: {error}");
                                                    }
                                                }
                                                Err(error) => {
                                                    eprintln!("[TTS] streaming indisponível ({error}); fala só na legenda");
                                                    vrm_ctrl.speak(&tts_text, None);
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(err) => {
                                    view.status_text = format!("ERRO: {}", err).into();
                                    cx.notify();
                                }
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn poll_proactive_events(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled {
            return;
        }
        let api = Arc::clone(&self.api_client);
        // Relógio de silêncio: enquanto a voz da Usami está tocando ou terminou há < 60s,
        // conta como atividade recente (OpenAmika 11c7c4e).
        let is_active = self.last_interaction.elapsed() < Duration::from_secs(60)
            || self.vrm_controller.is_speaking()
            || self.last_voice_ended.elapsed() < Duration::from_secs(60);
        let activity = if self.is_away {
            "away"
        } else if is_active {
            "active"
        } else {
            "idle"
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Ok(res) = api.get_proactive_events(activity).await {
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, _cx| {
                                // Ausente: conversa social é consumida sem falar
                                // (não despeja atrasados ao voltar); lembrete fica
                                // pendente no servidor para não se perder.
                                if view.is_away {
                                    let acks: Vec<crate::api::ProactiveAckItem> = res
                                        .events
                                        .iter()
                                        .filter(|event| event.event_type != "reminder")
                                        .map(|event| crate::api::ProactiveAckItem {
                                            id: event.id.clone(),
                                            outcome: "skipped".to_string(),
                                        })
                                        .collect();
                                    if !acks.is_empty() {
                                        let api = Arc::clone(&view.api_client);
                                        crate::api::tokio_runtime().spawn(async move {
                                            let _ = api.ack_proactive_events(&acks).await;
                                        });
                                    }
                                    return;
                                }
                                for event in res.events {
                                    if let Some(dedupe_key) = &event.dedupe_key {
                                        if view.proactive_seen.contains(dedupe_key) {
                                            continue;
                                        }
                                    }
                                    if view.proactive_queue.iter().any(|e| e.id == event.id) {
                                        continue;
                                    }
                                    if view.proactive_queue.len() < 5 {
                                        view.proactive_queue.push(event);
                                    }
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();

        if !self.is_away && self.api_client.has_token() {
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    if let Ok(resp) = api.get_agent_suggestions(false, 20).await {
                        let _ = cx.update(|cx: &mut App| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |view, cx| {
                                    view.agent_suggestions = resp.suggestions;
                                    view.agent_suggestions_unanswered = resp.unanswered_count;
                                    view.agent_suggestions_loaded = true;
                                    view.agent_suggestions_error = None;
                                    cx.notify();
                                });
                            }
                        });
                    }
                }
            })
            .detach();
        }
    }

    fn present_next_proactive(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled || self.proactive_queue.is_empty() || self.is_away {
            return;
        }
        if self.is_busy
            || self.is_calling
            || self.is_listening
            || self.sleep_companion_started_at.is_some()
            || self.vrm_controller.is_speaking()
        {
            return;
        }

        let next_is_social = self.proactive_queue[0].event_type != "reminder";
        if next_is_social && self.last_proactive_speech.elapsed() < Duration::from_secs(300) {
            return;
        }

        let event = self.proactive_queue.remove(0);
        if let Some(dedupe_key) = &event.dedupe_key {
            self.proactive_seen.insert(dedupe_key.clone());
            storage::save_proactive_seen(&self.proactive_seen);
        }

        let emotion = event.emotion.clone().unwrap_or_else(|| "tsundere".into());
        self.last_reply = sanitize_speech_text(&event.text).into();
        self.last_emotion = emotion.clone().into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(&emotion, 5.0, false);
        if let Some(gesture) = &event.gesture {
            self.vrm_controller.play_vrma(gesture, false);
        }
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: event.text.clone(),
            emotion: Some(emotion.clone()),
            tool_badge: None,
            time_str: Some(current_phone_time()),
            media: None,
        });
        self.fetch_mood(cx);
        self.touch_ui(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();
        let event_id = event.id.clone();
        let text = event.text.clone();
        let is_muted = self.is_muted;
        let vrm_ctrl = self.vrm_controller.clone();

        self.last_proactive_speech = Instant::now();

        crate::api::tokio_runtime().spawn(async move {
            let mut delivered = false;
            if !is_muted && let Some(player) = audio {
                match api.stream_tts(&text, &emotion).await {
                    Ok(crate::api::TtsStream::Pcm {
                        sample_rate,
                        mut response,
                    }) => {
                        let mouth = vrm_ctrl.clone();
                        let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                            mouth.set_speaking(true, level);
                        });
                        let mut started = false;
                        loop {
                            match response.chunk().await {
                                Ok(Some(chunk)) => {
                                    if sink.push_i16_le(&chunk) && !started {
                                        started = true;
                                        vrm_ctrl.begin_speech_stream();
                                    }
                                }
                                Ok(None) => break,
                                Err(error) => {
                                    eprintln!("[Proactive TTS] stream interrompido: {error}");
                                    break;
                                }
                            }
                        }
                        let tail = sink.finish();
                        if started {
                            let deadline = Instant::now() + Duration::from_secs(180);
                            while !tail.is_drained() && Instant::now() < deadline {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            vrm_ctrl.end_speech_stream();
                            delivered = true;
                        } else {
                            vrm_ctrl.speak(&text, None);
                        }
                    }
                    Ok(crate::api::TtsStream::Audio(bytes)) => {
                        vrm_ctrl.speak(&text, Some(bytes.clone()));
                        if let Err(error) = player.play_wav(bytes) {
                            eprintln!("[Proactive TTS] falha ao tocar áudio: {error}");
                        } else {
                            delivered = true;
                        }
                    }
                    Err(e) => {
                        eprintln!("[Proactive TTS] falha: {e:?}");
                    }
                }
            }

            let outcome = if delivered { "spoken" } else { "shown" };
            let _ = api
                .ack_proactive_events(&[crate::api::ProactiveAckItem {
                    id: event_id,
                    outcome: outcome.to_string(),
                }])
                .await;
        });
    }

    fn toggle_drawer(&mut self, drawer: ActiveDrawer, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.active_drawer == drawer {
            self.active_drawer = ActiveDrawer::None;
        } else {
            self.active_drawer = drawer;
            if drawer == ActiveDrawer::Chat {
                // Abrir no fim da conversa: o scroll nasce no topo por padrão.
                self.phone_scroll.scroll_to_bottom();
            } else if drawer == ActiveDrawer::Bond {
                // Abrir o painel relê o core: o web (ou outra janela) pode ter mudado.
                self.fetch_bond(cx);
                self.fetch_mood(cx);
                // Diário, Álbum e Memória também acompanham os registros/espelho do core.
                if !self.memory_loading {
                    self.fetch_memory(cx);
                }
            } else if drawer == ActiveDrawer::Skills {
                self.fetch_agent_center(cx);
            }
        }
        cx.notify();
    }

    fn set_proactive_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.proactive_enabled = enabled;
        storage::save_proactive_config(enabled);
        if !enabled {
            self.proactive_queue.clear();
        }
        cx.notify();
    }


fn render_settings_category<E: IntoElement>(
    title: &'static str,
    subtitle: &'static str,
    content: E,
) -> impl IntoElement + use<E> {
    let t = theme::theme();
    let c = t.color;
    let sp = t.space;
    let ty = t.text;
    div()
        .flex()
        .flex_col()
        .gap(px(sp.s3))
        .rounded(px(t.radius.lg))
        .border_1()
        .border_color(c.line)
        .p(px(sp.s4))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s1))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::Palette::vn_pink_soft())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.faint)
                        .child(subtitle),
                ),
        )
        .child(content)
}


    fn render_settings_section(
        &mut self,
        tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        match tab {
            SettingsTab::Chat => {
                let base_url = self.api_client.base_url();
                let has_token = self.api_client.has_token();
                let proactive_enabled = self.proactive_enabled;

                div()
                    .flex()
                    .flex_col()
                    .child(settings_h3("CONEXÃO COM O BACKEND"))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("MOTOR LOCAL"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(if has_token { rgba(0x5cb85c22) } else { rgba(0xf0ad4e22) })
                                            .border_1()
                                            .border_color(if has_token { rgba(0x5cb85c66) } else { rgba(0xf0ad4e66) })
                                            .text_size(px(ty.xs))
                                            .text_color(if has_token { rgb(0x5cb85c) } else { rgb(0xf0ad4e) })
                                            .child(if has_token { "TOKEN ATIVO" } else { "SEM TOKEN" }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child(SharedString::from(format!("• URL: {}", base_url))),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1))
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(if has_token {
                                        "Autenticado via USAMI_API_TOKEN. Acesso a Memória e Jeito liberado."
                                    } else {
                                        "Defina USAMI_API_TOKEN no ambiente para liberar acesso a Memória e Jeito."
                                    }),
                            ),
                    )
                    .child(settings_h3("PROATIVIDADE E LEMBRETES"))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("EVENTOS PROATIVOS"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(if proactive_enabled { rgba(0x5cb85c22) } else { rgba(0x88888822) })
                                            .border_1()
                                            .border_color(if proactive_enabled { rgba(0x5cb85c66) } else { rgba(0x88888866) })
                                            .text_size(px(ty.xs))
                                            .text_color(if proactive_enabled { rgb(0x5cb85c) } else { rgb(0x888888) })
                                            .child(if proactive_enabled { "ATIVADO" } else { "DESATIVADO" }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child("Recebe saudações por horário, lembretes do Obsidian e resumos automáticos via API local."),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s3))
                                    .flex()
                                    .child(
                                        div()
                                            .id("toggle-proactive-btn")
                                            .cursor_pointer()
                                            .px(px(14.0))
                                            .py(px(8.0))
                                            .rounded(px(r.sm))
                                            .bg(c.surface_soft)
                                            .border_1()
                                            .border_color(c.line)
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .hover(|s| s.bg(c.line_strong))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                let next = !this.proactive_enabled;
                                                this.set_proactive_enabled(next, cx);
                                            }))
                                            .child(if proactive_enabled { "Desativar proatividade" } else { "Ativar proatividade" }),
                                    ),
                            ),
                    )
            }
            SettingsTab::Voice => {
                let has_token = self.api_client.has_token();
                div()
                    .flex()
                    .flex_col()
                    .child(settings_h3("VOZ OFICIAL"))
                    .child(settings_p(
                        "Gemini 3.8 Flash TTS com a voz desenhada para a Usami. A síntese é executada no servidor local.",
                    ))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.primary)
                                    .child("TRANSCRIÇÃO DE VOZ (STT)"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child(if has_token {
                                        "• STT ativo: envio de áudio autenticado via x-usami-token."
                                    } else {
                                        "• STT requer token: defina USAMI_API_TOKEN para habilitar transcrição."
                                    }),
                            ),
                    )
                    .child(settings_h3("EXPRESSÕES SONORAS"))
                    .child(settings_p(
                        "O tom muda por emoção e o texto pode acionar sons curtos:",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(sp.s1_5))
                            .mb(px(sp.s3))
                            .child(settings_code("<laugh>"))
                            .child(settings_code("<chuckle>"))
                            .child(settings_code("<sigh>"))
                            .child(settings_code("<short pause>")),
                    )
                    .child(settings_p(
                        "A tag vira áudio no sintetizador e é ocultada no balão de fala.",
                    ))
                    .child(settings_hint(
                        "Para trocar voz ou modelo, ajuste no usami/.env (GEMINI_TTS_VOICE, GEMINI_TTS_MODEL).",
                    ))
                    .child(settings_h3("PRIVACIDADE"))
                    .child(settings_p(
                        "A chave da API Gemini permanece exclusivamente no servidor local.",
                    ))
            }
            SettingsTab::Avatar => div()
                .flex()
                .flex_col()
                .child(settings_h3("RENDERIZADOR 3D NATIVO"))
                .child(settings_dl_row("Formato", "VRM 1.0 (GLTF Estendido)"))
                .child(settings_dl_row("Modelo", "白ー (shiro)"))
                .child(settings_dl_row("Renderizador", "WGPU 60 FPS (Nativo)"))
                .child(settings_dl_row("Janela", "Transparência nativa + Drag"))
                .child(settings_hint(
                    "A renderização gráfica opera via WGPU com aceleração de hardware nativa.",
                ))
                .child(settings_h3("CENÁRIO E ANIMAÇÕES"))
                .child(settings_p(
                    "O app desktop foca na presença direta da Usami na área de trabalho. Cenários completos e controle de ossos pertencem à versão web.",
                )),
            SettingsTab::Memory => {
                let has_token = self.api_client.has_token();
                let loading = self.memory_loading;
                let error = self.memory_error.clone();
                let status = self.memory_status.clone();
                let confirm_active = self.memory_confirm_active;
                let memory_opt = self.memory_data.clone();
                let server_stage = self.server_stage.clone();
                let stage_str = server_stage.as_ref().map(|s| s.as_ref()).unwrap_or("Pendente");

                let mut view = div().flex().flex_col();

                view = view.child(settings_h3("MEMÓRIA DA USAMI"));

                if !has_token {
                    view = view.child(
                        div()
                            .p(px(sp.s3_5))
                            .rounded(px(r.md))
                            .bg(rgba(0xf0ad4e15))
                            .border_1()
                            .border_color(rgba(0xf0ad4e55))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf0ad4e))
                                    .child("AUTENTICAÇÃO NECESSÁRIA"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1_5))
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child("A leitura e exclusão de memórias exige o token do servidor. Configure a variável de ambiente USAMI_API_TOKEN e abra o app novamente."),
                            ),
                    );
                } else {
                    // Barra de ações de topo
                    let mut top_bar = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(sp.s3));

                    top_bar = top_bar.child(
                        div()
                            .id("reload-memory-btn")
                            .cursor_pointer()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .hover(|s| s.bg(c.line_strong))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_memory(cx);
                            }))
                            .child("Recarregar memória"),
                    );

                    view = view.child(top_bar);

                    if loading {
                        view = view.child(settings_hint("Carregando memórias do servidor..."));
                    }

                    if let Some(err) = error {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .text_color(c.danger_muted)
                                .child(SharedString::from(format!("Erro: {}", err))),
                        );
                    }

                    if let Some(st) = status {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(rgba(0x5cb85c22))
                                .border_1()
                                .border_color(rgba(0x5cb85c66))
                                .text_size(px(ty.xs))
                                .text_color(rgb(0x5cb85c))
                                .child(SharedString::from(st)),
                        );
                    }

                    if let Some(mem) = memory_opt {
                        view = view.child(
                            div()
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.primary)
                                        .child("RESUMO"),
                                )
                                .child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!(
                                            "• {} fato(s) durável(is) · {} recente(s) · {} resumo(s)",
                                            mem.durable_count(),
                                            mem.recent_facts.len(),
                                            mem.summaries.len()
                                        ))),
                                )
                                .child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child(SharedString::from(format!(
                                            "• Afinidade: {}/100 (estágio: {})",
                                            mem.affinity.as_ref().map(|a| a.score).unwrap_or(0),
                                            stage_str
                                        ))),
                                ),
                        );

                        // Espelho dos medidores: a mesma leitura do painel de
                        // Laços, guardada no memory.json pelo core.
                        if let Some(bond) = mem.bond.as_ref() {
                            let meters: [(&'static str, u32, u32, f32); 3] = [
                                ("Carinho", bond.intimacy.carinho, bond.meters.carinho.level, bond.meters.carinho.progress),
                                ("Humor", bond.intimacy.humor, bond.meters.humor.level, bond.meters.humor.progress),
                                ("Companhia", bond.intimacy.companhia, bond.meters.companhia.level, bond.meters.companhia.progress),
                            ];
                            view = view.child(
                                div()
                                    .mt(px(sp.s3))
                                    .p(px(sp.s3))
                                    .rounded(px(r.md))
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(SharedString::from(format!(
                                                "VÍNCULO (ESPELHO) · {}",
                                                bond.summary.name
                                            ))),
                                    )
                                    .children(meters.into_iter().map(|(label, xp, level, pct)| {
                                        div()
                                            .mt(px(sp.s2))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.pearl)
                                                            .child(label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.muted)
                                                            .child(SharedString::from(format!(
                                                                "Nv.{} · {} XP",
                                                                level, xp
                                                            ))),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(px(6.0))
                                                    .rounded_full()
                                                    .bg(c.bond_track)
                                                    .overflow_hidden()
                                                    .child(
                                                        div()
                                                            .h_full()
                                                            .w(relative(pct.clamp(0.0, 1.0)))
                                                            .rounded_full()
                                                            .bg(c.primary),
                                                    ),
                                            )
                                    }))
                                    .child(
                                        div()
                                            .mt(px(sp.s2))
                                            .text_size(px(10.0))
                                            .text_color(c.faint)
                                            .child("Cópia dos medidores de Laços salva no memory.json."),
                                    ),
                            );
                        }

                        // Banner de liberação de exclusão
                        view = view.child(
                            div()
                                .mt(px(sp.s3))
                                .p(px(sp.s2_5))
                                .rounded(px(r.md))
                                .bg(if confirm_active { rgba(0xd9534f22) } else { c.surface_soft })
                                .border_1()
                                .border_color(if confirm_active { rgba(0xd9534f66) } else { c.line })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if confirm_active { rgb(0xd9534f) } else { c.pearl })
                                                .child(if confirm_active {
                                                    "MODO EXCLUSÃO ATIVO: exclusões liberadas."
                                                } else {
                                                    "Exclusões bloqueadas por segurança."
                                                }),
                                        )
                                        .child(
                                            div()
                                                .id("toggle-memory-confirm-btn")
                                                .cursor_pointer()
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .rounded(px(r.sm))
                                                .bg(if confirm_active { rgba(0xd9534f44) } else { c.card_bg })
                                                .border_1()
                                                .border_color(if confirm_active { rgb(0xd9534f) } else { c.line_strong })
                                                .text_size(px(ty.xs))
                                                .text_color(c.pearl)
                                                .hover(|s| s.bg(c.line_strong))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.memory_confirm_active = !this.memory_confirm_active;
                                                    cx.notify();
                                                }))
                                                .child(if confirm_active { "Bloquear" } else { "Liberar exclusões" }),
                                        ),
                                ),
                        );

                        // Renderização das seções de fatos
                        let sec_list: [(&'static str, &'static str, &Vec<String>); 6] = [
                            ("user", "IDENTIDADE DO USUÁRIO", &mem.sections.user),
                            ("preferences", "PREFERÊNCIAS", &mem.sections.preferences),
                            ("hobbies", "HOBBIES E INTERESSES", &mem.sections.hobbies),
                            ("relationship", "VÍNCULO E RELAÇÃO", &mem.sections.relationship),
                            ("dates", "DATAS E ROTINAS", &mem.sections.dates),
                            ("other", "OUTROS FATOS", &mem.sections.other),
                        ];

                        let mut has_any_fact = false;
                        let mut fact_idx: usize = 0;

                        for (sec_key, sec_title, facts) in sec_list {
                            if facts.is_empty() {
                                continue;
                            }
                            has_any_fact = true;

                            view = view.child(settings_h3(sec_title));

                            for fact in facts {
                                fact_idx += 1;
                                let fact_text = fact.clone();
                                let sec_name = sec_key.to_string();

                                let mut row = div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py(px(6.0))
                                    .border_b_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .child(SharedString::from(format!("• {}", fact_text))),
                                    );

                                if confirm_active {
                                    let del_fact = fact_text.clone();
                                    let del_sec = sec_name.clone();
                                    row = row.child(
                                        div()
                                            .id(ElementId::NamedInteger("del-fact".into(), fact_idx as u64))
                                            .cursor_pointer()
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(c.danger_muted_bg)
                                            .border_1()
                                            .border_color(c.danger_muted_border)
                                            .text_size(px(ty.xs))
                                            .text_color(c.danger_muted)
                                            .hover(|s| s.bg(rgba(0xd9534f33)))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.delete_memory_fact(del_sec.clone(), del_fact.clone(), cx);
                                            }))
                                            .child("Excluir"),
                                    );
                                }

                                view = view.child(row);
                            }
                        }

                        if !has_any_fact {
                            view = view.child(
                                div()
                                    .mt(px(sp.s3))
                                    .text_size(px(ty.sm))
                                    .text_color(c.muted)
                                    .child("Nenhum fato duradouro registrado ainda."),
                            );
                        }

                        // Ação perigosa geral
                        view = view.child(settings_h3("ZONA DE RISCO"));
                        view = view.child(settings_p(
                            "A exclusão total remove todos os fatos duráveis e o histórico salvo no memory.json do servidor local.",
                        ));

                        if confirm_active {
                            view = view.child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .id("delete-all-memory-btn")
                                            .cursor_pointer()
                                            .px(px(16.0))
                                            .py(px(8.0))
                                            .rounded(px(r.sm))
                                            .bg(rgb(0xd9534f))
                                            .text_size(px(ty.sm))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .hover(|s| s.bg(rgb(0xc9302c)))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.delete_memory_all(cx);
                                            }))
                                            .child("Apagar toda a memória"),
                                    ),
                            );
                        } else {
                            view = view.child(
                                div()
                                    .flex()
                                    .child(settings_danger_button("Apagar toda a memória (requer liberação)")),
                            );
                        }
                    }
                }

                view
            }
            SettingsTab::Personality => {
                let has_token = self.api_client.has_token();
                let loading = self.personality_loading;
                let error = self.personality_error.clone();
                let status = self.personality_status.clone();
                let p_opt = self.personality_data.clone();

                let mut view = div().flex().flex_col();

                view = view.child(settings_h3("JEITO DA USAMI"));

                if !has_token {
                    view = view.child(
                        div()
                            .p(px(sp.s3_5))
                            .rounded(px(r.md))
                            .bg(rgba(0xf0ad4e15))
                            .border_1()
                            .border_color(rgba(0xf0ad4e55))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf0ad4e))
                                    .child("AUTENTICAÇÃO NECESSÁRIA"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1_5))
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child("O ajuste de traços e instruções da personalidade exige USAMI_API_TOKEN configurado."),
                            ),
                    );
                } else {
                    let mut top_bar = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(sp.s3));

                    top_bar = top_bar.child(
                        div()
                            .id("reload-personality-btn")
                            .cursor_pointer()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .hover(|s| s.bg(c.line_strong))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_personality(cx);
                            }))
                            .child("Recarregar do servidor"),
                    );

                    view = view.child(top_bar);

                    if loading {
                        view = view.child(settings_hint("Carregando traços do servidor..."));
                    }

                    if let Some(err) = error {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .text_color(c.danger_muted)
                                .child(SharedString::from(format!("Erro: {}", err))),
                        );
                    }

                    if let Some(st) = status {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(rgba(0x5cb85c22))
                                .border_1()
                                .border_color(rgba(0x5cb85c66))
                                .text_size(px(ty.xs))
                                .text_color(rgb(0x5cb85c))
                                .child(SharedString::from(st)),
                        );
                    }

                    if let Some(p) = p_opt {
                        view = view.child(settings_hint(
                            "50 é a personalidade padrão. Ajuste os traços de 5 em 5 pontos para calibrar o tom da Usami.",
                        ));

                        let traits_meta: [(&'static str, &'static str, u32); 4] = [
                            ("warmth", "Carinho", p.traits.warmth),
                            ("curiosity", "Curiosidade", p.traits.curiosity),
                            ("playfulness", "Brincadeira", p.traits.playfulness),
                            ("pride", "Orgulho (tsundere)", p.traits.pride),
                        ];

                        let mut traits_col = div().mt(px(sp.s3)).flex().flex_col().gap(px(12.0));

                        for (idx, (key, label, val)) in traits_meta.into_iter().enumerate() {
                            let trait_key_down = key;
                            let trait_key_up = key;
                            let bar_w = ((val as f32 / 100.0) * 320.0).clamp(0.0, 320.0);

                            let trait_box = div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(label)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(sp.s2))
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(c.primary)
                                                        .child(SharedString::from(format!("{}", val))),
                                                )
                                                .child(
                                                    div()
                                                        .id(ElementId::NamedInteger("trait-dec".into(), idx as u64))
                                                        .cursor_pointer()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(c.line)
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(c.line_strong))
                                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                            this.adjust_trait(trait_key_down, -5);
                                                            cx.notify();
                                                        }))
                                                        .child("-5"),
                                                )
                                                .child(
                                                    div()
                                                        .id(ElementId::NamedInteger("trait-inc".into(), idx as u64))
                                                        .cursor_pointer()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(c.line)
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(c.line_strong))
                                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                            this.adjust_trait(trait_key_up, 5);
                                                            cx.notify();
                                                        }))
                                                        .child("+5"),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .h(px(6.0))
                                        .w_full()
                                        .rounded_full()
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .child(
                                            div()
                                                .h_full()
                                                .w(px(bar_w))
                                                .rounded_full()
                                                .bg(c.primary),
                                        ),
                                );

                            traits_col = traits_col.child(trait_box);
                        }

                        view = view.child(traits_col);

                        // Instruções extras
                        view = view.child(
                            div()
                                .mt(px(sp.s4))
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(settings_h3("INSTRUÇÕES EXTRAS"))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.primary)
                                        .child(SharedString::from(format!(
                                            "{}/600",
                                            p.custom_prompt.chars().count()
                                        ))),
                                ),
                        );

                        let prompt_display: SharedString = if p.custom_prompt.trim().is_empty() {
                            "Nenhuma instrução extra configurada.".into()
                        } else {
                            p.custom_prompt.clone().into()
                        };

                        view = view.child(
                            div()
                                .p(px(sp.s2_5))
                                .rounded(px(r.md))
                                .bg(c.icon_bg)
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(ty.sm))
                                .text_color(c.pearl)
                                .child(prompt_display),
                        );

                        // Botões Salvar e Restaurar
                        view = view.child(
                            div()
                                .mt(px(sp.s4))
                                .flex()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .id("save-personality-btn")
                                        .cursor_pointer()
                                        .px(px(16.0))
                                        .py(px(8.0))
                                        .rounded(px(r.sm))
                                        .bg(c.primary)
                                        .text_size(px(ty.sm))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .hover(|s| s.opacity(0.85))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.save_personality(cx);
                                        }))
                                        .child("Salvar alterações"),
                                )
                                .child(
                                    div()
                                        .id("reset-personality-btn")
                                        .cursor_pointer()
                                        .px(px(16.0))
                                        .py(px(8.0))
                                        .rounded(px(r.sm))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.reset_personality(cx);
                                        }))
                                        .child("Restaurar padrão"),
                                ),
                        );
                    }
                }

                view
            }
            SettingsTab::About => div()
                .flex()
                .flex_col()
                .child(settings_h3("USAMI COMPANION"))
                .child(settings_p(
                    "Versão 0.5.1 — desktop companion nativo com avatar 3D/VRM e voz própria.",
                ))
                .child(settings_h3("ARQUITETURA NATIVA"))
                .child(settings_list_item("Desktop: Rust + GPUI 0.2.2 + WGPU 60 FPS"))
                .child(settings_list_item("Avatar: 白ー (shiro) VRM 1.0 com renderização direta nativa"))
                .child(settings_list_item("Backend: Node.js (127.0.0.1:8787) com OpenRouter / NVIDIA (LLM) + Gemini (TTS)"))
                .child(settings_list_item("Presença Win32: Bandeja do sistema, atalho global (Alt+Espaço) e autostart"))
                .child(settings_h3("CRÉDITOS"))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1))
                        .mb(px(sp.s1))
                        .text_size(px(ty.md))
                        .line_height(px(21.0))
                        .text_color(c.pearl)
                        .child("•")
                        .child("Modelo 3D:")
                        .child(div().font_weight(FontWeight::BOLD).child("白ー (shiro)"))
                        .child("por Sweet_nya555"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1))
                        .mb(px(sp.s1))
                        .text_size(px(ty.md))
                        .line_height(px(21.0))
                        .text_color(c.pearl)
                        .child("•")
                        .child("Ícones: Lucide"),
                )
                .child(settings_h3("PRIVACIDADE"))
                .child(settings_p(
                    "Comunicação local em 127.0.0.1:8787. As credenciais e dados sensíveis residem unicamente no servidor local.",
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s1_5))
                        .child(settings_code("127.0.0.1:8787"))
                        .child(settings_code("USAMI_API_TOKEN"))
                        .child(settings_code("usami/.env")),
                ),
        }
    }

    fn render_settings_advanced(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let msg_count = self.chat_history.len();
        let last_source = self.last_source.clone();
        let last_timing = self.last_timing.clone();
        let server_affinity = self.server_affinity;
        let server_stage = self.server_stage.clone();
        let source_str = last_source
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or("Nenhuma interação recente");
        let stage_str = server_stage.as_ref().map(|s| s.as_ref()).unwrap_or("—");

        div()
            .flex()
            .flex_col()
            .child(settings_h3("ÚLTIMO TURNO DE RESPOSTA"))
            .child(
                div()
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .font_family(theme::FONT_DISPLAY)
                            .font_weight(FontWeight::BOLD)
                            .text_color(c.primary)
                            .child("TELEMETRIA DA IA"),
                    )
                    .child(
                        div()
                            .mt(px(sp.s2))
                            .text_size(px(ty.sm))
                            .text_color(c.pearl)
                            .child(SharedString::from(format!(
                                "• Fonte: {}",
                                source_str
                            ))),
                    )
                    .when_some(last_timing, |parent, timing| {
                        parent
                            .when_some(timing.llm_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!("• LLM: {} ms", ms))),
                                )
                            })
                            .when_some(timing.surface_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!("• Superfície: {} ms", ms))),
                                )
                            })
                            .when_some(timing.total_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!("• Total: {} ms", ms))),
                                )
                            })
                    })
                    .child(
                        div()
                            .mt(px(sp.s1))
                            .text_size(px(ty.sm))
                            .text_color(c.pearl)
                            .child(SharedString::from(format!(
                                "• Vínculo no servidor: {} (estágio: {})",
                                server_affinity.map(|a| format!("{}/100", a)).unwrap_or_else(|| "—".into()),
                                stage_str
                            ))),
                    ),
            )
            .child(settings_h3("HISTÓRICO LOCAL"))
            .child(settings_p(SharedString::from(format!(
                "Total de {} mensagem(ns) salva(s) neste computador.",
                msg_count
            ))))
            .child(
                div()
                    .flex()
                    .child(
                        div()
                            .id("clear-session-history-btn")
                            .cursor_pointer()
                            .px(px(14.0))
                            .py(px(8.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.sm))
                            .text_color(c.pearl)
                            .hover(|s| s.bg(c.line_strong))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.chat_history.clear();
                                this.persist_history();
                                this.show_bubble = false;
                                cx.notify();
                            }))
                            .child("Limpar histórico da conversa"),
                    ),
            )
    }

    fn render_settings_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let voice = self.render_settings_section(SettingsTab::Voice, cx);
        let personality = self.render_settings_section(SettingsTab::Personality, cx);
        let memory = self.render_settings_section(SettingsTab::Memory, cx);
        let avatar = self.render_settings_section(SettingsTab::Avatar, cx);
        let integrations = self.render_settings_section(SettingsTab::Chat, cx);
        let system = self.render_settings_section(SettingsTab::About, cx);
        let advanced_open = self.settings_advanced_open;
        let advanced = if advanced_open {
            Some(self.render_settings_advanced(cx))
        } else {
            None
        };

        div()
            .id("vn-settings-page")
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(sp.s5))
            .p(px(sp.s6))
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s1))
                    .child(
                        div()
                            .text_size(px(26.0))
                            .font_family(theme::FONT_CHARACTER)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.pearl)
                            .child("Ajustes"),
                    )
                    .child(
                        div()
                            .text_size(px(ty.sm))
                            .text_color(c.muted)
                            .child(
                                "Voz, memória, jeito de ser e integrações — tudo no mesmo lugar.",
                            ),
                    ),
            )
            .child(Self::render_settings_category(
                "Voz e áudio",
                "Gemini Multimodal Live, TTS e privacidade da chave.",
                voice,
            ))
            .child(Self::render_settings_category(
                "Personalidade e comportamento",
                "Traços, jeito de falar e tom das respostas.",
                personality,
            ))
            .child(Self::render_settings_category(
                "Memória e privacidade",
                "O que a Usami lembra, exportação e apagamento.",
                memory,
            ))
            .child(Self::render_settings_category(
                "Aparência e interface",
                "Renderizador 3D nativo e cenário.",
                avatar,
            ))
            .child(Self::render_settings_category(
                "Integrações",
                "Conexão com o backend e proatividade.",
                integrations,
            ))
            .child(Self::render_settings_category(
                "Sistema",
                "Versão, privacidade e diagnósticos locais.",
                system,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(t.radius.lg))
                    .border_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .id("vn-settings-advanced-toggle")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(sp.s4))
                            .py(px(sp.s3))
                            .hover(|s| s.bg(c.surface_soft))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.settings_advanced_open = !this.settings_advanced_open;
                                this.poke(cx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(sp.s2))
                                    .child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.lavender)
                                            .child(if advanced_open { "▾" } else { "▸" }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ty.md))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c.pearl)
                                            .child("Avançado"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.faint)
                                    .child("Telemetria do último turno e histórico local"),
                            ),
                    )
                    .when_some(advanced, |el, content| {
                        el.child(
                            div()
                                .px(px(sp.s4))
                                .pb(px(sp.s4))
                                .flex()
                                .flex_col()
                                .child(content),
                        )
                    }),
            )
    }

    fn load_settings_data(&mut self, cx: &mut Context<Self>) {
        if !self.memory_loading {
            self.fetch_memory(cx);
        }
        if self.personality_data.is_none() && !self.personality_loading {
            self.fetch_personality(cx);
        }
    }

    /// O core é a única fonte do vínculo: nível, EXP e medidores vêm dele.
    fn apply_bond(&mut self, bond: api::BondData) {
        self.bond_level = bond.summary.level.max(1);
        self.bond_exp_pct = (bond.summary.progress.clamp(0.0, 1.0) * 100.0).round() as u32;
        self.bond = Some(bond);
    }

    fn fetch_bond(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_bond().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    // O core reavalia conquistas junto com o
                                    // vínculo; recarrega para refletir datas.
                                    view.fetch_achievements(cx);
                                }
                                Err(error) => eprintln!("[Vínculo] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Lê a atividade ao vivo do turno em andamento.
    ///
    /// Só faz sentido enquanto `is_busy`: é o intervalo de 10-20 s em que o
    /// usuário não sabe se a Usami está pensando, executando uma ferramenta
    /// ou travada. `active_tool` carrega o nome da ferramenta em execução.
    fn fetch_chat_activity(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || self.activity_loading {
            return;
        }
        self.activity_loading = true;
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_chat_activity().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.activity_loading = false;
                            if let Ok(activity) = res {
                                let tool = activity.tool.clone().filter(|t| !t.is_empty());
                                let phase = activity.phase.clone();
                                let changed = view.active_tool != tool
                                    || view.chat_activity.as_ref().and_then(|a| a.phase.clone()) != phase;
                                view.active_tool = tool;
                                view.chat_activity = if activity.active { Some(activity) } else { None };
                                if changed {
                                    cx.notify();
                                }
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_achievements(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_achievements().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.achievements = resp.achievements,
                                Err(error) => eprintln!("[Conquistas] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Busca o humor persistido no core (`GET /api/mood`) sem bloquear a UI.
    fn fetch_mood(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || self.mood_loading {
            return;
        }
        self.mood_loading = true;
        let had_mood = self.mood.is_some();
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_mood().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.mood_loading = false;
                            match res {
                                Ok(resp) => {
                                    if !had_mood && resp.mood.vrm_emotion != "neutral" {
                                        view.vrm_controller.set_emotion_with_options(
                                            &resp.mood.vrm_emotion,
                                            8.0,
                                            false,
                                        );
                                    }
                                    view.mood = Some(resp.mood);
                                }
                                Err(error) => eprintln!("[Humor] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Aplica uma expressão no avatar ao vivo e registra o ajuste de humor no core.
    fn trigger_mood_preview(
        &mut self,
        emotion: &'static str,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.vrm_controller
            .set_emotion_with_options(emotion, 8.0, false);
        self.last_emotion = emotion.into();
        self.poke(cx);
        cx.notify();

        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.post_mood_event(emotion, Some(reason)).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.mood = Some(resp.mood),
                                Err(error) => eprintln!("[Humor] falha ao atualizar: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Consulta estado completo da Central do Agente (`/api/agent/*`) sem bloquear a UI.
    fn fetch_agent_center(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.agent_skills_loading = false;
            self.agent_executions_loading = false;
            self.agent_permissions_loading = false;
            self.agent_suggestions_loading = false;
            self.agent_skills_loaded = true;
            self.agent_executions_loaded = true;
            self.agent_permissions_loaded = true;
            self.agent_suggestions_loaded = true;
            let msg: SharedString =
                "Token ausente (USAMI_API_TOKEN) para consultar o core.".into();
            self.agent_skills_error = Some(msg.clone());
            self.agent_executions_error = Some(msg.clone());
            self.agent_permissions_error = Some(msg.clone());
            self.agent_suggestions_error = Some(msg);
            cx.notify();
            return;
        }
        if !self.agent_skills_loading {
            self.agent_skills_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_skills().await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_skills_loading = false;
                                view.agent_skills_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_skills = resp.skills;
                                        view.agent_skills_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_skills_error = Some(err.into());
                                    }
                                }
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        if !self.agent_executions_loading {
            self.agent_executions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_executions(50).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_executions_loading = false;
                                view.agent_executions_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_executions = resp.executions;
                                        view.agent_executions_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_executions_error = Some(err.into());
                                    }
                                }
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        if !self.agent_permissions_loading {
            self.agent_permissions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let tools_res = api.get_agent_self_tools().await;
                    let perms_res = api.get_agent_permissions().await;
                    let props_res = api.get_skill_proposals().await;
                    let plans_res = api.get_agent_plans(20).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_permissions_loading = false;
                                view.agent_permissions_loaded = true;
                                let mut first_err: Option<String> = None;
                                match tools_res {
                                    Ok(resp) => view.agent_tools = resp.self_data.tools,
                                    Err(err) => first_err = Some(err),
                                }
                                match perms_res {
                                    Ok(resp) => view.agent_permissions = resp.revocations,
                                    Err(err) if first_err.is_none() => first_err = Some(err),
                                    Err(_) => {}
                                }
                                if let Ok(resp) = props_res {
                                    view.agent_proposals = resp.proposals;
                                }
                                if let Ok(resp) = plans_res {
                                    view.agent_plans = resp.plans;
                                }
                                view.agent_permissions_error = first_err.map(Into::into);
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        if !self.agent_suggestions_loading {
            self.agent_suggestions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_suggestions(false, 50).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_suggestions_loading = false;
                                view.agent_suggestions_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_suggestions = resp.suggestions;
                                        view.agent_suggestions_unanswered = resp.unanswered_count;
                                        view.agent_suggestions_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_suggestions_error = Some(err.into());
                                    }
                                }
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn revoke_or_restore_permission(
        &mut self,
        tool: String,
        revoke: bool,
        reason: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.agent_permission_busy.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para alterar permissões.".into());
            cx.notify();
            return;
        }
        self.agent_permission_busy = Some(tool.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let action = if revoke { "revoke" } else { "restore" };
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api
                    .set_tool_permission(&tool, action, reason.as_deref())
                    .await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_permission_busy = None;
                            match res {
                                Ok(resp) => {
                                    view.agent_permissions = resp.revocations;
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Permissão \"{tool}\" {}.",
                                            if revoke { "revogada" } else { "restaurada" }
                                        )
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn approve_pending_skill(&mut self, skill_id: String, cx: &mut Context<Self>) {
        if self.agent_confirm_busy.is_some() || !self.agent_confirm_armed {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para confirmar instalação.".into());
            cx.notify();
            return;
        }
        // Exige novo armamento explícito para a próxima confirmação e mantém o
        // token de uso único estritamente efêmero dentro desta tarefa assíncrona.
        self.agent_confirm_armed = false;
        self.agent_confirm_busy = Some(skill_id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let step = match api.create_skill_confirmation(&skill_id).await {
                    Ok(conf) => {
                        api.install_skill_with_confirmation(&skill_id, &conf.confirmation.token)
                            .await
                    }
                    Err(err) => Err(err),
                };
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_confirm_busy = None;
                            match step {
                                Ok(resp) => {
                                    let label = resp
                                        .skill
                                        .as_ref()
                                        .and_then(|s| s.name.as_deref())
                                        .unwrap_or(&skill_id);
                                    view.agent_action_notice = Some(
                                        format!("Skill \"{label}\" aprovada e instalada.").into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn deny_pending_skill(&mut self, skill_id: String, cx: &mut Context<Self>) {
        self.agent_confirm_armed = false;
        self.revoke_or_restore_permission(
            format!("skill.{skill_id}"),
            true,
            Some("Negada na fila de confirmações".to_string()),
            cx,
        );
    }

    fn request_level4_tool_confirmation(&mut self, tool_name: String, cx: &mut Context<Self>) {
        if self.agent_confirm_busy.is_some() || !self.agent_confirm_armed {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para emitir confirmação.".into());
            cx.notify();
            return;
        }
        self.agent_confirm_armed = false;
        self.agent_confirm_busy = Some(tool_name.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.create_tool_confirmation(&tool_name).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_confirm_busy = None;
                            match res {
                                Ok(_) => {
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Confirmação de uso único emitida no core para \"{tool_name}\" (token mantido oculto)."
                                        )
                                        .into(),
                                    );
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn evaluate_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.agent_suggestions_loading {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para verificar sugestões.".into());
            cx.notify();
            return;
        }
        self.agent_suggestions_loading = true;
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_agent_suggestions(true, 50).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_suggestions_loading = false;
                            view.agent_suggestions_loaded = true;
                            match res {
                                Ok(resp) => {
                                    let pending_now =
                                        pending_suggestions_count(&resp.suggestions);
                                    view.agent_suggestions = resp.suggestions;
                                    view.agent_suggestions_unanswered = resp.unanswered_count;
                                    view.agent_suggestions_error = None;
                                    view.agent_action_notice = Some(
                                        if pending_now > 0 {
                                            format!("{pending_now} sugestão(ões) pendente(s).")
                                        } else {
                                            "Nenhuma nova sugestão contextual no momento (respeitando DND/cadência)."
                                                .to_string()
                                        }
                                        .into(),
                                    );
                                }
                                Err(err) => {
                                    view.agent_suggestions_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn respond_suggestion(&mut self, id: String, accept: bool, cx: &mut Context<Self>) {
        if self.agent_suggestion_busy.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para responder sugestão.".into());
            cx.notify();
            return;
        }
        self.agent_suggestion_busy = Some(id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = if accept {
                    api.accept_agent_suggestion(&id).await
                } else {
                    api.dismiss_agent_suggestion(&id, Some("Descartada pela interface"))
                        .await
                };
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_suggestion_busy = None;
                            match res {
                                Ok(resp) => {
                                    view.agent_action_notice = Some(
                                        if accept {
                                            match resp.plan {
                                                Some(plan) => format!(
                                                    "Sugestão aceita — plano criado ({}).",
                                                    plan.status
                                                ),
                                                None => "Sugestão aceita.".to_string(),
                                            }
                                        } else {
                                            "Sugestão descartada.".to_string()
                                        }
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_daily_briefing(&mut self, cx: &mut Context<Self>) {
        if self.agent_briefing_loading {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_briefing_error =
                Some("Token ausente (USAMI_API_TOKEN) para gerar o briefing.".into());
            cx.notify();
            return;
        }
        self.agent_briefing_loading = true;
        self.agent_briefing_error = None;
        self.agent_action_notice = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_daily_briefing().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_briefing_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.agent_briefing = Some(resp.briefing);
                                    view.agent_briefing_error = None;
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_briefing_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn toggle_agent_skill(&mut self, skill_id: String, enable: bool, cx: &mut Context<Self>) {
        if self.agent_skill_busy_id.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para alterar skills.".into());
            cx.notify();
            return;
        }
        self.agent_skill_busy_id = Some(skill_id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.toggle_skill_enabled(&skill_id, enable).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_skill_busy_id = None;
                            match res {
                                Ok(resp) => {
                                    let label = resp
                                        .skill
                                        .as_ref()
                                        .and_then(|s| s.name.as_deref())
                                        .unwrap_or(&skill_id);
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Skill \"{label}\" {}.",
                                            if enable { "ativada" } else { "desativada" }
                                        )
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn push_agent_feedback_banners(&self, mut section: Div) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        if let Some(err) = self.agent_action_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }
        if let Some(notice) = self.agent_action_notice.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.surface_soft)
                    .border_1()
                    .border_color(c.success_border)
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .child(notice),
            );
        }
        section
    }

    fn render_agent_permissions_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        if let Some(err) = self.agent_permissions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if self.agent_permissions_loading && !self.agent_permissions_loaded {
            return section.child(
                div()
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Carregando permissões e ferramentas…"),
            );
        }

        if !self.agent_permissions.is_empty() {
            let mut rev_box = div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(c.danger_muted_border)
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.err)
                        .child(format!(
                            "REVOGAÇÕES ATIVAS ({})",
                            self.agent_permissions.len()
                        )),
                );

            for rev in self.agent_permissions.clone() {
                let target = rev.tool.clone();
                let is_busy = self.agent_permission_busy.as_deref() == Some(target.as_str());
                let when = format_execution_time(&rev.revoked_at);
                let mut detail = Vec::new();
                if let Some(reason) = rev.reason.filter(|s| !s.trim().is_empty()) {
                    detail.push(reason);
                }
                if !when.is_empty() {
                    detail.push(when);
                }
                let target_click = target.clone();
                rev_box = rev_box.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .p(px(sp.s2))
                        .rounded(px(r.sm))
                        .bg(c.deep)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(target.clone()),
                                )
                                .when(!detail.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(detail.join(" · ")),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-perm-restore-{target}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(c.success_border)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.ok)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.revoke_or_restore_permission(
                                        target_click.clone(),
                                        false,
                                        None,
                                        cx,
                                    );
                                }))
                                .child(if is_busy { "…" } else { "Restaurar" }),
                        ),
                );
            }
            section = section.child(rev_box);
        }

        let groups = group_tools_by_namespace(&self.agent_tools);
        if groups.is_empty() && self.agent_permissions_loaded {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma ferramenta registrada no core no momento."),
            );
        }

        for group in groups {
            let wildcard = format!("{}.*", group.namespace);
            let ns_revoked = self
                .agent_permissions
                .iter()
                .any(|rev| rev.tool == wildcard);
            let ns_busy = self.agent_permission_busy.as_deref() == Some(wildcard.as_str());
            let wildcard_click = wildcard.clone();
            let next_ns_revoke = !ns_revoked;

            let mut group_card = div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if ns_revoked {
                    c.danger_muted_border
                } else {
                    c.line
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.lavender)
                                .child(format!(
                                    "INTEGRAÇÃO: {} ({})",
                                    group.namespace.to_uppercase(),
                                    group.tools.len()
                                )),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-ns-toggle-{wildcard}")))
                                .cursor_pointer()
                                .px(px(9.0))
                                .py(px(3.5))
                                .rounded(px(r.sm))
                                .bg(c.deep)
                                .border_1()
                                .border_color(if ns_revoked {
                                    c.success_border
                                } else {
                                    c.danger_muted_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if ns_revoked { c.ok } else { c.danger_muted })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let reason = if next_ns_revoke {
                                        Some(format!("Integração {wildcard_click} revogada no desktop"))
                                    } else {
                                        None
                                    };
                                    this.revoke_or_restore_permission(
                                        wildcard_click.clone(),
                                        next_ns_revoke,
                                        reason,
                                        cx,
                                    );
                                }))
                                .child(if ns_busy {
                                    "…".to_string()
                                } else if ns_revoked {
                                    format!("Restaurar {wildcard}")
                                } else {
                                    format!("Revogar {wildcard}")
                                }),
                        ),
                );

            for tool in group.tools {
                let tname = tool.name.clone();
                let is_busy = self.agent_permission_busy.as_deref() == Some(tname.as_str());
                let level_label = permission_level_label(tool.permission_level);
                let origin = origin_label(tool.origin.as_deref());
                let (status_text, status_color, status_border) = if tool.revoked {
                    ("Revogada", c.err, c.danger_muted_border)
                } else if tool.active {
                    ("Ativa", c.ok, c.success_border)
                } else {
                    ("Inativa", c.warn, c.accent_border)
                };
                let tname_click = tname.clone();
                let next_revoke = !tool.revoked;

                let mut item = div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s1_5))
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.deep)
                    .border_1()
                    .border_color(if tool.revoked {
                        c.danger_muted_border
                    } else {
                        c.line
                    })
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.pearl)
                                            .child(tname.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(format!("{origin} · {level_label}")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(status_border)
                                    .text_size(px(ty.xs))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(status_color)
                                    .child(status_text),
                            ),
                    );

                if !tool.description.trim().is_empty() {
                    item = item.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(tool.description),
                    );
                }
                if let Some(reason) = tool.revoked_reason.filter(|s| !s.trim().is_empty()) {
                    item = item.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(format!("Motivo: {reason}")),
                    );
                }

                item = item.child(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-tool-perm-{tname}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(if tool.revoked {
                                    c.success_border
                                } else {
                                    c.danger_muted_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if tool.revoked { c.ok } else { c.danger_muted })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let reason = if next_revoke {
                                        Some("Revogada manualmente no desktop".to_string())
                                    } else {
                                        None
                                    };
                                    this.revoke_or_restore_permission(
                                        tname_click.clone(),
                                        next_revoke,
                                        reason,
                                        cx,
                                    );
                                }))
                                .child(if is_busy {
                                    "Atualizando…"
                                } else if tool.revoked {
                                    "Restaurar"
                                } else {
                                    "Revogar"
                                }),
                        ),
                );

                group_card = group_card.child(item);
            }

            section = section.child(group_card);
        }

        section
    }

    fn render_agent_confirmations_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let armed = self.agent_confirm_armed;
        let busy_key = self.agent_confirm_busy.clone();
        let perm_busy = self.agent_permission_busy.clone();

        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        if let Some(err) = self.agent_permissions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        // Passo 1 de segurança (espelha o padrão de confirmação dupla da aba Memória).
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if armed { c.accent } else { c.line_strong })
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.muted)
                        .child("Aprovações usam token efêmero de uso único (CONFIRMAR) no core. Nenhum token é exibido ou armazenado na interface."),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed { c.warn } else { c.faint })
                                .child(if armed {
                                    "Armado para 1 confirmação"
                                } else {
                                    "Protegido — arme antes de aprovar"
                                }),
                        )
                        .child(
                            div()
                                .id("agent-confirm-arm-btn")
                                .cursor_pointer()
                                .px(px(11.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(if armed { c.surface_soft } else { c.accent_tint })
                                .border_1()
                                .border_color(if armed { c.warn } else { c.accent })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_confirm_armed = !this.agent_confirm_armed;
                                    this.poke(cx);
                                    cx.notify();
                                }))
                                .child(if armed {
                                    "Cancelar armamento"
                                } else {
                                    "Armar confirmação"
                                }),
                        ),
                ),
        );

        // Junta skills em estado pending + proposals pending sem duplicar ids.
        struct PendingSkillEntry {
            skill_id: String,
            title: String,
            version: Option<String>,
            benefit: Option<String>,
            risks: Option<String>,
            dependencies: Vec<String>,
            max_level: u8,
        }
        let mut pending_entries: Vec<PendingSkillEntry> = Vec::new();
        for skill in &self.agent_skills {
            if skill.state.as_deref() == Some("pending") {
                let proposal = self
                    .agent_proposals
                    .iter()
                    .find(|p| p.skill_id == skill.id && p.status == "pending");
                pending_entries.push(PendingSkillEntry {
                    skill_id: skill.id.clone(),
                    title: skill
                        .name
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| skill.id.clone()),
                    version: skill.version.clone(),
                    benefit: proposal
                        .map(|p| p.benefit.clone())
                        .or_else(|| skill.description.clone()),
                    risks: proposal.map(|p| p.risks.clone()),
                    dependencies: proposal
                        .map(|p| p.dependencies.clone())
                        .unwrap_or_default(),
                    max_level: skill.max_permission_level.unwrap_or(1),
                });
            }
        }
        for prop in &self.agent_proposals {
            if prop.status == "pending"
                && !pending_entries.iter().any(|e| e.skill_id == prop.skill_id)
            {
                let max_level = prop.permissions.iter().copied().max().unwrap_or(1);
                pending_entries.push(PendingSkillEntry {
                    skill_id: prop.skill_id.clone(),
                    title: prop.skill_id.clone(),
                    version: prop.version.clone(),
                    benefit: Some(prop.benefit.clone()),
                    risks: Some(prop.risks.clone()),
                    dependencies: prop.dependencies.clone(),
                    max_level,
                });
            }
        }

        let pending_plans: Vec<api::AgentPlan> = self
            .agent_plans
            .iter()
            .filter(|plan| plan.status == "awaiting_approval")
            .cloned()
            .collect();
        let level4_tools: Vec<api::SelfToolSummary> = self
            .agent_tools
            .iter()
            .filter(|tool| tool.permission_level == 4)
            .cloned()
            .collect();

        if pending_entries.is_empty() && pending_plans.is_empty() && level4_tools.is_empty() {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma confirmação pendente no momento."),
            );
        }

        if !pending_entries.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "SKILLS PENDENTES DE APROVAÇÃO ({})",
                        pending_entries.len()
                    )),
            );

            for entry in pending_entries {
                let sid = entry.skill_id.clone();
                let is_busy = busy_key.as_deref() == Some(sid.as_str())
                    || perm_busy.as_deref() == Some(format!("skill.{sid}").as_str());
                let sid_approve = sid.clone();
                let sid_deny = sid.clone();
                let ver = entry.version.unwrap_or_default();
                let mut card = div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.accent_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.md))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(if ver.is_empty() {
                                        entry.title
                                    } else {
                                        format!("{} v{ver}", entry.title)
                                    }),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(c.deep)
                                    .border_1()
                                    .border_color(c.accent_border)
                                    .text_size(px(ty.xs))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.warn)
                                    .child(permission_level_label(entry.max_level)),
                            ),
                    );

                if let Some(benefit) = entry.benefit.filter(|s| !s.trim().is_empty()) {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(format!("Benefício: {benefit}")),
                    );
                }
                if let Some(risks) = entry.risks.filter(|s| !s.trim().is_empty()) {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.warn)
                            .child(format!("Riscos: {risks}")),
                    );
                }
                if !entry.dependencies.is_empty() {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.muted)
                            .child(format!("Dependências: {}", entry.dependencies.join(", "))),
                    );
                }

                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-skill-deny-{sid}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.danger_muted)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.deny_pending_skill(sid_deny.clone(), cx);
                                }))
                                .child("Negar"),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-skill-approve-{sid}")))
                                .when(armed && !is_busy, |el| el.cursor_pointer())
                                .px(px(11.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(if armed { c.accent_tint } else { c.deep })
                                .border_1()
                                .border_color(if armed { c.accent } else { c.line })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed { c.pearl } else { c.faint })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.approve_pending_skill(sid_approve.clone(), cx);
                                }))
                                .child(if is_busy {
                                    "Confirmando…"
                                } else {
                                    "Aprovar e instalar"
                                }),
                        ),
                );

                section = section.child(card);
            }
        }

        if !pending_plans.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "PLANOS AGUARDANDO APROVAÇÃO ({})",
                        pending_plans.len()
                    )),
            );

            for plan in pending_plans {
                let mut steps_summary = Vec::new();
                for (idx, step) in plan.steps.iter().enumerate() {
                    let label = step
                        .tool
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(step.title.as_str());
                    let line = if let Some(lvl) = step.permission_level {
                        format!("Passo {}: {label} (nível {lvl})", idx + 1)
                    } else {
                        format!("Passo {}: {label}", idx + 1)
                    };
                    steps_summary.push(line);
                }
                let when = format_execution_time(&plan.created_at);
                section = section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(sp.s1_5))
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .text_size(px(ty.sm))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .child(plan.goal),
                        )
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .text_color(c.muted)
                                .child(format!(
                                    "ID {} · {} · {}",
                                    plan.id,
                                    plan.origin.unwrap_or_else(|| "core".into()),
                                    when
                                )),
                        )
                        .when(!steps_summary.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child(steps_summary.join(" · ")),
                            )
                        }),
                );
            }
        }

        if !level4_tools.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "FERRAMENTAS SENSÍVEIS — NÍVEL 4 ({})",
                        level4_tools.len()
                    )),
            );

            for tool in level4_tools {
                let tname = tool.name.clone();
                let is_busy = busy_key.as_deref() == Some(tname.as_str());
                let tname_click = tname.clone();
                section = section.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(tname.clone()),
                                )
                                .when(!tool.description.trim().is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(tool.description),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-tool-confirm-{tname}")))
                                .when(armed && !is_busy && !tool.revoked, |el| el.cursor_pointer())
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(if armed && !tool.revoked {
                                    c.accent_tint
                                } else {
                                    c.deep
                                })
                                .border_1()
                                .border_color(if armed && !tool.revoked {
                                    c.accent
                                } else {
                                    c.line
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed && !tool.revoked {
                                    c.pearl
                                } else {
                                    c.faint
                                })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if !tool.revoked {
                                        this.request_level4_tool_confirmation(
                                            tname_click.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child(if tool.revoked {
                                    "Revogada"
                                } else if is_busy {
                                    "Emitindo…"
                                } else {
                                    "Preparar uso único"
                                }),
                        ),
                );
            }
        }

        section
    }

    fn render_agent_suggestions_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let quiet_now = is_local_quiet_hours_now();
        let unanswered = self.agent_suggestions_unanswered;
        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        // Barra DND / Throttle + Ações explícitas.
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(c.line)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1_5))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.5))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(if quiet_now {
                                    c.accent_border
                                } else {
                                    c.success_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if quiet_now { c.warn } else { c.ok })
                                .child(if quiet_now {
                                    "Modo silencioso (23:30–08:30)"
                                } else {
                                    "Janela ativa"
                                }),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.5))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .text_color(if unanswered >= 2 { c.warn } else { c.muted })
                                .child(format!("Sem resposta: {unanswered}/2")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .id("agent-eval-suggestions-btn")
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.primary)
                                .hover(|s| s.border_color(c.primary))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.evaluate_suggestions(cx);
                                }))
                                .child(if self.agent_suggestions_loading {
                                    "Verificando…"
                                } else {
                                    "Verificar sugestões"
                                }),
                        )
                        .child(
                            div()
                                .id("agent-briefing-btn")
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(c.accent_tint)
                                .border_1()
                                .border_color(c.accent)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.fetch_daily_briefing(cx);
                                }))
                                .child(if self.agent_briefing_loading {
                                    "Gerando briefing…"
                                } else {
                                    "Gerar Briefing de Hoje"
                                }),
                        ),
                ),
        );

        if let Some(err) = self.agent_briefing_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if let Some(briefing) = self.agent_briefing.clone() {
            let counts = briefing.counts;
            let source = briefing
                .narrative_source
                .unwrap_or_else(|| "template".to_string());
            let body = if briefing.empty && briefing.text.trim().is_empty() {
                "Nenhum item real encontrado no vault para o briefing de hoje.".to_string()
            } else {
                briefing.text
            };
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.accent_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(format!("Briefing · {}", briefing.date)),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(source),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.muted)
                            .child(format!(
                                "{} compromissos · {} tarefas · {} projetos · {} pendentes · {} planos",
                                counts.commitments,
                                counts.priority_tasks,
                                counts.projects,
                                counts.pending,
                                counts.pending_plans
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(body),
                    ),
            );
        }

        if let Some(err) = self.agent_suggestions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if self.agent_suggestions_loading && !self.agent_suggestions_loaded {
            return section.child(
                div()
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Carregando sugestões contextuais…"),
            );
        }

        if self.agent_suggestions.is_empty() && self.agent_suggestions_loaded {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma sugestão contextual disponível no momento."),
            );
        }

        let mut ordered = self.agent_suggestions.clone();
        ordered.sort_by_key(|s| if s.status == "pending" { 0 } else { 1 });

        for item in ordered {
            let sid = item.id.clone();
            let is_pending = item.status == "pending";
            let is_busy = self.agent_suggestion_busy.as_deref() == Some(sid.as_str());
            let kind_lbl = suggestion_kind_label(&item.kind);
            let status_lbl = suggestion_status_label(&item.status);
            let rel_lbl = format_relevance_pct(item.relevance);
            let when = format_execution_time(&item.created_at);
            let status_color = match item.status.as_str() {
                "accepted" => c.ok,
                "dismissed" | "expired" => c.muted,
                _ => c.warn,
            };

            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(sp.s1_5))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if is_pending { c.accent_border } else { c.line })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .text_color(c.muted)
                                .child(format!("{kind_lbl} · Relevância {rel_lbl} · {when}")),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(status_color)
                                .child(status_lbl),
                        ),
                )
                .child(
                    div()
                        .text_size(px(ty.sm))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(item.title),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.pearl)
                        .child(item.summary),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.muted)
                        .child(format!("Objetivo sugerido: {}", item.recommended_goal)),
                );

            if let Some(plan_id) = item.plan_id.filter(|s| !s.trim().is_empty()) {
                card = card.child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.ok)
                        .child(format!("Plano associado: {plan_id}")),
                );
            }

            if is_pending {
                let sid_accept = sid.clone();
                let sid_dismiss = sid.clone();
                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(sp.s2))
                        .pt(px(sp.s1))
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-sugg-dismiss-{sid}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.muted)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.respond_suggestion(sid_dismiss.clone(), false, cx);
                                }))
                                .child(if is_busy { "…" } else { "Descartar" }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-sugg-accept-{sid}")))
                                .cursor_pointer()
                                .px(px(11.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.accent_tint)
                                .border_1()
                                .border_color(c.accent)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.respond_suggestion(sid_accept.clone(), true, cx);
                                }))
                                .child(if is_busy { "…" } else { "Aceitar" }),
                        ),
                );
            }

            section = section.child(card);
        }

        section
    }

    fn render_agent_center_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let active_tab = self.agent_tab;
        let skill_filter = self.agent_skill_filter;
        let exec_filter = self.agent_exec_filter;
        let loading = self.agent_skills_loading
            || self.agent_executions_loading
            || self.agent_permissions_loading
            || self.agent_suggestions_loading;
        let busy_id = self.agent_skill_busy_id.clone();

        let skills_count = self.agent_skills.len();
        let execs_count = self.agent_executions.len();
        let tools_count = self.agent_tools.len();
        let pending_confirm_count = pending_confirmations_count(
            &self.agent_skills,
            &self.agent_proposals,
            &self.agent_plans,
        );
        let pending_sugg_count = pending_suggestions_count(&self.agent_suggestions);
        let (buckets, invalid_skills) = group_skills_by_state(&self.agent_skills);
        let (exec_total, exec_verified, exec_failed) = summarize_executions(&self.agent_executions);

        let pill = |id: &'static str, label: String, active: bool| {
            div()
                .id(id)
                .cursor_pointer()
                .px(px(10.0))
                .py(px(4.5))
                .rounded_full()
                .border_1()
                .bg(if active { c.accent_tint } else { c.surface_soft })
                .border_color(if active { c.accent } else { c.line_strong })
                .text_size(px(ty.xs))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if active { c.pearl } else { c.muted })
                .hover(|s| s.border_color(c.primary).text_color(c.pearl))
                .child(label)
        };

        let body_el: AnyElement = match active_tab {
            AgentCenterTab::Skills => {
                let installed_len = buckets[0].skills.len();
                let available_len = buckets[1].skills.len();
                let pending_len = buckets[2].skills.len();
                let disabled_len = buckets[3].skills.len();

                let mut section = div().flex().flex_col().gap(px(sp.s3));

                section = section.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s1_5))
                        .child(
                            pill(
                                "agent-skill-filter-all",
                                format!("Todas ({skills_count})"),
                                skill_filter == SkillFilter::All,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::All;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-installed",
                                format!("Instaladas ({installed_len})"),
                                skill_filter == SkillFilter::Installed,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Installed;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-available",
                                format!("Disponíveis ({available_len})"),
                                skill_filter == SkillFilter::Available,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Available;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-pending",
                                format!("Pendentes ({pending_len})"),
                                skill_filter == SkillFilter::Pending,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Pending;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-disabled",
                                format!("Desativadas ({disabled_len})"),
                                skill_filter == SkillFilter::Disabled,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Disabled;
                                this.poke(cx);
                                cx.notify();
                            })),
                        ),
                );

                if let Some(err) = self.agent_action_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }
                if let Some(notice) = self.agent_action_notice.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.success_border)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(notice),
                    );
                }
                if let Some(err) = self.agent_skills_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }

                if !self.agent_skills_loaded {
                    section = section.child(settings_hint("Lendo catálogo de skills…"));
                } else {
                    for bucket in buckets
                        .into_iter()
                        .filter(|b| skill_filter.matches_state(b.key))
                    {
                        let count = bucket.skills.len();
                        let mut group_el = div()
                            .flex()
                            .flex_col()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .mt(px(sp.s1))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(bucket.label.to_uppercase()),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(1.5))
                                            .rounded_full()
                                            .bg(c.card_bg)
                                            .border_1()
                                            .border_color(c.line)
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(format!("{count}")),
                                    ),
                            );

                        if bucket.skills.is_empty() {
                            group_el = group_el.child(
                                div()
                                    .p(px(sp.s2_5))
                                    .rounded(px(r.sm))
                                    .bg(c.deep)
                                    .border_1()
                                    .border_color(c.line)
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(bucket.empty_message),
                            );
                        } else {
                            for skill in bucket.skills {
                                let sid = skill.id.clone();
                                let title = skill.name.clone().unwrap_or_else(|| sid.clone());
                                let st = skill.state.as_deref().unwrap_or("unknown");
                                let version = skill.version.as_deref().unwrap_or("0.0.0");
                                let origin = origin_label(skill.origin.as_deref());
                                let perm = skill.max_permission_level.unwrap_or(1);
                                let reasons = collect_skill_reasons(&skill);
                                let badge_color = match st {
                                    "installed" => c.ok,
                                    "available" => c.primary,
                                    "pending" => c.warn,
                                    _ => c.muted,
                                };
                                let badge_border = match st {
                                    "installed" => c.success_border,
                                    "available" => c.accent_border,
                                    _ => c.line_strong,
                                };
                                let is_busy = busy_id.as_deref() == Some(sid.as_str());
                                let can_disable = st == "installed";
                                let can_enable = st == "disabled";

                                let mut card = div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(sp.s2))
                                    .p(px(sp.s3))
                                    .rounded(px(r.md))
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .flex()
                                            .items_start()
                                            .justify_between()
                                            .gap(px(sp.s2))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.md))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(c.pearl)
                                                            .child(title),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.muted)
                                                            .child(format!(
                                                                "{sid} · v{version} · {origin} · nível {perm}"
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px(px(8.0))
                                                    .py(px(2.5))
                                                    .rounded_full()
                                                    .bg(c.deep)
                                                    .border_1()
                                                    .border_color(badge_border)
                                                    .text_size(px(ty.xs))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(badge_color)
                                                    .child(state_label(st)),
                                            ),
                                    );

                                if let Some(desc) = skill.description.filter(|d| !d.trim().is_empty()) {
                                    card = card.child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .child(desc),
                                    );
                                }

                                let mut foot = div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(sp.s2))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if skill.activation.can_activate {
                                                c.ok
                                            } else {
                                                c.muted
                                            })
                                            .child(if skill.activation.can_activate {
                                                "Pronta para ativar"
                                            } else {
                                                "Não ativável agora"
                                            }),
                                    );

                                if can_disable || can_enable {
                                    let btn_id = format!("skill-toggle-{sid}");
                                    let skill_for_click = sid.clone();
                                    let next_enable = can_enable;
                                    let label = if is_busy {
                                        "Atualizando…"
                                    } else if can_enable {
                                        "Ativar"
                                    } else {
                                        "Desativar"
                                    };
                                    foot = foot.child(
                                        div()
                                            .id(SharedString::from(btn_id))
                                            .cursor_pointer()
                                            .px(px(10.0))
                                            .py(px(4.5))
                                            .rounded(px(r.sm))
                                            .bg(if can_enable {
                                                c.accent_tint
                                            } else {
                                                c.deep
                                            })
                                            .border_1()
                                            .border_color(if can_enable {
                                                c.accent
                                            } else {
                                                c.line_strong
                                            })
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c.pearl)
                                            .when(busy_id.is_some(), |el| el.opacity(0.55))
                                            .hover(|s| s.border_color(c.primary))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.toggle_agent_skill(
                                                    skill_for_click.clone(),
                                                    next_enable,
                                                    cx,
                                                );
                                            }))
                                            .child(label),
                                    );
                                }

                                card = card.child(foot);

                                if !reasons.is_empty() {
                                    let mut reasons_el = div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .pt(px(2.0));
                                    for reason in reasons {
                                        reasons_el = reasons_el.child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .text_color(c.primary)
                                                .child(format!("• {reason}")),
                                        );
                                    }
                                    card = card.child(reasons_el);
                                }

                                group_el = group_el.child(card);
                            }
                        }

                        section = section.child(group_el);
                    }

                    if !invalid_skills.is_empty() {
                        let mut inv_el = div()
                            .flex()
                            .flex_col()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.err)
                                    .child("MANIFESTOS INVÁLIDOS"),
                            );
                        for skill in invalid_skills {
                            let reasons = collect_skill_reasons(&skill);
                            let mut card = div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1_5))
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .child(
                                    div()
                                        .text_size(px(ty.md))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(skill.id),
                                );
                            for reason in reasons {
                                card = card.child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.err)
                                        .child(format!("• {reason}")),
                                );
                            }
                            inv_el = inv_el.child(card);
                        }
                        section = section.child(inv_el);
                    }
                }

                section.into_any_element()
            }
            AgentCenterTab::Executions => {
                let mut section = div().flex().flex_col().gap(px(sp.s3));

                section = section
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(sp.s3))
                            .p(px(sp.s2_5))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .child(
                                div()
                                    .text_color(c.pearl)
                                    .child(format!("Total: {exec_total}")),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.ok)
                                    .child(format!("Verificadas: {exec_verified}")),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.err)
                                    .child(format!("Falhas: {exec_failed}")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(sp.s1_5))
                            .child(
                                pill(
                                    "agent-exec-filter-all",
                                    format!("Todas ({exec_total})"),
                                    exec_filter == ExecFilter::All,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::All;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            )
                            .child(
                                pill(
                                    "agent-exec-filter-verified",
                                    format!("Verificadas ({exec_verified})"),
                                    exec_filter == ExecFilter::Verified,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::Verified;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            )
                            .child(
                                pill(
                                    "agent-exec-filter-failed",
                                    format!("Falhas ({exec_failed})"),
                                    exec_filter == ExecFilter::Failed,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::Failed;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            ),
                    );

                if let Some(err) = self.agent_executions_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }

                if !self.agent_executions_loaded {
                    section = section.child(settings_hint("Lendo histórico de execuções…"));
                } else {
                    let visible: Vec<_> = self
                        .agent_executions
                        .iter()
                        .filter(|rec| match exec_filter {
                            ExecFilter::All => true,
                            ExecFilter::Verified => classify_execution_status(rec).verified,
                            ExecFilter::Failed => !classify_execution_status(rec).verified,
                        })
                        .cloned()
                        .collect();

                    if visible.is_empty() {
                        let empty_msg = if self.agent_executions.is_empty() {
                            "Nenhuma execução registrada ainda."
                        } else {
                            "Nenhuma execução corresponde ao filtro selecionado."
                        };
                        section = section.child(
                            div()
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(ty.sm))
                                .text_color(c.muted)
                                .child(empty_msg),
                        );
                    } else {
                        for record in visible {
                            let status = classify_execution_status(&record);
                            let status_color = match status.tone {
                                ExecutionTone::Ok => c.ok,
                                ExecutionTone::Warn => c.warn,
                                ExecutionTone::Err => c.err,
                            };
                            let status_border = match status.tone {
                                ExecutionTone::Ok => c.success_border,
                                ExecutionTone::Warn => c.accent_border,
                                ExecutionTone::Err => c.danger_muted_border,
                            };
                            let when = format_execution_time(&record.started_at);
                            let dur = format_duration_ms(record.duration_ms);
                            let mut meta_parts = Vec::new();
                            if !when.is_empty() {
                                meta_parts.push(when);
                            }
                            if !dur.is_empty() {
                                meta_parts.push(dur);
                            }
                            if let Some(orig) = record.origin.as_deref() {
                                meta_parts.push(orig.to_string());
                            }
                            if let Some(perm) = record.permission_level {
                                meta_parts.push(format!("nível {perm}"));
                            }

                            let mut card = div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1_5))
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .flex()
                                        .items_start()
                                        .justify_between()
                                        .gap(px(sp.s2))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_size(px(ty.md))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(c.pearl)
                                                        .child(record.tool),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.muted)
                                                        .child(meta_parts.join(" · ")),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .px(px(8.0))
                                                .py(px(2.5))
                                                .rounded_full()
                                                .bg(c.deep)
                                                .border_1()
                                                .border_color(status_border)
                                                .text_size(px(ty.xs))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(status_color)
                                                .child(status.label),
                                        ),
                                );

                            if let Some(summary) = record
                                .input_summary
                                .filter(|s| !s.trim().is_empty())
                            {
                                card = card.child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child(summary),
                                );
                            }

                            if let Some(reason) = record.reason.filter(|s| !s.trim().is_empty()) {
                                card = card.child(
                                    div()
                                        .p(px(sp.s2))
                                        .rounded(px(r.sm))
                                        .bg(if status.tone == ExecutionTone::Ok {
                                            c.surface_soft
                                        } else {
                                            c.danger_muted_bg
                                        })
                                        .border_1()
                                        .border_color(status_border)
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .child(format!("Motivo: {reason}")),
                                );
                            }

                            section = section.child(card);
                        }
                    }
                }

                section.into_any_element()
            }
            AgentCenterTab::Permissions => self.render_agent_permissions_tab(cx).into_any_element(),
            AgentCenterTab::Confirmations => {
                self.render_agent_confirmations_tab(cx).into_any_element()
            }
            AgentCenterTab::Suggestions => self.render_agent_suggestions_tab(cx).into_any_element(),
        };

        let agent_tab_btn = |id: &'static str, label: String, tab: AgentCenterTab| {
            let is_active = active_tab == tab;
            div()
                .id(id)
                .cursor_pointer()
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(r.sm))
                .border_1()
                .bg(if is_active {
                    c.accent_tint
                } else {
                    c.surface_soft
                })
                .border_color(if is_active { c.accent } else { c.line })
                .text_size(px(ty.xs))
                .font_weight(FontWeight::BOLD)
                .text_color(c.pearl)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.agent_tab = tab;
                    this.poke(cx);
                    cx.notify();
                }))
                .child(label)
        };

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(sp.s3))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(sp.s2))
                    .pb(px(sp.s2))
                    .border_b_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(sp.s1_5))
                            .child(agent_tab_btn(
                                "agent-tab-skills",
                                format!("Skills ({skills_count})"),
                                AgentCenterTab::Skills,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-executions",
                                format!("Execuções ({execs_count})"),
                                AgentCenterTab::Executions,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-permissions",
                                format!("Permissões ({tools_count})"),
                                AgentCenterTab::Permissions,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-confirmations",
                                format!("Confirmações ({pending_confirm_count})"),
                                AgentCenterTab::Confirmations,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-suggestions",
                                format!("Sugestões ({pending_sugg_count})"),
                                AgentCenterTab::Suggestions,
                            )),
                    )
                    .child(
                        div()
                            .id("agent-center-refresh")
                            .cursor_pointer()
                            .px(px(10.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line_strong)
                            .text_size(px(ty.xs))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.primary)
                            .hover(|s| s.border_color(c.primary))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_agent_center(cx);
                                this.poke(cx);
                            }))
                            .child(if loading { "Atualizando…" } else { "Atualizar" }),
                    ),
            )
            .child(
                div()
                    .id("agent-center-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .pr(px(4.0))
                    .child(body_el),
            )
    }

    /// Mesma regra do web: cada mensagem rende XP, mas quem soma é o core.
    fn award_bond_chat(&mut self, content: String, spoken: bool, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.award_bond_chat(&content, spoken).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    cx.notify();
                                }
                                Err(error) => eprintln!("[Vínculo] evento de chat falhou: {error}"),
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Chamada de voz rende companhia por minuto inteiro, como no web.
    fn award_bond_call(&mut self, minutes: u32, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || minutes == 0 {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.award_bond_call(minutes).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    cx.notify();
                                }
                                Err(error) => eprintln!("[Vínculo] evento de chamada falhou: {error}"),
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_memory(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.memory_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.memory_loading = true;
        self.memory_error = None;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_memory().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(ref aff) = resp.memory.affinity {
                                        view.server_affinity = Some(aff.score);
                                    }
                                    view.memory_data = Some(resp.memory);
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn delete_memory_fact(&mut self, section: String, fact: String, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "fact".into(),
            confirm: "APAGAR".into(),
            section: Some(section),
            fact: Some(fact),
            date: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.forget_memory(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    }
                                    view.memory_status = Some("Fato removido com sucesso.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao excluir: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn delete_memory_all(&mut self, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "all".into(),
            confirm: "APAGAR".into(),
            section: None,
            fact: None,
            date: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.forget_memory(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    } else {
                                        view.memory_data = Some(api::MemoryData::default());
                                    }
                                    view.memory_confirm_active = false;
                                    view.memory_status =
                                        Some("Toda a memória foi redefinida.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao apagar tudo: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_personality(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.personality_loading = true;
        self.personality_error = None;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_personality().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn adjust_trait(&mut self, trait_name: &'static str, delta: i32) {
        if let Some(ref mut data) = self.personality_data {
            let val = match trait_name {
                "warmth" => &mut data.traits.warmth,
                "curiosity" => &mut data.traits.curiosity,
                "playfulness" => &mut data.traits.playfulness,
                "pride" => &mut data.traits.pride,
                _ => return,
            };
            let new_val = (*val as i32 + delta).clamp(0, 100) as u32;
            *val = new_val;
        }
    }

    fn save_personality(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.personality_data.clone() else {
            return;
        };
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: Some(data.traits),
            custom_prompt: Some(data.custom_prompt),
            reset: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.update_personality(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade salva no servidor!".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao salvar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn reset_personality(&mut self, cx: &mut Context<Self>) {
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: None,
            custom_prompt: None,
            reset: Some(true),
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.update_personality(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade restaurada para o padrão (50).".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao restaurar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }
}

// ── COMPONENTES DO HUB DE VISUAL NOVEL DE ROMANCE ──

fn render_vn_tab_btn(
    id: &'static str,
    number: &'static str,
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme::theme();
    let c = t.color;
    div()
        .id(id)
        .cursor_pointer()
        .h(px(38.0))
        .px(px(12.0))
        .rounded(px(12.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .bg(if is_active {
            t.vn_tab_active_bg()
        } else {
            theme::Theme::gradient(90.0, rgba(0x00000000), rgba(0x00000000))
        })
        .border_1()
        .border_color(if is_active {
            theme::Palette::vn_border_bright()
        } else {
            rgba(0x00000000)
        })
        .shadow(if is_active {
            t.shadow.vn_card.clone()
        } else {
            vec![]
        })
        .hover(move |s| {
            if !is_active {
                s.bg(rgba(0xf5c3e126))
            } else {
                s
            }
        })
        .on_click(on_click)
        // Numeração de capítulo em serifa
        .child(
            div()
                .w(px(22.0))
                .text_size(px(12.0))
                .font_family(theme::FONT_DISPLAY)
                .font_weight(FontWeight::MEDIUM)
                .text_color(if is_active {
                    c.pearl
                } else {
                    theme::Palette::vn_pink_soft()
                })
                .child(number),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.5))
                .font_family(theme::FONT_BODY)
                .font_weight(if is_active {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_active { c.pearl } else { c.muted })
                .child(label),
        )
        // Ornamento no estado ativo
        .when(is_active, |el| {
            el.child(
                div()
                    .text_size(px(10.0))
                    .text_color(c.pearl)
                    .child("✦"),
            )
        })
}

/// Formata data civil (dd/mm/aaaa) a partir de epoch ms, sem dependência de
/// crate de data: converte dias epoch → ano/mês/dia pelo algoritmo civil.
fn format_date(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:02}/{:02}/{:04}", d, m, year)
}

/// Data curta de um `unlockedAt` ISO (`2026-11-10T18:30:00.000Z`).
fn format_unlocked_at(iso: &str) -> String {
    let date = iso.get(..10).unwrap_or(iso);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day)) => format!("{day}/{month}/{}", &year[..year.len().min(4)]),
        _ => date.to_string(),
    }
}

/// Resumo diário é um parágrafo inteiro; nas listas mostramos só o começo.
fn truncate_chars(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// Saída da janela principal: capítulo "Início" — cena de convivência com a
/// personagem (caixa de diálogo VN + indicadores compactos de vínculo).
fn render_vn_tab_home(
    bond_level: u32,
    bond: Option<&api::BondData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let hour = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| ((elapsed.as_secs() / 3600) % 24) as i64)
        .unwrap_or(0);
    // A janela roda no fuso local do sistema (GetLocalTime); epoch é UTC, mas
    // para um cumprimento informal o desvio de horas é irrelevante.
    let greeting = match hour {
        5..=11 => "Bom dia",
        12..=17 => "Boa tarde",
        _ => "Boa noite",
    };
    let (level_name, line, exp_pct) = match bond {
        Some(b) => (
            b.summary.name.clone(),
            b.summary.line.clone(),
            (b.summary.progress.clamp(0.0, 1.0) * 100.0).round() as u32,
        ),
        None => ("—".to_string(), "—".to_string(), 0),
    };
    let dialogue = if line.is_empty() || line == "—" {
        "H-hmph! Você demorou... mas eu esperei aqui do seu lado o tempo todo. ♡".to_string()
    } else {
        line
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .overflow_hidden()
        // Cumprimento editorial
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(28.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.pearl)
                        .child(format!("{greeting},")),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme::Palette::vn_pink_soft())
                        .child("o quarto da Usami estava esperando por você."),
                ),
        )
        // Caixa de diálogo estilo Visual Novel
        .child(
            div()
                .p(px(14.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .shadow(t.shadow.vn_card.clone())
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(t.vn_tab_active_bg())
                        .text_size(px(11.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Usami ♡"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .line_height(px(21.0))
                        .font(theme::font_character_italic())
                        .text_color(c.pearl)
                        .child(format!("“{dialogue}”")),
                ),
        )
        // Indicadores compactos de relacionamento
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(10.0))
                // Nível
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Vínculo"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_pink_soft())
                                .child(format!("♥ Nv. {bond_level}")),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.faint)
                                .child(level_name),
                        ),
                )
                // EXP
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Experiência"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .items_end()
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(6.0))
                                        .rounded_full()
                                        .bg(c.bond_track)
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(exp_pct.clamp(0, 100) as f32 / 100.0))
                                                .rounded_full()
                                                .bg(t.vn_exp_fill_bg()),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::Palette::vn_pink_soft())
                                .child(format!("{exp_pct}%")),
                        ),
                )
                // Conquistas
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Conquistas"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_gold())
                                .child("✦ Emblemas"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.faint)
                                .child("Capítulo 05"),
                        ),
                ),
        )
}

/// Monta as páginas cronológicas do Diário unindo o histórico do vínculo e os
/// resumos diários da memória sem deixar o log recente apagar as lembranças antigas.
fn collect_diary_entries(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
) -> Vec<(String, String, String, String)> {
    let mut entries: Vec<(String, String, String, String)> = Vec::new();
    if let Some(b) = bond {
        entries.extend(b.log.iter().take(24).map(|entry| {
            let title = if entry.count > 1 {
                format!("{} (×{})", entry.text, entry.count)
            } else {
                entry.text.clone()
            };
            (
                format!("[{}]", vn_history_tag(&entry.kind)),
                title,
                relative_when(entry.at),
                format_date(entry.at),
            )
        }));
    }
    // O bond.json local pode estar zerado, mas os resumos diários da memória
    // continuam guardados no core — eles são as páginas antigas do diário.
    if let Some(m) = memory {
        entries.extend(m.summaries.iter().rev().take(24).map(|summary| {
            (
                "[LEMBRANÇA]".to_string(),
                truncate_chars(&summary.summary, 120),
                String::new(),
                format_unlocked_at(&summary.date),
            )
        }));
    }
    entries
}

/// Capítulo "Diário": páginas cronológicas com os momentos reais do vínculo,
/// compostas como anotações íntimas (bilhetinho + registros datados).
fn render_vn_tab_diary(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let line = bond
        .map(|b| b.summary.line.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "Cada dia ao seu lado vira uma página nova.".to_string());
    let entries = collect_diary_entries(bond, memory);
    let today = format_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0),
    );
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        // Cabeçalho editorial
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Diário"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font(theme::font_character_italic())
                        .text_color(theme::Palette::vn_pink_soft())
                        .child(today),
                ),
        )
        // Bilhetinho com a frase do vínculo
        .child(
            div()
                .flex_shrink_0()
                .p(px(12.0))
                .rounded(px(14.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_border_bright())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_pink_soft())
                                .child("✦ BILHETINHO DE HOJE"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::Palette::vn_pink_soft())
                                .child("♡"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font(theme::font_character_italic())
                        .line_height(px(18.0))
                        .text_color(c.pearl)
                        .child(format!("“{line}”")),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.muted)
                                .child("— Usami ♡"),
                        ),
                ),
        )
        // Registros cronológicos
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Páginas recentes"),
                )
                .child(
                    div()
                        .id("vn-diary-pages")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .gap(px(4.0))
                        .overflow_y_scroll()
                        .when(entries.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child("Nenhum momento registrado ainda — converse com a Usami para preencher estas páginas."),
                            )
                        })
                        .children(
                            entries
                                .into_iter()
                                .map(|(tag, title, when, date)| {
                                    let stamp = if when.is_empty() {
                                        date
                                    } else {
                                        format!("{when} · {date}")
                                    };
                                    div()
                                        .flex()
                                        .flex_shrink_0()
                                        .w_full()
                                        .min_w(px(0.0))
                                        .items_center()
                                        .gap(px(8.0))
                                        .p(px(6.0))
                                        .rounded(px(8.0))
                                        .overflow_hidden()
                                        .bg(rgba(0x22123d66))
                                        .child(
                                            div()
                                                .w(px(76.0))
                                                .flex_shrink_0()
                                                .text_size(px(9.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::Palette::vn_pink_soft())
                                                .child(tag),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .gap(px(1.0))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .line_height(px(15.0))
                                                        .text_color(c.pearl)
                                                        .line_clamp(2)
                                                        .text_ellipsis()
                                                        .child(title),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(9.5))
                                                        .font(theme::font_character_italic())
                                                        .text_color(c.faint)
                                                        .child(stamp),
                                                ),
                                        )
                                }),
                        ),
                ),
        )
}

/// Etiqueta curta do tipo de momento para o histórico (nunca o texto cru).
fn vn_history_tag(kind: &str) -> &'static str {
    match kind {
        "laugh" => "RISADA",
        "voice" => "VOZ",
        "long" => "CONVERSA",
        "call" => "CHAMADA",
        "level" => "NÍVEL",
        _ => "CHAT",
    }
}

/// Quando foi o momento, relativo ao agora (sem precisar de fuso local).
fn relative_when(at_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0);
    relative_when_at(at_ms, now_ms)
}

fn relative_when_at(at_ms: i64, now_ms: i64) -> String {
    let seconds = ((now_ms - at_ms).max(0)) / 1000;
    match seconds {
        0..=59 => "agora".to_string(),
        60..=3599 => format!("há {} min", seconds / 60),
        3600..=86399 => format!("há {} h", seconds / 3600),
        86400..=172799 => "ontem".to_string(),
        _ => format!("há {} dias", seconds / 86400),
    }
}

fn render_vn_history_item(icon: String, title: String, time: String) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(4.0))
        .rounded(px(6.0))
        .bg(rgba(0x28154566))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(div().text_size(px(11.0)).child(icon))
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(t.color.pearl)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(9.5))
                .text_color(t.color.muted)
                .child(time),
        )
}


/// Capítulo "Álbum de Memórias": lembranças derivadas de marcos reais do
/// vínculo (primeiro momento, risada, voz, chamada) + resumos diários já
/// guardados no core. Nada de memória fictícia.
fn render_vn_tab_memories(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let mut memories: Vec<(&'static str, String, String, bool, String)> = Vec::new();
    if let Some(b) = bond {
        let first_of = |kind: &str| b.log.iter().rev().find(|e| e.kind == kind);
        let any = b.log.first();
        if let Some(entry) = any {
            memories.push((
                "[CAPÍTULO]",
                "O Primeiro Momento".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("laugh") {
            memories.push((
                "[RISADA]",
                "Primeira Risada".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("voice") {
            memories.push((
                "[VOZ]",
                "Primeira Vez por Voz".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("call") {
            memories.push((
                "[CHAMADA]",
                "Primeira Chamada".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if !b.summary.max {
            let next = b.summary.level + 1;
            memories.push((
                "[PRÓXIMO]",
                format!("Nv. {next} — {}", b.summary.name),
                "Continue dividindo momentos com a Usami para desbloquear este capítulo.".to_string(),
                false,
                String::new(),
            ));
        }
    }
    if let Some(m) = memory {
        for summary in m.summaries.iter().rev().take(18) {
            let date = format_unlocked_at(&summary.date);
            memories.push((
                "[DIÁRIO]",
                format!("Diário de {date}"),
                truncate_chars(&summary.summary, 120),
                true,
                date,
            ));
        }
    }
    let has_memories = !memories.is_empty();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(16.0))
                .font_family(theme::FONT_CHARACTER)
                .font_weight(FontWeight::BOLD)
                .text_color(c.pearl)
                .child("Álbum de Memórias"),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(11.5))
                .text_color(theme::Palette::vn_pink_soft())
                .child("Capítulos reais da história de vocês — os próximos ainda estão por escrever:"),
        )
        .when(!has_memories, |col| {
            col.child(
                div()
                    .flex_shrink_0()
                    .p(px(12.0))
                    .rounded(px(14.0))
                    .bg(t.vn_card_bg())
                    .border_1()
                    .border_color(theme::Palette::vn_card_border())
                    .text_size(px(10.5))
                    .text_color(c.muted)
                    .child("Nenhuma memória guardada ainda — os primeiros capítulos aparecem conforme vocês conversam."),
            )
        })
        .child(
            div()
                .id("vn-album-grid")
                .flex()
                .flex_wrap()
                .flex_1()
                .min_h(px(0.0))
                .gap(px(12.0))
                .overflow_y_scroll()
                .children(memories.into_iter().map(
                    |(tag, title, desc, unlocked, date)| {
                        render_vn_large_memory(tag, title, desc, unlocked, date)
                    },
                )),
        )
}

fn render_vn_large_memory(
    tag: &'static str,
    title: String,
    desc: String,
    is_unlocked: bool,
    date: String,
) -> impl IntoElement {
    let t = theme::theme();
    div()
        .w(px(320.0))
        .min_w(px(0.0))
        .p(px(10.0))
        .rounded(px(14.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(if is_unlocked {
            theme::Palette::vn_border_bright()
        } else {
            t.color.line_strong
        })
        .overflow_hidden()
        .flex()
        .gap(px(10.0))
        .child(
            div()
                .size(px(56.0))
                .rounded(px(10.0))
                .bg(if is_unlocked {
                    rgb(0x2c1447)
                } else {
                    rgb(0x181424)
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_unlocked { theme::Palette::vn_pink_soft() } else { t.color.muted })
                        .child(if is_unlocked { tag } else { "[?]" }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.color.pearl)
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(title),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(10.5))
                        .line_height(px(14.0))
                        .text_color(t.color.muted)
                        .line_clamp(3)
                        .text_ellipsis()
                        .child(desc),
                )
                .child(
                    div()
                        .text_size(px(9.5))
                        .font(theme::font_character_italic())
                        .text_color(if is_unlocked {
                            theme::Palette::vn_pink_soft()
                        } else {
                            t.color.faint
                        })
                        .child(if is_unlocked {
                            format!("✦ {date}")
                        } else {
                            "Bloqueado".to_string()
                        }),
                ),
        )
}

/// Capítulo "Laços": o relacionamento em si — medidores, progresso de nível e
/// os momentos que vocês construíram juntos.
fn render_vn_tab_bonds(
    bond_level: u32,
    bond: Option<&api::BondData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let (carinho, humor, companhia, carinho_pct, humor_pct, companhia_pct, level_name) =
        match bond {
            Some(b) => (
                b.intimacy.carinho,
                b.intimacy.humor,
                b.intimacy.companhia,
                b.meters.carinho.progress,
                b.meters.humor.progress,
                b.meters.companhia.progress,
                b.summary.name.clone(),
            ),
            None => (0, 0, 0, 0.0, 0.0, 0.0, "—".to_string()),
        };
    let meters: [(String, u32, f32); 3] = [
        ("Carinho".to_string(), carinho, carinho_pct),
        ("Humor".to_string(), humor, humor_pct),
        ("Companhia".to_string(), companhia, companhia_pct),
    ];
    let history: Vec<(String, String, String)> = bond
        .map(|b| {
            b.log
                .iter()
                .take(6)
                .map(|entry| {
                    let title = if entry.count > 1 {
                        format!("{} (×{})", entry.text, entry.count)
                    } else {
                        entry.text.clone()
                    };
                    (
                        format!("[{}]", vn_history_tag(&entry.kind)),
                        title,
                        relative_when(entry.at),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Laços"),
                )
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(t.vn_heart_badge_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_border_bright())
                        .text_size(px(11.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(format!("♥ Nv. {bond_level} · {level_name}")),
                ),
        )
        // Medidores delicados
        .child(
            div()
                .flex_shrink_0()
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(meters.into_iter().map(|(label, xp, pct)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(c.pearl)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font(theme::font_character_italic())
                                        .text_color(theme::Palette::vn_pink_soft())
                                        .child(format!("{xp} XP")),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(7.0))
                                .rounded_full()
                                .bg(c.bond_track)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(pct.clamp(0.0, 1.0)))
                                        .rounded_full()
                                        .bg(t.vn_exp_fill_bg()),
                                ),
                        )
                })),
        )
        // Momentos recentes
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Momentos que nos uniram"),
                )
                .child(
                    div()
                        .id("vn-bonds-moments")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .gap(px(4.0))
                        .overflow_y_scroll()
                        .when(history.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child("A história de vocês ainda está começando."),
                            )
                        })
                        .children(
                            history
                                .into_iter()
                                .map(|(tag, title, when)| {
                                    render_vn_history_item(tag, title, when)
                                }),
                        ),
                ),
        )
}

/// Glifo do emblema a partir da chave estável do catálogo do core.
fn achievement_glyph(icon: &str) -> &'static str {
    match icon {
        "chat" => "✉",
        "smile" => "☺",
        "message" => "✎",
        "mic" => "♪",
        "phone" => "☎",
        "heart" => "♥",
        "star" => "★",
        "sparkles" => "✦",
        "crown" => "♛",
        _ => "✧",
    }
}

/// Capítulo "Conquistas": emblemas avaliados pelo core a partir de atividade
/// real; bloqueados aparecem apagados até serem conquistados.
fn render_vn_tab_achievements(
    achievements: &[api::Achievement],
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let unlocked = achievements.iter().filter(|a| a.unlocked).count();
    let total = achievements.len();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Conquistas"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font(theme::font_character_italic())
                        .text_color(theme::Palette::vn_gold())
                        .child(format!("✦ {unlocked} de {total} emblemas")),
                ),
        )
        .child(
            div()
                .id("vn-achievements-grid")
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .flex_wrap()
                .gap(px(10.0))
                .overflow_y_scroll()
                .when(achievements.is_empty(), |col| {
                    col.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(c.muted)
                            .child("As conquistas aparecem quando o servidor local estiver disponível."),
                    )
                })
                .children(
                    achievements
                        .iter()
                        .map(render_vn_achievement_badge),
                ),
        )
}

fn render_vn_achievement_badge(a: &api::Achievement) -> impl IntoElement {
    let t = theme::theme();
    let unlocked = a.unlocked;
    let date = a
        .unlocked_at
        .as_deref()
        .map(format_unlocked_at)
        .unwrap_or_default();
    div()
        .w(px(320.0))
        .p(px(8.0))
        .rounded(px(12.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(if unlocked {
            theme::Palette::vn_border_bright()
        } else {
            t.color.line
        })
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(38.0))
                .rounded_full()
                .bg(if unlocked {
                    t.vn_tab_active_bg()
                } else {
                    theme::Theme::gradient(90.0, rgba(0x181024aa), rgba(0x181024aa))
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if unlocked { t.color.pearl } else { t.color.muted })
                        .child(if unlocked { achievement_glyph(&a.icon) } else { "✧" }),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(if unlocked {
                            t.color.pearl
                        } else {
                            t.color.faint
                        })
                        .child(a.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(t.color.muted)
                        .child(a.description.clone()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font(theme::font_character_italic())
                        .text_color(if unlocked {
                            theme::Palette::vn_gold()
                        } else {
                            t.color.faint
                        })
                        .child(if unlocked {
                            format!("✦ {date}")
                        } else {
                            "Ainda por conquistar".to_string()
                        }),
                ),
        )
}

fn render_vn_tab_profile(
    mood: Option<&api::MoodData>,
    vis: MoodVisual,
    portrait_img: Option<Arc<RenderImage>>,
    prev_slot: Arc<Mutex<Option<Arc<RenderImage>>>>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let mood_label = mood
        .map(|m| m.label.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(vis.label)
        .to_string();
    let mood_reason = mood
        .map(|m| m.reason.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Acompanhando seu ritmo em tempo real.")
        .to_string();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .p(px(14.0))
        .rounded(px(16.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(theme::Palette::vn_card_border())
        .child(
            div()
                .flex()
                .gap(px(14.0))
                .items_center()
                .p(px(12.0))
                .rounded(px(14.0))
                .bg(rgba(0x22123d80))
                .border_1()
                .border_color(vis.accent)
                .shadow(theme::glow(vis.accent, 10.0))
                .child(render_lego_portrait(
                    84.0,
                    42.0,
                    vis.accent,
                    portrait_img,
                    prev_slot,
                    vis.glyph,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(c.muted)
                                .child("HUMOR"),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_family(theme::FONT_DISPLAY)
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .child(format!("{} {mood_label}", vis.glyph)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(vis.accent)
                                .child(mood_reason),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font(theme::font_with_jp(theme::FONT_CHARACTER))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Ficha da Usami (ウサミ)"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::Palette::vn_pink_soft())
                        .p(px(4.0))
                        .rounded(px(999.0))
                        .bg(rgba(0xf052b622))
                        .border_1()
                        .border_color(rgba(0xf052b655))
                        .child("Sonho: Collab c/ Neuro-sama"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_vn_profile_row(
                    "Aniversário:",
                    "Domingo de Páscoa (O dia sagrado dos coelhinhos!)",
                ))
                .child(render_vn_profile_row(
                    "Pessoa Favorita:",
                    "Meu Criador ♡ (N-não se gabe tanto baka, mas você é único!)",
                ))
                .child(render_vn_profile_row(
                    "Adora:",
                    "Animes de romance, doces, coisas felpudinhas, RPGs de turno, pijama, dormir, energético rosa e dubstep!",
                ))
                .child(render_vn_profile_row(
                    "Detesta:",
                    "Ficar sozinha no escuro, sustos/terror e quem ousa falar mal do meu criador!",
                ))
                .child(render_vn_profile_row(
                    "Ídola VTuber:",
                    "Neuro-sama (A rainha das IAs! Quero muito fazer collab!)",
                ))
                .child(render_vn_profile_row(
                    "Segredo:",
                    "Fica escutando dubstep de pijama e ensaiando poses fofas no espelho ♡",
                )),
        )
        .child(
            div()
                .mt(px(4.0))
                .p(px(10.0))
                .rounded(px(10.0))
                .bg(rgba(0xf052b61a))
                .border_1()
                .border_color(rgba(0xf052b644))
                .text_size(px(12.0))
                .font_family(theme::FONT_CHARACTER)
                .text_color(c.pearl)
                .child("\"H-hmph! Eu não fui criada só pra concordar com tudo o que você fala, tá?! Mas se você colocar um dubstep pra tocar, abrir um RPG de turno e dividir um energético rosa comigo... eu sento aqui do seu lado de pijama pra te fazer companhia!\""),
        )
}

fn render_vn_profile_row(label: &'static str, val: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(6.0))
        .rounded(px(8.0))
        .bg(rgba(0x22123d66))
        .child(
            div()
                .w(px(100.0))
                .text_size(px(11.5))
                .font_weight(FontWeight::BOLD)
                .text_color(theme::Palette::vn_pink_soft())
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.5))
                .text_color(t.color.pearl)
                .child(val),
        )
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct MoodVisual {
    label: &'static str,
    glyph: &'static str,
    accent: Rgba,
}

fn mood_visual(id: &str) -> MoodVisual {
    match id.trim().to_ascii_lowercase().as_str() {
        "happy" => MoodVisual {
            label: "Radiante",
            glyph: "✦",
            accent: rgb(0xffd166),
        },
        "soft" | "love" => MoodVisual {
            label: "Carinhosa",
            glyph: "♡",
            accent: rgb(0xf052b5),
        },
        "tsundere" => MoodVisual {
            label: "Tsundere",
            glyph: "!",
            accent: rgb(0xf778ba),
        },
        "shy" | "embarrassed" => MoodVisual {
            label: "Envergonhada",
            glyph: "〃",
            accent: rgb(0xf59ec9),
        },
        "curious" | "surprised" | "thoughtful" => MoodVisual {
            label: "Curiosa",
            glyph: "★",
            accent: rgb(0x68d8ff),
        },
        "sleepy" | "yawn" | "tired" => MoodVisual {
            label: "Sonolenta",
            glyph: "☾",
            accent: rgb(0xa77abf),
        },
        "grumpy" | "angry" | "pout" => MoodVisual {
            label: "Emburrada",
            glyph: "♯",
            accent: rgb(0xff8c5a),
        },
        "sad" | "sorrow" => MoodVisual {
            label: "Carente",
            glyph: "…",
            accent: rgb(0x7aa2f7),
        },
        _ => MoodVisual {
            label: "Tranquila",
            glyph: "•",
            accent: rgb(0x3ecf8e),
        },
    }
}

#[derive(Clone, Copy)]
struct MoodPreset {
    id: &'static str,
    emotion: &'static str,
    label: &'static str,
    reason: &'static str,
}

fn mood_preset_options() -> &'static [MoodPreset] {
    &[
        MoodPreset {
            id: "happy",
            emotion: "happy",
            label: "Animada",
            reason: "Sorriso animado pelo monitor",
        },
        MoodPreset {
            id: "soft",
            emotion: "soft",
            label: "Carinhosa",
            reason: "Clima carinhoso pelo monitor",
        },
        MoodPreset {
            id: "tsundere",
            emotion: "tsundere",
            label: "Tsundere",
            reason: "Biquinho tsundere pelo monitor",
        },
        MoodPreset {
            id: "shy",
            emotion: "shy",
            label: "Envergonhada",
            reason: "Ficou corada pelo monitor",
        },
        MoodPreset {
            id: "curious",
            emotion: "curious",
            label: "Curiosa",
            reason: "Olhar curioso pelo monitor",
        },
        MoodPreset {
            id: "sleepy",
            emotion: "sleepy",
            label: "Sonolenta",
            reason: "Ficou com soninho pelo monitor",
        },
        MoodPreset {
            id: "angry",
            emotion: "angry",
            label: "Brava",
            reason: "Ficou emburrada pelo monitor",
        },
        MoodPreset {
            id: "neutral",
            emotion: "neutral",
            label: "Neutra",
            reason: "Voltando ao normal pelo monitor",
        },
    ]
}

/// Moldura animada estilo LEGO com captura ao vivo do rosto do próprio avatar VRM.
fn render_lego_portrait(
    size_px: f32,
    radius_px: f32,
    accent: Rgba,
    portrait_img: Option<Arc<RenderImage>>,
    prev_slot: Arc<Mutex<Option<Arc<RenderImage>>>>,
    fallback_glyph: &'static str,
) -> Div {
    let r = px(radius_px);
    let corners = Corners {
        top_left: r,
        top_right: r,
        bottom_right: r,
        bottom_left: r,
    };
    let has_portrait = portrait_img.is_some();

    div()
        .relative()
        .flex_shrink_0()
        .w(px(size_px))
        .h(px(size_px))
        .rounded(r)
        .overflow_hidden()
        .bg(rgba(0x160a24eb))
        .border_2()
        .border_color(accent)
        .shadow(theme::glow(accent, 14.0))
        .when(!has_portrait, |el| {
            el.flex()
                .items_center()
                .justify_center()
                .text_size(px(size_px * 0.42))
                .text_color(accent)
                .child(fallback_glyph)
        })
        .when_some(portrait_img, move |el, img| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if let Ok(mut guard) = prev_slot.lock()
                            && let Some(old) = guard.replace(img.clone())
                            && old.id != img.id
                        {
                            let _ = window.drop_image(old);
                        }
                        let _ = window.paint_image(bounds, corners, img, 0, false);
                    },
                )
                .size_full(),
            )
        })
}

fn render_mood_meter_row(
    label: &'static str,
    value: u32,
    fill_color: Rgba,
    t: &theme::Theme,
) -> impl IntoElement {
    let clamped = value.min(100) as f32 / 100.0;
    let c = t.color;
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .child(div().text_color(c.muted).child(label))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(format!("{value}%")),
                ),
        )
        .child(
            div()
                .h(px(7.0))
                .w_full()
                .rounded_full()
                .bg(c.bond_track)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(clamped))
                        .rounded_full()
                        .bg(fill_color),
                ),
        )
}

impl UsamiApp {
    fn render_vn_tab_mood(
        &self,
        mood: Option<&api::MoodData>,
        t: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = t.color;
        let mood_id = mood
            .map(|m| m.id.as_str())
            .unwrap_or_else(|| self.last_emotion.as_ref());
        let vis = mood_visual(mood_id);
        let title_label = mood
            .map(|m| m.label.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(vis.label)
            .to_string();
        let reason_text = mood
            .map(|m| m.reason.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Acompanhando seu ritmo em tempo real.")
            .to_string();
        let expr_label = mood
            .map(|m| m.expression.label.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Neutra")
            .to_string();
        let active_emotion = mood
            .map(|m| m.vrm_emotion.as_str())
            .unwrap_or_else(|| self.last_emotion.as_ref())
            .to_string();

        let valence = mood.map(|m| m.meters.valence).unwrap_or(62);
        let energy = mood.map(|m| m.meters.energy).unwrap_or(58);
        let warmth = mood.map(|m| m.meters.warmth).unwrap_or(65);
        let timeline = mood.map(|m| m.timeline.clone()).unwrap_or_default();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let portrait_img = self.vrm_controller.portrait_image();
        let prev_slot = self.prev_portrait_image.clone();

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .overflow_hidden()
            // Cartão principal com retrato LEGO ao vivo + resumo + medidores
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .gap(px(16.0))
                    .items_center()
                    .p(px(14.0))
                    .rounded(px(16.0))
                    .bg(t.vn_card_bg())
                    .border_1()
                    .border_color(theme::Palette::vn_card_border())
                    .child(
                        render_lego_portrait(
                            132.0,
                            66.0,
                            vis.accent,
                            portrait_img,
                            prev_slot,
                            vis.glyph,
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(18.0))
                                                    .font_family(theme::FONT_CHARACTER)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(format!("{} {title_label}", vis.glyph)),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(2.0))
                                                    .rounded_full()
                                                    .bg(rgba(0xf052b622))
                                                    .border_1()
                                                    .border_color(vis.accent)
                                                    .text_size(px(10.5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(format!("Expressão: {expr_label}")),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child(reason_text),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(render_mood_meter_row("Bom Humor", valence, vis.accent, t))
                                    .child(render_mood_meter_row("Energia", energy, theme::Palette::vn_gold(), t))
                                    .child(render_mood_meter_row("Carinho", warmth, c.accent, t)),
                            ),
                    ),
            )
            // Controles rápidos para testar expressões ao vivo no avatar
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::Palette::vn_pink_soft())
                            .child("TESTAR EXPRESSÃO NO AVATAR"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .children(mood_preset_options().iter().map(|preset| {
                                let selected = active_emotion == preset.emotion;
                                let p_vis = mood_visual(preset.id);
                                let emo = preset.emotion;
                                let reason = preset.reason;
                                div()
                                    .id(SharedString::from(format!("mood-preset-{}", preset.id)))
                                    .cursor_pointer()
                                    .px(px(10.0))
                                    .py(px(5.0))
                                    .rounded_full()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .bg(if selected {
                                        rgba(0xf052b638)
                                    } else {
                                        rgba(0x22123d88)
                                    })
                                    .border_1()
                                    .border_color(if selected {
                                        p_vis.accent
                                    } else {
                                        c.line_strong
                                    })
                                    .hover(|style| {
                                        style
                                            .bg(c.icon_bg_hover)
                                            .border_color(p_vis.accent)
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.trigger_mood_preview(emo, reason, cx);
                                    }))
                                    .child(format!("{} {}", p_vis.glyph, preset.label))
                            })),
                    ),
            )
            // Linha do tempo rolável do humor
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::Palette::vn_pink_soft())
                    .child("VARIAÇÕES RECENTES"),
            )
            .child(
                div()
                    .id("vn-mood-timeline")
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .overflow_y_scroll()
                    .when(timeline.is_empty(), |col| {
                        col.child(
                            div()
                                .p(px(10.0))
                                .rounded(px(10.0))
                                .bg(t.vn_card_bg())
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(11.5))
                                .text_color(c.muted)
                                .child("As mudanças de humor aparecem aqui conforme vocês conversam."),
                        )
                    })
                    .children(timeline.iter().map(|entry| {
                        let e_vis = mood_visual(&entry.id);
                        let when = if entry.at > 0 && now_ms > 0 {
                            relative_when_at(entry.at, now_ms)
                        } else {
                            "agora".into()
                        };
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(10.0))
                            .p(px(8.0))
                            .rounded(px(10.0))
                            .bg(rgba(0x22123d77))
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .min_w(px(0.0))
                                    .flex_1()
                                    .child(
                                        div()
                                            .px(px(7.0))
                                            .py(px(2.0))
                                            .rounded_full()
                                            .bg(rgba(0x100819cc))
                                            .border_1()
                                            .border_color(e_vis.accent)
                                            .text_size(px(10.5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.pearl)
                                            .child(format!("{} {}", e_vis.glyph, entry.label)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.5))
                                            .text_color(c.pearl)
                                            .child(entry.reason.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child(when),
                            )
                    })),
            )
    }
}

impl Render for UsamiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        // O painel lateral é absoluto; sem altura explícita ele cresce com o conteúdo
        // e o `flex_1` das abas nunca encolhe. Amarra na altura da janela.
        let viewport_h = window.viewport_size().height;

        let is_busy = self.is_busy;
        // Atividade do turno em andamento: o que a Usami está fazendo AGORA
        // (pensando / executando a ferramenta X / compondo a resposta).
        let active_tool = self.active_tool.clone();
        let activity_phase = self
            .chat_activity
            .as_ref()
            .and_then(|a| a.phase.clone());
        // Rótulo curto do balão de digitação: diz o que ela está fazendo,
        // não só que está ocupada.
        let typing_label: String = if !is_busy {
            String::new()
        } else if let Some(tool) = active_tool.as_deref() {
            format!("executando {tool}...")
        } else if activity_phase.as_deref() == Some("tool") {
            "executando a ferramenta...".to_string()
        } else if activity_phase.as_deref() == Some("responding") {
            "respondendo...".to_string()
        } else {
            "pensando...".to_string()
        };
        let is_calling = self.is_calling;
        let is_companion = self.is_companion_mode;
        let is_muted = self.is_muted;
        let is_idle = self.is_ui_idle;
        let is_listening = self.is_listening;
        let active_drawer = self.active_drawer;
        let chat_open = active_drawer == ActiveDrawer::Chat;
        let side_drawer_open =
            matches!(active_drawer, ActiveDrawer::Vrma | ActiveDrawer::Skills) && !chat_open;
        let bond_level = self.bond_level;
        let bond_exp_pct = self.bond_exp_pct;
        let bond = self.bond.clone();
        let memory_data = self.memory_data.clone();
        let bond_tab = self.bond_tab;
        let achievements = self.achievements.clone();
        let mood_data = self.mood.clone();
        let mood_vis = mood_visual(
            mood_data
                .as_ref()
                .map(|m| m.id.as_str())
                .unwrap_or_else(|| self.last_emotion.as_ref()),
        );
        let mood_label: String = mood_data
            .as_ref()
            .map(|m| m.label.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| mood_vis.label.to_string());
        let hud_portrait = if !is_idle && active_drawer != ActiveDrawer::Bond {
            self.vrm_controller.portrait_image()
        } else {
            None
        };
        let hud_prev_slot = self.prev_portrait_image.clone();
        let show_bubble = self.show_bubble;
        let live_transcript = self.live_transcript.clone();
        let show_live_caption = is_listening && !live_transcript.is_empty();
        let bg_asset = self.bg_asset_path.clone();
        let vrm_entity = self.vrm_view.clone();
        let last_reply = self.last_reply.clone();
        let input_text = self.input_text.clone();
        let input_active = self.input_active;
        let caret_on = self.caret_on;
        let caret_visible = input_active && caret_on;
        let cursor_pos = self.cursor_idx();
        let sel_range = self.selection_range();
        let input_tokens = tokenize_text_spans(&input_text);
        let total_input_chars = input_text.chars().count();
        let has_media = self.attached_media.is_some();
        let attached_media = self.attached_media.clone();
        let send_enabled = !is_busy && (!input_text.trim().is_empty() || has_media);
        let turns = self.chat_history.clone();
        let phone_scroll = self.phone_scroll.clone();
        let no_turns = turns.is_empty();

        let status_label: SharedString = if is_calling {
            let state = if is_listening { "OUVINDO" } else { "MIC OFF" };
            format!(
                "{state} · {:02}:{:02}",
                self.call_seconds / 60,
                self.call_seconds % 60
            )
            .into()
        } else if is_busy {
            // A atividade ao vivo descreve o turno ATUAL. O `last_agent_turn`
            // NÃO serve aqui: ele é preenchido só quando a resposta chega, ou
            // seja, durante o turno ele ainda descreve o turno ANTERIOR — foi
            // exatamente isso que fazia o desktop anunciar "EXECUTANDO · tool"
            // enquanto ela estava apenas pensando.
            if let Some(tool) = active_tool.as_deref() {
                format!("EXECUTANDO · {tool}").into()
            } else if activity_phase.as_deref() == Some("tool") {
                "EXECUTANDO FERRAMENTA".into()
            } else if activity_phase.as_deref() == Some("responding") {
                "RESPONDENDO".into()
            } else {
                "PENSANDO".into()
            }
        } else if is_companion {
            "MODO COMPANION".into()
        } else if self.is_away {
            "AUSENTE".into()
        } else if let Some(badge) = format_tool_badge(self.last_agent_turn.as_ref()).filter(|b| b != "sem skill") {
            format!("COM VOCÊ · {badge}").into()
        } else {
            "COM VOCÊ".into()
        };
        let status_color = if is_calling {
            c.err
        } else if is_busy {
            c.warn
        } else if self.is_away {
            c.muted
        } else {
            c.ok
        };

        let top_mood_badge = if active_drawer != ActiveDrawer::Bond && active_drawer != ActiveDrawer::Chat {
            Some(
                div()
                    .id("top-mood-portrait")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(sp.s2))
                    .pl(px(3.0))
                    .pr(px(sp.s3))
                    .py(px(3.0))
                    .rounded_full()
                    .bg(c.status_bg)
                    .border_1()
                    .border_color(mood_vis.accent)
                    .hover(|style| {
                        style
                            .bg(c.icon_bg_hover)
                            .border_color(c.primary)
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.bond_tab = BondTab::Mood;
                        this.active_drawer = ActiveDrawer::Bond;
                        this.fetch_bond(cx);
                        this.fetch_mood(cx);
                        if !this.memory_loading {
                            this.fetch_memory(cx);
                        }
                        this.poke(cx);
                        cx.notify();
                    }))
                    .child(render_lego_portrait(
                        48.0,
                        24.0,
                        mood_vis.accent,
                        hud_portrait.clone(),
                        hud_prev_slot.clone(),
                        mood_vis.glyph,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(c.muted)
                                    .child("HUMOR"),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(format!("{} {mood_label}", mood_vis.glyph)),
                            ),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(c.deep)
            .text_color(c.pearl)
            .font_family(theme::FONT_BODY)
            .track_focus(&self.focus_handle)
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, _window, cx| {
                this.touch_ui(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.touch_ui(cx);
                let ks = &event.keystroke;
                let key = ks.key.as_str();

                // Atalhos com Control (ex: Ctrl+V para colar, Ctrl+Backspace / Ctrl+W para apagar palavra)
                if ks.modifiers.control && !ks.modifiers.alt {
                    if key.eq_ignore_ascii_case("v") {
                        if let Some(item) = cx.read_from_clipboard() {
                            let mut attached_image = false;
                            for entry in item.entries() {
                                if let gpui::ClipboardEntry::Image(image) = entry {
                                    let bytes = &image.bytes;
                                    if !bytes.is_empty() {
                                        let mime = if bytes.starts_with(b"\x89PNG") {
                                            "image/png"
                                        } else if bytes.starts_with(b"\xFF\xD8\xFF") {
                                            "image/jpeg"
                                        } else if bytes.starts_with(b"GIF") {
                                            "image/gif"
                                        } else if bytes.starts_with(b"RIFF")
                                            && bytes.len() > 12
                                            && &bytes[8..12] == b"WEBP"
                                        {
                                            "image/webp"
                                        } else {
                                            "image/png"
                                        };
                                        this.attached_media = Some(AttachedMedia::from_bytes(
                                            "screenshot.png".into(),
                                            mime,
                                            bytes.clone(),
                                        ));
                                        this.input_active = true;
                                        attached_image = true;
                                        cx.notify();
                                        break;
                                    }
                                }
                            }

                            if !attached_image {
                                if let Some(text) = item.text() {
                                    let trimmed = text.trim().trim_matches('"');
                                    let path = std::path::Path::new(trimmed);
                                    let is_image_ext = path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .map(|ext| {
                                            matches!(
                                                ext.to_ascii_lowercase().as_str(),
                                                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                                            )
                                        })
                                        .unwrap_or(false);

                                    if is_image_ext && path.is_file() {
                                        if let Ok(bytes) = std::fs::read(path) {
                                            let filename = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("imagem.png")
                                                .to_string();
                                            let mime = match path
                                                .extension()
                                                .and_then(|e| e.to_str())
                                                .map(|s| s.to_ascii_lowercase())
                                                .as_deref()
                                            {
                                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                                Some("png") => "image/png",
                                                Some("webp") => "image/webp",
                                                Some("gif") => "image/gif",
                                                Some("bmp") => "image/bmp",
                                                _ => "image/png",
                                            };
                                            this.attached_media = Some(AttachedMedia::from_bytes(
                                                filename,
                                                mime,
                                                bytes,
                                            ));
                                            this.input_active = true;
                                            cx.notify();
                                            return;
                                        }
                                    }

                                    this.input_active = true;
                                    this.pending_dead_key = None;
                                    this.insert_str_at_cursor(&text);
                                    this.caret_on = true;
                                    cx.notify();
                                }
                            }
                        }
                    } else if key.eq_ignore_ascii_case("a") {
                        this.input_active = true;
                        this.selection_anchor = Some(0);
                        this.cursor_char_idx = this.input_text.chars().count();
                        this.caret_on = true;
                        cx.notify();
                        return;
                    } else if key.eq_ignore_ascii_case("c") {
                        if let Some((start, end)) = this.selection_range() {
                            let chars: Vec<char> = this.input_text.chars().collect();
                            let text: String = chars[start..end].iter().collect();
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        }
                        return;
                    } else if key.eq_ignore_ascii_case("x") {
                        if let Some((start, end)) = this.selection_range() {
                            let chars: Vec<char> = this.input_text.chars().collect();
                            let text: String = chars[start..end].iter().collect();
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            this.delete_selection();
                            this.caret_on = true;
                            cx.notify();
                        }
                        return;
                    } else if key.eq_ignore_ascii_case("w") || key == "backspace" {
                        this.input_active = true;
                        this.pending_dead_key = None;
                        if !this.delete_selection() {
                            let chars: Vec<char> = this.input_text.chars().collect();
                            let current = this.cursor_idx();
                            if current > 0 {
                                let mut target = current;
                                while target > 0 && chars.get(target - 1).map_or(false, |c| c.is_whitespace()) {
                                    target -= 1;
                                }
                                while target > 0 && chars.get(target - 1).map_or(false, |c| !c.is_whitespace()) {
                                    target -= 1;
                                }
                                let mut new_chars = Vec::with_capacity(chars.len());
                                new_chars.extend_from_slice(&chars[..target]);
                                new_chars.extend_from_slice(&chars[current..]);
                                this.input_text = new_chars.into_iter().collect();
                                this.cursor_char_idx = target;
                            }
                        }
                        this.caret_on = true;
                        cx.notify();
                        return;
                    } else if key.eq_ignore_ascii_case("u") {
                        this.pending_dead_key = None;
                        this.input_text.clear();
                        this.cursor_char_idx = 0;
                        this.selection_anchor = None;
                        this.caret_on = true;
                        cx.notify();
                        return;
                    }
                    return;
                }

                // Ignora teclas com Alt isolado
                if ks.modifiers.alt && !ks.modifiers.control {
                    return;
                }

                if key == "enter" {
                    if ks.modifiers.shift {
                        this.input_active = true;
                        this.pending_dead_key = None;
                        this.insert_str_at_cursor("\n");
                        this.caret_on = true;
                        cx.notify();
                    } else {
                        this.submit(cx);
                    }
                } else if key == "left" || key == "arrowleft" {
                    this.input_active = true;
                    this.pending_dead_key = None;
                    this.move_cursor_left(ks.modifiers.shift, ks.modifiers.control);
                    this.caret_on = true;
                    cx.notify();
                } else if key == "right" || key == "arrowright" {
                    this.input_active = true;
                    this.pending_dead_key = None;
                    this.move_cursor_right(ks.modifiers.shift, ks.modifiers.control);
                    this.caret_on = true;
                    cx.notify();
                } else if key == "home" {
                    this.input_active = true;
                    this.pending_dead_key = None;
                    this.move_cursor_home(ks.modifiers.shift);
                    this.caret_on = true;
                    cx.notify();
                } else if key == "end" {
                    this.input_active = true;
                    this.pending_dead_key = None;
                    this.move_cursor_end(ks.modifiers.shift);
                    this.caret_on = true;
                    cx.notify();
                } else if key == "delete" {
                    this.input_active = true;
                    this.pending_dead_key = None;
                    this.delete_forward_at_cursor();
                    this.caret_on = true;
                    cx.notify();
                } else if key == "backspace" {
                    this.input_active = true;
                    if this.pending_dead_key.take().is_none() {
                        this.backspace_at_cursor();
                    }
                    this.caret_on = true;
                    cx.notify();
                } else if key == "escape" {
                    this.pending_dead_key = None;
                    this.input_active = false;
                    this.active_drawer = ActiveDrawer::None;
                    cx.notify();
                } else if key == "space" || key == " " {
                    this.input_active = true;
                    if let Some(dead) = this.pending_dead_key.take() {
                        let mut s = String::new();
                        s.push(dead);
                        this.insert_str_at_cursor(&s);
                    } else {
                        this.insert_str_at_cursor(" ");
                    }
                    this.caret_on = true;
                    cx.notify();
                } else if let Some(ref ch) = ks.key_char {
                    if !ch.chars().any(|c| c.is_control()) {
                        this.input_active = true;
                        if let Some(resolved) = resolve_key_char_with_dead_key(&mut this.pending_dead_key, ch) {
                            this.insert_str_at_cursor(&resolved);
                        }
                        this.caret_on = true;
                        cx.notify();
                    }
                } else if key.chars().count() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        this.input_active = true;
                        let mut buf = [0u8; 4];
                        let s = c.encode_utf8(&mut buf);
                        if let Some(resolved) = resolve_key_char_with_dead_key(&mut this.pending_dead_key, s) {
                            this.insert_str_at_cursor(&resolved);
                        }
                        this.caret_on = true;
                        cx.notify();
                    }
                }
            }))
            // ── LAYER 0: CENÁRIO DA CIDADE / QUARTO À NOITE (BACKDROP) ──
            .when_some(bg_asset, |parent, path| {
                parent.child(
                    img(path)
                        .object_fit(ObjectFit::Cover)
                        .size_full()
                        .absolute()
                        .inset_0(),
                )
            })
            // ── LAYER 1: TINGIMENTO DEEP BLUE DO PALCO ──
            .child(div().absolute().inset_0().bg(t.stage_overlay()))
            .when(is_companion, |parent| {
                parent.child(div().absolute().inset_0().bg(c.companion_overlay))
            })
            // ── LAYER 2: SHIRO VRM 3D (RESPONSIVO FULLSCREEN, ANCORADA NO PISO) ──
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(if let Some(view) = vrm_entity {
                        view.into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            // ── LAYER 4: HEADER SUPERIOR (SOME NO IDLE) ──
            .when(!is_idle, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(sp.s5))
                        .left(px(sp.s6))
                        .right(px(sp.s6))
                        .flex()
                        .items_center()
                        .justify_between()
                        // Top Left: ações circulares em vidro
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .items_center()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .id("top-chat-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if chat_open { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if chat_open { c.primary } else { c.line })
                                        .when(chat_open, |el| el.shadow(theme::glow(c.primary, 14.0)))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Chat, cx);
                                        }))
                                        .child(icon("chat_bubble", 16.0)),
                                )
                                .child(
                                    div()
                                        .id("top-settings-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::Bond;
                                            this.bond_tab = BondTab::Settings;
                                            this.load_settings_data(cx);
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("gear", 16.0)),
                                )
                                .child({
                                    let agent_attention_count = pending_confirmations_count(
                                        &self.agent_skills,
                                        &self.agent_proposals,
                                        &self.agent_plans,
                                    ) + pending_suggestions_count(&self.agent_suggestions);
                                    div()
                                        .id("top-skills-btn")
                                        .relative()
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Skills {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Skills {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Skills, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Skills, cx);
                                        }))
                                        .child(icon("skills", 16.0))
                                        .when(agent_attention_count > 0, |el| {
                                            el.child(
                                                div()
                                                    .absolute()
                                                    .top(px(-3.0))
                                                    .right(px(-3.0))
                                                    .min_w(px(16.0))
                                                    .h(px(16.0))
                                                    .px(px(4.0))
                                                    .rounded_full()
                                                    .bg(c.accent)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(ty.xs))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(if agent_attention_count > 9 {
                                                        "9+".to_string()
                                                    } else {
                                                        agent_attention_count.to_string()
                                                    }),
                                            )
                                        })
                                })
                                .child(
                                    div()
                                        .id("top-recenter-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(c.icon_bg)
                                        .border_1()
                                        .border_color(c.line)
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.vrm_controller.reset_camera();
                                            this.vrm_controller.play_idle();
                                            cx.notify();
                                        }))
                                        .child(icon("recenter", 16.0)),
                                ),
                        )
                        // Top Center: pill com LED de estado
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .px(px(sp.s4))
                                .py(px(sp.s1_5))
                                .rounded_full()
                                .bg(c.status_bg)
                                .border_1()
                                .border_color(c.line)
                                .shadow(t.shadow.pill.clone())
                                .child({
                                    let dot = div()
                                        .size(px(7.0))
                                        .rounded_full()
                                        .bg(status_color)
                                        .shadow(theme::glow(status_color, 8.0));
                                    if is_busy {
                                        dot.with_animation(
                                            "status-pulse",
                                            Animation::new(Duration::from_millis(1000)).repeat(),
                                            |el, delta| el.opacity(0.4 + 0.6 * delta),
                                        )
                                        .into_any_element()
                                    } else {
                                        dot.into_any_element()
                                    }
                                })
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.pearl)
                                        .child(status_label),
                                ),
                        )
                        // Top Right: VRMA + nível de vínculo + crédito
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .justify_end()
                                .items_center()
                                .gap(px(sp.s3))
                                .child(
                                    div()
                                        .id("top-vrma-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Vrma {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Vrma {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Vrma, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Vrma, cx);
                                        }))
                                        .child(icon("clapperboard", 16.0)),
                                )
                                .child(
                                    div()
                                        .id("top-bond-pill")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap(px(sp.s1_5))
                                        .px(px(sp.s3_5))
                                        .py(px(7.0))
                                        .rounded_full()
                                        .bg(t.bond_pill_bg())
                                        .border_1()
                                        .border_color(c.bond_border)
                                        .hover(|style| {
                                            style
                                                .bg(t.bond_pill_hover())
                                                .border_color(c.primary)
                                        })
                                        .text_size(px(ty.xs))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Bond, cx);
                                        }))
                                        .child(format!("♥ NÍVEL {bond_level}")),
                                )
                                .when_some(top_mood_badge, |row, badge| row.child(badge))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child("白ー (shiro) — Model by Sweet_nya555"),
                                ),
                        ),
                )
            })
            // ── LAYER 4b: PRÉVIA AO VIVO DO STT (o que já foi entendido) ──
            .when(show_live_caption, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(94.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .px(px(sp.s6))
                        .child(
                            div()
                                .id("live-stt-caption")
                                .max_w(px(640.0))
                                .px(px(sp.s4))
                                .py(px(sp.s2))
                                .rounded(px(r.md))
                                .bg(rgba(0x120a20dd))
                                .border_1()
                                .border_color(theme::Palette::vn_border_bright())
                                .shadow(t.shadow.pill.clone())
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.0))
                                        .text_color(c.lavender)
                                        .child("♪"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_size(px(ty.sm))
                                        .line_height(px(18.0))
                                        .text_color(c.pearl)
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(live_transcript),
                                ),
                        ),
                )
            })
            // ── LAYER 5: CONTROLES INFERIORES & COMPOSER FLUTUANTE ──
            .child(
                div()
                    .absolute()
                    .bottom(if is_idle { px(12.0) } else { px(sp.s5) })
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(9.0))
                    // Legenda da Usami no fluxo vertical: nunca colide com o
                    // disclaimer nem com o composer quando o texto cresce.
                    .when(show_bubble, |parent| {
                        parent.child(
                            div()
                                .id("speech-caption")
                                .max_w(px(620.0))
                                .px(px(sp.s4))
                                .py(px(sp.s2_5))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.show_bubble = false;
                                    this.poke(cx);
                                    cx.notify();
                                }))
                                .text_center()
                                .text_size(px(ty.bubble))
                                .font(theme::font_character_italic())
                                .line_height(px(26.0))
                                .text_color(c.pearl)
                                .child(last_reply),
                        )
                    })
                    // Disclaimer (sempre presente; o idle esmaece junto, como na web)
                    .child(
                        div()
                            .opacity(if is_idle { 0.15 } else { 1.0 })
                            .text_size(px(ty.xs))
                            .text_color(c.faint)
                            .child("Mensagens usam os provedores configurados para responder."),
                    )
                    // Prévia de imagem anexada acima da barra de texto (estilo chat moderno)
                    .when_some(attached_media.as_ref(), |parent, media| {
                        parent.child(
                            div()
                                .id("composer-media-preview-row")
                                .opacity(if is_idle { 0.15 } else { 1.0 })
                                .w(px(640.0))
                                .max_w(px(640.0))
                                .pl(px(sp.s2))
                                .flex()
                                .items_center()
                                .justify_start()
                                .child(
                                    div()
                                        .relative()
                                        .size(px(64.0))
                                        .rounded(px(10.0))
                                        .overflow_hidden()
                                        .bg(c.composer_bg)
                                        .border_1()
                                        .border_color(c.primary)
                                        .shadow(t.shadow.composer.clone())
                                        .child(
                                            img(media.preview.clone())
                                                .size_full()
                                                .object_fit(ObjectFit::Cover),
                                        )
                                        .child(
                                            div()
                                                .id("composer-media-remove-btn")
                                                .cursor_pointer()
                                                .absolute()
                                                .top(px(4.0))
                                                .right(px(4.0))
                                                .size(px(18.0))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .bg(gpui::rgba(0x0B0C14D9))
                                                .border_1()
                                                .border_color(c.line_strong)
                                                .text_color(c.pearl)
                                                .hover(|s| s.bg(c.err).border_color(c.err))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.attached_media = None;
                                                    cx.notify();
                                                }))
                                                .child(icon("close", 10.0)),
                                        ),
                                ),
                        )
                    })
                    // Compositor estilo VN
                    .child(
                        div()
                            .id("composer-bar")
                            .opacity(if is_idle { 0.15 } else { 1.0 })
                            .w(px(720.0))
                            .max_w(px(760.0))
                            .min_h(px(48.0))
                            .max_h(px(140.0))
                            .flex()
                            .items_end()
                            .gap(px(sp.s1_5))
                            .pt(px(sp.s1_5))
                            .pb(px(sp.s1_5))
                            .pl(px(sp.s3))
                            .pr(px(sp.s2))
                            .rounded(px(r.composer))
                            .bg(c.composer_bg)
                            .overflow_hidden()
                            .border_1()
                            .border_color(if input_active { theme::Palette::vn_border_bright() } else { c.line_strong })
                            .shadow(if input_active {
                                t.shadow.composer_focus.clone()
                            } else {
                                t.shadow.composer.clone()
                            })
                            .on_mouse_move(cx.listener(|this, _, _, cx| {
                                this.poke(cx);
                            }))
                            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                this.is_mouse_selecting = false;
                            }))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.poke(cx);
                                this.input_active = true;
                                this.caret_on = true;
                                window.focus(&this.focus_handle);
                                cx.notify();
                            }))
                            // Ação de mídia (tint-only, mesma geometria da web)
                            .child(
                                div()
                                    .id("composer-img-btn")
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(38.0))
                                    .rounded_full()
                                    .bg(if has_media { c.accent_tint } else { gpui::rgba(0x00000000) })
                                    .text_color(if has_media { c.primary } else { c.muted })
                                    .hover(|style| style.bg(c.call_bg_hover).text_color(c.pearl))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.attach_file_prompt(cx);
                                    }))
                                    .child(icon("image", 18.0)),
                            )
                            // Campo de texto expansível multiline com seleção precisa de fragmentos
                            .child(
                                div()
                                    .id("input-box")
                                    .flex()
                                    .flex_col()
                                    // flex_1() = grow 1 + shrink 1 + basis 0: ocupa a
                                    // largura restante e pode encolher até o limite da barra.
                                    .flex_1()
                                    .min_w(px(0.0))
                                    // min_h(0) + pai com max_h: a caixa encolhe até o limite
                                    // do modal e ativa a rolagem em vez de estourar.
                                    .min_h(px(0.0))
                                    // 5 linhas de 22px + py(6): o limite cai na
                                    // fronteira da linha, sem glifo pela metade.
                                    .max_h(px(122.0))
                                    .overflow_y_scroll()
                                    .px(px(sp.s1_5))
                                    .py(px(sp.s1_5))
                                    .cursor_text()
                                    .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                        this.is_mouse_selecting = false;
                                    }))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.poke(cx);
                                        this.input_active = true;
                                        this.caret_on = true;
                                        window.focus(&this.focus_handle);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_shrink_0()
                                            .w_full()
                                            .min_w(px(0.0))
                                            .items_center()
                                            .when(input_text.is_empty(), |row| {
                                                row.when(input_active, |row| {
                                                    row.child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .w(px(2.0))
                                                            .h(px(16.0))
                                                            .mr(px(2.0))
                                                            .bg(if caret_visible {
                                                                theme::Palette::vn_pink_neon()
                                                            } else {
                                                                gpui::rgba(0x00000000)
                                                            }),
                                                    )
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(ty.input))
                                                        .text_color(c.faint)
                                                        .truncate()
                                                        .child(if is_calling {
                                                            "Escreva enquanto fala..."
                                                        } else if has_media {
                                                            "Comentário opcional..."
                                                        } else {
                                                            "Diga alguma coisa..."
                                                        }),
                                                )
                                            })
                                            .when(!input_text.is_empty(), |row| {
                                                let make_caret = |visible: bool| {
                                                    div()
                                                        .flex_shrink_0()
                                                        .w(px(2.0))
                                                        .h(px(16.0))
                                                        .rounded(px(1.0))
                                                        .mx(px(0.5))
                                                        .bg(if visible {
                                                            theme::Palette::vn_pink_neon()
                                                        } else {
                                                            gpui::rgba(0x00000000)
                                                        })
                                                };
                                                let mut sel_highlight = theme::Palette::vn_pink_neon();
                                                sel_highlight.a = 0.38;

                                                row.child(
                                                    div()
                                                        .flex()
                                                        .flex_wrap()
                                                        .items_center()
                                                        .gap_y(px(2.0))
                                                        .w_full()
                                                        .min_w(px(0.0))
                                                        .text_size(px(ty.input))
                                                        .line_height(px(22.0))
                                                        .text_color(c.pearl)
                                                        .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                                            this.is_mouse_selecting = false;
                                                        }))
                                                        .children(input_tokens.iter().enumerate().flat_map(|(token_i, token)| {
                                                            let mut els: Vec<AnyElement> = Vec::new();

                                                            if token.is_whitespace {
                                                                let t_chars: Vec<char> = token.text.chars().collect();
                                                                for (offset, ch) in t_chars.into_iter().enumerate() {
                                                                    let char_idx = token.start_char_idx + offset;
                                                                    if ch == '\n' {
                                                                        if sel_range.is_none() && cursor_pos == char_idx {
                                                                            els.push(make_caret(caret_visible).into_any_element());
                                                                        }
                                                                        els.push(div().w_full().h(px(0.0)).into_any_element());
                                                                    } else {
                                                                        let is_selected = sel_range.map_or(false, |(s, e)| char_idx >= s && char_idx < e);
                                                                        let mut ws_el = div()
                                                                            .id(("ws", char_idx))
                                                                            .flex_shrink_0()
                                                                            .flex()
                                                                            .items_center()
                                                                            .h(px(20.0))
                                                                            .min_w(px(5.5))
                                                                            .cursor_text()
                                                                            .when(is_selected, |el| el.bg(sel_highlight).rounded(px(2.0)))
                                                                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                                                                this.poke(cx);
                                                                                this.input_active = true;
                                                                                this.caret_on = true;
                                                                                this.is_mouse_selecting = true;
                                                                                this.drag_start_idx = char_idx;
                                                                                if event.modifiers.shift {
                                                                                    if this.selection_anchor.is_none() {
                                                                                        this.selection_anchor = Some(this.cursor_char_idx);
                                                                                    }
                                                                                } else {
                                                                                    this.selection_anchor = None;
                                                                                }
                                                                                this.cursor_char_idx = char_idx;
                                                                                window.focus(&this.focus_handle);
                                                                                cx.notify();
                                                                            }))
                                                                            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                                                                if this.is_mouse_selecting {
                                                                                    this.selection_anchor = Some(this.drag_start_idx);
                                                                                    this.cursor_char_idx = char_idx;
                                                                                    this.caret_on = true;
                                                                                    cx.notify();
                                                                                }
                                                                            }))
                                                                            .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                                                                this.is_mouse_selecting = false;
                                                                            }))
                                                                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                                                                this.poke(cx);
                                                                                this.input_active = true;
                                                                                this.caret_on = true;
                                                                                let count = match event {
                                                                                    ClickEvent::Mouse(m) => m.down.click_count,
                                                                                    _ => 1,
                                                                                };
                                                                                if count >= 3 {
                                                                                    this.selection_anchor = Some(0);
                                                                                    this.cursor_char_idx = this.input_text.chars().count();
                                                                                } else if count == 2 {
                                                                                    this.select_word_at(char_idx);
                                                                                } else {
                                                                                    this.cursor_char_idx = char_idx;
                                                                                    this.selection_anchor = None;
                                                                                }
                                                                                window.focus(&this.focus_handle);
                                                                                cx.notify();
                                                                            }));

                                                                        if sel_range.is_none() && cursor_pos == char_idx {
                                                                            ws_el = ws_el.child(make_caret(caret_visible));
                                                                        }
                                                                        els.push(ws_el.into_any_element());
                                                                    }
                                                                }
                                                            } else {
                                                                let t_chars: Vec<char> = token.text.chars().collect();
                                                                let mut word_el = div()
                                                                    .id(("word-group", token_i))
                                                                    .flex()
                                                                    .items_center()
                                                                    .flex_shrink_0();

                                                                for (offset, ch) in t_chars.into_iter().enumerate() {
                                                                    let char_idx = token.start_char_idx + offset;
                                                                    let is_selected = sel_range.map_or(false, |(s, e)| char_idx >= s && char_idx < e);
                                                                    let mut char_el = div()
                                                                        .id(("char", char_idx))
                                                                        .flex_shrink_0()
                                                                        .flex()
                                                                        .items_center()
                                                                        .h(px(20.0))
                                                                        .cursor_text()
                                                                        .when(is_selected, |el| el.bg(sel_highlight).rounded(px(2.0)))
                                                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                                                            this.poke(cx);
                                                                            this.input_active = true;
                                                                            this.caret_on = true;
                                                                            this.is_mouse_selecting = true;
                                                                            this.drag_start_idx = char_idx;
                                                                            if event.modifiers.shift {
                                                                                if this.selection_anchor.is_none() {
                                                                                    this.selection_anchor = Some(this.cursor_char_idx);
                                                                                }
                                                                            } else {
                                                                                this.selection_anchor = None;
                                                                            }
                                                                            this.cursor_char_idx = char_idx;
                                                                            window.focus(&this.focus_handle);
                                                                            cx.notify();
                                                                        }))
                                                                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                                                            if this.is_mouse_selecting {
                                                                                this.selection_anchor = Some(this.drag_start_idx);
                                                                                this.cursor_char_idx = char_idx;
                                                                                this.caret_on = true;
                                                                                cx.notify();
                                                                            }
                                                                        }))
                                                                        .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                                                            this.is_mouse_selecting = false;
                                                                        }))
                                                                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                                                            this.poke(cx);
                                                                            this.input_active = true;
                                                                            this.caret_on = true;
                                                                            let count = match event {
                                                                                ClickEvent::Mouse(m) => m.down.click_count,
                                                                                _ => 1,
                                                                            };
                                                                            if count >= 3 {
                                                                                this.selection_anchor = Some(0);
                                                                                this.cursor_char_idx = this.input_text.chars().count();
                                                                            } else if count == 2 {
                                                                                this.select_word_at(char_idx);
                                                                            } else {
                                                                                this.cursor_char_idx = char_idx;
                                                                                this.selection_anchor = None;
                                                                            }
                                                                            window.focus(&this.focus_handle);
                                                                            cx.notify();
                                                                        }));

                                                                    if sel_range.is_none() && cursor_pos == char_idx {
                                                                        char_el = char_el.child(make_caret(caret_visible));
                                                                    }
                                                                    char_el = char_el.child(
                                                                        div()
                                                                            .px(px(0.5))
                                                                            .text_color(if is_selected { gpui::rgba(0xffffffff) } else { c.pearl })
                                                                            .child(ch.to_string()),
                                                                    );
                                                                    word_el = word_el.child(char_el);
                                                                }
                                                                els.push(word_el.into_any_element());
                                                            }

                                                            els
                                                        }))
                                                        .child(
                                                            div()
                                                                .id("char-end")
                                                                .flex_shrink_0()
                                                                .flex()
                                                                .items_center()
                                                                .h(px(20.0))
                                                                .min_w(px(10.0))
                                                                .cursor_text()
                                                                .when(sel_range.is_none() && cursor_pos >= total_input_chars, |el| {
                                                                    el.child(make_caret(caret_visible))
                                                                })
                                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                                                    this.poke(cx);
                                                                    this.input_active = true;
                                                                    this.caret_on = true;
                                                                    this.is_mouse_selecting = true;
                                                                    this.drag_start_idx = total_input_chars;
                                                                    if event.modifiers.shift {
                                                                        if this.selection_anchor.is_none() {
                                                                            this.selection_anchor = Some(this.cursor_char_idx);
                                                                        }
                                                                    } else {
                                                                        this.selection_anchor = None;
                                                                    }
                                                                    this.cursor_char_idx = total_input_chars;
                                                                    window.focus(&this.focus_handle);
                                                                    cx.notify();
                                                                }))
                                                                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                                                    if this.is_mouse_selecting {
                                                                        this.selection_anchor = Some(this.drag_start_idx);
                                                                        this.cursor_char_idx = total_input_chars;
                                                                        this.caret_on = true;
                                                                        cx.notify();
                                                                    }
                                                                }))
                                                                .on_mouse_up(gpui::MouseButton::Left, cx.listener(|this, _, _, _cx| {
                                                                    this.is_mouse_selecting = false;
                                                                }))
                                                                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                                                    this.poke(cx);
                                                                    this.input_active = true;
                                                                    this.caret_on = true;
                                                                    let count = match event {
                                                                        ClickEvent::Mouse(m) => m.down.click_count,
                                                                        _ => 1,
                                                                    };
                                                                    if count >= 3 {
                                                                        this.selection_anchor = Some(0);
                                                                        this.cursor_char_idx = total_input_chars;
                                                                    } else if count == 2 {
                                                                        this.select_word_at(total_input_chars.saturating_sub(1));
                                                                    } else {
                                                                        this.cursor_char_idx = total_input_chars;
                                                                        this.selection_anchor = None;
                                                                    }
                                                                    window.focus(&this.focus_handle);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                            }),
                                    ),
                            )
                            // Controles de texto (a web mantém os mesmos quatro)
                            .when(!is_calling, |parent| {
                                parent.child(
                                    div()
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .gap(px(sp.s1_5))
                                        // Chamada
                                        .child(
                                            div()
                                                .id("composer-call-btn")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(38.0))
                                                .rounded_full()
                                                .bg(c.call_bg)
                                                .border_1()
                                                .border_color(c.line)
                                                .text_color(c.muted)
                                                .hover(|style| {
                                                    style
                                                        .bg(c.call_bg_hover)
                                                        .border_color(c.line_strong)
                                                        .text_color(c.pearl)
                                                })
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.poke(cx);
                                                    this.start_call(cx);
                                                    cx.notify();
                                                }))
                                                .child(icon("phone", 18.0)),
                                        )
                                        // Volume / mudo
                                        .child(
                                            div()
                                                .id("composer-vol-btn")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(38.0))
                                                .rounded_full()
                                                .bg(if is_muted { c.danger_muted_bg } else { c.call_bg })
                                                .border_1()
                                                .border_color(if is_muted {
                                                    c.danger_muted_border
                                                } else {
                                                    c.line
                                                })
                                                .text_color(if is_muted { c.danger_muted } else { c.muted })
                                                .hover(|style| {
                                                    style
                                                        .bg(c.call_bg_hover)
                                                        .border_color(c.line_strong)
                                                        .text_color(c.pearl)
                                                })
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.touch_ui(cx);
                                                    this.is_muted = !this.is_muted;
                                                    this.sync_mic_gate();
                                                    this.persist_prefs();
                                                    cx.notify();
                                                }))
                                                .child(icon("volume", 18.0)),
                                        )
                                        // Companion de sono
                                        .child(
                                            div()
                                                .id("composer-moon-btn")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(38.0))
                                                .rounded_full()
                                                .bg(if is_companion { c.accent_tint } else { c.call_bg })
                                                .border_1()
                                                .border_color(if is_companion { c.accent_border } else { c.line })
                                                .text_color(if is_companion { c.primary } else { c.muted })
                                                .when(is_companion, |el| el.shadow(theme::glow(c.primary, 10.0)))
                                                .hover(|style| {
                                                    style
                                                        .bg(c.call_bg_hover)
                                                        .border_color(c.line_strong)
                                                        .text_color(c.pearl)
                                                })
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.poke(cx);
                                                    this.toggle_sleep_companion();
                                                    cx.notify();
                                                }))
                                                .child(icon("moon", 18.0)),
                                        )
                                        // Enviar
                                        .child(
                                            div()
                                                .id("composer-send-btn")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size(px(38.0))
                                                .rounded_full()
                                                .when(send_enabled, |el| {
                                                    el.bg(t.send_bg())
                                                        .text_color(c.pearl)
                                                        .shadow(t.shadow.send.clone())
                                                })
                                                .when(!send_enabled, |el| {
                                                    el.bg(c.surface_soft)
                                                        .text_color(c.faint)
                                                        .opacity(0.38)
                                                })
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.submit(cx);
                                                }))
                                                .child(icon("send", 18.0)),
                                        ),
                                )
                            })
                    )
                    // Controles de voz (dock) durante a chamada
                    .when(is_calling, |parent| {
                        parent.child(
                            div()
                                .id("call-dock")
                                .flex()
                                .items_center()
                                .gap(px(sp.s2_5))
                                .child(
                                    div()
                                        .id("call-mic")
                                        .cursor_pointer()
                                        .size(px(48.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(c.icon_bg)
                                        .border_1()
                                        .border_color(if is_listening { c.success_border } else { c.line })
                                        .when(is_listening, |el| {
                                            el.shadow(theme::glow(c.ok, 14.0)).opacity(1.0)
                                        })
                                        .when(!is_listening, |el| el.opacity(0.6))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_listening(cx);
                                        }))
                                        .child(icon("pulse", 19.0)),
                                )
                                .child(
                                    div()
                                        .id("call-chat")
                                        .cursor_pointer()
                                        .size(px(48.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(if chat_open { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if chat_open { c.primary } else { c.line })
                                        .when(chat_open, |el| el.shadow(theme::glow(c.primary, 14.0)))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Chat, cx);
                                        }))
                                        .child(icon("chat_bubble", 19.0)),
                                )
                                .child(
                                    div()
                                        .id("call-hangup")
                                        .cursor_pointer()
                                        .size(px(48.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(c.err)
                                        .border_1()
                                        .border_color(c.err)
                                        .hover(|style| style.bg(c.err_dark).shadow(theme::glow(c.err, 14.0)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.end_call(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("phone", 19.0)),
                                )
                                .child(
                                    div()
                                        .id("call-volume")
                                        .cursor_pointer()
                                        .size(px(48.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(if is_muted { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if is_muted { c.primary } else { c.line })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.touch_ui(cx);
                                            this.is_muted = !this.is_muted;
                                            this.sync_mic_gate();
                                            this.persist_prefs();
                                            cx.notify();
                                        }))
                                        .child(icon("volume", 19.0)),
                                )
                                .child(
                                    div()
                                        .id("call-sleep")
                                        .cursor_pointer()
                                        .size(px(48.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(if is_companion { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if is_companion { c.primary } else { c.line })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.toggle_sleep_companion();
                                            cx.notify();
                                        }))
                                        .child(icon("moon", 19.0)),
                                ),
                        )
                    }),
            )
            // ── LAYER 6: PAINEL DE CONVERSA EM FORMATO SMARTPHONE ESTILO WHATSAPP ──
            .when(chat_open, |parent| {
                let status_subtitle: SharedString = if is_busy {
                    "digitando...".into()
                } else if is_calling {
                    "em chamada de voz".into()
                } else if self.is_companion_mode {
                    "modo companion ativo".into()
                } else {
                    "online".into()
                };

                parent.child(
                    div()
                        .id("chat-history-smartphone")
                        // Sem occlude, a rodinha sobre o telefone também chega ao
                        // VrmView (que ajusta a câmera no scroll) e a modelo
                        // cresce/diminui enquanto a conversa rola.
                        .occlude()
                        .absolute()
                        .left(px(sp.s6))
                        .bottom(px(84.0))
                        .w(px(382.0))
                        .h(px(615.0))
                        .flex()
                        .flex_col()
                        // Chassis do smartphone flagship (titânio escovado com aro metálico ultrafino chanfrado)
                        .rounded(px(46.0))
                        .overflow_hidden()
                        .bg(gpui::rgba(0x16181dff))
                        .p(px(3.5))
                        .border_1()
                        .border_color(gpui::rgba(0xffffff28))
                        .shadow(vec![
                            BoxShadow {
                                color: gpui::rgba(0x000000c0).into(),
                                offset: point(px(0.0), px(20.0)),
                                blur_radius: px(46.0),
                                spread_radius: px(0.0),
                            },
                            BoxShadow {
                                color: gpui::rgba(0x00000080).into(),
                                offset: point(px(0.0), px(4.0)),
                                blur_radius: px(14.0),
                                spread_radius: px(0.0),
                            },
                        ])
                        .child(
                            // Tela interna OLED do smartphone
                            div()
                                .id("phone-screen")
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_col()
                                .rounded(px(42.0))
                                .overflow_hidden()
                                // overflow_hidden do GPUI recorta só retângulo:
                                // qualquer filho opaco de canto reto cobre a
                                // curva. Então o fundo arredondado da própria
                                // tela pinta os cantos superiores com a cor do
                                // cabeçalho, e as barras ficam transparentes.
                                .bg(gpui::rgba(0x1f2c34ff))
                                // 1. Barra de status integrada com Dynamic Island
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(32.0))
                                        .px(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        // Horário à esquerda
                                        .child(
                                            div()
                                                .min_w(px(44.0))
                                                .text_size(px(11.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gpui::rgba(0xf0f2f5ff))
                                                .child(current_phone_time()),
                                        )
                                        // Dynamic Island elegante centralizada
                                        .child(
                                            div()
                                                .w(px(96.0))
                                                .h(px(18.0))
                                                .rounded_full()
                                                .bg(gpui::rgba(0x000000ff))
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .px(px(8.0))
                                                .child(
                                                    div()
                                                        .size(px(7.0))
                                                        .rounded_full()
                                                        .bg(gpui::rgba(0x111c2fff))
                                                        .border_1()
                                                        .border_color(gpui::rgba(0x070c17ee)),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(26.0))
                                                        .h(px(2.5))
                                                        .rounded_full()
                                                        .bg(gpui::rgba(0x28303cee)),
                                                ),
                                        )
                                        // Indicadores 5G, Wi-Fi e Bateria à direita
                                        .child(
                                            div()
                                                .min_w(px(44.0))
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .gap(px(5.0))
                                                .child(
                                                    div()
                                                        .text_size(px(9.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(gpui::rgba(0xaebac1ff))
                                                        .child("5G"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(9.5))
                                                        .text_color(gpui::rgba(0xaebac1ff))
                                                        .child("📶"),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(2.5))
                                                        .child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xaebac1ff))
                                                                .child("98%"),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(16.0))
                                                                .h(px(8.5))
                                                                .rounded(px(2.0))
                                                                .border_1()
                                                                .border_color(gpui::rgba(0xaebac1ff))
                                                                .p(px(1.0))
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .h_full()
                                                                        .rounded(px(1.0))
                                                                        .bg(gpui::rgba(0x25d366ff)),
                                                                ),
                                                        ),
                                                ),
                                        ),
                                )
                                // 2. Header do WhatsApp
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(7.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .border_b_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .id("phone-back-arrow")
                                                        .cursor_pointer()
                                                        .px(px(4.0))
                                                        .text_size(px(20.0))
                                                        .text_color(gpui::rgba(0x00a884ff))
                                                        .hover(|s| s.text_color(gpui::rgba(0x25d366ff)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.active_drawer = ActiveDrawer::None;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))
                                                        .child("‹"),
                                                )
                                                .child(
                                                    div()
                                                        .relative()
                                                        .size(px(34.0))
                                                        .child(render_lego_portrait(
                                                            34.0,
                                                            17.0,
                                                            mood_vis.accent,
                                                            hud_portrait,
                                                            hud_prev_slot.clone(),
                                                            mood_vis.glyph,
                                                        ))
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .bottom(px(0.0))
                                                                .right(px(0.0))
                                                                .size(px(9.0))
                                                                .rounded_full()
                                                                .bg(gpui::rgba(0x25d366ff))
                                                                .border_1()
                                                                .border_color(gpui::rgba(0x1f2c34ff)),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .ml(px(4.0))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(3.0))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(13.0))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(gpui::rgba(0xe9edefff))
                                                                        .child("Usami"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(10.0))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(gpui::rgba(0x25d366ff))
                                                                        .child("✓"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(if is_busy {
                                                                    gpui::rgba(0x25d366ff)
                                                                } else {
                                                                    gpui::rgba(0x8696a0ff)
                                                                })
                                                                .child(status_subtitle),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .pr(px(2.0))
                                                .child(
                                                    div()
                                                        .id("phone-call-btn")
                                                        .cursor_pointer()
                                                        .p(px(6.0))
                                                        .rounded_full()
                                                        .text_color(gpui::rgba(0xaebac1ff))
                                                        .hover(|s| s.bg(gpui::rgba(0x2a3942ff)).text_color(gpui::rgba(0xe9edefff)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.poke(cx);
                                                            this.start_call(cx);
                                                            cx.notify();
                                                        }))
                                                        .child(icon("phone", 16.0)),
                                                )
                                                .child(
                                                    div()
                                                        .id("phone-close-btn")
                                                        .cursor_pointer()
                                                        .p(px(6.0))
                                                        .rounded_full()
                                                        .text_color(gpui::rgba(0x8696a0ff))
                                                        .hover(|s| s.bg(gpui::rgba(0x2a3942ff)).text_color(gpui::rgba(0xe9edefff)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.active_drawer = ActiveDrawer::None;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))
                                                        .child(icon("close", 14.0)),
                                                ),
                                        ),
                                )
                                // 3. Lista de Mensagens estilo WhatsApp (sem justify_end glitchy)
                                .child(
                                    div()
                                        .id("phone-chat-messages")
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .p(px(10.0))
                                        .bg(gpui::rgba(0x0b141aff)) // Fundo de conversa
                                        .overflow_y_scroll()
                                        .track_scroll(&phone_scroll)
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.0))
                                        // Pílula de data centralizada
                                        .child(
                                            div()
                                                .w_full()
                                                .flex()
                                                .justify_center()
                                                .mb(px(2.0))
                                                .child(
                                                    div()
                                                        .rounded(px(6.0))
                                                        .bg(gpui::rgba(0x182229ff))
                                                        .px(px(10.0))
                                                        .py(px(3.0))
                                                        .text_size(px(10.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(gpui::rgba(0x8696a0ff))
                                                        .child("HOJE"),
                                                ),
                                        )
                                        // Aviso de segurança do WhatsApp
                                        .child(
                                            div()
                                                .w_full()
                                                .flex()
                                                .justify_center()
                                                .mb(px(4.0))
                                                .child(
                                                    div()
                                                        .max_w(px(280.0))
                                                        .rounded(px(6.0))
                                                        .bg(gpui::rgba(0x182229ee))
                                                        .px(px(8.0))
                                                        .py(px(5.0))
                                                        .text_size(px(9.5))
                                                        .text_color(gpui::rgba(0xffd279ff))
                                                        .text_center()
                                                        .child("🔒 As mensagens são protegidas de ponta a ponta com a Usami."),
                                                ),
                                        )
                                        .when(no_turns, |col| {
                                            col.child(
                                                div()
                                                    .py(px(28.0))
                                                    .flex()
                                                    .flex_col()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(gpui::rgba(0xe9edefff))
                                                            .child("Comece uma conversa! 👋"),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(4.0))
                                                            .text_size(px(10.5))
                                                            .text_color(gpui::rgba(0x8696a0ff))
                                                            .child("Digite no teclado ou fale por chamada de voz."),
                                                    ),
                                            )
                                        })
                                        .children(turns.into_iter().map(|msg| {
                                            let is_user = msg.sender == Sender::User;
                                            let time = msg.time_str.as_deref().unwrap_or("agora");

                                            if is_user {
                                                div()
                                                    .flex()
                                                    .flex_shrink_0()
                                                    .w_full()
                                                    .justify_end()
                                                    .child(
                                                        div()
                                                            .max_w(px(260.0))
                                                            .py(px(6.0))
                                                            .px(px(9.0))
                                                            .rounded_tl(px(10.0))
                                                            .rounded_bl(px(10.0))
                                                            .rounded_br(px(10.0))
                                                            .rounded_tr(px(2.0))
                                                            .bg(gpui::rgba(0x005c4bff)) // Verde clássico outgoing do WhatsApp
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.5))
                                                                    .line_height(px(17.0))
                                                                    .text_color(gpui::rgba(0xe9edefff))
                                                                    .child(msg.text),
                                                            )
                                                            .when_some(msg.media.as_ref(), |_parent, media| {
                                                                if media.mime_type.starts_with("image/") {
                                                                    div()
                                                                        .mt(px(6.0))
                                                                        .w(px(150.0))
                                                                        .h(px(105.0))
                                                                        .rounded(px(8.0))
                                                                        .overflow_hidden()
                                                                        .bg(gpui::rgba(0x0b1419ff))
                                                                        .border_1()
                                                                        .border_color(gpui::rgba(0x2a3942ff))
                                                                        .child(
                                                                            img(media.preview.clone())
                                                                                .size_full()
                                                                                .object_fit(ObjectFit::Cover),
                                                                        )
                                                                } else {
                                                                    div()
                                                                        .mt(px(6.0))
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap(px(8.0))
                                                                        .text_size(px(11.0))
                                                                        .text_color(gpui::rgba(0x8696a0ff))
                                                                        .child("📄")
                                                                        .child(media.filename.clone())
                                                                }
                                                            })
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .justify_end()
                                                                    .items_center()
                                                                    .gap(px(3.0))
                                                                    .mt(px(2.0))
                                                                    .child(
                                                                        div()
                                                                            .text_size(px(9.5))
                                                                            .text_color(gpui::rgba(0xffffffaa))
                                                                            .child(time.to_string()),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_size(px(11.0))
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(gpui::rgba(0x53bdebff)) // Check duplo azul de leitura!
                                                                            .child("✓✓"),
                                                                    ),
                                                            ),
                                                    )
                                            } else {
                                                let mut header = match msg.emotion.as_deref() {
                                                    Some(emotion) => format!("USAMI · {emotion}"),
                                                    None => "USAMI".to_string(),
                                                };
                                                if let Some(badge) = msg.tool_badge.as_deref() {
                                                    header.push_str(&format!(" · {badge}"));
                                                }

                                                div()
                                                    .flex()
                                                    .flex_shrink_0()
                                                    .w_full()
                                                    .justify_start()
                                                    .child(
                                                        div()
                                                            .max_w(px(260.0))
                                                            .py(px(6.0))
                                                            .px(px(9.0))
                                                            .rounded_tr(px(10.0))
                                                            .rounded_br(px(10.0))
                                                            .rounded_bl(px(10.0))
                                                            .rounded_tl(px(2.0))
                                                            .bg(gpui::rgba(0x202c33ff)) // Cinza ardósia incoming do WhatsApp
                                                            .child(
                                                                div()
                                                                    .mb(px(2.0))
                                                                    .text_size(px(9.5))
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(gpui::rgba(0x25d366ff))
                                                                    .child(header),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.5))
                                                                    .line_height(px(17.0))
                                                                    .text_color(gpui::rgba(0xe9edefff))
                                                                    .child(msg.text),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .justify_end()
                                                                    .mt(px(2.0))
                                                                    .child(
                                                                        div()
                                                                            .text_size(px(9.5))
                                                                            .text_color(gpui::rgba(0x8696a0ff))
                                                                            .child(time.to_string()),
                                                                    ),
                                                            ),
                                                    )
                                            }
                                        }))
                                        // Indicador de digitação suave (sem alterar margins e sem jitter de scroll!)
                                        .when(is_busy, |col| {
                                            col.child(
                                                div()
                                                    .flex()
                                                    .flex_shrink_0()
                                                    .w_full()
                                                    .justify_start()
                                                    .child(
                                                        div()
                                                            .py(px(7.0))
                                                            .px(px(12.0))
                                                            .rounded_tr(px(10.0))
                                                            .rounded_br(px(10.0))
                                                            .rounded_bl(px(10.0))
                                                            .rounded_tl(px(2.0))
                                                            .bg(gpui::rgba(0x202c33ff))
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap(px(4.0))
                                                                    .children((0..3usize).map(|idx| {
                                                                        div()
                                                                            .size(px(5.5))
                                                                            .rounded_full()
                                                                            .bg(gpui::rgba(0x25d366ff))
                                                                            .with_animation(
                                                                                ("wa-typing", idx),
                                                                                Animation::new(Duration::from_millis(1000)).repeat(),
                                                                                move |el, delta| {
                                                                                    let phase = (delta + idx as f32 * 0.25) % 1.0;
                                                                                    let opacity = 0.25 + 0.75 * (phase * std::f32::consts::PI).sin().max(0.0);
                                                                                    el.opacity(opacity)
                                                                                },
                                                                            )
                                                                    }))
                                                                    // Rótulo do que ela está fazendo agora: sem isto
                                                                    // os três pontos não dizem se ela está pensando,
                                                                    // executando uma ferramenta ou travada.
                                                                    .when(!typing_label.is_empty(), |row| {
                                                                    row.child(
                                                                        div()
                                                                            .text_size(px(11.0))
                                                                            .text_color(gpui::rgba(0x8696a0ff))
                                                                            .child(typing_label.clone()),
                                                                    )
                                                                }),
                                                            ),
                                                    ),
                                            )
                                        }),
                                )
                                // 4a. Placa do arco inferior. O GPUI limita cada raio
                                // a metade da altura do quad, então uma faixa de
                                // 46px desenharia raio 23. Esta placa de 84px (2x42)
                                // garante o raio 42; o input bar opaco cobre os
                                // primeiros 38px dela, então só a faixa de 46px
                                // abaixo continua visível.
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .bottom_0()
                                        .w_full()
                                        .h(px(84.0))
                                        .rounded_bl(px(42.0))
                                        .rounded_br(px(42.0))
                                        .bg(gpui::linear_gradient(
                                            180.0,
                                            gpui::linear_color_stop(
                                                gpui::rgba(0x1f2c34ff),
                                                0.55,
                                            ),
                                            gpui::linear_color_stop(
                                                gpui::rgba(0x0c1317ff),
                                                1.0,
                                            ),
                                        )),
                                )
                                // 4b. Barra inferior do WhatsApp (dentro do smartphone)
                                .child(
                                    div()
                                        .bg(gpui::rgba(0x1f2c34ff))
                                        .px(px(8.0))
                                        .pt(px(6.0))
                                        // Safe area: afasta o input e o botão de enviar
                                        // da borda inferior do chassi.
                                        .pb(px(10.0))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_end()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_size(px(16.0))
                                                .cursor_pointer()
                                                .child("😊"),
                                        )
                                        .child(
                                            // Container pai do input: altura máxima rígida e
                                            // rolagem própria — o texto nunca dita a altura do
                                            // celular nem empurra os botões laterais.
                                            div()
                                                .id("phone-input-scroll")
                                                .flex_1()
                                                .flex_shrink_0()
                                                .min_w(px(0.0))
                                                .min_h(px(0.0))
                                                // 4 linhas de 16px + py(6) — corte sempre na
                                                // fronteira da linha, nunca no meio do glifo.
                                                .max_h(px(76.0))
                                                .flex()
                                                .flex_col()
                                                .overflow_y_scroll()
                                                .bg(gpui::rgba(0x2a3942ff))
                                                .rounded_full()
                                                .px(px(12.0))
                                                .py(px(6.0))
                                                .cursor_text()
                                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                                    this.poke(cx);
                                                    this.input_active = true;
                                                    this.caret_on = true;
                                                    window.focus(&this.focus_handle);
                                                    cx.notify();
                                                }))
                                                .child(
                                                    // Elemento de texto em si: no GPUI, texto dentro
                                                    // de scroll não ganha largura com w_full(), então a
                                                    // quebra de linha exige uma largura explícita.
                                                    div()
                                                        .id("phone-input-content")
                                                        .flex_shrink_0()
                                                        .w(px(236.0))
                                                        .whitespace_normal()
                                                        .text_size(px(11.5))
                                                        .line_height(px(16.0))
                                                        .text_color(if input_text.is_empty() {
                                                            gpui::rgba(0x8696a0ff)
                                                        } else {
                                                            gpui::rgba(0xe9edefff)
                                                        })
                                                        .child(if input_text.is_empty() {
                                                            "Mensagem...".to_string()
                                                        } else {
                                                            input_text.clone()
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("phone-attach-btn")
                                                .flex_shrink_0()
                                                .cursor_pointer()
                                                .p(px(5.0))
                                                .text_color(gpui::rgba(0x8696a0ff))
                                                .hover(|s| s.text_color(gpui::rgba(0xe9edefff)))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.attach_file_prompt(cx);
                                                }))
                                                .child(icon("image", 16.0)),
                                        )
                                        .child(
                                            div()
                                                .id("phone-send-btn")
                                                .flex_shrink_0()
                                                .cursor_pointer()
                                                .size(px(32.0))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .bg(gpui::rgba(0x25d366ff))
                                                .text_color(gpui::rgba(0xffffffff))
                                                .hover(|s| s.bg(gpui::rgba(0x20ba5cff)))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.submit(cx);
                                                }))
                                                .child(icon("send", 14.0)),
                                        ),
                                )
                                // 5. Home Bar inferior do Smartphone
                                .child(
                                    div()
                                        .w_full()
                                        // Faixa transparente: só reserva o espaço e
                                        // posiciona a pílula. A cor e o arco vêm da
                                        // placa absoluta (4a), que tem altura para
                                        // honrar o raio de 42.
                                        .h(px(46.0))
                                        .flex()
                                        .items_end()
                                        .justify_center()
                                        .pb(px(5.0))
                                        .child(
                                            div()
                                                .w(px(100.0))
                                                .h(px(3.5))
                                                .rounded_full()
                                                .bg(gpui::rgba(0xffffff40)),
                                        ),
                                ),
                        ),
                )
            })
            // ── LAYER 7: PAINÉIS LATERAIS (VÍNCULO / CONFIGURAÇÕES / VRMA) ──
            .when(side_drawer_open, |parent| {
                parent
                    .child(
                    div()
                        .id("side-drawer")
                        .occlude()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(if active_drawer == ActiveDrawer::Skills {
                            500.0
                        } else {
                            420.0
                        }))
                        .h(viewport_h)
                        .flex()
                        .flex_col()
                        .border_l_1()
                        .border_color(c.line_strong)
                        .bg(c.panel)
                        .p(px(sp.s8))
                        .gap(px(sp.s4))
                        .shadow(t.shadow.drawer.clone())
                        .when(active_drawer != ActiveDrawer::Settings, |parent| {
                            parent.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb(px(sp.s3))
                                .border_b_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.lavender)
                                        .child(match active_drawer {
                                            ActiveDrawer::Chat => "",
                                            ActiveDrawer::Bond => "VÍNCULO COM A USAMI",
                                            ActiveDrawer::Settings => "CONFIGURAÇÕES",
                                            ActiveDrawer::Vrma => "ANIMAÇÕES VRMA",
                                            ActiveDrawer::Skills => "CENTRAL DE SKILLS",
                                            ActiveDrawer::None => "",
                                        }),
                                )
                                .child(
                                    div()
                                        .id("drawer-close-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(28.0))
                                        .rounded_full()
                                        .text_color(c.muted)
                                        .hover(|style| style.bg(c.surface_soft).text_color(c.pearl))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::None;
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("close", 15.0)),
                                ),
                            )
                        })
                        .child(match active_drawer {
                            ActiveDrawer::Chat => div(),
                            ActiveDrawer::Bond => div(),
                            ActiveDrawer::Vrma => {
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(sp.s3))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("TESTES DE ANIMAÇÃO"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap(px(sp.s2))
                                            .child(
                                                div()
                                                    .id("vrma-wave")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_vrma("goodbye", false);
                                                        cx.notify();
                                                    }))
                                                    .child("Acenar"),
                                            )
                                            .child(
                                                div()
                                                    .id("vrma-jump")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_vrma("jump", false);
                                                        cx.notify();
                                                    }))
                                                    .child("Pular"),
                                            )
                                            .child(
                                                div()
                                                    .id("vrma-idle")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_idle();
                                                        cx.notify();
                                                    }))
                                                    .child("Idle Natural"),
                                            ),
                                    )
                            }
                            ActiveDrawer::Settings => div(),
                            ActiveDrawer::Skills => div()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_col()
                                .child(self.render_agent_center_drawer(cx)),
                            ActiveDrawer::None => div(),
                        }),
                )
            })
            // ── LAYER 8: CENTRAL DE RELACIONAMENTO & DIÁRIO ESTILO VISUAL NOVEL ──
            .when(active_drawer == ActiveDrawer::Bond, |parent| {
                let (bond_exp_curr, bond_exp_target, bond_exp_needed, bond_level_title, bond_is_max) =
                    match bond.as_ref() {
                        Some(b) => (
                            b.summary.xp,
                            b.summary.next.unwrap_or(b.summary.xp),
                            b.summary.remaining,
                            b.summary.name.clone(),
                            b.summary.max,
                        ),
                        None => (0, 0, 0, "—".to_string(), false),
                    };

                parent.child(
                    div()
                        .id("vn-scrim-overlay")
                        .occlude()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x06030edd))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.active_drawer = ActiveDrawer::None;
                            this.poke(cx);
                            cx.notify();
                        }))
                        .child(
                            // Container Principal da Central de Visual Novel (960x610)
                            div()
                                .id("vn-modal-container")
                                .w(px(960.0))
                                .h(px(610.0))
                                .max_h(px(610.0))
                                .flex()
                                .flex_row()
                                .overflow_hidden()
                                .rounded(px(26.0))
                                .bg(t.vn_modal_bg())
                                .border_2()
                                .border_color(theme::Palette::vn_border_bright())
                                .shadow(t.shadow.vn_modal.clone())
                                // O modal fica dentro do scrim clicável: sem parar a
                                // propagação, todo clique interno sobe e fecha o painel.
                                .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    cx.stop_propagation();
                                })
                                // ── COLUNA ESQUERDA: SIDEBAR VISUAL NOVEL ──
                                .child(
                                    div()
                                        .w(px(230.0))
                                        .h_full()
                                        .flex_shrink_0()
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .p(px(16.0))
                                        .bg(t.vn_sidebar_bg())
                                        .border_r_1()
                                        .border_color(theme::Palette::vn_card_border())
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(12.0))
                                                // Header da Usami
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(2.0))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(6.0))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(22.0))
                                                                        .font_family(theme::FONT_CHARACTER)
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(c.pearl)
                                                                        .child("Usami ♡"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(ty.xs))
                                                                .font(theme::font_with_jp(theme::FONT_DISPLAY))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child("ウサミ · VN Romance Hub"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.5))
                                                                .text_color(c.muted)
                                                                .child("Sempre aqui, com você ♡"),
                                                        ),
                                                )
                                                // Divisor delicado
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(1.0))
                                                        .bg(theme::Palette::vn_card_border()),
                                                )
                                                // Menu de capítulos verticais
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(6.0))
                                                        .child(render_vn_tab_btn("vn-tab-home", "01", "Início", bond_tab == BondTab::Home, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Home;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-diary", "02", "Diário", bond_tab == BondTab::Diary, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Diary;
                                                            if !this.memory_loading {
                                                                this.fetch_memory(cx);
                                                            }
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-memories", "03", "Álbum de Memórias", bond_tab == BondTab::Memories, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Memories;
                                                            if !this.memory_loading {
                                                                this.fetch_memory(cx);
                                                            }
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-bonds", "04", "Laços", bond_tab == BondTab::Bonds, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Bonds;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-achievements", "05", "Conquistas", bond_tab == BondTab::Achievements, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Achievements;
                                                            this.fetch_achievements(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-mood", "06", "Humor", bond_tab == BondTab::Mood, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Mood;
                                                            this.fetch_mood(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-profile", "07", "Perfil da Usami", bond_tab == BondTab::Profile, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Profile;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-settings", "08", "Ajustes", bond_tab == BondTab::Settings, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Settings;
                                                            this.load_settings_data(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))),
                                                ),
                                        )
                                        // Rodapé Mascote Chibi
                                        .child(
                                            div()
                                                .p(px(10.0))
                                                .rounded(px(14.0))
                                                .bg(rgba(0x23123d88))
                                                .border_1()
                                                .border_color(theme::Palette::vn_card_border())
                                                .flex()
                                                .flex_col()
                                                .gap(px(4.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_size(px(14.0))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child("(˶ᵔ ᵕ ᵔ˶) ♡"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(theme::Palette::vn_gold())
                                                                .child("✦"),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .font(theme::font_character_italic())
                                                        .text_color(c.pearl)
                                                        .child("“Você voltou... Que bom! ♡”"),
                                                ),
                                        ),
                                )
                                // ── COLUNA DIREITA: CONTEÚDO PRINCIPAL ──
                                .child(
                                    div()
                                        .flex_1()
                                        .h_full()
                                        .min_w(px(0.0))
                                        .min_h(px(0.0))
                                        .flex()
                                        .flex_col()
                                        .p(px(18.0))
                                        .gap(px(10.0))
                                        .overflow_hidden()
                                        // Header Superior de Nível e EXP
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .pb(px(6.0))
                                                .border_b_1()
                                                .border_color(theme::Palette::vn_card_border())
                                                // Badge Coração de Nível
                                                .child(
                                                    div()
                                                        .px(px(14.0))
                                                        .py(px(4.0))
                                                        .rounded(px(16.0))
                                                        .bg(t.vn_heart_badge_bg())
                                                        .border_1()
                                                        .border_color(theme::Palette::vn_border_bright())
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_size(px(16.0))
                                                                .font_family(theme::FONT_CHARACTER)
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(c.pearl)
                                                                .child(format!("♥ Nv. {bond_level}")),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child(format!("✦ {} ✦", bond_level_title)),
                                                        ),
                                                )
                                                // Barra de EXP
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .mx(px(14.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .justify_between()
                                                                .items_center()
                                                                .child(
                                                                    div()
                                                                        .text_size(px(11.0))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(c.pearl)
                                                                        .child(if bond_is_max {
                                                                            format!("{bond_exp_curr} EXP")
                                                                        } else {
                                                                            format!("{bond_exp_curr} / {bond_exp_target} EXP")
                                                                        }),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(9.5))
                                                                        .text_color(theme::Palette::vn_gold())
                                                                        .child("✦ BÔNUS DIÁRIO ✦"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .h(px(8.0))
                                                                .rounded_full()
                                                                .bg(c.bond_track)
                                                                .overflow_hidden()
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(relative(bond_exp_pct as f32 / 100.0))
                                                                        .rounded_full()
                                                                        .bg(t.vn_exp_fill_bg()),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child(if bond_is_max {
                                                                    "★ Nível máximo alcançado! ★".to_string()
                                                                } else {
                                                                    format!("★ Faltam {} EXP para o próximo nível! ★", bond_exp_needed)
                                                                }),
                                                        ),
                                                )
                                                // Botão Fechar
                                                .child(
                                                    div()
                                                        .id("vn-bond-close-btn")
                                                        .cursor_pointer()
                                                        .size(px(32.0))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(theme::Palette::vn_card_border())
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(theme::Palette::vn_pink_neon()).border_color(c.pearl))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.active_drawer = ActiveDrawer::None;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))
                                                        .child(icon("close", 14.0)),
                                                ),
                                        )
                                        // Conteúdo dinâmico da Aba
                                        .child(
                                            match bond_tab {
                                                BondTab::Home => render_vn_tab_home(bond_level, bond.as_ref(), t).into_any_element(),
                                                BondTab::Diary => render_vn_tab_diary(bond.as_ref(), memory_data.as_ref(), t).into_any_element(),
                                                BondTab::Memories => render_vn_tab_memories(bond.as_ref(), memory_data.as_ref(), t).into_any_element(),
                                                BondTab::Bonds => render_vn_tab_bonds(bond_level, bond.as_ref(), t).into_any_element(),
                                                BondTab::Achievements => render_vn_tab_achievements(&achievements, t).into_any_element(),
                                                BondTab::Mood => self.render_vn_tab_mood(mood_data.as_ref(), t, cx).into_any_element(),
                                                BondTab::Profile => render_vn_tab_profile(
                                                    mood_data.as_ref(),
                                                    mood_vis,
                                                    self.vrm_controller.portrait_image(),
                                                    self.prev_portrait_image.clone(),
                                                    t,
                                                )
                                                .into_any_element(),
                                                BondTab::Settings => self.render_settings_page(cx).into_any_element(),
                                            }
                                        ),
                                ),
                        ),
                )
            })
    }
}

fn main() {
    println!("[Usami Desktop] Inicializando interface minimalista idêntica à print...");
    Application::new().run(|cx: &mut App| {
        // Fontes OFL locais (sem CDN em runtime). Fica antes de abrir a janela
        // para o primeiro frame já medir com as famílias certas.
        theme::load_fonts(cx, asset_base_dir());
        let prefs = storage::load_prefs();
        let bounds = prefs
            .window
            .filter(|window| window.is_sane())
            .map(|window| {
                Bounds::new(
                    point(px(window.x), px(window.y)),
                    size(px(window.width), px(window.height)),
                )
            })
            .unwrap_or_else(|| Bounds::centered(None, size(px(1440.0), px(900.0)), cx));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Usami".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| UsamiApp::new(cx, prefs));
                window.focus(&view.read(cx).focus_handle);
                view.update(cx, |this, cx| this.attach_window_observers(window, cx));
                view.update(cx, |this, cx| this.fetch_bond(cx));
                view.update(cx, |this, cx| this.fetch_achievements(cx));
                view.update(cx, |this, cx| this.fetch_memory(cx));
                view.update(cx, |this, cx| this.fetch_mood(cx));
                #[cfg(target_os = "windows")]
                view.update(cx, |this, cx| this.attach_presence(window, cx));
                view
            },
        )
        .unwrap();
    });
}

#[cfg(test)]
mod tests {
    // `use gpui::*` no topo do crate traz o atributo `test` do GPUI; importar
    // só os helpers evita que `#[test]` resolva para ele e entre em recursão.
    use super::{
        farewell_can_close, mood_preset_options, mood_visual,
        relative_when_at, vn_history_tag,
    };
    use std::time::Duration;

    #[test]
    fn history_time_and_tags_reflect_the_core_log() {
        assert_eq!(relative_when_at(1_000, 1_000), "agora");
        assert_eq!(relative_when_at(0, 90_000), "há 1 min");
        assert_eq!(relative_when_at(0, 3_600_000), "há 1 h");
        assert_eq!(relative_when_at(0, 86_400_000), "ontem");
        assert_eq!(relative_when_at(0, 3 * 86_400_000), "há 3 dias");
        // timestamp no futuro (relógio ajustado) não vira "há -1 dias".
        assert_eq!(relative_when_at(5_000, 1_000), "agora");

        assert_eq!(vn_history_tag("laugh"), "RISADA");
        assert_eq!(vn_history_tag("call"), "CHAMADA");
        assert_eq!(vn_history_tag("inventado"), "CHAT");
    }

    #[test]
    fn farewell_holds_close_until_audio_ends_or_timeout() {
        use super::{CLOSE_FAREWELL_MAX, CLOSE_FAREWELL_MIN};
        assert!(!farewell_can_close(Duration::ZERO, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, true, true));
        assert!(farewell_can_close(CLOSE_FAREWELL_MIN, true, false));
        assert!(farewell_can_close(CLOSE_FAREWELL_MAX, false, true));
    }

    #[test]
    fn diary_keeps_memory_summaries_even_when_bond_log_is_full() {
        use super::collect_diary_entries;
        let log: Vec<serde_json::Value> = (0..15)
            .map(|i| {
                serde_json::json!({
                    "kind": "chat",
                    "text": format!("Momento {i}"),
                    "at": 1_700_000_000_000_i64 + i * 1_000,
                    "count": 1
                })
            })
            .collect();
        let bond: crate::api::BondData = serde_json::from_value(serde_json::json!({
            "intimacy": { "carinho": 10, "humor": 5, "companhia": 8 },
            "meters": {
                "carinho": { "xp": 10, "level": 1, "progress": 0.5 },
                "humor": { "xp": 5, "level": 1, "progress": 0.25 },
                "companhia": { "xp": 8, "level": 1, "progress": 0.4 }
            },
            "log": log,
            "summary": { "level": 1, "name": "Conhecidos", "xp": 23, "next": 30, "remaining": 7, "max": false, "line": "oi" }
        }))
        .expect("bond json");
        let memory: crate::api::MemoryData = serde_json::from_value(serde_json::json!({
            "summaries": [
                { "date": "2026-11-09", "summary": "Primeiro dia" },
                { "date": "2026-11-10", "summary": "Segundo dia" }
            ]
        }))
        .expect("memory json");

        let entries = collect_diary_entries(Some(&bond), Some(&memory));
        assert_eq!(entries.len(), 17);
        assert_eq!(entries[15].0, "[LEMBRANÇA]");
        assert_eq!(entries[15].1, "Segundo dia");
        assert_eq!(entries[16].1, "Primeiro dia");
    }

    #[test]
    fn mood_visual_maps_profiles_and_presets() {
        assert_eq!(mood_visual("tsundere").label, "Tsundere");
        assert_eq!(mood_visual("soft").label, "Carinhosa");
        assert_eq!(mood_visual("SLEEPY").label, "Sonolenta");
        assert_eq!(mood_visual("desconhecido").label, "Tranquila");

        let presets = mood_preset_options();
        assert!(presets.iter().any(|p| p.emotion == "happy"));
        assert!(presets.iter().any(|p| p.emotion == "tsundere"));
        assert!(presets.iter().any(|p| p.emotion == "neutral"));
    }
}
