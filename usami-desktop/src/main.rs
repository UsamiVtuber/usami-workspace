pub mod api;
pub mod audio;
pub mod recorder;
mod theme;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
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

/// Abas do painel de configurações (mesma ordem do web).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Chat,
    Voice,
    Avatar,
    Memory,
    Personality,
    About,
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

/// Pasta empacotada (release): `assets/` ao lado do executável. Em
/// desenvolvimento não existe, então cai para a pasta da crate.
fn asset_base_dir() -> &'static Path {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
            && dir.join("assets").is_dir()
        {
            return dir.to_path_buf();
        }
        manifest_dir()
    })
}

fn asset_path(relative: &str) -> PathBuf {
    asset_base_dir().join(relative)
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

// ── Peças do painel de configurações (mesmos estilos de `h3`, `p`, `.settings-hint`,
// `code`, `.rig-actions` e `.upload` do web) ──

fn settings_h3(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s6))
        .mb(px(t.space.s2))
        .text_size(px(t.text.h3))
        .font_family(theme::FONT_DISPLAY)
        .font_weight(FontWeight::BOLD)
        .text_color(t.color.primary)
        .child(label)
}

fn settings_p(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1_5))
        .mb(px(t.space.s3))
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.muted)
        .child(text.into())
}

fn settings_hint(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1))
        .text_size(px(t.text.xs))
        .line_height(px(16.5))
        .text_color(t.color.muted)
        .child(text.into())
}

fn settings_code(text: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .px(px(t.space.s1_5))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(t.color.surface_soft)
        .font_family("Consolas")
        .text_size(px(t.text.xs))
        .text_color(t.color.pearl)
        .child(text)
}

fn settings_disabled_button(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .px(px(15.0))
        .py(px(9.0))
        .rounded(px(t.radius.sm))
        .bg(t.color.card_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.sm))
        .text_color(t.color.pearl)
        .opacity(0.4)
        .child(label)
}

fn settings_danger_button(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .px(px(15.0))
        .py(px(9.0))
        .rounded(px(t.radius.sm))
        .bg(t.color.danger_muted_bg)
        .border_1()
        .border_color(t.color.danger_muted_border)
        .text_size(px(t.text.sm))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.color.danger_muted)
        .opacity(0.4)
        .child(label)
}

fn settings_disabled_input(placeholder: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .h(px(38.0))
        .w_full()
        .px(px(t.space.s2_5))
        .flex()
        .items_center()
        .rounded(px(8.0))
        .bg(t.color.icon_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.sm))
        .text_color(t.color.faint)
        .opacity(0.55)
        .child(placeholder)
}

fn settings_dl_row(key: &'static str, value: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .justify_between()
        .items_center()
        .py(px(9.0))
        .border_b_1()
        .border_color(t.color.line)
        .text_size(px(t.text.sm))
        .child(div().text_color(t.color.muted).child(key))
        .child(div().text_color(t.color.pearl).child(value.into()))
}

fn settings_disabled_slider() -> impl IntoElement {
    let t = theme::theme();
    div()
        .h(px(16.0))
        .w_full()
        .my(px(t.space.s2_5))
        .flex()
        .items_center()
        .opacity(0.4)
        .child(
            div()
                .h(px(6.0))
                .w_full()
                .rounded_full()
                .bg(t.color.surface_soft)
                .border_1()
                .border_color(t.color.line_strong),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .size(px(14.0))
                .rounded_full()
                .bg(t.color.primary),
        )
}

// `.trait-row` do web: rótulo à esquerda, valor em destaque à direita e o
// slider logo abaixo. O trilho repete `settings_disabled_slider` sem as margens
// verticais, que aqui vêm do `gap` da linha.
fn settings_trait_row(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .text_size(px(t.text.md))
                .text_color(t.color.pearl)
                .child(label)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.color.primary)
                        .child("50"),
                ),
        )
        .child(
            div()
                .h(px(16.0))
                .w_full()
                .flex()
                .items_center()
                .opacity(0.4)
                .child(
                    div()
                        .h(px(6.0))
                        .w_full()
                        .rounded_full()
                        .bg(t.color.surface_soft)
                        .border_1()
                        .border_color(t.color.line_strong),
                )
                .child(
                    // Traço neutro: o valor de referência é 50, então o pino
                    // fica no centro do trilho.
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(px(14.0))
                                .rounded_full()
                                .bg(t.color.primary),
                        ),
                ),
        )
}

