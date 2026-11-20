//! Camadas do palco 3D e cenário de fundo do desktop (Camadas 0, 1 e 2).

use crate::UsamiApp;
use crate::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl UsamiApp {
    pub(crate) fn render_stage(&self, t: &theme::Theme) -> impl IntoElement {
        let c = t.color;
        let bg_asset = self.bg_asset_path.clone();
        let is_companion = self.is_companion_mode;
        let vrm_entity = self.vrm_view.clone();

        div()
            .absolute()
            .inset_0()
            .size_full()
            // ── LAYER 0: CENÁRIO DA CIDADE / QUARTO À NOITE (BACKDROP) ──
            .when_some(bg_asset, |parent, path| {
                parent.child(
                    img(path)
                        .object_fit(ObjectFit::Cover)
                        .size_full()
                        .absolute()
                        .inset_0(),
                )
            })
            // ── LAYER 1: TINGIMENTO DEEP BLUE DO PALCO ──
            .child(div().absolute().inset_0().bg(t.stage_overlay()))
            .when(is_companion, |parent| {
                parent.child(div().absolute().inset_0().bg(c.companion_overlay))
            })
            // ── LAYER 2: SHIRO VRM 3D (RESPONSIVO FULLSCREEN, ANCORADA NO PISO) ──
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(if let Some(view) = vrm_entity {
                        view.into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}
