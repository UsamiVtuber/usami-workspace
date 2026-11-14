//! Overlay transparente WeebUI para o modo LoL Gamer Companion (Usami.GG).
//! Segue a estética Midnight Romance editorial sem uso de ícones genéricos Lucide.

use crate::UsamiApp;
use crate::theme;
use crate::ui::widgets::icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl UsamiApp {
    pub(crate) fn render_lol_overlay(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;

        let champion = self.lol_champion.as_deref().unwrap_or("Aguardando Campeão");
        let patch_label = self
            .lol_patch
            .as_deref()
            .map(|p| format!("v{p}"))
            .unwrap_or_else(|| "vLocal".to_string());
        let (k, d, a) = self.lol_kda;
        let cs = self.lol_cs;
        let mins = (self.lol_game_time / 60.0).floor() as u32;
        let secs = (self.lol_game_time % 60.0).floor() as u32;
        let click_through = self.lol_click_through;
        let expr_label = self.lol_last_expression.label_pt();
        let role_label = self.lol_role.label_pt();
        let reply = self.last_reply.clone();
        let show_bubble = self.show_bubble && !reply.is_empty();
        let is_listening = self.is_listening;
        let ptt_down = self.mic_gate.is_ptt_down();
        let mic_label = if ptt_down {
            "🎙️ PTT GRAVANDO (V/Mouse)"
        } else {
            "🎙️ PTT: Segure V ou Mouse"
        };
        let live_transcript = self.live_transcript.clone();
        let show_live_transcript = is_listening && !live_transcript.is_empty();

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .justify_between()
            .p(px(sp.s5))
            // Palco VRM transparente (sem backdrop opaco para enxergar o jogo atrás)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_end()
                    .justify_center()
                    .when_some(self.vrm_view.clone(), |parent, view| parent.child(view)),
            )
            // Cabeçalho compacto Usami.GG (canto superior direito)
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(sp.s2))
                    .when(show_live_transcript, |parent| {
                        parent.child(
                            div()
                                .px(px(sp.s3))
                                .py(px(sp.s1_5))
                                .rounded(px(12.0))
                                .bg(rgba(0x20102FCC))
                                .border_1()
                                .border_color(c.primary)
                                .text_size(px(11.0))
                                .text_color(c.pearl)
                                .child(format!("Ouvindo: {live_transcript}")),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(sp.s2_5))
                            .px(px(sp.s3))
                            .py(px(sp.s2))
                            .rounded(px(14.0))
                            .bg(rgba(0x100819CC))
                            .border_1()
                            .border_color(c.line_strong)
                            .child(icon("lol_companion", 18.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(sp.s2))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_family(theme::FONT_DISPLAY)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.primary)
                                                    .child("USAMI.GG"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(c.lavender)
                                                    .child(patch_label),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(c.pearl)
                                            .child(if self.lol_loading {
                                                "Carregando partida...".to_string()
                                            } else {
                                                format!(
                                                    "{champion} ({role_label}) · {mins:02}:{secs:02} · {k}/{d}/{a} · {cs} CS"
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(3.0))
                                            .rounded(px(999.0))
                                            .bg(if ptt_down {
                                                rgba(0xF052B555)
                                            } else {
                                                rgba(0x20102F88)
                                            })
                                            .border_1()
                                            .border_color(if ptt_down {
                                                c.accent
                                            } else {
                                                c.line
                                            })
                                            .text_size(px(10.0))
                                            .text_color(if ptt_down { c.accent } else { c.lavender })
                                            .child(mic_label),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(3.0))
                                            .rounded(px(999.0))
                                            .bg(if click_through {
                                                rgba(0xF052B533)
                                            } else {
                                                rgba(0xA77ABF44)
                                            })
                                            .border_1()
                                            .border_color(if click_through {
                                                c.accent
                                            } else {
                                                c.primary
                                            })
                                            .text_size(px(10.0))
                                            .text_color(c.pearl)
                                            .child(if click_through {
                                                "TRAVADO (Ctrl+Alt+L)"
                                            } else {
                                                "ARRASTÁVEL (Ctrl+Alt+L)"
                                            }),
                                    ),
                            ),
                    ),
            )
            // Balão editorial inferior sincronizado com a expressão do VRM
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .relative()
                        .w_full()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .max_w(px(520.0))
                                .px(px(sp.s5))
                                .py(px(sp.s3_5))
                                .rounded(px(18.0))
                                .bg(rgba(0x140A20DF))
                                .border_1()
                                .border_color(c.primary)
                                .shadow(theme::glow(c.accent, 14.0))
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
                                                .text_size(px(10.5))
                                                .font_family(theme::FONT_DISPLAY)
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(c.accent)
                                                .child(format!("USAMI · {expr_label}")),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.5))
                                        .line_height(px(19.0))
                                        .font_family(theme::FONT_BODY)
                                        .text_color(c.pearl)
                                        .child(reply),
                                ),
                        ),
                )
            })
    }
}
