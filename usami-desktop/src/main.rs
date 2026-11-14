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
mod memory_personality;
mod bond;
mod proactive;
mod input_editor;
mod lifecycle;
mod views;
mod voice;
mod text_input;

use crate::agent_center_model::{
    pending_confirmations_count, pending_suggestions_count, AgentCenterTab, ExecFilter, SkillFilter,
};
use crate::assets::{asset_base_dir, asset_path, env_path, first_existing};
use crate::api::UsamiApiClient;
use crate::audio::AudioPlayer;
use crate::media::AttachedMedia;
use crate::model::{chat_message_from_stored, current_phone_time, format_tool_badge, stored_turns, ActiveDrawer, BondTab, ChatMessage, Sender};
use crate::ui::widgets::icon;
use crate::views::vn_bond::{
    mood_visual, render_lego_portrait, render_vn_tab_achievements, render_vn_tab_bonds,
    render_vn_tab_btn, render_vn_tab_diary, render_vn_tab_home, render_vn_tab_memories,
    render_vn_tab_profile,
};
use crate::speech::sanitize_speech_text;
use crate::text_input::tokenize_text_spans;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_vrm::{VrmController, VrmView};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};


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
    pub(crate) fn attach_presence(&mut self, window: &Window, cx: &mut Context<Self>) {
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
    pub(crate) fn touch_ui(&mut self, cx: &mut Context<Self>) {
        self.last_user_activity = Instant::now();
        self.idle_prods = 0;
        self.poke(cx);
    }

    pub(crate) fn push_history(&mut self, mut message: ChatMessage) {
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

    fn persist_history(&self) {
        storage::save_conversation(&stored_turns(&self.chat_history));
    }

    pub(crate) fn persist_prefs(&mut self) {
        self.prefs.muted = self.is_muted;
        storage::save_prefs(&self.prefs);
        self.last_prefs_save = Instant::now();
    }



    pub(crate) fn attach_file_prompt(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
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
                this.handle_key_down(event, cx);
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
            // ── LAYER 6: PAINEL DE CONVERSA EM FORMATO SMARTPHONE ESTILO WHATSAPP ──
            .when(chat_open, |parent| {
                parent.child(self.render_phone_drawer(cx))
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

