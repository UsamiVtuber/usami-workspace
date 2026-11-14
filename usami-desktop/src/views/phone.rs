//! Painel de conversa no formato Smartphone ("Celular da Usami").
//!
//! Camada 6 do render: frame físico do celular, notch, status bar, header de contato,
//! balões de mensagem estilo WhatsApp com caudas/status de entrega, e barra de input.

use crate::model::{current_phone_time, ActiveDrawer, Sender};
use crate::theme;
use crate::ui::widgets::icon;
use crate::views::vn_bond::{mood_visual, render_lego_portrait};
use crate::UsamiApp;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::time::Duration;

impl UsamiApp {
    pub(crate) fn render_phone_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let sp = t.space;

        let is_busy = self.is_busy;
        let is_calling = self.is_calling;
        let input_text = self.input_text.clone();
        let mood_vis = mood_visual(
            self.mood
                .as_ref()
                .map(|m| m.id.as_str())
                .unwrap_or_else(|| self.last_emotion.as_ref()),
        );

                let active_tool = self.active_tool.clone();
        let activity_phase = self
            .chat_activity
            .as_ref()
            .and_then(|a| a.phase.clone());
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

        let hud_portrait = if !self.is_ui_idle {
            self.vrm_controller.portrait_image()
        } else {
            None
        };
        let hud_prev_slot = self.prev_portrait_image.clone();
        let turns = self.chat_history.clone();
        let phone_scroll = self.phone_scroll.clone();
        let no_turns = turns.is_empty();

        let status_subtitle: SharedString = if is_busy {
                    "digitando...".into()
                } else if is_calling {
                    "em chamada de voz".into()
                } else if self.is_companion_mode {
                    "modo companion ativo".into()
                } else {
                    "online".into()
                };


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
                        )
    }
}
