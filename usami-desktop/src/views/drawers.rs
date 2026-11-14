//! Painel lateral secundário para animações VRMA e Central de Skills.

use crate::model::ActiveDrawer;
use crate::theme;
use crate::ui::widgets::icon;
use crate::UsamiApp;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl UsamiApp {
    pub(crate) fn render_side_drawer(&mut self, viewport_h: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let active_drawer = self.active_drawer;

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
                        })
    }
}
