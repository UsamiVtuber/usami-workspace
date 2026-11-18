//! Layout do modo mascote: a janela vira só o avatar e o balão, sobre o desktop.

use crate::UsamiApp;
use crate::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl UsamiApp {
    pub(crate) fn render_mascot(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let reply = self.last_reply.clone();
        let show_bubble = self.show_bubble && !reply.is_empty();

        div()
            .id("mascot-root")
            .relative()
            .size_full()
            .bg(rgba(0x00000000))
            .text_color(c.pearl)
            .font_family(theme::FONT_BODY)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, _cx| {
                    this.mascot_mouse_down();
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &MouseMoveEvent, _window, _cx| {
                    this.mascot_mouse_move(event);
                }),
            )
            // Use UFCS to call on_click via the trait
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                this.mascot_click(event, window, cx);
            }))
            // ── AVATAR (portrait, ancorado no chão da janelinha) ──
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_end()
                    .justify_center()
                    .when_some(self.vrm_view.clone(), |parent, view| parent.child(view)),
            )
            // ── BALÃO / DICA NO TOPO ──
            .child(
                div()
                    .absolute()
                    .top(px(10.0))
                    .left(px(10.0))
                    .right(px(10.0))
                    .flex()
                    .justify_center()
                    .when(show_bubble, |parent| {
                        parent.child(
                            div()
                                .w_full()
                                .rounded(px(16.0))
                                .bg(rgba(0x140A20E6))
                                .border_1()
                                .border_color(c.primary)
                                .shadow(theme::glow(c.accent, 12.0))
                                .px(px(14.0))
                                .py(px(10.0))
                                .text_size(px(13.0))
                                .line_height(px(18.0))
                                .child(div().w_full().child(reply)),
                        )
                    })
                    .when(!show_bubble, |parent| {
                        parent.child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(999.0))
                                .bg(rgba(0x20102FB0))
                                .text_size(px(10.0))
                                .text_color(c.lavender)
                                .child("Clique para falar · duplo clique para voltar"),
                        )
                    }),
            )
    }
}
