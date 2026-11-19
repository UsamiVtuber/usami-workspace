//! Layout do modo mascote: a janela vira só o avatar, o balão, a barra de
//! mensagem e os painéis abertos pelo clique direito.

use crate::UsamiApp;
use crate::theme;
use crate::ui::widgets::icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

const MENU_WIDTH: f32 = 210.0;
const MENU_HEIGHT: f32 = 172.0;

impl UsamiApp {
    pub(crate) fn render_mascot(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let reply = self.last_reply.clone();
        let input_text = self.input_text.clone();
        let is_busy = self.is_busy;
        let is_listening = self.is_listening;
        let live_transcript = self.live_transcript.clone();
        let active_tool = self.active_tool.clone();
        let input_active = self.input_active;
        let has_text = !input_text.trim().is_empty();
        let follow = self.prefs.mascot_follow;
        let menu_at = self.mascot_menu_at;
        let settings_open = self.mascot_settings_open;
        let reply_visible = self.show_bubble && !reply.is_empty();

        // Enquanto o turno roda, o balão não pode mostrar a resposta anterior.
        let bubble_text: Option<SharedString> = if is_busy {
            Some(match active_tool.as_deref() {
                Some(tool) => format!("Executando {tool}…").into(),
                None => "Pensando…".into(),
            })
        } else if is_listening {
            Some(if live_transcript.trim().is_empty() {
                "Ouvindo…".into()
            } else {
                live_transcript.clone()
            })
        } else if reply_visible {
            Some(reply.clone())
        } else {
            None
        };

        let focus_handle = self.focus_handle.clone();

        div()
            .id("mascot-root")
            .relative()
            .size_full()
            .bg(rgba(0x00000000))
            .text_color(c.pearl)
            .font_family(theme::FONT_BODY)
            .cursor_pointer()
            .when(!settings_open, |el| {
                el.track_focus(&focus_handle).on_key_down(cx.listener(
                    |this, event: &KeyDownEvent, _window, cx| {
                        this.handle_key_down(event, cx);
                    },
                ))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, _cx| {
                    this.mascot_mouse_down();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.mascot_open_menu(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &MouseMoveEvent, _window, _cx| {
                    this.mascot_mouse_move(event);
                }),
            )
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
                    .when_some(bubble_text.clone(), |parent, text| {
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
                                .child(div().w_full().child(text)),
                        )
                    })
                    .when(bubble_text.is_none(), |parent| {
                        parent.child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(999.0))
                                .bg(rgba(0x20102FB0))
                                .text_size(px(10.0))
                                .text_color(c.lavender)
                                .child("Direito: menu · duplo: voltar"),
                        )
                    }),
            )
            // ── BARRA DE MENSAGEM ──
            .child(
                div()
                    .absolute()
                    .bottom(px(8.0))
                    .left(px(10.0))
                    .right(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .id("mascot-mic")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(36.0))
                            .flex_shrink_0()
                            .rounded_full()
                            .border_1()
                            .border_color(if is_listening { c.primary } else { c.line })
                            .bg(if is_listening { c.icon_bg_active } else { c.icon_bg })
                            .text_size(px(15.0))
                            .hover(|s| s.bg(c.icon_bg_hover))
                            .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_listening(cx);
                                cx.stop_propagation();
                            }))
                            .child("🎤"),
                    )
                    .child(
                        div()
                            .id("mascot-input")
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(36.0))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(rgba(0x140A20E6))
                            .border_1()
                            .border_color(if input_active { c.primary } else { c.line })
                            .px(px(12.0))
                            .text_size(px(12.5))
                            .text_color(if input_text.is_empty() { c.faint } else { c.pearl })
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.input_active = true;
                                this.caret_on = true;
                                this.touch_ui(cx);
                                window.focus(&this.focus_handle);
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(if input_text.is_empty() {
                                "Manda pra ela…".to_string()
                            } else {
                                input_text.clone()
                            }),
                    )
                    .child(
                        div()
                            .id("mascot-send")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(36.0))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(c.primary)
                            .text_color(rgba(0xffffffff))
                            .hover(|s| s.opacity(0.88))
                            .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if !this.input_text.trim().is_empty() {
                                    this.submit(cx);
                                } else {
                                    this.toggle_listening(cx);
                                }
                                cx.stop_propagation();
                            }))
                            .child(if has_text {
                                icon("send", 15.0).into_any_element()
                            } else {
                                icon("volume", 15.0).into_any_element()
                            }),
                    ),
            )
            // ── MENU DE CONTEXTO (clique direito) ──
            .when_some(menu_at, |parent, at| {
                let (x, y) = crate::mascot::menu_position(
                    f32::from(at.x),
                    f32::from(at.y),
                    MENU_WIDTH,
                    MENU_HEIGHT,
                    crate::mascot::MASCOT_WIDTH,
                    crate::mascot::MASCOT_HEIGHT,
                    56.0,
                );

                parent
                    .child(
                        div()
                            .id("mascot-menu-backdrop")
                            .occlude()
                            .absolute()
                            .inset_0()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    this.mascot_close_menu(cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    this.mascot_close_menu(cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mascot_close_menu(cx);
                                cx.stop_propagation();
                            })),
                    )
                    .child(
                        div()
                            .id("mascot-menu")
                            .occlude()
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(MENU_WIDTH))
                            .p(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .rounded(px(14.0))
                            .bg(rgba(0x140A20F2))
                            .border_1()
                            .border_color(c.line_strong)
                            .shadow(theme::glow(c.primary, 16.0))
                            .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                            .on_click(stop_mascot_click)
                            .child(
                                mascot_menu_item(
                                    "mascot-menu-voice",
                                    if is_listening {
                                        "Parar de ouvir"
                                    } else {
                                        "Falar por voz"
                                    },
                                    "volume",
                                    Some(is_listening),
                                    &c,
                                )
                                .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.toggle_listening(cx);
                                    this.mascot_close_menu(cx);
                                    cx.stop_propagation();
                                })),
                            )
                            .child(
                                mascot_menu_item(
                                    "mascot-menu-follow",
                                    "Seguir o cursor",
                                    "recenter",
                                    Some(follow),
                                    &c,
                                )
                                .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.mascot_toggle_follow(cx);
                                    this.mascot_close_menu(cx);
                                    cx.stop_propagation();
                                })),
                            )
                            .child(
                                mascot_menu_item(
                                    "mascot-menu-settings",
                                    "Configurações",
                                    "gear",
                                    None,
                                    &c,
                                )
                                .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.mascot_open_settings(cx);
                                    cx.stop_propagation();
                                })),
                            )
                            .child(
                                mascot_menu_item(
                                    "mascot-menu-back",
                                    "Voltar ao app",
                                    "close",
                                    None,
                                    &c,
                                )
                                .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.exit_mascot(window, cx);
                                    cx.stop_propagation();
                                })),
                            ),
                    )
            })
            // ── CONFIGURAÇÕES SOBREPOSTAS ──
            .when(settings_open, |parent| {
                parent.child(
                    div()
                        .id("mascot-settings-overlay")
                        .occlude()
                        .absolute()
                        .inset(px(4.0))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(16.0))
                        .bg(t.vn_modal_bg())
                        .border_1()
                        .border_color(c.line_strong)
                        .shadow(theme::glow(c.primary, 18.0))
                        .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                        .on_click(stop_mascot_click)
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_end()
                                .px(px(8.0))
                                .py(px(6.0))
                                .border_b_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .id("mascot-settings-close")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(26.0))
                                        .rounded_full()
                                        .bg(c.icon_bg)
                                        .hover(|s| s.bg(c.icon_bg_hover))
                                        .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.mascot_close_settings(cx);
                                            cx.stop_propagation();
                                        }))
                                        .child(icon("close", 11.0)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(self.render_settings_page(cx)),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .px(px(10.0))
                                .py(px(8.0))
                                .border_t_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .id("mascot-settings-expand")
                                        .cursor_pointer()
                                        .text_size(px(11.5))
                                        .text_color(c.primary)
                                        .hover(|s| s.text_color(c.pearl))
                                        .on_mouse_down(MouseButton::Left, stop_mascot_mouse_down)
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                this.mascot_open_settings_in_app(window, cx);
                                                cx.stop_propagation();
                                            },
                                        ))
                                        .child("Abrir no app completo ↗"),
                                ),
                        ),
                )
            })
    }
}

/// Cliques nos controles do mascote não podem borbulhar para o root: ele
/// inicia o arrasto e dispara a cutucada.
fn stop_mascot_mouse_down(_: &MouseDownEvent, _: &mut Window, cx: &mut App) {
    cx.stop_propagation();
}

fn stop_mascot_click(_: &ClickEvent, _: &mut Window, cx: &mut App) {
    cx.stop_propagation();
}

fn mascot_menu_item(
    id: &'static str,
    label: &'static str,
    icon_name: &'static str,
    active: Option<bool>,
    c: &theme::Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(36.0))
        .px(px(10.0))
        .rounded(px(8.0))
        .text_size(px(12.5))
        .text_color(c.pearl)
        .hover(|s| s.bg(c.icon_bg_hover))
        .child(icon(icon_name, 14.0))
        .child(div().flex_1().child(label))
        .when(active.is_some_and(|on| on), |el| {
            el.child(div().text_color(c.primary).child("✓"))
        })
}
