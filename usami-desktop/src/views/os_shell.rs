//! Gerenciador de janelas flutuantes OS-Style & Conectores Plug-and-Play (Fase 3 — RFC-001).
//!
//! Respeita as restrições conhecidas do GPUI 0.2.2 (`KE-002`):
//!  - `.occlude()` em toda janela flutuante para impedir vazamento de scroll/clique;
//!  - `.id(...)` explícito em containers de scroll;
//!  - largura definida em textos dentro de scroll para evitar colisão em 1 linha.

use crate::theme;
use crate::UsamiApp;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Debug, Clone)]
pub struct FloatingWindowState {
    pub id: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub z_order: u32,
    pub visible: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectorTile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub connected: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct OsShellState {
    pub windows: Vec<FloatingWindowState>,
    pub connectors: Vec<ConnectorTile>,
    next_z: u32,
}

impl Default for OsShellState {
    fn default() -> Self {
        Self {
            windows: vec![FloatingWindowState {
                id: "connectors".into(),
                title: "Conectores Plug-and-Play".into(),
                x: 24.0,
                y: 84.0,
                width: 340.0,
                height: 360.0,
                z_order: 1,
                visible: false,
                minimized: false,
            }],
            connectors: vec![
                ConnectorTile {
                    id: "gmail".into(),
                    name: "Gmail".into(),
                    description: "Leitura, rascunhos e envio via OAuth2 PKCE.".into(),
                    connected: true,
                    enabled: true,
                },
                ConnectorTile {
                    id: "google-calendar".into(),
                    name: "Google Calendar".into(),
                    description: "Agenda, criação de eventos e lembretes.".into(),
                    connected: true,
                    enabled: true,
                },
                ConnectorTile {
                    id: "spotify".into(),
                    name: "Spotify".into(),
                    description: "Controle de faixas e playlists em tempo real.".into(),
                    connected: false,
                    enabled: false,
                },
                ConnectorTile {
                    id: "web-research".into(),
                    name: "Web Research (TinyFish)".into(),
                    description: "Busca semântica e extração em páginas web.".into(),
                    connected: true,
                    enabled: true,
                },
            ],
            next_z: 2,
        }
    }
}

impl OsShellState {
    pub fn focus_window(&mut self, id: &str) {
        let z = self.next_z;
        self.next_z = self.next_z.saturating_add(1);
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.z_order = z;
            w.minimized = false;
        }
    }

    pub fn toggle_window(&mut self, id: &str) {
        let should_focus = if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.visible = !w.visible;
            w.visible
        } else {
            false
        };
        if should_focus {
            self.focus_window(id);
        }
    }

    pub fn set_window_visible(&mut self, id: &str, visible: bool) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.visible = visible;
        }
        if visible {
            self.focus_window(id);
        }
    }

    pub fn toggle_minimized(&mut self, id: &str) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = !w.minimized;
        }
    }

    pub fn toggle_connector(&mut self, id: &str) {
        if let Some(c) = self.connectors.iter_mut().find(|c| c.id == id) {
            c.enabled = !c.enabled;
        }
    }

    pub fn ordered_visible_windows(&self) -> Vec<FloatingWindowState> {
        let mut list: Vec<FloatingWindowState> = self
            .windows
            .iter()
            .filter(|w| w.visible)
            .cloned()
            .collect();
        list.sort_by_key(|w| w.z_order);
        list
    }
}

impl UsamiApp {
    pub(crate) fn render_os_shell_layer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let visible_windows = self.os_shell.ordered_visible_windows();
        let connectors = self.os_shell.connectors.clone();

