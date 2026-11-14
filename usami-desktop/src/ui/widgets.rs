//! Componentes visuais atômicos de interface reutilizáveis.

use crate::assets::asset_path;
use crate::theme;
use gpui::*;

pub(crate) fn icon(name: &str, size_px: f32) -> impl IntoElement {
    img(asset_path(&format!("assets/icons/{name}.svg")))
        .size(px(size_px))
        .flex_shrink_0()
}

// ── Peças do painel de configurações ──

pub(crate) fn settings_h3(label: &'static str) -> impl IntoElement {
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

pub(crate) fn settings_p(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1_5))
        .mb(px(t.space.s3))
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.muted)
        .child(text.into())
}

pub(crate) fn settings_hint(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1))
        .text_size(px(t.text.xs))
        .line_height(px(16.5))
        .text_color(t.color.muted)
        .child(text.into())
}

pub(crate) fn settings_code(text: &'static str) -> impl IntoElement {
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

pub(crate) fn settings_danger_button(label: &'static str) -> impl IntoElement {
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

pub(crate) fn settings_dl_row(key: &'static str, value: impl Into<SharedString>) -> impl IntoElement {
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

pub(crate) fn settings_list_item(text: &'static str) -> impl IntoElement {
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
