pub mod api;
pub mod audio;
pub mod recorder;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_vrm::{VrmController, VrmView};
use crate::api::UsamiApiClient;
use crate::audio::AudioPlayer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveDrawer {
    None,
    Chat,
    Bond,
    Settings,
    Vrma,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sender {
    User,
    Usami,
}

#[derive(Clone)]
struct ChatMessage {
    sender: Sender,
    text: String,
    emotion: Option<String>,
}

// Caminhos relativos à crate: o app roda de qualquer lugar do disco e continua
// achando ícones, fundo, VRM e VRMA. Variáveis abaixo permitem apontar outra
// instalação sem recompilar.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn asset_path(relative: &str) -> PathBuf {
    manifest_dir().join(relative)
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

fn first_existing(candidates: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|path| path.exists())
}

fn icon(name: &str, size_px: f32) -> impl IntoElement {
    img(asset_path(&format!("assets/icons/{name}.svg")))
        .size(px(size_px))
        .flex_shrink_0()
}

struct UsamiApp {
    focus_handle: FocusHandle,
    input_text: String,
    status_text: SharedString,
    last_reply: SharedString,
    last_emotion: SharedString,
    show_bubble: bool,
    is_busy: bool,
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
    active_drawer: ActiveDrawer,
    chat_history: Vec<ChatMessage>,
    bond_level: u32,
    bond_exp_pct: u32,
    bg_asset_path: Option<PathBuf>,
    api_client: Arc<UsamiApiClient>,
    audio_player: Option<Arc<AudioPlayer>>,
    vrm_controller: VrmController,
    vrm_view: Option<Entity<VrmView>>,
}