        div()
            .children(visible_windows.into_iter().map(|win| {
                let win_id = win.id.clone();
                let focus_id = win.id.clone();
                let min_id = win.id.clone();
                let close_id = win.id.clone();
                let body_h = if win.minimized { 0.0 } else { win.height };

                div()
                    .id(SharedString::from(format!("os-win-{win_id}")))
                    .absolute()
                    .left(px(win.x))
                    .top(px(win.y))
                    .w(px(win.width))
                    .rounded(px(14.0))
                    .bg(c.panel)
                    .border_1()
                    .border_color(c.line_strong)
                    .shadow(t.shadow.drawer.clone())
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            this.os_shell.focus_window(&focus_id);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .px(px(sp.s3))
                            .py(px(sp.s2))
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.pearl)
                                    .child(win.title.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("os-min-{min_id}")))
                                            .px(px(7.0))
                                            .py(px(2.0))
                                            .rounded(px(6.0))
                                            .bg(c.surface_soft)
                                            .cursor_pointer()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.os_shell.toggle_minimized(&min_id);
                                                cx.notify();
                                            }))
                                            .child(if win.minimized { "+" } else { "–" }),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("os-close-{close_id}")))
                                            .px(px(7.0))
                                            .py(px(2.0))
                                            .rounded(px(6.0))
                                            .bg(c.surface_soft)
                                            .cursor_pointer()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.os_shell.set_window_visible(&close_id, false);
                                                cx.notify();
                                            }))
                                            .child("×"),
                                    ),
                            ),
                    )
                    .when(!win.minimized, |parent| {
                        parent.child(
                            div()
                                .id(SharedString::from(format!("os-scroll-{win_id}")))
                                .h(px(body_h))
                                .overflow_y_scroll()
                                .p(px(sp.s3))
                                .flex()
                                .flex_col()
                                .gap(px(sp.s2))
                                .children(connectors.iter().cloned().map(|conn| {
                                    let conn_id = conn.id.clone();
                                    let pill_bg = if conn.enabled { c.accent } else { c.surface_soft };
                                    div()
                                        .w(px(win.width - 36.0))
                                        .p(px(sp.s2_5))
                                        .rounded(px(10.0))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(sp.s2))
                                        .child(
                                            div()
                                                .w(px(win.width - 130.0))
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_size(px(ty.sm))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(c.pearl)
                                                        .child(conn.name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.muted)
                                                        .child(format!(
                                                            "{} · {}",
                                                            if conn.connected { "Conectado" } else { "Não conectado" },
                                                            conn.description
                                                        )),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!("conn-toggle-{conn_id}")))
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .rounded_full()
                                                .bg(pill_bg)
                                                .cursor_pointer()
                                                .text_size(px(ty.xs))
                                                .text_color(c.pearl)
                                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                    this.os_shell.toggle_connector(&conn_id);
                                                    cx.notify();
                                                }))
                                                .child(if conn.enabled { "Ativo" } else { "Off" }),
                                        )
                                })),
                        )
                    })
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{FloatingWindowState, OsShellState};

    #[::core::prelude::v1::test]
    fn focus_window_brings_to_top_and_unminimizes() {
        let mut shell = OsShellState::default();
        shell.windows.push(FloatingWindowState {
            id: "logs".into(),
            title: "Logs".into(),
            x: 80.0,
            y: 120.0,
            width: 300.0,
            height: 220.0,
            z_order: 2,
            visible: true,
            minimized: false,
        });
        shell.set_window_visible("connectors", true);
        shell.toggle_minimized("connectors");
        shell.focus_window("connectors");

        let ordered = shell.ordered_visible_windows();
        assert_eq!(ordered.last().map(|w| w.id.as_str()), Some("connectors"));
        assert!(!ordered.last().unwrap().minimized);
    }

    #[::core::prelude::v1::test]
    fn toggle_connector_flips_enabled_state() {
        let mut shell = OsShellState::default();
        let before = shell.connectors.iter().find(|c| c.id == "spotify").unwrap().enabled;
        shell.toggle_connector("spotify");
        let after = shell.connectors.iter().find(|c| c.id == "spotify").unwrap().enabled;
        assert_eq!(after, !before);
    }
}
