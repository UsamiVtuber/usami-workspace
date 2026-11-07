pub mod api;
pub mod audio;

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

fn icon(name: &str, size_px: f32) -> impl IntoElement {
    let path = PathBuf::from(format!(r"A:\Usami Project\usami-desktop\assets\icons\{}.svg", name));
    img(path)
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
    is_companion_mode: bool,
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

        let candidate_path = PathBuf::from(r"A:\Usami Project\usami-desktop\assets\background.png");
        let bg_asset_path = if candidate_path.exists() {
            Some(candidate_path)
        } else {
            let alt = PathBuf::from(r"A:\Usami Project\usami\public\background.png");
            if alt.exists() { Some(alt) } else { None }
        };

        let vrm_path = [
            PathBuf::from(r"A:\Usami Project\usami\public\models\shiro.vrm"),
            PathBuf::from("../usami/public/models/shiro.vrm"),
            PathBuf::from("assets/shiro.vrm"),
        ]
        .into_iter()
        .find(|p| p.exists());

        let vrma_dir = [
            PathBuf::from(r"A:\Usami Project\usami\public\animations\vrma"),
            PathBuf::from("../usami/public/animations/vrma"),
            PathBuf::from("assets/vrma"),
        ]
        .into_iter()
        .find(|p| p.exists());

        let vrm_controller = VrmController::new();
        vrm_controller.set_emotion_with_options("tsundere", 4.5, false);

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
        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                let has_drawer = view.active_drawer != ActiveDrawer::None;
                                let is_idle = !has_drawer && view.last_interaction.elapsed() > Duration::from_millis(4500);
                                if view.is_ui_idle != is_idle {
                                    view.is_ui_idle = is_idle;
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
            is_companion_mode: false,
            is_muted: false,
            is_ui_idle: false,
            last_interaction: Instant::now(),
            active_drawer: ActiveDrawer::None,
            chat_history: initial_history,
            bond_level: 2,
            bond_exp_pct: 74,
            bg_asset_path,
            api_client: Arc::new(UsamiApiClient::new("http://127.0.0.1:8787")),
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

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.poke(cx);
        let text = self.input_text.trim().to_string();
        if text.is_empty() || self.is_busy {
            return;
        }

        self.input_text.clear();
        self.is_busy = true;
        self.show_bubble = true;
        self.status_text = "PENSANDO".into();

        self.chat_history.push(ChatMessage {
            sender: Sender::User,
            text: text.clone(),
            emotion: None,
        });

        cx.notify();

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();

        cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.send_chat(&text).await;

                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.is_busy = false;
                            view.poke(cx);
                            match res {
                                Ok(response) => {
                                    let emotion = response.gesture.unwrap_or_else(|| "tsundere".into());
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

                                    view.vrm_controller.set_emotion(&emotion);

                                    view.chat_history.push(ChatMessage {
                                        sender: Sender::Usami,
                                        text: response.reply.clone(),
                                        emotion: Some(emotion),
                                    });

                                    if view.bond_exp_pct < 100 {
                                        view.bond_exp_pct += 2;
                                    } else {
                                        view.bond_level += 1;
                                        view.bond_exp_pct = 5;
                                    }

                                    cx.notify();

                                    if !view.is_muted {
                                        if let Some(player) = audio {
                                            let api_tts = Arc::clone(&view.api_client);
                                            let tts_text = response.reply;
                                            std::thread::spawn(move || {
                                                let rt = tokio::runtime::Builder::new_current_thread()
                                                    .enable_all()
                                                    .build()
                                                    .unwrap();
                                                rt.block_on(async {
                                                    if let Ok(wav_bytes) = api_tts.fetch_tts_audio(&tts_text, "neutral").await {
                                                        let _ = player.play_wav(wav_bytes);
                                                    }
                                                });
                                            });
                                        }
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
        let active_drawer = self.active_drawer;
        let bond_level = self.bond_level;
        let show_bubble = self.show_bubble;
        let bg_asset = self.bg_asset_path.clone();
        let vrm_entity = self.vrm_view.clone();

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
                let key = &event.keystroke.key;
                if key == "enter" {
                    this.submit(cx);
                } else if key == "backspace" {
                    this.input_text.pop();
                    cx.notify();
                } else if key == "escape" {
                    this.active_drawer = ActiveDrawer::None;
                    cx.notify();
                } else if let Some(ref ch) = event.keystroke.key_char {
                    this.input_text.push_str(ch);
                    cx.notify();
                } else if key.chars().count() == 1 && !key.starts_with("ctrl") && !key.starts_with("alt") {
                    this.input_text.push_str(key);
                    cx.notify();
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
            // ── LAYER 2: SHIRO VRM 3D (CENTRO DA TELA, ANCORADA NO PISO, SEM CAIXAS) ──
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(
                        div()
                            .w(px(840.0))
                            .h(px(720.0))
                            .flex()
                            .items_end()
                            .justify_center()
                            .child(if let Some(view) = vrm_entity {
                                view.into_any_element()
                            } else {
                                div().into_any_element()
                            }),
                    ),
            )
            // ── LAYER 3: FLOATING SPEECH BUBBLE (.bubble.her) ──
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(88.0))
                        .left(px(120.0))
                        .max_w(px(460.0))
                        .p_4()
                        .rounded_2xl()
                        .bg(rgba(0x160b1ad9))
                        .border_1()
                        .border_color(rgba(0xf5ebe420))
                        .shadow_lg()
                        .id("floating-bubble")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_bubble = false;
                            this.poke(cx);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .mb_1p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xd8b4e2))
                                        .child("Usami"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xc8a8e0))
                                        .child(format!("· {}", self.last_emotion)),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .line_height(px(22.0))
                                .text_color(rgb(0xf5ebe4))
                                .child(self.last_reply.clone()),
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
                                        .child(self.status_text.clone()),
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
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.poke(cx);
                                        window.focus(&this.focus_handle);
                                    }))
                                    .child(
                                        if self.input_text.is_empty() {
                                            div()
                                                .text_sm()
                                                .text_color(rgba(0xf5ebe46b))
                                                .child("Diga alguma coisa...")
                                        } else {
                                            div()
                                                .text_sm()
                                                .text_color(rgb(0xf5ebe4))
                                                .child(self.input_text.clone())
                                        }
                                    ),
                            )
                            // Ações Embutidas à Direita: Fone, Volume, Lua, Enviar
                            .child(
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
                                            .bg(if is_calling { rgb(0xe5484d) } else { rgba(0xffffff0d) })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.poke(cx);
                                                this.is_calling = !this.is_calling;
                                                this.status_text = if this.is_calling {
                                                    "EM CHAMADA".into()
                                                } else {
                                                    "COM VOCÊ".into()
                                                };
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
                                                this.is_companion_mode = !this.is_companion_mode;
                                                if this.is_companion_mode {
                                                    this.last_emotion = "sleepy".into();
                                                    this.status_text = "MODO COMPANION".into();
                                                    this.vrm_controller.set_emotion("sleepy");
                                                } else {
                                                    this.last_emotion = "tsundere".into();
                                                    this.status_text = "COM VOCÊ".into();
                                                    this.vrm_controller.set_emotion_with_options("tsundere", 4.0, false);
                                                    this.vrm_controller.play_idle();
                                                }
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
                            ),
                    )
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
                                                .child(div().mt_1().text_xs().text_color(rgb(0xf5ebe4)).child("• Backend: 127.0.0.1:8787")),
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
                let view = cx.new(|cx| UsamiApp::new(cx));
                window.focus(&view.read(cx).focus_handle);
                view
            },
        )
        .unwrap();
    });
}