impl UsamiApp {
    fn new(cx: &mut Context<Self>) -> Self {
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
        cx.spawn(move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                loop {
                    gate.set_agent(speaking_controller.is_speaking());
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                }
            }
        })
        .detach();

        let vrm_view = vrm_path.and_then(|path| {
            match VrmView::new(&path, vrma_dir, vrm_controller.clone()) {
                Ok(v) => Some(cx.new(|_cx| v)),
                Err(e) => {
                    eprintln!("[gpui-vrm] Falha ao inicializar VrmView: {:?}", e);
                    None
                }
            }
        });

        let initial_history = vec![
            ChatMessage {
                sender: Sender::Usami,
                text: "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!".into(),
                emotion: Some("tsundere".into()),
            },
        ];

        // Loop em background para detectar inatividade e suavizar a UI (ui-idle)
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                loop {
                    cx.background_executor().timer(Duration::from_millis(500)).await;
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
                                    let remaining = 30_u32.saturating_sub((elapsed.as_secs() / 60) as u32);
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
                                let is_idle = !has_drawer && view.last_interaction.elapsed() > Duration::from_millis(4500);
                                if view.is_ui_idle != is_idle {
                                    view.is_ui_idle = is_idle;
                                    changed = true;
                                }
                                if changed {
                                    cx.notify();
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            input_text: String::new(),
            status_text: "COM VOCÊ".into(),
            last_reply: "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!".into(),
            last_emotion: "tsundere".into(),
            show_bubble: true,
            is_busy: false,
            is_calling: false,
            call_started_at: None,
            call_seconds: 0,
            is_listening: false,
            listener: None,
            mic_gate,
            is_companion_mode: false,
            sleep_companion_started_at: None,
            sleep_companion_minutes_left: 30,
            is_muted: false,
            is_ui_idle: false,
            last_interaction: Instant::now(),
            active_drawer: ActiveDrawer::None,
            chat_history: initial_history,
            bond_level: 2,
            bond_exp_pct: 74,
            bg_asset_path,
            api_client: Arc::new(UsamiApiClient::from_env()),
            audio_player: player,
            vrm_controller,
            vrm_view,
        }
    }

    fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_interaction = Instant::now();
        if self.is_ui_idle {
            self.is_ui_idle = false;
            cx.notify();
        }
    }

    fn start_call(&mut self, cx: &mut Context<Self>) {
        self.is_calling = true;
        self.call_started_at = Some(Instant::now());
        self.call_seconds = 0;
        self.status_text = "SUA VEZ · 00:00".into();
        self.start_listening(cx);
    }

    fn end_call(&mut self) {
        self.stop_listening();
        self.is_calling = false;
        self.call_started_at = None;
        self.call_seconds = 0;
        self.status_text = if self.is_companion_mode { "MODO COMPANION" } else { "COM VOCÊ" }.into();
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
        self.status_text = if self.is_calling { "SUA VEZ" } else { "COM VOCÊ" }.into();
        self.vrm_controller.set_emotion_with_options("tsundere", 4.0, false);
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
                    self.status_text = "MIC: canal de áudio indisponível.".into();
                    cx.notify();
                    return;
                };
                self.listener = Some(listener);
                self.is_listening = true;
                self.sync_mic_gate();
                self.status_text = "OUVINDO...".into();

                // Consome as falas detectadas pela VAD e manda para o STT. O loop
                // vive enquanto o listener existir; trocar de chamada recria tudo.
                let api = Arc::clone(&self.api_client);
                cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        let mut segments = segments;
                        while let Some(wav) = segments.recv().await {
                            let transcript = api.transcribe(wav).await;
                            let active = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || !view.is_calling {
                                                return false;
                                            }
                                            match transcript {
                                                Ok(text) if !text.trim().is_empty() => {
                                                    view.input_text = text.trim().to_string();
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
                self.sync_mic_gate();
                self.status_text = format!("MIC: {error}").into();
            }
        }
        cx.notify();
    }

    fn stop_listening(&mut self) {
        self.is_listening = false;
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

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.poke(cx);
        let text = self.input_text.trim().to_string();
        if text.is_empty() || self.is_busy {
            return;
        }

        self.input_text.clear();
        self.is_busy = true;
        self.sync_mic_gate();
        self.show_bubble = true;
        self.status_text = "PENSANDO".into();

        // Reação imediata de escuta e reflexão enquanto o LLM processa
        self.vrm_controller.play_vrma("thinking", false);
        self.vrm_controller.set_expression("19-prefab_doubt", 0.45);

        self.chat_history.push(ChatMessage {
            sender: Sender::User,
            text: text.clone(),
            emotion: None,
        });

        cx.notify();

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
                            view.sync_mic_gate();
                            view.poke(cx);
                            match res {
                                Ok(response) => {
                                    let emotion = response.gesture.clone().unwrap_or_else(|| "tsundere".into());
                                    view.last_reply = response.reply.clone().into();
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

                                    view.chat_history.push(ChatMessage {
                                        sender: Sender::Usami,
                                        text: response.reply.clone(),
                                        emotion: Some(emotion.clone()),
                                    });

                                    if view.bond_exp_pct < 100 {
                                        view.bond_exp_pct += 2;
                                    } else {
                                        view.bond_level += 1;
                                        view.bond_exp_pct = 5;
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

    fn toggle_drawer(&mut self, drawer: ActiveDrawer, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.active_drawer == drawer {
            self.active_drawer = ActiveDrawer::None;
        } else {
            self.active_drawer = drawer;
        }
        cx.notify();
    }
}

impl Render for UsamiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_busy = self.is_busy;
        let is_calling = self.is_calling;
        let is_companion = self.is_companion_mode;
        let is_muted = self.is_muted;
        let is_idle = self.is_ui_idle;
        let is_listening = self.is_listening;
        let active_drawer = self.active_drawer;
        let bond_level = self.bond_level;
        let show_bubble = self.show_bubble;
        let bg_asset = self.bg_asset_path.clone();
        let vrm_entity = self.vrm_view.clone();
        let status_label: SharedString = if is_calling {
            let state = if is_listening { "OUVINDO" } else { "MIC OFF" };
            format!("{state} · {:02}:{:02}", self.call_seconds / 60, self.call_seconds % 60).into()
        } else if is_busy {
            "PENSANDO".into()
        } else if is_companion {
            "MODO COMPANION".into()
        } else {
            "COM VOCÊ".into()
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x110714))
            .text_color(rgb(0xf5ebe4))
            .track_focus(&self.focus_handle)
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, _window, cx| {
                this.poke(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.poke(cx);
                let ks = &event.keystroke;
                let key = ks.key.as_str();

                // Atalhos com Control (ex: Ctrl+V para colar, Ctrl+Backspace / Ctrl+W para apagar palavra)
                if ks.modifiers.control && !ks.modifiers.alt {
                    if key.eq_ignore_ascii_case("v") {
                        if let Some(item) = cx.read_from_clipboard()
                            && let Some(text) = item.text()
                        {
                            this.input_text.push_str(&text);
                            cx.notify();
                        }
                    } else if key.eq_ignore_ascii_case("w") || key == "backspace" {
                        if let Some(idx) = this.input_text.rfind(' ') {
                            this.input_text.truncate(idx);
                        } else {
                            this.input_text.clear();
                        }
                        cx.notify();
                    } else if key.eq_ignore_ascii_case("u") {
                        this.input_text.clear();
                        cx.notify();
                    }
                    return;
                }

                // Ignora teclas com Alt isolado
                if ks.modifiers.alt && !ks.modifiers.control {
                    return;
                }

                if key == "enter" {
                    this.submit(cx);
                } else if key == "backspace" {
                    this.input_text.pop();
                    cx.notify();
                } else if key == "escape" {
                    this.active_drawer = ActiveDrawer::None;
                    cx.notify();
                } else if key == "space" || key == " " {
                    this.input_text.push(' ');
                    cx.notify();
                } else if let Some(ref ch) = ks.key_char {
                    if !ch.chars().any(|c| c.is_control()) {
                        this.input_text.push_str(ch);
                        cx.notify();
                    }
                } else if key.chars().count() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        this.input_text.push(c);
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
            // ── LAYER 1: SUTIL OVERLAY COMPANION / ESCURECIMENTO ──
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(if is_companion {
                        rgba(0x0e061299)
                    } else {
                        rgba(0x0e061230)
                    }),
            )
            // ── LAYER 2: SHIRO VRM 3D (RESPONSIVO FULLSCREEN, ANCORADA NO PISO, SEM CAIXA DE RECORTE) ──
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
            // ── LAYER 3: FLOATING SPEECH BUBBLE COM RABICHO ANIME (.bubble.her) ──
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(88.0))
                        .left(px(120.0))
                        .max_w(px(460.0))
                        .relative()
                        .px_5()
                        .py_4()
                        .rounded(px(24.0))
                        .rounded_br(px(10.0))
                        .bg(rgba(0x160b1af2))
                        .border_1()
                        .border_color(rgba(0xf5ebe425))
                        .shadow_xl()
                        .id("floating-bubble")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_bubble = false;
                            this.poke(cx);
                            cx.notify();
                        }))
                        // Conteúdo do cabeçalho do balão
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .mb_1p5()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xffa6c9))
                                                .child("USAMI"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0xd8b4e2))
                                                .child(format!("· {}", self.last_emotion)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgba(0xf5ebe450))
                                        .child("✕"),
                                ),
                        )
                        // Texto da resposta
                        .child(
                            div()
                                .text_sm()
                                .line_height(px(23.0))
                                .text_color(rgb(0xf9f9fb))
                                .child(self.last_reply.clone()),
                        )
                        // Rabicho de balão de fala apontando diretamente para o rosto da Usami
                        .child(
                            div()
                                .absolute()
                                .right(px(-20.0))
                                .bottom(px(12.0))
                                .child(
                                    img(asset_path("assets/icons/speech_tail.svg"))
                                        .w(px(22.0))
                                        .h(px(26.0)),
                                ),
                        ),
                )
            })
            // ── LAYER 4: HEADER SUPERIOR (TRANSLÚCIDO E MINIMALISTA, SOME NO IDLE) ──
            .when(!is_idle, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(20.0))
                        .left(px(24.0))
                        .right(px(24.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        // Top Left: 3 Botões Redondos em Vidro
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                // Botão Conversa
                                .child(
                                    div()
                                        .id("top-chat-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Chat { rgba(0x9a44f255) } else { rgba(0x1a0d1bc7) })
                                        .border_1()
                                        .border_color(rgba(0xf5ebe420))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Chat, cx);
                                        }))
                                        .child(icon("chat_bubble", 16.0)),
                                )
                                // Botão Configurações
                                .child(
                                    div()
                                        .id("top-settings-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Settings { rgba(0x9a44f255) } else { rgba(0x1a0d1bc7) })
                                        .border_1()
                                        .border_color(rgba(0xf5ebe420))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Settings, cx);
                                        }))
                                        .child(icon("gear", 16.0)),
                                )
                                // Botão Recentralizar
                                .child(
                                    div()
                                        .id("top-recenter-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(rgba(0x1a0d1bc7))
                                        .border_1()
                                        .border_color(rgba(0xf5ebe420))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.vrm_controller.reset_camera();
                                            this.vrm_controller.play_idle();
                                            cx.notify();
                                        }))
                                        .child(icon("recenter", 16.0)),
                                ),
                        )
                        // Top Center: Pill com LED Verde "● COM VOCÊ"
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .py_1p5()
                                .rounded_full()
                                .bg(rgba(0x160b17d1))
                                .border_1()
                                .border_color(rgba(0xf5ebe420))
                                .shadow_md()
                                .child(
                                    div()
                                        .size_2()
                                        .rounded_full()
                                        .bg(if is_calling {
                                            rgb(0xe5484d)
                                        } else if is_busy {
                                            rgb(0xe8c547)
                                        } else {
                                            rgb(0x43b581)
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xf5ebe4))
                                        .child(status_label),
                                ),
                        )
                        // Top Right: Clapperboard + Pill de Nível + Crédito
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                // Botão Testador VRMA
                                .child(
                                    div()
                                        .id("top-vrma-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Vrma { rgba(0x9a44f255) } else { rgba(0x1a0d1bc7) })
                                        .border_1()
                                        .border_color(rgba(0xf5ebe420))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Vrma, cx);
                                        }))
                                        .child(icon("clapperboard", 16.0)),
                                )
                                // Pill Roxo de Vínculo ♥ NÍVEL 2
                                .child(
                                    div()
                                        .id("top-bond-pill")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .px_3p5()
                                        .py_1p5()
                                        .rounded_full()
                                        .bg(rgba(0x9357cf73))
                                        .border_1()
                                        .border_color(rgba(0xd8b4e259))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xf5ebe4))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Bond, cx);
                                        }))
                                        .child(format!("♥ NÍVEL {}", bond_level)),
                                )
                                // Crédito autoral
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgba(0xf5ebe4a6))
                                        .child("白ー (shiro) — Model by Sweet_nya555"),
                                ),
                        ),
                )
            })
            // ── LAYER 5: CONTROLES INFERIORES & COMPOSER FLUTUANTE ──
            .child(
                div()
                    .absolute()
                    .bottom(px(20.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2p5()
                    // Disclaimer (some no idle)
                    .when(!is_idle, |parent| {
                        parent.child(
                            div()
                                .text_xs()
                                .text_color(rgba(0xf5ebe46b))
                                .child("Mensagens usam os provedores configurados para responder."),
                        )
                    })
                    // BARRA DE COMPOSER ARREDONDADA
                    .child(
                        div()
                            .id("composer-bar")
                            .w(px(640.0))
                            .max_w(px(640.0))
                            .flex()
                            .items_center()
                            .px_3()
                            .py_1p5()
                            .rounded_full()
                            // No idle, fica com opacidade reduzida como na print!
                            .bg(if is_idle {
                                rgba(0x160b192b) // Opacidade reduzida (~17%)
                            } else {
                                rgba(0x160b19b8) // Ativo translúcido (~72%)
                            })
                            .border_1()
                            .border_color(if is_idle {
                                rgba(0xf5ebe412)
                            } else {
                                rgba(0xf5ebe438)
                            })
                            .shadow_lg()
                            .on_mouse_move(cx.listener(|this, _, _, cx| {
                                this.poke(cx);
                            }))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.poke(cx);
                                window.focus(&this.focus_handle);
                            }))
                            // Ícone Imagem / Anexo
                            .child(
                                div()
                                    .id("composer-img-btn")
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(36.0))
                                    .rounded_full()
                                    .text_color(rgba(0xf5ebe4a6))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.poke(cx);
                                    }))
                                    .child(icon("image", 18.0)),
                            )
                            // Campo de Texto
                            .child(
                                div()
                                    .id("input-box")
                                    .flex_1()
                                    .px_2()
                                    .py_2()
                                    .cursor_text()
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.poke(cx);
                                        window.focus(&this.focus_handle);
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .child(
                                                if self.input_text.is_empty() {
                                                    div()
                                                        .text_sm()
                                                        .text_color(rgba(0xf5ebe46b))
                                                        .child("Diga alguma coisa...")
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(rgb(0xf5ebe4))
                                                                .child(self.input_text.clone()),
                                                        )
                                                        // Cursor vertical suave indicando foco ativo após o texto digitado
                                                        .child(
                                                            div()
                                                                .w(px(1.5))
                                                                .h(px(15.0))
                                                                .ml(px(1.0))
                                                                .bg(rgb(0xffa6c9)),
                                                        )
                                                        .into_any_element()
                                                }
                                            ),
                                    ),
                            )
                            // No modo texto a barra mantém os mesmos quatro controles da web.
                            .when(!is_calling, |parent| parent.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    // Botão Fone / Chamada
                                    .child(
                                        div()
                                            .id("composer-call-btn")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(36.0))
                                            .rounded_full()
                                            .bg(rgba(0xffffff0d))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.poke(cx);
                                                this.start_call(cx);
                                                cx.notify();
                                            }))
                                            .child(icon("phone", 16.0)),
                                    )
                                    // Botão Volume / Mudo
                                    .child(
                                        div()
                                            .id("composer-vol-btn")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(36.0))
                                            .rounded_full()
                                            .bg(if is_muted { rgba(0xe5484d55) } else { rgba(0xffffff0d) })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.poke(cx);
                                                this.is_muted = !this.is_muted;
                                                this.sync_mic_gate();
                                                cx.notify();
                                            }))
                                            .child(icon("volume", 16.0)),
                                    )
                                    // Botão Lua / Companion
                                    .child(
                                        div()
                                            .id("composer-moon-btn")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(36.0))
                                            .rounded_full()
                                            .bg(if is_companion { rgba(0x9a44f266) } else { rgba(0xffffff0d) })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.poke(cx);
                                                this.toggle_sleep_companion();
                                                cx.notify();
                                            }))
                                            .child(icon("moon", 16.0)),
                                    )
                                    // Botão Enviar
                                    .child(
                                        div()
                                            .id("composer-send-btn")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(36.0))
                                            .rounded_full()
                                            .bg(if is_busy { rgba(0xffffff15) } else { rgba(0x9a44f2aa) })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.submit(cx);
                                            }))
                                            .child(icon("send", 16.0)),
                                    ),
                            ))
                    )
                    // Em chamada a web move os controles de voz para o dock abaixo do compositor.
                    .when(is_calling, |parent| parent.child(
                        div()
                            .id("call-dock")
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                div().id("call-mic").cursor_pointer().size(px(48.0)).rounded_full().flex().items_center().justify_center()
                                    .bg(if self.is_listening { rgba(0xe5484dcc) } else { rgba(0x202329d9) })
                                    .border_1()
                                    .border_color(if self.is_listening { rgba(0xffa6c9) } else { rgba(0xf5ebe420) })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.toggle_listening(cx);
                                    }))
                                    .child(icon("pulse", 19.0)),
                            )
                            .child(
                                div().id("call-chat").cursor_pointer().size(px(48.0)).rounded_full().flex().items_center().justify_center()
                                    .bg(rgba(0x202329d9)).border_1().border_color(rgba(0xf5ebe420))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.toggle_drawer(ActiveDrawer::Chat, cx); }))
                                    .child(icon("chat_bubble", 19.0)),
                            )
                            .child(
                                div().id("call-hangup").cursor_pointer().size(px(48.0)).rounded_full().flex().items_center().justify_center()
                                    .bg(rgb(0xe5484d))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.poke(cx); this.end_call(); cx.notify(); }))
                                    .child(icon("phone", 19.0)),
                            )
                            .child(
                                div().id("call-volume").cursor_pointer().size(px(48.0)).rounded_full().flex().items_center().justify_center()
                                    .bg(if is_muted { rgba(0xe5484d55) } else { rgba(0x202329d9) })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.poke(cx); this.is_muted = !this.is_muted; this.sync_mic_gate(); cx.notify(); }))
                                    .child(icon("volume", 19.0)),
                            )
                            .child(
                                div().id("call-sleep").cursor_pointer().size(px(48.0)).rounded_full().flex().items_center().justify_center()
                                    .bg(if is_companion { rgba(0x9a44f266) } else { rgba(0x202329d9) })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.poke(cx); this.toggle_sleep_companion(); cx.notify(); }))
                                    .child(icon("moon", 19.0)),
                            ),
                    ))
                    // Botão Pílula: 💬 MOSTRAR CONVERSA (some no idle)
                    .when(!is_idle, |parent| {
                        parent.child(
                            div()
                                .id("bottom-show-chat-btn")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .py_1p5()
                                .rounded_full()
                                .bg(rgba(0x1a0d1bd1))
                                .border_1()
                                .border_color(rgba(0xf5ebe420))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xf5ebe4))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.toggle_drawer(ActiveDrawer::Chat, cx);
                                }))
                                .child(icon("chat_bubble", 14.0))
                                .child("MOSTRAR CONVERSA"),
                        )
                    }),
            )
            // ── LAYER 6: DRAWERS LATERAIS ──
            .when(active_drawer != ActiveDrawer::None, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(380.0))
                        .flex()
                        .flex_col()
                        .border_l_1()
                        .border_color(rgba(0xf5ebe420))
                        .bg(rgba(0x160b1af5))
                        .p_5()
                        .gap_4()
                        .shadow_lg()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb_3()
                                .border_b_1()
                                .border_color(rgba(0xf5ebe415))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xd8b4e2))
                                        .child(match active_drawer {
                                            ActiveDrawer::Chat => "HISTÓRICO DA CONVERSA",
                                            ActiveDrawer::Bond => "VÍNCULO COM A USAMI",
                                            ActiveDrawer::Settings => "CONFIGURAÇÕES & MODELO",
                                            ActiveDrawer::Vrma => "🎬 ANIMAÇÕES VRMA",
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
                                        .size(px(30.0))
                                        .rounded_lg()
                                        .bg(rgba(0xffffff10))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::None;
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("close", 14.0)),
                                ),
                        )
                        .child(
                            match active_drawer {
                                ActiveDrawer::Chat => {
                                    let turns = self.chat_history.clone();
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .gap_3()
                                                .overflow_hidden()
                                                .children(turns.into_iter().map(|msg| {
                                                    let is_user = msg.sender == Sender::User;
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .items_start()
                                                        .p_3()
                                                        .rounded_xl()
                                                        .bg(if is_user { rgba(0x521e7399) } else { rgba(0x28162de6) })
                                                        .border_1()
                                                        .border_color(rgba(0xf5ebe415))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_2()
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::SEMIBOLD)
                                                                        .text_color(if is_user { rgb(0xd8b4e2) } else { rgb(0xc8a8e0) })
                                                                        .child(if is_user { "Você" } else { "Usami" }),
                                                                )
                                                                .when(!is_user && msg.emotion.is_some(), |p| {
                                                                    p.child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(rgb(0xf7a8d2))
                                                                            .child(format!("· {}", msg.emotion.as_deref().unwrap_or("tsundere"))),
                                                                    )
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .mt_1()
                                                                .text_sm()
                                                                .text_color(rgb(0xf5ebe4))
                                                                .child(msg.text),
                                                        )
                                                })),
                                        )
                                }
                                ActiveDrawer::Bond => {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .child(
                                            div()
                                                .p_5()
                                                .rounded_xl()
                                                .bg(rgba(0x2d153be6))
                                                .border_1()
                                                .border_color(rgba(0xd8b4e255))
                                                .child(
                                                    div()
                                                        .text_base()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(rgb(0xffd1ec))
                                                        .child(format!("Nível {} — Amiga Próxima", bond_level)),
                                                )
                                                .child(
                                                    div()
                                                        .mt_3()
                                                        .w_full()
                                                        .h(px(8.0))
                                                        .rounded_full()
                                                        .bg(rgb(0x190b21))
                                                        .child(
                                                            div()
                                                                .h_full()
                                                                .w(px(2.6 * self.bond_exp_pct as f32))
                                                                .rounded_full()
                                                                .bg(rgb(0xf27ab9)),
                                                        ),
                                                ),
                                        )
                                }
                                ActiveDrawer::Vrma => {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xd8b4e2))
                                                .child("TESTES DE ANIMAÇÃO"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id("vrma-wave")
                                                        .cursor_pointer()
                                                        .px_3()
                                                        .py_2()
                                                        .rounded_lg()
                                                        .bg(rgba(0xffffff10))
                                                        .text_xs()
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.vrm_controller.play_vrma("goodbye", false);
                                                            cx.notify();
                                                        }))
                                                        .child("👋 Acenar"),
                                                )
                                                .child(
                                                    div()
                                                        .id("vrma-jump")
                                                        .cursor_pointer()
                                                        .px_3()
                                                        .py_2()
                                                        .rounded_lg()
                                                        .bg(rgba(0xffffff10))
                                                        .text_xs()
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.vrm_controller.play_vrma("jump", false);
                                                            cx.notify();
                                                        }))
                                                        .child("🎉 Pular"),
                                                )
                                                .child(
                                                    div()
                                                        .id("vrma-idle")
                                                        .cursor_pointer()
                                                        .px_3()
                                                        .py_2()
                                                        .rounded_lg()
                                                        .bg(rgba(0xffffff10))
                                                        .text_xs()
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.vrm_controller.play_idle();
                                                            cx.notify();
                                                        }))
                                                        .child("🔄 Idle Natural"),
                                                ),
                                        )
                                }
                                ActiveDrawer::Settings => {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(
                                            div()
                                                .p_4()
                                                .rounded_xl()
                                                .bg(rgba(0xffffff08))
                                                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(0xd8b4e2)).child("STATUS DO MOTOR"))
                                                .child(div().mt_2().text_xs().text_color(rgb(0xf5ebe4)).child("• Renderizador: WGPU 60 FPS (Nativo)"))
                                                .child(div().mt_1().text_xs().text_color(rgb(0xf5ebe4)).child("• Modelo: 白ー (shiro) VRM 1.0"))
                                                .child(div().mt_1().text_xs().text_color(rgb(0xf5ebe4)).child(SharedString::from(format!("• Backend: {}", self.api_client.base_url())))),
                                        )
                                }
                                ActiveDrawer::None => div(),
                            }
                        ),
                )
            })
    }
}

fn main() {
    println!("[Usami Desktop] Inicializando interface minimalista idêntica à print...");
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(780.0)), cx);
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
                let view = cx.new(UsamiApp::new);
                window.focus(&view.read(cx).focus_handle);
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
    use super::{asset_path, first_existing, manifest_dir};

    #[test]
    fn icon_paths_resolve_from_crate_dir() {
        let path = asset_path("assets/icons/chat.svg");
        assert!(path.is_absolute());
        assert!(path.starts_with(manifest_dir()));
        assert!(path.exists(), "ícone esperado em {}", path.display());
    }

    #[test]
    fn first_existing_skips_missing_candidates() {
        let dir = std::env::temp_dir().join(format!(
            "usami-desktop-path-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("cria diretório temporário");
        let missing = dir.join("missing.png");
        let present = dir.join("present.png");
        std::fs::write(&present, b"x").expect("cria arquivo de teste");

        assert_eq!(
            first_existing([Some(missing.clone()), Some(present.clone())]),
            Some(present)
        );
        assert_eq!(first_existing([Some(missing), None]), None);

        std::fs::remove_dir_all(&dir).ok();
    }
}
