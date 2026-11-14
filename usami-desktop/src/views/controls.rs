use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::text_input::tokenize_text_spans;
use crate::theme;
use crate::{icon, ActiveDrawer, UsamiApp};

impl UsamiApp {
    pub(crate) fn render_bottom_controls(
        &mut self,
        is_idle: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let is_busy = self.is_busy;
        let is_calling = self.is_calling;
        let is_listening = self.is_listening;
        let is_muted = self.is_muted;
        let is_companion = self.is_companion_mode;
        let show_bubble = self.show_bubble;
        let last_reply = self.last_reply.clone();
        let input_text = self.input_text.clone();
        let input_active = self.input_active;
        let caret_on = self.caret_on;
        let caret_visible = input_active && caret_on;
        let cursor_pos = self.cursor_idx();
        let sel_range = self.selection_range();
        let input_tokens = tokenize_text_spans(&input_text);
        let total_input_chars = input_text.chars().count();
        let has_media = self.attached_media.is_some();
        let attached_media = self.attached_media.clone();
        let send_enabled = !is_busy && (!input_text.trim().is_empty() || has_media);
        let chat_open = self.active_drawer == ActiveDrawer::Chat;

        div()
            .absolute()
            .bottom(if is_idle { px(12.0) } else { px(sp.s5) })
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(9.0))
            // Legenda da Usami no fluxo vertical: nunca colide com o
            // disclaimer nem com o composer quando o texto cresce.
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .id("speech-caption")
                        .max_w(px(620.0))
                        .px(px(sp.s4))
                        .py(px(sp.s2_5))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_bubble = false;
                            this.poke(cx);
                            cx.notify();
                        }))
                        .text_center()
                        .text_size(px(ty.bubble))
                        .font(theme::font_character_italic())
                        .line_height(px(26.0))
                        .text_color(c.pearl)
                        .child(last_reply),
                )
            })
            // Disclaimer (sempre presente; o idle esmaece junto, como na web)
            .child(
                div()
                    .opacity(if is_idle { 0.15 } else { 1.0 })
                    .text_size(px(ty.xs))
                    .text_color(c.faint)
                    .child("Mensagens usam os provedores configurados para responder."),
            )
            // Prévia de imagem anexada acima da barra de texto (estilo chat moderno)
            .when_some(attached_media.as_ref(), |parent, media| {
                parent.child(
                    div()
                        .id("composer-media-preview-row")
                        .opacity(if is_idle { 0.15 } else { 1.0 })
                        .w(px(640.0))
                        .max_w(px(640.0))
                        .pl(px(sp.s2))
                        .flex()
                        .items_center()
                        .justify_start()
                        .child(
                            div()
                                .relative()
                                .size(px(64.0))
                                .rounded(px(10.0))
                                .overflow_hidden()
                                .bg(c.composer_bg)
                                .border_1()
                                .border_color(c.primary)
                                .shadow(t.shadow.composer.clone())
                                .child(
                                    img(media.preview.clone())
                                        .size_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                                .child(
                                    div()
                                        .id("composer-media-remove-btn")
                                        .cursor_pointer()
                                        .absolute()
                                        .top(px(4.0))
                                        .right(px(4.0))
                                        .size(px(18.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(gpui::rgba(0x0B0C14D9))
                                        .border_1()
                                        .border_color(c.line_strong)
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.err).border_color(c.err))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.attached_media = None;
                                            cx.notify();
                                        }))
                                        .child(icon("close", 10.0)),
                                ),
                        ),
                )
            })
            // Compositor estilo VN
            .child(
                div()
                    .id("composer-bar")
                    .opacity(if is_idle { 0.15 } else { 1.0 })
                    .w(px(720.0))
                    .max_w(px(760.0))
                    .min_h(px(48.0))
                    .max_h(px(140.0))
                    .flex()
                    .items_end()
                    .gap(px(sp.s1_5))
                    .pt(px(sp.s1_5))
                    .pb(px(sp.s1_5))
                    .pl(px(sp.s3))
                    .pr(px(sp.s2))
                    .rounded(px(r.composer))
                    .bg(c.composer_bg)
                    .overflow_hidden()
                    .border_1()
                    .border_color(if input_active {
                        theme::Palette::vn_border_bright()
                    } else {
                        c.line_strong
                    })
                    .shadow(if input_active {
                        t.shadow.composer_focus.clone()
                    } else {
                        t.shadow.composer.clone()
                    })
                    .on_mouse_move(cx.listener(|this, _, _, cx| {
                        this.poke(cx);
                    }))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, _cx| {
                            this.is_mouse_selecting = false;
                        }),
                    )
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
                            .bg(if has_media {
                                c.accent_tint
                            } else {
                                gpui::rgba(0x00000000)
                            })
                            .text_color(if has_media { c.primary } else { c.muted })
                            .hover(|style| style.bg(c.call_bg_hover).text_color(c.pearl))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.attach_file_prompt(cx);
                            }))
                            .child(icon("image", 18.0)),
                    )
                    // Campo de texto expansível multiline com seleção precisa de fragmentos
                    .child(
                        div()
                            .id("input-box")
                            .flex()
                            .flex_col()
                            // flex_1() = grow 1 + shrink 1 + basis 0: ocupa a
                            // largura restante e pode encolher até o limite da barra.
                            .flex_1()
                            .min_w(px(0.0))
                            // min_h(0) + pai com max_h: a caixa encolhe até o limite
                            // do modal e ativa a rolagem em vez de estourar.
                            .min_h(px(0.0))
                            // 5 linhas de 22px + py(6): o limite cai na
                            // fronteira da linha, sem glifo pela metade.
                            .max_h(px(122.0))
                            .overflow_y_scroll()
                            .px(px(sp.s1_5))
                            .py(px(sp.s1_5))
                            .cursor_text()
                            .on_mouse_up(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, _cx| {
                                    this.is_mouse_selecting = false;
                                }),
                            )
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
                                    .flex_shrink_0()
                                    .w_full()
                                    .min_w(px(0.0))
                                    .items_center()
                                    .when(input_text.is_empty(), |row| {
                                        row.when(input_active, |row| {
                                            row.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .w(px(2.0))
                                                    .h(px(16.0))
                                                    .mr(px(2.0))
                                                    .bg(if caret_visible {
                                                        theme::Palette::vn_pink_neon()
                                                    } else {
                                                        gpui::rgba(0x00000000)
                                                    }),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_size(px(ty.input))
                                                .text_color(c.faint)
                                                .truncate()
                                                .child(if is_calling {
                                                    "Escreva enquanto fala..."
                                                } else if has_media {
                                                    "Comentário opcional..."
                                                } else {
                                                    "Diga alguma coisa..."
                                                }),
                                        )
                                    })
                                    .when(!input_text.is_empty(), |row| {
                                        let make_caret = |visible: bool| {
                                            div()
                                                .flex_shrink_0()
                                                .w(px(2.0))
                                                .h(px(16.0))
                                                .rounded(px(1.0))
                                                .mx(px(0.5))
                                                .bg(if visible {
                                                    theme::Palette::vn_pink_neon()
                                                } else {
                                                    gpui::rgba(0x00000000)
                                                })
                                        };
                                        let mut sel_highlight = theme::Palette::vn_pink_neon();
                                        sel_highlight.a = 0.38;

                                        row.child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .items_center()
                                                .gap_y(px(2.0))
                                                .w_full()
                                                .min_w(px(0.0))
                                                .text_size(px(ty.input))
                                                .line_height(px(22.0))
                                                .text_color(c.pearl)
                                                .on_mouse_up(
                                                    gpui::MouseButton::Left,
                                                    cx.listener(|this, _, _, _cx| {
                                                        this.is_mouse_selecting = false;
                                                    }),
                                                )
                                                .children(input_tokens.iter().enumerate().flat_map(
                                                    |(token_i, token)| {
                                                        let mut els: Vec<AnyElement> = Vec::new();

                                                        if token.is_whitespace {
                                                            let t_chars: Vec<char> =
                                                                token.text.chars().collect();
                                                            for (offset, ch) in
                                                                t_chars.into_iter().enumerate()
                                                            {
                                                                let char_idx =
                                                                    token.start_char_idx + offset;
                                                                if ch == '\n' {
                                                                    if sel_range.is_none()
                                                                        && cursor_pos == char_idx
                                                                    {
                                                                        els.push(
                                                                            make_caret(
                                                                                caret_visible,
                                                                            )
                                                                            .into_any_element(),
                                                                        );
                                                                    }
                                                                    els.push(
                                                                        div()
                                                                            .w_full()
                                                                            .h(px(0.0))
                                                                            .into_any_element(),
                                                                    );
                                                                } else {
                                                                    let is_selected = sel_range
                                                                        .map_or(false, |(s, e)| {
                                                                            char_idx >= s
                                                                                && char_idx < e
                                                                        });
                                                                    let mut ws_el = div()
                                                                        .id(("ws", char_idx))
                                                                        .flex_shrink_0()
                                                                        .flex()
                                                                        .items_center()
                                                                        .h(px(20.0))
                                                                        .min_w(px(5.5))
                                                                        .cursor_text()
                                                                        .when(is_selected, |el| {
                                                                            el.bg(sel_highlight)
                                                                                .rounded(px(2.0))
                                                                        })
                                                                        .on_mouse_down(
                                                                            gpui::MouseButton::Left,
                                                                            cx.listener(
                                                                                move |this,
                                                                                      event: &MouseDownEvent,
                                                                                      window,
                                                                                      cx| {
                                                                                    this.poke(cx);
                                                                                    this.input_active =
                                                                                        true;
                                                                                    this.caret_on =
                                                                                        true;
                                                                                    this.is_mouse_selecting =
                                                                                        true;
                                                                                    this.drag_start_idx =
                                                                                        char_idx;
                                                                                    if event.modifiers.shift {
                                                                                        if this.selection_anchor.is_none() {
                                                                                            this.selection_anchor = Some(this.cursor_char_idx);
                                                                                        }
                                                                                    } else {
                                                                                        this.selection_anchor = None;
                                                                                    }
                                                                                    this.cursor_char_idx =
                                                                                        char_idx;
                                                                                    window.focus(
                                                                                        &this.focus_handle,
                                                                                    );
                                                                                    cx.notify();
                                                                                },
                                                                            ),
                                                                        )
                                                                        .on_mouse_move(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                if this.is_mouse_selecting {
                                                                                    this.selection_anchor =
                                                                                        Some(
                                                                                            this.drag_start_idx,
                                                                                        );
                                                                                    this.cursor_char_idx =
                                                                                        char_idx;
                                                                                    this.caret_on =
                                                                                        true;
                                                                                    cx.notify();
                                                                                }
                                                                            },
                                                                        ))
                                                                        .on_mouse_up(
                                                                            gpui::MouseButton::Left,
                                                                            cx.listener(
                                                                                |this, _, _, _cx| {
                                                                                    this.is_mouse_selecting =
                                                                                        false;
                                                                                },
                                                                            ),
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            move |this,
                                                                                  event: &ClickEvent,
                                                                                  window,
                                                                                  cx| {
                                                                                this.poke(cx);
                                                                                this.input_active =
                                                                                    true;
                                                                                this.caret_on =
                                                                                    true;
                                                                                let count = match event {
                                                                                    ClickEvent::Mouse(m) => m.down.click_count,
                                                                                    _ => 1,
                                                                                };
                                                                                if count >= 3 {
                                                                                    this.selection_anchor = Some(0);
                                                                                    this.cursor_char_idx = this.input_text.chars().count();
                                                                                } else if count == 2 {
                                                                                    this.select_word_at(char_idx);
                                                                                } else {
                                                                                    this.cursor_char_idx = char_idx;
                                                                                    this.selection_anchor = None;
                                                                                }
                                                                                window.focus(&this.focus_handle);
                                                                                cx.notify();
                                                                            },
                                                                        ));

                                                                    if sel_range.is_none()
                                                                        && cursor_pos == char_idx
                                                                    {
                                                                        ws_el = ws_el.child(
                                                                            make_caret(
                                                                                caret_visible,
                                                                            ),
                                                                        );
                                                                    }
                                                                    els.push(
                                                                        ws_el.into_any_element(),
                                                                    );
                                                                }
                                                            }
                                                        } else {
                                                            let t_chars: Vec<char> =
                                                                token.text.chars().collect();
                                                            let mut word_el = div()
                                                                .id(("word-group", token_i))
                                                                .flex()
                                                                .items_center()
                                                                .flex_shrink_0();

                                                            for (offset, ch) in
                                                                t_chars.into_iter().enumerate()
                                                            {
                                                                let char_idx =
                                                                    token.start_char_idx + offset;
                                                                let is_selected = sel_range
                                                                    .map_or(false, |(s, e)| {
                                                                        char_idx >= s
                                                                            && char_idx < e
                                                                    });
                                                                let mut char_el = div()
                                                                    .id(("char", char_idx))
                                                                    .flex_shrink_0()
                                                                    .flex()
                                                                    .items_center()
                                                                    .h(px(20.0))
                                                                    .cursor_text()
                                                                    .when(is_selected, |el| {
                                                                        el.bg(sel_highlight)
                                                                            .rounded(px(2.0))
                                                                    })
                                                                    .on_mouse_down(
                                                                        gpui::MouseButton::Left,
                                                                        cx.listener(
                                                                            move |this,
                                                                                  event: &MouseDownEvent,
                                                                                  window,
                                                                                  cx| {
                                                                                this.poke(cx);
                                                                                this.input_active =
                                                                                    true;
                                                                                this.caret_on =
                                                                                    true;
                                                                                this.is_mouse_selecting =
                                                                                    true;
                                                                                this.drag_start_idx =
                                                                                    char_idx;
                                                                                if event.modifiers.shift {
                                                                                    if this.selection_anchor.is_none() {
                                                                                        this.selection_anchor = Some(this.cursor_char_idx);
                                                                                    }
                                                                                } else {
                                                                                    this.selection_anchor = None;
                                                                                }
                                                                                this.cursor_char_idx =
                                                                                    char_idx;
                                                                                window.focus(
                                                                                    &this.focus_handle,
                                                                                );
                                                                                cx.notify();
                                                                            },
                                                                        ),
                                                                    )
                                                                    .on_mouse_move(cx.listener(
                                                                        move |this, _, _, cx| {
                                                                            if this.is_mouse_selecting {
                                                                                this.selection_anchor =
                                                                                    Some(
                                                                                        this.drag_start_idx,
                                                                                    );
                                                                                this.cursor_char_idx =
                                                                                    char_idx;
                                                                                this.caret_on =
                                                                                    true;
                                                                                cx.notify();
                                                                            }
                                                                        },
                                                                    ))
                                                                    .on_mouse_up(
                                                                        gpui::MouseButton::Left,
                                                                        cx.listener(
                                                                            |this, _, _, _cx| {
                                                                                this.is_mouse_selecting =
                                                                                    false;
                                                                            },
                                                                        ),
                                                                    )
                                                                    .on_click(cx.listener(
                                                                        move |this,
                                                                              event: &ClickEvent,
                                                                              window,
                                                                              cx| {
                                                                            this.poke(cx);
                                                                            this.input_active =
                                                                                true;
                                                                            this.caret_on = true;
                                                                            let count = match event {
                                                                                ClickEvent::Mouse(m) => m.down.click_count,
                                                                                _ => 1,
                                                                            };
                                                                            if count >= 3 {
                                                                                this.selection_anchor = Some(0);
                                                                                this.cursor_char_idx = this.input_text.chars().count();
                                                                            } else if count == 2 {
                                                                                this.select_word_at(char_idx);
                                                                            } else {
                                                                                this.cursor_char_idx = char_idx;
                                                                                this.selection_anchor = None;
                                                                            }
                                                                            window.focus(&this.focus_handle);
                                                                            cx.notify();
                                                                        },
                                                                    ));

                                                                if sel_range.is_none()
                                                                    && cursor_pos == char_idx
                                                                {
                                                                    char_el = char_el.child(
                                                                        make_caret(caret_visible),
                                                                    );
                                                                }
                                                                char_el = char_el.child(
                                                                    div()
                                                                        .px(px(0.5))
                                                                        .text_color(if is_selected {
                                                                            gpui::rgba(0xffffffff)
                                                                        } else {
                                                                            c.pearl
                                                                        })
                                                                        .child(ch.to_string()),
                                                                );
                                                                word_el = word_el.child(char_el);
                                                            }
                                                            els.push(word_el.into_any_element());
                                                        }

                                                        els
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .id("char-end")
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .h(px(20.0))
                                                        .min_w(px(10.0))
                                                        .cursor_text()
                                                        .when(
                                                            sel_range.is_none()
                                                                && cursor_pos >= total_input_chars,
                                                            |el| el.child(make_caret(caret_visible)),
                                                        )
                                                        .on_mouse_down(
                                                            gpui::MouseButton::Left,
                                                            cx.listener(
                                                                move |this,
                                                                      event: &MouseDownEvent,
                                                                      window,
                                                                      cx| {
                                                                    this.poke(cx);
                                                                    this.input_active = true;
                                                                    this.caret_on = true;
                                                                    this.is_mouse_selecting = true;
                                                                    this.drag_start_idx =
                                                                        total_input_chars;
                                                                    if event.modifiers.shift {
                                                                        if this.selection_anchor.is_none() {
                                                                            this.selection_anchor = Some(this.cursor_char_idx);
                                                                        }
                                                                    } else {
                                                                        this.selection_anchor = None;
                                                                    }
                                                                    this.cursor_char_idx =
                                                                        total_input_chars;
                                                                    window.focus(&this.focus_handle);
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                        .on_mouse_move(cx.listener(
                                                            move |this, _, _, cx| {
                                                                if this.is_mouse_selecting {
                                                                    this.selection_anchor =
                                                                        Some(this.drag_start_idx);
                                                                    this.cursor_char_idx =
                                                                        total_input_chars;
                                                                    this.caret_on = true;
                                                                    cx.notify();
                                                                }
                                                            },
                                                        ))
                                                        .on_mouse_up(
                                                            gpui::MouseButton::Left,
                                                            cx.listener(|this, _, _, _cx| {
                                                                this.is_mouse_selecting = false;
                                                            }),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this,
                                                                  event: &ClickEvent,
                                                                  window,
                                                                  cx| {
                                                                this.poke(cx);
                                                                this.input_active = true;
                                                                this.caret_on = true;
                                                                let count = match event {
                                                                    ClickEvent::Mouse(m) => {
                                                                        m.down.click_count
                                                                    }
                                                                    _ => 1,
                                                                };
                                                                if count >= 3 {
                                                                    this.selection_anchor = Some(0);
                                                                    this.cursor_char_idx =
                                                                        total_input_chars;
                                                                } else if count == 2 {
                                                                    this.select_word_at(
                                                                        total_input_chars
                                                                            .saturating_sub(1),
                                                                    );
                                                                } else {
                                                                    this.cursor_char_idx =
                                                                        total_input_chars;
                                                                    this.selection_anchor = None;
                                                                }
                                                                window.focus(&this.focus_handle);
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
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
                                        .bg(if is_muted {
                                            c.danger_muted_bg
                                        } else {
                                            c.call_bg
                                        })
                                        .border_1()
                                        .border_color(if is_muted {
                                            c.danger_muted_border
                                        } else {
                                            c.line
                                        })
                                        .text_color(if is_muted {
                                            c.danger_muted
                                        } else {
                                            c.muted
                                        })
                                        .hover(|style| {
                                            style
                                                .bg(c.call_bg_hover)
                                                .border_color(c.line_strong)
                                                .text_color(c.pearl)
                                        })
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.touch_ui(cx);
                                            this.is_muted = !this.is_muted;
                                            this.sync_mic_gate();
                                            this.persist_prefs();
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
                                        .bg(if is_companion {
                                            c.accent_tint
                                        } else {
                                            c.call_bg
                                        })
                                        .border_1()
                                        .border_color(if is_companion {
                                            c.accent_border
                                        } else {
                                            c.line
                                        })
                                        .text_color(if is_companion { c.primary } else { c.muted })
                                        .when(is_companion, |el| {
                                            el.shadow(theme::glow(c.primary, 10.0))
                                        })
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
                                .border_color(if is_listening {
                                    c.success_border
                                } else {
                                    c.line
                                })
                                .when(is_listening, |el| {
                                    el.shadow(theme::glow(c.ok, 14.0)).opacity(1.0)
                                })
                                .when(!is_listening, |el| el.opacity(0.6))
                                .hover(|style| {
                                    style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                })
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
                                .bg(if chat_open {
                                    c.icon_bg_active
                                } else {
                                    c.icon_bg
                                })
                                .border_1()
                                .border_color(if chat_open { c.primary } else { c.line })
                                .when(chat_open, |el| el.shadow(theme::glow(c.primary, 14.0)))
                                .hover(|style| {
                                    style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                })
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
                                .hover(|style| {
                                    style.bg(c.err_dark).shadow(theme::glow(c.err, 14.0))
                                })
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.poke(cx);
                                    this.end_call(cx);
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
                                .bg(if is_muted {
                                    c.icon_bg_active
                                } else {
                                    c.icon_bg
                                })
                                .border_1()
                                .border_color(if is_muted { c.primary } else { c.line })
                                .hover(|style| {
                                    style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                })
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.touch_ui(cx);
                                    this.is_muted = !this.is_muted;
                                    this.sync_mic_gate();
                                    this.persist_prefs();
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
                                .bg(if is_companion {
                                    c.icon_bg_active
                                } else {
                                    c.icon_bg
                                })
                                .border_1()
                                .border_color(if is_companion { c.primary } else { c.line })
                                .hover(|style| {
                                    style.bg(c.icon_bg_hover).border_color(c.line_strong)
                                })
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.poke(cx);
                                    this.toggle_sleep_companion();
                                    cx.notify();
                                }))
                                .child(icon("moon", 19.0)),
                        ),
                )
            })
    }
}
