//! Painel de conversa no formato Smartphone ("Celular da Usami").
//!
//! Camada 6 do render: frame físico de smartphone flagship (titânio escovado, tela OLED curva,
//! Dynamic Island, status bar integrada com horário/5G/bateria), interface do WhatsApp completa
//! com balões outgoing verde clássico e ticks duplos azuis de lido, balões incoming ardósia com
//! selo de humor e badge de skill da Usami, suporte a mídia anexada com preview flutuante,
//! animação suave de digitação e barra de mensagem com alternância de microfone/envio.
//! Além disso, inclui a tela de Configurações nativa do WhatsApp no próprio aparelho.

use crate::UsamiApp;
use crate::asset_path;
use crate::model::{ActiveDrawer, Sender, current_phone_time};
use crate::theme;
use crate::ui::widgets::icon;
use crate::views::vn_bond::{mood_visual, render_lego_portrait};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::time::Duration;

impl UsamiApp {
    pub(crate) fn render_phone_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let sp = t.space;

        let is_busy = self.is_busy;
        let is_listening = self.is_listening;
        let live_transcript = self.live_transcript.clone();
        let is_calling = self.is_calling;
        let is_muted = self.is_muted;
        let is_settings_view = self.phone_view_settings;
        let input_text = self.input_text.clone();
        let has_media = self.attached_media.is_some();
        let attached_media = self.attached_media.clone();
        let can_send = !is_busy && (!input_text.trim().is_empty() || has_media);
        let bond_level = self.bond_level;
        let proactive_enabled = self.proactive_enabled;

        let mood_vis = mood_visual(
            self.mood
                .as_ref()
                .map(|m| m.id.as_str())
                .unwrap_or_else(|| self.last_emotion.as_ref()),
        );

        let active_tool = self.active_tool.clone();
        let activity_phase = self.chat_activity.as_ref().and_then(|a| a.phase.clone());

        let typing_label: String = if !is_busy {
            String::new()
        } else if let Some(tool) = active_tool.as_deref() {
            format!("executando {tool}...")
        } else if activity_phase.as_deref() == Some("tool") {
            "executando ferramenta...".to_string()
        } else if activity_phase.as_deref() == Some("responding") {
            "respondendo...".to_string()
        } else {
            "digitando...".to_string()
        };

        let hud_portrait = if !self.is_ui_idle {
            self.vrm_controller.portrait_image()
        } else {
            None
        };
        let hud_prev_slot = self.prev_portrait_image.clone();
        let turns = self.chat_history.clone();
        let expanded_transcripts = self.expanded_voice_transcripts.clone();
        let duplicate_last_user = turns
            .last()
            .is_some_and(|m| m.sender == Sender::User && m.text.trim() == live_transcript.trim());
        let show_live_bubble =
            is_listening && !is_busy && !live_transcript.is_empty() && !duplicate_last_user;
        let phone_scroll = self.phone_scroll.clone();
        let phone_settings_scroll = self.phone_settings_scroll.clone();
        let no_turns = turns.is_empty();

        let status_subtitle: SharedString = if is_busy {
            if !typing_label.is_empty() {
                typing_label.clone().into()
            } else {
                "digitando...".into()
            }
        } else if is_calling {
            "em chamada de voz".into()
        } else if self.is_companion_mode {
            "modo companion ativo".into()
        } else {
            "online".into()
        };

        let bond_title = self
            .bond
            .as_ref()
            .map(|b| b.summary.name.clone())
            .unwrap_or_else(|| "Confidentes".to_string());
        let bond_xp = self.bond.as_ref().map(|b| b.summary.xp).unwrap_or(132);

