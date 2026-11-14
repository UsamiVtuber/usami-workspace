//! Header superior do desktop e legenda ao vivo de STT (Camadas 4 e 4b).

use crate::agent_center_model::{pending_confirmations_count, pending_suggestions_count};
use crate::model::ActiveDrawer;
use crate::theme;
use crate::ui::widgets::icon;
use crate::UsamiApp;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl UsamiApp {
    pub(crate) fn render_hud_top(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;

        let active_drawer = self.active_drawer;
        let chat_open = active_drawer == ActiveDrawer::Chat;
        let is_idle = self.is_ui_idle;
        let is_listening = self.is_listening;
        let live_transcript = self.live_transcript.clone();
        let show_live_caption = is_listening && !live_transcript.is_empty();

        let bond_level = self.bond_level;

        div()
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
                                .gap(px(sp.s2_5))
                                .child(
                                    div()
                                        .id("top-bond-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Bond {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Bond {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Bond, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| {
                                            style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                        })
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Bond, cx);
                                        }))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_family(theme::FONT_DISPLAY)
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if active_drawer == ActiveDrawer::Bond {
                                                    c.primary
                                                } else {
                                                    c.pearl
                                                })
                                                .child(format!("Lv{bond_level}")),
                                        ),
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
                                        .hover(|style| {
                                            style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                        })
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Skills, cx);
                                        }))
                                        .child(icon("skills", 16.0))
                                        .when(agent_attention_count > 0, |parent| {
                                            parent.child(
                                                div()
                                                    .absolute()
                                                    .top(px(-2.0))
                                                    .right(px(-2.0))
                                                    .min_w(px(16.0))
                                                    .h(px(16.0))
                                                    .px(px(3.0))
                                                    .rounded_full()
                                                    .bg(c.accent)
                                                    .border_1()
                                                    .border_color(c.panel)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(9.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(if agent_attention_count > 99 {
                                                        "99+".to_string()
                                                    } else {
                                                        agent_attention_count.to_string()
                                                    }),
                                            )
                                        })
                                })
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
                                        .hover(|style| {
                                            style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                        })
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.load_settings_data(cx);
                                            this.phone_view_settings = true;
                                            this.active_drawer = ActiveDrawer::Chat;
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("gear", 16.0)),
                                ),
                        )
                        // Top Right: Créditos e modelo ativo
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
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
                        .child(
                            div()
                                .max_w(px(520.0))
                                .px(px(sp.s4))
                                .py(px(sp.s2))
                                .rounded_full()
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(c.line_strong)
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .size(px(8.0))
                                        .rounded_full()
                                        .bg(c.accent),
                                )
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(live_transcript),
                                ),
                        ),
                )
            })
    }
}
