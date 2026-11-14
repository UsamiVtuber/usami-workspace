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

use crate::agent_center_model::{AgentCenterTab, ExecFilter, SkillFilter};
use crate::assets::{asset_base_dir, asset_path, env_path, first_existing};
use crate::api::UsamiApiClient;
use crate::audio::AudioPlayer;
use crate::media::AttachedMedia;
use crate::model::{chat_message_from_stored, current_phone_time, format_tool_badge, stored_turns, ActiveDrawer, BondTab, ChatMessage, Sender};
use crate::ui::widgets::icon;
use crate::speech::sanitize_speech_text;
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
    phone_view_settings: bool,
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
            phone_view_settings: false,
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
        let viewport_h = window.viewport_size().height;
        let is_idle = self.is_ui_idle;
        let active_drawer = self.active_drawer;
        let chat_open = active_drawer == ActiveDrawer::Chat;
        let side_drawer_open =
            matches!(active_drawer, ActiveDrawer::Vrma | ActiveDrawer::Skills) && !chat_open;

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
            // ── LAYER 1..4: CENÁRIO & VRM ──
            .child(self.render_stage(t))
            // ── LAYER TOP: HUD STATUS, MODO & AÇÕES ──
            .child(self.render_hud_top(cx))
            // ── LAYER 5: CONTROLES INFERIORES & COMPOSER FLUTUANTE ──
            .child(self.render_bottom_controls(is_idle, cx))
            // ── LAYER 6: PAINEL DE CONVERSA EM FORMATO SMARTPHONE ESTILO WHATSAPP ──
            .when(chat_open, |parent| {
                parent.child(self.render_phone_drawer(cx))
            })
            // ── LAYER 7: PAINÉIS LATERAIS (VÍNCULO / CONFIGURAÇÕES / VRMA) ──
            .when(side_drawer_open, |parent| {
                parent.child(self.render_side_drawer(viewport_h, cx))
            })
            // ── LAYER 8: CENTRAL DE RELACIONAMENTO & DIÁRIO ESTILO VISUAL NOVEL ──
            .when(active_drawer == ActiveDrawer::Bond, |parent| {
                parent.child(self.render_vn_bond_overlay(cx))
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