        div()
            .id("chat-history-smartphone")
            // Sem occlude, o scroll sobre o telefone afetaria a câmera da Usami no fundo 3D
            .occlude()
            .absolute()
            .left(px(sp.s6))
            .bottom(px(76.0))
            .w(px(392.0))
            .flex()
            .flex_col()
            .items_center()
            // Orelhas de coelho da capinha Velvet Bunny saindo do topo do chassi
            .child(
                img(asset_path("assets/icons/phone_bunny_ears.svg"))
                    .w(px(260.0))
                    .h(px(88.0))
                    .mb(px(-14.0))
                    .flex_shrink_0(),
            )
            // Chassis premium de smartphone (acabamento grafite aveludado com borda prata)
            .child(
                div()
                    .w(px(392.0))
                    .h(px(610.0))
                    .flex()
                    .flex_col()
                    .rounded(px(48.0))
                    .overflow_hidden()
                    .bg(gpui::rgba(0x141316ff))
                    .p(px(3.5))
                    .border_1()
                    .border_color(gpui::rgba(0xc8cad459))
                    .shadow(vec![
                        BoxShadow {
                            color: gpui::rgba(0x000000cc).into(),
                            offset: point(px(0.0), px(24.0)),
                            blur_radius: px(50.0),
                            spread_radius: px(0.0),
                        },
                        BoxShadow {
                            color: gpui::rgba(0xa8244033).into(),
                            offset: point(px(0.0), px(0.0)),
                            blur_radius: px(26.0),
                            spread_radius: px(0.0),
                        },
                        BoxShadow {
                            color: gpui::rgba(0x10b9811c).into(),
                            offset: point(px(0.0), px(0.0)),
                            blur_radius: px(28.0),
                            spread_radius: px(0.0),
                        },
                    ])
                    .child(
                // Tela interna OLED com cantos curvos perfeitos
                div()
                    .id("phone-screen")
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .rounded(px(44.5))
                    .overflow_hidden()
                    .bg(gpui::rgba(0x1e1c22ff))
                    // ── 1. BARRA DE STATUS + DYNAMIC ISLAND ──
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(34.0))
                            .px(px(18.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_tl(px(44.5))
                            .rounded_tr(px(44.5))
                            // Horário real local à esquerda
                            .child(
                                div()
                                    .min_w(px(54.0))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(gpui::rgba(0xf0f2f5ff))
                                    .child(current_phone_time()),
                            )
                            // Dynamic Island elegante centralizada matematicamente na tela interna (385px de largura)
                            .child(
                                div()
                                    .absolute()
                                    .top(px(7.0))
                                    .left(px((385.0 - 104.0) / 2.0))
                                    .w(px(104.0))
                                    .h(px(20.0))
                                    .rounded_full()
                                    .bg(gpui::rgba(0x000000ff))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px(px(9.0))
                                    .child(
                                        div()
                                            .size(px(8.5))
                                            .rounded_full()
                                            .bg(gpui::rgba(0x101a2eff))
                                            .border_1()
                                            .border_color(gpui::rgba(0x070c17ee)),
                                    )
                                    .child(
                                        div()
                                            .w(px(28.0))
                                            .h(px(3.0))
                                            .rounded_full()
                                            .bg(gpui::rgba(0x222a36ee)),
                                    ),
                            )
                            // Indicadores 5G, Wi-Fi e Bateria verde à direita
                            .child(
                                div()
                                    .min_w(px(54.0))
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(5.0))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::rgba(0xaebac1ff))
                                            .child("5G"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(gpui::rgba(0xaebac1ff))
                                            .child("📶"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(3.0))
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(gpui::rgba(0xaebac1ff))
                                                    .child("98%"),
                                            )
                                            .child(
                                                div()
                                                    .w(px(17.0))
                                                    .h(px(9.0))
                                                    .rounded(px(2.5))
                                                    .border_1()
                                                    .border_color(gpui::rgba(0xaebac1ff))
                                                    .p(px(1.0))
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .h_full()
                                                            .rounded(px(1.0))
                                                            .bg(gpui::rgba(0x10b981ff)),
                                                    ),
                                            ),
                                    ),
                            ),
                    )
                    // ── 2. CABEÇALHO (WHATSAPP HEADER) ──
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
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
                                    // Seta de voltar: se estiver nas configurações, volta pro chat; se estiver no chat, fecha o celular
                                    .child(
                                        div()
                                            .id("phone-back-arrow")
                                            .cursor_pointer()
                                            .px(px(4.0))
                                            .text_size(px(22.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::rgba(0xf5d0d8ff))
                                            .hover(|s| s.text_color(gpui::rgba(0x10b981ff)))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                if this.phone_view_settings {
                                                    this.phone_view_settings = false;
                                                } else {
                                                    this.active_drawer = ActiveDrawer::None;
                                                }
                                                this.poke(cx);
                                                cx.notify();
                                            }))
                                            .child("‹"),
                                    )
                                    .when(!is_settings_view, |row| {
                                        row.child(
                                            div()
                                                .relative()
                                                .size(px(36.0))
                                                .child(render_lego_portrait(
                                                    36.0,
                                                    18.0,
                                                    mood_vis.accent,
                                                    hud_portrait.clone(),
                                                    hud_prev_slot.clone(),
                                                    mood_vis.glyph,
                                                ))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .bottom(px(0.0))
                                                        .right(px(0.0))
                                                        .size(px(10.0))
                                                        .rounded_full()
                                                        .bg(gpui::rgba(0x10b981ff))
                                                        .border_1()
                                                        .border_color(gpui::rgba(0x1e1c22ff)),
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
                                                        .gap(px(4.0))
                                                        .child(
                                                            div()
                                                                .text_size(px(13.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Usami"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0x10b981ff))
                                                                .child("✓"),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(if is_busy {
                                                            gpui::rgba(0x10b981ff)
                                                        } else {
                                                            gpui::rgba(0xc8cad4ff)
                                                        })
                                                        .child(status_subtitle),
                                                ),
                                        )
                                    })
                                    .when(is_settings_view, |row| {
                                        row.child(
                                            div()
                                                .text_size(px(15.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gpui::rgba(0xe9edefff))
                                                .child("Configurações"),
                                        )
                                    }),
                            )
                            // Ações do cabeçalho
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .pr(px(2.0))
                                    .when(!is_settings_view, |actions| {
                                        actions
                                            .child(
                                                div()
                                                    .id("phone-call-btn")
                                                    .cursor_pointer()
                                                    .p(px(7.0))
                                                    .rounded_full()
                                                    .text_color(gpui::rgba(0xaebac1ff))
                                                    .hover(|s| {
                                                        s.bg(gpui::rgba(0x2a3942ff))
                                                            .text_color(gpui::rgba(0xe9edefff))
                                                    })
                                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                        this.poke(cx);
                                                        this.start_call(cx);
                                                        cx.notify();
                                                    }))
                                                    .child(icon("phone", 16.0)),
                                            )
                                            // Botão de Configurações nativo do WhatsApp
                                            .child(
                                                div()
                                                    .id("phone-settings-btn")
                                                    .cursor_pointer()
                                                    .p(px(7.0))
                                                    .rounded_full()
                                                    .text_color(gpui::rgba(0xaebac1ff))
                                                    .hover(|s| {
                                                        s.bg(gpui::rgba(0x2a3942ff))
                                                            .text_color(gpui::rgba(0xe9edefff))
                                                    })
                                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                        this.poke(cx);
                                                        this.load_settings_data(cx);
                                                        this.phone_view_settings = true;
                                                        cx.notify();
                                                    }))
                                                    .child(icon("gear", 15.0)),
                                            )
                                    })
                                    .child(
                                        div()
                                            .id("phone-close-btn")
                                            .cursor_pointer()
                                            .p(px(7.0))
                                            .rounded_full()
                                            .text_color(gpui::rgba(0xc8cad4ff))
                                            .hover(|s| {
                                                s.bg(gpui::rgba(0x2a3942ff))
                                                    .text_color(gpui::rgba(0xe9edefff))
                                            })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.active_drawer = ActiveDrawer::None;
                                                this.poke(cx);
                                                cx.notify();
                                            }))
                                            .child(icon("close", 14.0)),
                                    ),
                            ),
                    )
                    // ── 3. CORPO: CONVERSAS DO WHATSAPP (OU CONFIGURAÇÕES) ──
                    .when(!is_settings_view, |screen| {
                        screen
                            .child(
                                div()
                                    .id("phone-chat-messages")
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .p(px(10.0))
                                    .bg(gpui::rgba(0x141316ff))
                                    .overflow_y_scroll()
                                    .track_scroll(&phone_scroll)
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.0))
                                    // Pílula de data centralizada "HOJE"
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .justify_center()
                                            .mb(px(2.0))
                                            .child(
                                                div()
                                                    .rounded(px(7.0))
                                                    .bg(gpui::rgba(0x182229ff))
                                                    .px(px(10.0))
                                                    .py(px(3.5))
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(gpui::rgba(0xc8cad4ff))
                                                    .child("HOJE"),
                                            ),
                                    )
                                    // Aviso clássico de privacidade do WhatsApp
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .justify_center()
                                            .mb(px(4.0))
                                            .child(
                                                div()
                                                    .max_w(px(290.0))
                                                    .rounded(px(8.0))
                                                    .bg(gpui::rgba(0x182229ee))
                                                    .px(px(10.0))
                                                    .py(px(6.0))
                                                    .text_size(px(9.5))
                                                    .text_color(gpui::rgba(0xffd279ff))
                                                    .text_center()
                                                    .child("🔒 As mensagens são protegidas com a Usami em canal local privado."),
                                            ),
                                    )
                                    .when(no_turns, |col| {
                                        col.child(
                                            div()
                                                .py(px(32.0))
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_size(px(13.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(gpui::rgba(0xe9edefff))
                                                        .child("Conversa iniciada! 👋"),
                                                )
                                                .child(
                                                    div()
                                                        .mt(px(4.0))
                                                        .text_size(px(11.0))
                                                        .text_color(gpui::rgba(0xc8cad4ff))
                                                        .child("Envie uma mensagem ou foto para a Usami."),
                                                ),
                                        )
                                    })
                                    .children(turns.clone().into_iter().enumerate().map(|(idx, msg)| {
                                        let is_user = msg.sender == Sender::User;
                                        let time = msg.time_str.as_deref().unwrap_or("agora");

                                        if is_user {
                                            let is_voice_note = msg
                                                .media
                                                .as_ref()
                                                .is_some_and(|m| m.mime_type.starts_with("audio/"));
                                            let transcript_open = expanded_transcripts.contains(&idx);
                                            let audio_b64 = msg
                                                .media
                                                .as_ref()
                                                .filter(|_| is_voice_note)
                                                .map(|m| m.data_base64.clone());
                                            let audio_path = msg
                                                .media
                                                .as_ref()
                                                .filter(|_| is_voice_note)
                                                .and_then(|m| m.stored_path.clone());

                                            div()
                                                .flex()
                                                .flex_shrink_0()
                                                .w_full()
                                                .justify_end()
                                                .child(
                                                    div()
                                                        .max_w(px(270.0))
                                                        .py(px(6.5))
                                                        .px(px(10.0))
                                                        .rounded_tl(px(12.0))
                                                        .rounded_bl(px(12.0))
                                                        .rounded_br(px(12.0))
                                                        .rounded_tr(px(2.0))
                                                        .bg(gpui::rgba(0x5e1526ff))
                                                        .shadow(vec![BoxShadow {
                                                            color: gpui::rgba(0x00000030).into(),
                                                            offset: point(px(0.0), px(1.0)),
                                                            blur_radius: px(3.0),
                                                            spread_radius: px(0.0),
                                                        }])
                                                        .when(is_voice_note, |parent| {
                                                            parent
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap(px(8.0))
                                                                        .child(
                                                                            div()
                                                                                .id(("voice-play", idx))
                                                                                .w(px(28.0))
                                                                                .h(px(28.0))
                                                                                .rounded_full()
                                                                                .bg(gpui::rgba(0xffffff22))
                                                                                .hover(|s| s.bg(gpui::rgba(0xffffff38)))
                                                                                .cursor_pointer()
                                                                                .flex()
                                                                                .items_center()
                                                                                .justify_center()
                                                                                .text_size(px(12.0))
                                                                                .text_color(gpui::rgba(0xffffffff))
                                                                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                                                    if let Some(player) = this.audio_player.as_ref() {
                                                                                        let bytes = audio_b64
                                                                                            .as_deref()
                                                                                            .and_then(crate::api::base64_decode)
                                                                                            .filter(|b| !b.is_empty())
                                                                                            .or_else(|| {
                                                                                                audio_path.as_deref().and_then(|rel| {
                                                                                                    std::fs::read(crate::storage::data_dir().join(rel)).ok()
                                                                                                })
                                                                                            });
                                                                                        if let Some(wav) = bytes {
                                                                                            let _ = player.play_wav(wav);
                                                                                        }
                                                                                    }
                                                                                    this.poke(cx);
                                                                                }))
                                                                                .child("▶"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .flex_1()
                                                                                .flex()
                                                                                .items_center()
                                                                                .gap(px(2.5))
                                                                                .children([6.0, 12.0, 9.0, 15.0, 8.0, 14.0, 10.0, 16.0, 11.0, 7.0, 13.0, 8.0].into_iter().map(|h| {
                                                                                    div()
                                                                                        .w(px(2.5))
                                                                                        .h(px(h))
                                                                                        .rounded_full()
                                                                                        .bg(gpui::rgba(0xffffff99))
                                                                                })),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .id(("voice-transcript-toggle", idx))
                                                                                .px(px(6.0))
                                                                                .py(px(2.5))
                                                                                .rounded_full()
                                                                                .bg(gpui::rgba(0x00000033))
                                                                                .hover(|s| s.bg(gpui::rgba(0x00000055)))
                                                                                .cursor_pointer()
                                                                                .text_size(px(9.5))
                                                                                .text_color(gpui::rgba(0xf3f4f6dd))
                                                                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                                                    if !this.expanded_voice_transcripts.remove(&idx) {
                                                                                        this.expanded_voice_transcripts.insert(idx);
                                                                                    }
                                                                                    this.poke(cx);
                                                                                    cx.notify();
                                                                                }))
                                                                                .child(if transcript_open { "Ocultar" } else { "Transcrição" }),
                                                                        ),
                                                                )
                                                                .when(transcript_open, |parent| {
                                                                    parent.child(
                                                                        div()
                                                                            .mt(px(5.0))
                                                                            .pt(px(4.0))
                                                                            .border_t_1()
                                                                            .border_color(gpui::rgba(0xffffff22))
                                                                            .text_size(px(12.0))
                                                                            .line_height(px(17.0))
                                                                            .text_color(gpui::rgba(0xe9edefdd))
                                                                            .child(msg.text.clone()),
                                                                    )
                                                                })
                                                        })
                                                        .when(!is_voice_note, |parent| {
                                                            parent
                                                                .child(
                                                                    div()
                                                                        .text_size(px(12.8))
                                                                        .line_height(px(18.0))
                                                                        .text_color(gpui::rgba(0xe9edefff))
                                                                        .child(msg.text.clone()),
                                                                )
                                                                .when_some(msg.media.as_ref(), |_parent, media| {
                                                                    if media.mime_type.starts_with("image/") {
                                                                        div()
                                                                            .mt(px(6.0))
                                                                            .w(px(160.0))
                                                                            .h(px(110.0))
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
                                                                            .text_color(gpui::rgba(0xc8cad4ff))
                                                                            .child("📄")
                                                                            .child(media.filename.clone())
                                                                    }
                                                                })
                                                        })
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .justify_end()
                                                                .items_center()
                                                                .gap(px(3.5))
                                                                .mt(px(2.5))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(9.5))
                                                                        .text_color(gpui::rgba(0xffffffaa))
                                                                        .child(time.to_string()),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(11.5))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(gpui::rgba(0x53bdebff))
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
                                                        .max_w(px(270.0))
                                                        .py(px(6.5))
                                                        .px(px(10.0))
                                                        .rounded_tr(px(12.0))
                                                        .rounded_br(px(12.0))
                                                        .rounded_bl(px(12.0))
                                                        .rounded_tl(px(2.0))
                                                        .bg(gpui::rgba(0x27242cff))
                                                        .shadow(vec![BoxShadow {
                                                            color: gpui::rgba(0x00000030).into(),
                                                            offset: point(px(0.0), px(1.0)),
                                                            blur_radius: px(3.0),
                                                            spread_radius: px(0.0),
                                                        }])
                                                        .child(
                                                            div()
                                                                .mb(px(2.5))
                                                                .text_size(px(9.8))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0x10b981ff))
                                                                .child(header),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.8))
                                                                .line_height(px(18.0))
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child(msg.text),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .justify_end()
                                                                .mt(px(2.5))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(9.5))
                                                                        .text_color(gpui::rgba(0xc8cad4ff))
                                                                        .child(time.to_string()),
                                                                ),
                                                        ),
                                                )
                                        }
                                    }))
                                    .when(show_live_bubble, |col| {
                                        col.child(
                                            div()
                                                .flex()
                                                .flex_shrink_0()
                                                .w_full()
                                                .justify_end()
                                                .child(
                                                    div()
                                                        .max_w(px(238.0))
                                                        .px(px(10.0))
                                                        .py(px(6.0))
                                                        .rounded_tl(px(12.0))
                                                        .rounded_bl(px(12.0))
                                                        .rounded_br(px(12.0))
                                                        .rounded_tr(px(2.0))
                                                        .bg(gpui::rgba(0x4b1d2acc))
                                                        .border_1()
                                                        .border_color(gpui::rgba(0xf472b666))
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .line_height(px(17.0))
                                                                .text_color(gpui::rgba(0xf3f4f6ee))
                                                                .child(format!("🎙 {live_transcript}")),
                                                        ),
                                                ),
                                        )
                                    })
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
                                                        .rounded_tr(px(12.0))
                                                        .rounded_br(px(12.0))
                                                        .rounded_bl(px(12.0))
                                                        .rounded_tl(px(2.0))
                                                        .bg(gpui::rgba(0x27242cff))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(5.0))
                                                                .children((0..3usize).map(|idx| {
                                                                    div()
                                                                        .size(px(6.0))
                                                                        .rounded_full()
                                                                        .bg(gpui::rgba(0x10b981ff))
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
                                                                .when(!typing_label.is_empty(), |row| {
                                                                    row.child(
                                                                        div()
                                                                            .ml(px(3.0))
                                                                            .text_size(px(11.0))
                                                                            .text_color(gpui::rgba(0xc8cad4ff))
                                                                            .child(typing_label.clone()),
                                                                    )
                                                                }),
                                                        ),
                                                ),
                                        )
                                    }),
                            )
                            // Prévia de mídia anexada
                            .when_some(attached_media.as_ref(), |parent, media| {
                                parent.child(
                                    div()
                                        .bg(gpui::rgba(0x1e1c22ff))
                                        .px(px(10.0))
                                        .pt(px(6.0))
                                        .flex()
                                        .items_center()
                                        .justify_start()
                                        .child(
                                            div()
                                                .relative()
                                                .size(px(48.0))
                                                .rounded(px(8.0))
                                                .overflow_hidden()
                                                .bg(gpui::rgba(0x0b1419ff))
                                                .border_1()
                                                .border_color(gpui::rgba(0x10b981ff))
                                                .child(
                                                    img(media.preview.clone())
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                                .child(
                                                    div()
                                                        .id("phone-media-remove")
                                                        .cursor_pointer()
                                                        .absolute()
                                                        .top(px(2.0))
                                                        .right(px(2.0))
                                                        .size(px(16.0))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(gpui::rgba(0x000000cc))
                                                        .hover(|s| s.bg(gpui::rgba(0xef4444ff)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.attached_media = None;
                                                            cx.notify();
                                                        }))
                                                        .child(icon("close", 10.0)),
                                                ),
                                        ),
                                )
                            })
                            // Barra de entrada do WhatsApp
                            .child(
                                div()
                                    .bg(gpui::rgba(0x1e1c22ff))
                                    .px(px(8.0))
                                    .pt(px(6.0))
                                    .pb(px(8.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_end()
                                    .gap(px(6.0))
                                    .rounded_bl(px(44.5))
                                    .rounded_br(px(44.5))
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .p(px(5.0))
                                            .text_size(px(18.0))
                                            .cursor_pointer()
                                            .child("😊"),
                                    )
                                    .child(
                                        div()
                                            .id("phone-input-scroll")
                                            .flex_1()
                                            .flex_shrink_0()
                                            .min_w(px(0.0))
                                            .min_h(px(0.0))
                                            .max_h(px(76.0))
                                            .flex()
                                            .items_center()
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
                                                div()
                                                    .id("phone-input-content")
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .whitespace_normal()
                                                    .text_size(px(12.0))
                                                    .line_height(px(16.0))
                                                    .text_color(if input_text.is_empty() {
                                                        gpui::rgba(0xc8cad4ff)
                                                    } else {
                                                        gpui::rgba(0xe9edefff)
                                                    })
                                                    .child(if input_text.is_empty() {
                                                        "Mensagem...".to_string()
                                                    } else {
                                                        input_text.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .id("phone-attach-btn")
                                                    .flex_shrink_0()
                                                    .cursor_pointer()
                                                    .ml(px(6.0))
                                                    .p(px(2.0))
                                                    .text_color(if has_media {
                                                        gpui::rgba(0x10b981ff)
                                                    } else {
                                                        gpui::rgba(0xc8cad4ff)
                                                    })
                                                    .hover(|s| s.text_color(gpui::rgba(0xe9edefff)))
                                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                        this.attach_file_prompt(cx);
                                                    }))
                                                    .child(icon("image", 16.0)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("phone-send-btn")
                                            .flex_shrink_0()
                                            .cursor_pointer()
                                            .size(px(36.0))
                                            .rounded_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .bg(gpui::rgba(0x10b981ff))
                                            .text_color(gpui::rgba(0xffffffff))
                                            .shadow(vec![BoxShadow {
                                                color: gpui::rgba(0x10b98140).into(),
                                                offset: point(px(0.0), px(2.0)),
                                                blur_radius: px(6.0),
                                                spread_radius: px(0.0),
                                            }])
                                            .hover(|s| s.bg(gpui::rgba(0x20ba5cff)))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                if !this.input_text.trim().is_empty() || this.attached_media.is_some() {
                                                    this.submit(cx);
                                                } else {
                                                    this.toggle_listening(cx);
                                                }
                                            }))
                                            .child(if can_send {
                                                icon("send", 15.0).into_any_element()
                                            } else {
                                                icon("pulse_white", 16.0).into_any_element()
                                            }),
                                    ),
                            )
                    })
                    // ── 3b. CORPO: CONFIGURAÇÕES NATIVAS DO WHATSAPP DA USAMI ──
                    .when(is_settings_view, |screen| {
                        screen.child(
                            div()
                                .id("phone-settings-screen")
                                .flex_1()
                                .min_h(px(0.0))
                                .p(px(14.0))
                                .pb(px(36.0))
                                .bg(gpui::rgba(0x141316ff))
                                .overflow_y_scroll()
                                .track_scroll(&phone_settings_scroll)
                                .flex()
                                .flex_col()
                                .gap(px(14.0))
                                // Card de perfil do WhatsApp
                                .child(
                                    div()
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .child(
                                            div()
                                                .relative()
                                                .size(px(52.0))
                                                .child(render_lego_portrait(
                                                    52.0,
                                                    26.0,
                                                    mood_vis.accent,
                                                    hud_portrait.clone(),
                                                    hud_prev_slot.clone(),
                                                    mood_vis.glyph,
                                                )),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .child(
                                                            div()
                                                                .text_size(px(15.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Usami"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0x10b981ff))
                                                                .child("✓"),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(gpui::rgba(0x10b981ff))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(format!("♥ Nv. {bond_level} · {bond_title} ({bond_xp} XP)")),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(gpui::rgba(0xc8cad4ff))
                                                        .child("Disponível ♡ · Tsundere Companion"),
                                                ),
                                        ),
                                )
                                // Grupo 1: Notificações & Proatividade
                                .child(
                                    div()
                                        .id("btn-settings-proactive")
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(gpui::rgba(0x1e1c22ff)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.set_proactive_enabled(!this.proactive_enabled, cx);
                                            cx.notify();
                                        }))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.0))
                                                .child(icon("pulse", 18.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Proatividade da Usami"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(gpui::rgba(0xc8cad4ff))
                                                                .child(if proactive_enabled {
                                                                    "Ativada · Ela puxa assunto espontaneamente"
                                                                } else {
                                                                    "Pausada · Apenas responde quando chamada"
                                                                }),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .w(px(38.0))
                                                .h(px(20.0))
                                                .rounded_full()
                                                .bg(if proactive_enabled {
                                                    gpui::rgba(0x10b981ff)
                                                } else {
                                                    gpui::rgba(0x374248ff)
                                                })
                                                .p(px(2.0))
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .child(
                                                    div()
                                                        .size(px(16.0))
                                                        .rounded_full()
                                                        .bg(gpui::rgba(0xffffffff)),
                                                ),
                                        ),
                                )
                                // Grupo 2: Voz & Microfone
                                .child(
                                    div()
                                        .id("btn-settings-audio")
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(gpui::rgba(0x1e1c22ff)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.touch_ui(cx);
                                            this.is_muted = !this.is_muted;
                                            this.sync_mic_gate();
                                            this.persist_prefs();
                                            cx.notify();
                                        }))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.0))
                                                .child(icon("phone", 18.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Voz & Ligações"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(gpui::rgba(0xc8cad4ff))
                                                                .child(if is_muted {
                                                                    "Microfone mudo"
                                                                } else {
                                                                    "Microfone ativo · Chamadas habilitadas"
                                                                }),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .w(px(38.0))
                                                .h(px(20.0))
                                                .rounded_full()
                                                .bg(if !is_muted {
                                                    gpui::rgba(0x10b981ff)
                                                } else {
                                                    gpui::rgba(0x374248ff)
                                                })
                                                .p(px(2.0))
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .child(
                                                    div()
                                                        .size(px(16.0))
                                                        .rounded_full()
                                                        .bg(gpui::rgba(0xffffffff)),
                                                ),
                                        ),
                                )
                                // Grupo 3: Conversas (Limpar histórico)
                                .child(
                                    div()
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.0))
                                                .child(icon("chat_bubble", 18.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Conversas"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(gpui::rgba(0xc8cad4ff))
                                                                .child(format!("{} mensagens armazenadas", turns.len())),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("btn-clear-chat")
                                                .cursor_pointer()
                                                .py(px(6.0))
                                                .px(px(10.0))
                                                .rounded(px(8.0))
                                                .bg(gpui::rgba(0xef444422))
                                                .border_1()
                                                .border_color(gpui::rgba(0xef444455))
                                                .text_center()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gpui::rgba(0xf87171ff))
                                                .hover(|s| s.bg(gpui::rgba(0xef444444)))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.chat_history.clear();
                                                    crate::storage::save_conversation(&crate::model::stored_turns(&this.chat_history));
                                                    this.phone_view_settings = false;
                                                    this.poke(cx);
                                                    cx.notify();
                                                }))
                                                .child("Limpar histórico de mensagens"),
                                        ),
                                )
                                // Grupo 4: Central de Vínculo VN & Diário
                                .child(
                                    div()
                                        .id("btn-settings-bond")
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(gpui::rgba(0x1e1c22ff)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Bond, cx);
                                        }))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(10.0))
                                                .child(icon("heart_filled", 18.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(gpui::rgba(0xe9edefff))
                                                                .child("Diário & Central de Vínculo"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(gpui::rgba(0xc8cad4ff))
                                                                .child("Abrir memórias, conquistas e afinidade"),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .text_color(gpui::rgba(0xc8cad4ff))
                                                .child("›"),
                                        ),
                                )
                                // Grupo 5: Sobre o Usami
                                .child(
                                    div()
                                        .p(px(12.0))
                                        .rounded(px(16.0))
                                        .bg(gpui::rgba(0x182229ff))
                                        .border_1()
                                        .border_color(gpui::rgba(0x24323cff))
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(icon("gear", 18.0))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_size(px(12.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(gpui::rgba(0xe9edefff))
                                                        .child("Sobre o Usami"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(gpui::rgba(0xc8cad4ff))
                                                        .child("Versão 0.5.1 · Core local 127.0.0.1:8787"),
                                                ),
                                        ),
                                ),
                        )
                    })
                    // ── 4. HOME INDICATOR BAR (BASE DO SMARTPHONE) ──
                    .child(
                        div()
                            .w(px(340.0))
                            .mx_auto()
                            .h(px(10.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if is_settings_view {
                                gpui::rgba(0x141316ff)
                            } else {
                                gpui::rgba(0x1e1c22ff)
                            })
                            .rounded_bl(px(44.5))
                            .rounded_br(px(44.5))
                            .child(
                                div()
                                    .w(px(72.0))
                                    .h(px(2.5))
                                    .rounded_full()
                                    .bg(gpui::rgba(0xffffff44)),
                            ),
                    ),
                ),
            )
    }
}