fn settings_disabled_textarea() -> impl IntoElement {
    let t = theme::theme();
    div()
        .min_h(px(88.0))
        .w_full()
        .p(px(t.space.s2_5))
        .rounded(px(10.0))
        .bg(t.color.icon_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.faint)
        .opacity(0.55)
        .child("Ex.: me chame de capitão; fale mais de coelhos.")
}

// Item de lista do web (`aside ul`): marcador e texto na cor do painel.
fn settings_list_item(text: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .items_start()
        .gap(px(t.space.s2))
        .mb(px(t.space.s1))
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.pearl)
        .child("•")
        .child(div().flex_1().child(text))
}

struct UsamiApp {
    focus_handle: FocusHandle,
    input_text: String,
    input_active: bool,
    caret_on: bool,
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
    settings_tab: SettingsTab,
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
                                // Caret pisca enquanto o composer está ativo (o mesmo
                                // timer já redesenha a UI, então não há custo extra).
                                if view.input_active {
                                    view.caret_on = !view.caret_on;
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
            input_active: false,
            caret_on: true,
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
            settings_tab: SettingsTab::Chat,
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
        self.input_active = false;
        self.is_busy = true;
        self.sync_mic_gate();
        self.show_bubble = true;
        self.status_text = "PENSANDO".into();

        // Reação imediata de escuta e reflexão enquanto o LLM processa
        self.vrm_controller.play_vrma("thinking", false);
        // Limpa as expressões do turno anterior: sem isso elas se acumulam na cabeça.
        self.vrm_controller.clear_expressions();
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
        let is_calling = self.is_calling;
        let is_companion = self.is_companion_mode;
        let is_muted = self.is_muted;
        let is_idle = self.is_ui_idle;
        let is_listening = self.is_listening;
        let active_drawer = self.active_drawer;
        let settings_tab = self.settings_tab;
        let chat_open = active_drawer == ActiveDrawer::Chat;
        let side_drawer_open = matches!(
            active_drawer,
            ActiveDrawer::Settings | ActiveDrawer::Vrma
        ) && !chat_open;
        let bond_level = self.bond_level;
        let bond_exp_pct = self.bond_exp_pct;
        let show_bubble = self.show_bubble;
        let bg_asset = self.bg_asset_path.clone();
        let vrm_entity = self.vrm_view.clone();
        let last_reply = self.last_reply.clone();
        let last_emotion = self.last_emotion.clone();
        let input_text = self.input_text.clone();
        let input_active = self.input_active;
        let caret_on = self.caret_on;
        let caret_visible = input_active && caret_on;
        let send_enabled = !is_busy && !input_text.trim().is_empty();
        let turns = self.chat_history.clone();
        let no_turns = turns.is_empty();

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
        let status_color = if is_calling {
            c.err
        } else if is_busy {
            c.warn
        } else {
            c.ok
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
                            this.input_active = true;
                            this.input_text.push_str(&text);
                            cx.notify();
                        }
                    } else if key.eq_ignore_ascii_case("w") || key == "backspace" {
                        this.input_active = true;
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
                    this.input_active = true;
                    this.input_text.pop();
                    cx.notify();
                } else if key == "escape" {
                    this.input_active = false;
                    this.active_drawer = ActiveDrawer::None;
                    cx.notify();
                } else if key == "space" || key == " " {
                    this.input_active = true;
                    this.input_text.push(' ');
                    cx.notify();
                } else if let Some(ref ch) = ks.key_char {
                    if !ch.chars().any(|c| c.is_control()) {
                        this.input_active = true;
                        this.input_text.push_str(ch);
                        cx.notify();
                    }
                } else if key.chars().count() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        this.input_active = true;
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
            // ── LAYER 3: FLOATING SPEECH BUBBLE (.bubble.her) ──
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(135.0))
                        .left(px(140.0))
                        .max_w(px(440.0))
                        .relative()
                        .pt(px(14.0))
                        .pb(px(16.0))
                        .px(px(sp.s5))
                        .rounded_tl(px(r.xl))
                        .rounded_tr(px(r.xl))
                        .rounded_br(px(r.xl))
                        .rounded_bl(px(sp.s1))
                        .bg(c.bubble_bg)
                        .border_1()
                        .border_color(c.line_strong)
                        .shadow(t.shadow.bubble.clone())
                        .id("floating-bubble")
                        .cursor_pointer()
                        .hover(|style| style.border_color(c.line_glow))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_bubble = false;
                            this.poke(cx);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .mb(px(sp.s1_5))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(7.0))
                                        .child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .font_family(theme::FONT_DISPLAY)
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(c.primary)
                                                .child("USAMI"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .font_family(theme::FONT_DISPLAY)
                                                .text_color(c.lavender)
                                                .child(format!("· {last_emotion}")),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.faint)
                                        .child("✕"),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(ty.bubble))
                                .font_family(theme::FONT_CHARACTER)
                                .italic()
                                .line_height(px(24.0))
                                .text_color(c.pearl)
                                .child(last_reply),
                        )
                        // Rabicho embaixo à esquerda (.bubble.her::after), na cor do corpo.
                        .child(
                            div()
                                .absolute()
                                .left(px(20.0))
                                .bottom(px(-11.0))
                                .child(
                                    img(asset_path("assets/icons/speech_tail.svg"))
                                        .w(px(20.0))
                                        .h(px(11.0)),
                                ),
                        ),
                )
            })
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
                                        .bg(if active_drawer == ActiveDrawer::Settings {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Settings {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Settings, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Settings, cx);
                                        }))
                                        .child(icon("gear", 16.0)),
                                )
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
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child("白ー (shiro) — Model by Sweet_nya555"),
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
                    .opacity(if is_idle { 0.15 } else { 1.0 })
                    // Disclaimer (sempre presente; o idle esmaece junto, como na web)
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.faint)
                            .child("Mensagens usam os provedores configurados para responder."),
                    )
                    // Compositor
                    .child(
                        div()
                            .id("composer-bar")
                            .w(px(640.0))
                            .max_w(px(640.0))
                            .flex()
                            .items_center()
                            .gap(px(sp.s1_5))
                            .pt(px(sp.s1_5))
                            .pb(px(sp.s1_5))
                            .pl(px(sp.s3))
                            .pr(px(sp.s2))
                            .rounded(px(r.composer))
                            .bg(c.composer_bg)
                            .border_1()
                            .border_color(if input_active { c.primary } else { c.line_strong })
                            .shadow(if input_active {
                                t.shadow.composer_focus.clone()
                            } else {
                                t.shadow.composer.clone()
                            })
                            .on_mouse_move(cx.listener(|this, _, _, cx| {
                                this.poke(cx);
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
                                    .text_color(c.muted)
                                    .hover(|style| style.bg(c.call_bg_hover).text_color(c.pearl))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.poke(cx);
                                    }))
                                    .child(icon("image", 18.0)),
                            )
                            // Campo de texto
                            .child(
                                div()
                                    .id("input-box")
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .px(px(sp.s1_5))
                                    .py(px(sp.s2))
                                    .cursor_text()
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
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .items_center()
                                            .when(
                                                input_text.is_empty(),
                                                |row| {
                                                    row.child(
                                                        div()
                                                            .text_size(px(ty.input))
                                                            .text_color(c.faint)
                                                            .truncate()
                                                            .child(if is_calling {
                                                                "Escreva enquanto fala..."
                                                            } else {
                                                                "Diga alguma coisa..."
                                                            }),
                                                    )
                                                },
                                            )
                                            .when(!input_text.is_empty(), |row| {
                                                row.child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.0))
                                                        .text_size(px(ty.input))
                                                        .text_color(c.pearl)
                                                        .line_clamp(1)
                                                        .text_ellipsis()
                                                        .child(input_text.clone()),
                                                )
                                            })
                                            .when(caret_visible, |row| {
                                                row.child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .w(px(1.5))
                                                        .h(px(16.0))
                                                        .ml(px(1.0))
                                                        .bg(c.primary),
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
                                                    this.poke(cx);
                                                    this.is_muted = !this.is_muted;
                                                    this.sync_mic_gate();
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
                                            this.end_call();
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
                                            this.poke(cx);
                                            this.is_muted = !this.is_muted;
                                            this.sync_mic_gate();
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
            // ── LAYER 6: HISTÓRICO DA CONVERSA (PAINEL FLUTUANTE DA WEB) ──
            .when(chat_open, |parent| {
                parent.child(
                    div()
                        .id("chat-history")
                        .absolute()
                        .left(px(sp.s6))
                        .bottom(px(84.0))
                        .w(px(390.0))
                        .max_h(px(483.0))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(r.xl))
                        .bg(c.chat_bg)
                        .border_1()
                        .border_color(c.line_strong)
                        .shadow(t.shadow.panel.clone())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pt(px(sp.s3))
                                .pr(px(sp.s3_5))
                                .pb(px(sp.s2_5))
                                .pl(px(sp.s4))
                                .border_b_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(7.0))
                                        .text_size(px(11.5))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.lavender)
                                        .child(icon("chat_bubble", 14.0))
                                        .child("CONVERSA"),
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
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_col()
                                .justify_end()
                                .gap(px(sp.s2_5))
                                .p(px(sp.s3_5))
                                .overflow_hidden()
                                .when(no_turns, |col| {
                                    col.justify_center().child(
                                        div()
                                            .py(px(22.0))
                                            .px(px(sp.s3_5))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .child(
                                                div()
                                                    .mb(px(sp.s1_5))
                                                    .text_size(px(ty.empty_title))
                                                    .font_family(theme::FONT_CHARACTER)
                                                    .italic()
                                                    .text_color(c.pearl)
                                                    .child("Comece do seu jeito."),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(ty.sm))
                                                    .line_height(px(18.0))
                                                    .text_color(c.muted)
                                                    .child("Escreva abaixo ou inicie uma chamada com a Usami."),
                                            ),
                                    )
                                })
                                .children(turns.into_iter().map(|msg| {
                                    let is_user = msg.sender == Sender::User;
                                    let bubble = div()
                                        .flex()
                                        .flex_col()
                                        .max_w(px(343.0))
                                        .when(is_user, |el| el.items_end())
                                        .when(!is_user, |el| el.items_start())
                                        .child(
                                            div()
                                                .mx(px(7.0))
                                                .mb(px(sp.s1))
                                                .text_size(px(ty.xs))
                                                .font_family(theme::FONT_DISPLAY)
                                                .text_color(if is_user { c.primary } else { c.muted })
                                                .child(if is_user {
                                                    SharedString::from("VOCÊ")
                                                } else if let Some(emotion) = msg.emotion.as_deref() {
                                                    SharedString::from(format!("USAMI · {emotion}"))
                                                } else {
                                                    SharedString::from("USAMI")
                                                }),
                                        )
                                        .child(
                                            div()
                                                .py(px(sp.s2_5))
                                                .px(px(sp.s3_5))
                                                .rounded(px(r.lg))
                                                .bg(c.chat_turn_bg)
                                                .border_1()
                                                .border_color(if is_user { c.chat_user_border } else { c.line })
                                                .text_size(px(ty.md))
                                                .line_height(px(19.0))
                                                .text_color(c.pearl)
                                                .child(msg.text),
                                        );
                                    div()
                                        .flex()
                                        .w_full()
                                        .when(is_user, |el| el.justify_end())
                                        .when(!is_user, |el| el.justify_start())
                                        .child(bubble)
                                }))
                                .when(is_busy, |col| {
                                    col.child(
                                        div()
                                            .flex()
                                            .w_full()
                                            .justify_start()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(5.0))
                                                    .py(px(sp.s2_5))
                                                    .px(px(sp.s3_5))
                                                    .rounded(px(r.lg))
                                                    .bg(c.chat_turn_bg)
                                                    .border_1()
                                                    .border_color(c.line)
                                                    .children((0..3usize).map(|idx| {
                                                        div()
                                                            .size(px(5.0))
                                                            .rounded_full()
                                                            .bg(c.lavender)
                                                            .with_animation(
                                                                ("typing", idx),
                                                                Animation::new(Duration::from_millis(1200)).repeat(),
                                                                move |el, delta| {
                                                                    let phase = (delta + idx as f32 * 0.15) % 1.0;
                                                                    let lift = if phase < 0.3 {
                                                                        phase / 0.3 * 3.0
                                                                    } else if phase < 0.6 {
                                                                        (1.0 - (phase - 0.3) / 0.3) * 3.0
                                                                    } else {
                                                                        0.0
                                                                    };
                                                                    let opacity = if phase < 0.6 {
                                                                        0.35 + phase / 0.6 * 0.65
                                                                    } else {
                                                                        1.0 - (phase - 0.6) / 0.4 * 0.65
                                                                    };
                                                                    el.opacity(opacity).mt(px(-lift))
                                                                },
                                                            )
                                                    })),
                                            ),
                                    )
                                }),
                        ),
                )
            })
            // ── LAYER 7: PAINÉIS LATERAIS (VÍNCULO / CONFIGURAÇÕES / VRMA) ──
            .when(side_drawer_open, |parent| {
                parent
                    .when(active_drawer == ActiveDrawer::Settings, |parent| {
                        parent.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .bg(c.scrim),
                        )
                    })
                    .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(420.0))
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
                                                    .child("👋 Acenar"),
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
                                                    .child("🎉 Pular"),
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
                                                    .child("🔄 Idle Natural"),
                                            ),
                                    )
                            }
                            ActiveDrawer::Settings => {
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
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .font_family(theme::FONT_DISPLAY)
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(c.primary)
                                                            .child("CONFIGURAÇÕES"),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s1_5))
                                                            .text_size(px(ty.h2))
                                                            .font_family(theme::FONT_CHARACTER)
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(c.pearl)
                                                            .child("Usami"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id("settings-close-btn")
                                                    .cursor_pointer()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .size(px(34.0))
                                                    .rounded_full()
                                                    .bg(c.surface_soft)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .text_color(c.pearl)
                                                    .hover(|style| style.bg(c.surface_hover))
                                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                        this.active_drawer = ActiveDrawer::None;
                                                        this.poke(cx);
                                                        cx.notify();
                                                    }))
                                                    .child(icon("close", 16.0)),
                                            ),
                                    )
                                    // Barra de abas (`.panel-tabs` do web). No web ela rola na
                                    // horizontal; aqui as seis precisam ficar alcançáveis, então
                                    // quebram em duas linhas com o mesmo visual.
                                    .child(
                                        div()
                                            .id("settings-tabs")
                                            .flex()
                                            .flex_wrap()
                                            .gap(px(6.0))
                                            .pb(px(sp.s2_5))
                                            .border_b_1()
                                            .border_color(c.line)
                                            .children(
                                                [
                                                    (SettingsTab::Chat, "CONVERSA", "settings-tab-chat"),
                                                    (SettingsTab::Voice, "VOZ", "settings-tab-voice"),
                                                    (SettingsTab::Avatar, "AVATAR", "settings-tab-avatar"),
                                                    (SettingsTab::Memory, "MEMÓRIA", "settings-tab-memory"),
                                                    (SettingsTab::Personality, "JEITO", "settings-tab-personality"),
                                                    (SettingsTab::About, "SOBRE", "settings-tab-about"),
                                                ]
                                                .into_iter()
                                                .map(|(tab, label, id)| {
                                                    let selected = settings_tab == tab;
                                                    div()
                                                        .id(id)
                                                        .flex_shrink_0()
                                                        .cursor_pointer()
                                                        .px(px(sp.s3_5))
                                                        .py(px(sp.s2))
                                                        .rounded(px(r.sm))
                                                        .border_1()
                                                        .border_color(if selected {
                                                            c.primary
                                                        } else {
                                                            rgba(0x00000000)
                                                        })
                                                        .bg(if selected {
                                                            c.icon_bg_active
                                                        } else {
                                                            rgba(0x00000000)
                                                        })
                                                        .when(selected, |el| {
                                                            el.shadow(theme::glow(rgba(0xffa6c94d), 12.0))
                                                        })
                                                        .text_size(px(ty.tab))
                                                        .font_family(theme::FONT_DISPLAY)
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(if selected { c.pearl } else { c.muted })
                                                        .hover(move |style| {
                                                            if selected {
                                                                style
                                                            } else {
                                                                style
                                                                    .bg(c.surface_soft)
                                                                    .border_color(c.line)
                                                                    .text_color(c.pearl)
                                                            }
                                                        })
                                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                            this.settings_tab = tab;
                                                            cx.notify();
                                                        }))
                                                        .child(label)
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("settings-content")
                                            .flex_1()
                                            .min_h(px(0.0))
                                            .flex()
                                            .flex_col()
                                            .overflow_y_scroll()
                                            .child(match settings_tab {
                                                SettingsTab::Chat => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .p(px(sp.s4))
                                                            .rounded(px(r.md))
                                                            .bg(c.card_bg)
                                                            .child(
                                                                div()
                                                                    .text_size(px(ty.xs))
                                                                    .font_family(theme::FONT_DISPLAY)
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(c.primary)
                                                                    .child("STATUS DO MOTOR"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .mt(px(sp.s2))
                                                                    .text_size(px(ty.sm))
                                                                    .text_color(c.pearl)
                                                                    .child("• Renderizador: WGPU 60 FPS (Nativo)"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .mt(px(sp.s1))
                                                                    .text_size(px(ty.sm))
                                                                    .text_color(c.pearl)
                                                                    .child("• Modelo: 白ー (shiro) VRM 1.0"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .mt(px(sp.s1))
                                                                    .text_size(px(ty.sm))
                                                                    .text_color(c.pearl)
                                                                    .child(SharedString::from(format!(
                                                                        "• Backend: {}",
                                                                        self.api_client.base_url()
                                                                    ))),
                                                            ),
                                                    ),
                                                SettingsTab::Voice => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(settings_h3("VOZ OFICIAL"))
                                                    .child(settings_p(
                                                        "Gemini 3.8 Flash TTS com a voz desenhada para a Usami. A configuração fica no servidor local, não neste navegador.",
                                                    ))
                                                    .child(settings_p(
                                                        "O tom muda por emoção e o texto pode pedir sons curtos como:",
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
                                                        "A tag vira áudio e não aparece no balão.",
                                                    ))
                                                    .child(settings_hint(
                                                        "Para trocar voz ou modelo, ajuste no .env e reinicie npm run api.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s1_5))
                                                            .mt(px(sp.s2))
                                                            .child(settings_code("GEMINI_TTS_VOICE"))
                                                            .child(settings_code("GEMINI_TTS_MODEL"))
                                                            .child(settings_code(".env"))
                                                            .child(settings_code("npm run api")),
                                                    )
                                                    .child(settings_h3("PRIVACIDADE"))
                                                    .child(settings_p(
                                                        "A chave da API permanece no servidor. O painel não pede nem guarda chaves.",
                                                    )),
                                                SettingsTab::Avatar => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(settings_hint(
                                                        "Os controles de modelo e cenário continuam no app web; aqui eles aparecem apenas como referência visual.",
                                                    ))
                                                    .child(settings_h3("CENÁRIO DE FUNDO"))
                                                    .child(settings_p(
                                                        "Personalize o fundo do palco e o nível de desfoque.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .mt(px(14.0))
                                                            .text_size(px(ty.sm))
                                                            .text_color(c.pearl)
                                                            .child("Desfoque: 0px"),
                                                    )
                                                    .child(settings_disabled_slider())
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s2))
                                                            .mt(px(sp.s3))
                                                            .child(settings_disabled_button("Trocar imagem de fundo")),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s2))
                                                            .mb(px(sp.s3))
                                                            .child(settings_disabled_button("Entrar em tela cheia")),
                                                    )
                                                    .child(settings_h3("MODELO"))
                                                    .child(settings_p(
                                                        "Teste os controles reais do modelo. O arquivo permanece neste navegador.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .my(px(18.0))
                                                            .w_full()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .py(px(14.0))
                                                            .rounded(px(r.md))
                                                            .bg(t.send_bg())
                                                            .border_1()
                                                            .border_color(rgba(0xffa6c959))
                                                            .text_size(px(ty.sm))
                                                            .text_color(c.pearl)
                                                            .opacity(0.4)
                                                            .child("Escolher GLB, VRM ou FBX"),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(14.0))
                                                            .text_size(px(ty.sm))
                                                            .text_color(c.pearl)
                                                            .child("Modelo local (public/)"),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s2))
                                                            .mb(px(14.0))
                                                            .child(settings_disabled_input("/models/exemplo.vrm")),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s2))
                                                            .mb(px(sp.s3))
                                                            .child(settings_disabled_button("Carregar caminho")),
                                                    )
                                                    .child(settings_p(
                                                        "Arquivos grandes não cabem no upload; caminhos servidos por /public. Ex.: /models/shiro.vrm",
                                                    ))
                                                    .child(settings_h3("INSPEÇÃO DO MODELO"))
                                                    .child(settings_dl_row("Formato", "VRM 1.0"))
                                                    .child(settings_dl_row("Modelo", "白ー (shiro)"))
                                                    .child(settings_dl_row("Renderizador", "WGPU 60 FPS (Nativo)"))
                                                    .child(settings_hint(
                                                        "Movimentos, meshes, animações, ossos e expressões seguem disponíveis apenas no app web.",
                                                    )),
                                                SettingsTab::Memory => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(settings_h3("O QUE ELA GUARDOU"))
                                                    .child(settings_hint(
                                                        "0 fato(s) durável(is) · 0 recente(s) · 0 resumo(s) · afinidade 0/100. Tudo fica no memory.json do servidor local.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s4))
                                                            .mb(px(sp.s1_5))
                                                            .text_size(px(ty.sm))
                                                            .text_color(c.pearl)
                                                            .child("Digite APAGAR para liberar as exclusões"),
                                                    )
                                                    .child(settings_disabled_input("APAGAR"))
                                                    .child(settings_hint(
                                                        "Sem a palavra, nenhum botão apaga. As exclusões são imediatas e não têm desfazer.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s2))
                                                            .text_size(px(ty.sm))
                                                            .text_color(c.muted)
                                                            .child("Nenhum fato duradouro ainda."),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(18.0))
                                                            .flex()
                                                            .child(settings_danger_button("Apagar tudo")),
                                                    )
                                                    .child(settings_hint(
                                                        "O histórico do navegador (aba Conversa) é separado disso.",
                                                    ))
                                                    .child(settings_hint(
                                                        "Este painel do app desktop ainda não lê nem apaga a memória; os fatos ficam no app web.",
                                                    )),
                                                SettingsTab::Personality => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(settings_h3("JEITO DA USAMI"))
                                                    .child(settings_hint(
                                                        "50 é a personalidade original. Os ajustes refinam o tom; as regras de ouro e a honestidade dela continuam as mesmas.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s3_5))
                                                            .flex()
                                                            .flex_col()
                                                            .gap(px(14.0))
                                                            .child(settings_trait_row("Carinho"))
                                                            .child(settings_trait_row("Curiosidade"))
                                                            .child(settings_trait_row("Brincadeira"))
                                                            .child(settings_trait_row("Orgulho (tsundere)"))
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .gap(px(6.0))
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .justify_between()
                                                                            .items_center()
                                                                            .text_size(px(ty.md))
                                                                            .text_color(c.pearl)
                                                                            .child("Instruções extras")
                                                                            .child(
                                                                                div()
                                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                                    .text_color(c.primary)
                                                                                    .child("0/600"),
                                                                            ),
                                                                    )
                                                                    .child(settings_disabled_textarea()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .mt(px(sp.s4))
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s2))
                                                            .child(settings_disabled_button("Salvar"))
                                                            .child(settings_disabled_button("Restaurar padrão")),
                                                    )
                                                    .child(settings_hint(
                                                        "Os sliders e o texto ficam desligados aqui; a personalidade é editada no app web.",
                                                    )),
                                                SettingsTab::About => div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(settings_h3("USAMI COMPANION"))
                                                    .child(settings_p(
                                                        "Versão 0.1.0 — companion com avatar 3D/Live2D e voz própria.",
                                                    ))
                                                    .child(settings_h3("TECNOLOGIAS"))
                                                    .child(settings_list_item("React 19 + Vite"))
                                                    .child(settings_list_item("Three.js + @pixiv/three-vrm (VRM/GLB)"))
                                                    .child(settings_list_item("Live2D Cubism 3 (opcional)"))
                                                    .child(settings_list_item("OpenRouter / NVIDIA (LLM) + Gemini (TTS)"))
                                                    .child(settings_list_item("Web Speech API para a chamada de voz"))
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
                                                            .child("por")
                                                            .child(
                                                                div()
                                                                    .text_color(c.primary)
                                                                    .underline()
                                                                    .child("Sweet_nya555"),
                                                            ),
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
                                                            .child("Ícones:")
                                                            .child(
                                                                div()
                                                                    .text_color(c.primary)
                                                                    .underline()
                                                                    .child("Lucide"),
                                                            ),
                                                    )
                                                    .child(settings_h3("PRIVACIDADE"))
                                                    .child(settings_p(
                                                        "Texto e voz passam pela API local em 127.0.0.1:8787, que lê as chaves do .env. O histórico da conversa fica no localStorage deste navegador.",
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap(px(sp.s1_5))
                                                            .child(settings_code("127.0.0.1:8787"))
                                                            .child(settings_code(".env"))
                                                            .child(settings_code("localStorage")),
                                                    ),

                                            }),
                                    )
                            }
                            ActiveDrawer::None => div(),
                        }),
                )
            })
            // ── LAYER 8: PAINEL DE VÍNCULO (SHEET FLUTUANTE DA WEB) ──
            .when(active_drawer == ActiveDrawer::Bond, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .pt(px(72.0))
                        .pr(px(sp.s6))
                        .pb(px(sp.s6))
                        .bg(t.bond_scrim_bg())
                        .child(
                            div()
                                .w(px(520.0))
                                .max_h(px(804.0))
                                .flex()
                                .flex_col()
                                .gap(px(sp.s3))
                                .p(px(18.0))
                                .rounded(px(r.sheet))
                                .bg(t.bond_sheet_bg())
                                .border_1()
                                .border_color(c.bond_border)
                                .shadow(t.shadow.sheet.clone())
                                .child(
                                    div().flex().justify_end().child(
                                        div()
                                            .id("bond-close-btn")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(34.0))
                                            .rounded_full()
                                            .bg(c.surface_soft)
                                            .border_1()
                                            .border_color(c.line_strong)
                                            .text_color(c.pearl)
                                            .hover(|style| {
                                                style.bg(c.accent_tint).border_color(c.primary)
                                            })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.active_drawer = ActiveDrawer::None;
                                                this.poke(cx);
                                                cx.notify();
                                            }))
                                            .child(icon("close", 16.0)),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(18.0))
                                        .pb(px(sp.s1))
                                        .pl(px(sp.s1))
                                        // Badge "Nv.N" do vínculo (`.bond-badge-level`):
                                        // Fraunces upright 600 a 30px sobre o coração.
                                        .child(
                                            div()
                                                .size(px(120.0))
                                                .flex_shrink_0()
                                                .child(icon("heart_empty", 120.0))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left_0()
                                                        .right_0()
                                                        .bottom_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .pb(px(sp.s2))
                                                        .child(
                                                            div()
                                                                .text_size(px(30.0))
                                                                .font_family(theme::FONT_CHARACTER)
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(c.pearl)
                                                                .child(format!("Nv.{bond_level}")),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .gap(px(sp.s2))
                                                .child(
                                                    div()
                                                        .text_size(px(20.0))
                                                        .font_family(theme::FONT_CHARACTER)
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(c.pearl)
                                                        .child(format!(
                                                            "Nível {bond_level} — Amiga Próxima"
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(320.0))
                                                        .h(px(12.0))
                                                        .rounded_full()
                                                        .bg(c.bond_track)
                                                        .child(
                                                            div()
                                                                .h_full()
                                                                .w(px(3.2 * bond_exp_pct as f32))
                                                                .rounded_full()
                                                                .bg(t.bond_fill_bg()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.muted)
                                                        .child(format!(
                                                            "{bond_exp_pct}% para o próximo nível"
                                                        )),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .p(px(sp.s4))
                                        .rounded(px(r.md))
                                        .bg(c.card_bg)
                                        .child(
                                            div()
                                                .text_size(px(ty.sm))
                                                .line_height(px(19.0))
                                                .text_color(c.pearl)
                                                .child(
                                                    "Converse com a Usami para fortalecer o vínculo de vocês.",
                                                ),
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
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
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
