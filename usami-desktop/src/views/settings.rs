//! Drawer e tela de Configurações do app.
//!
//! Reúne abas de Perfil, Memória, Personalidade, Proatividade, Sistema e Avançado.

use crate::UsamiApp;
use crate::model::SettingsTab;
use crate::storage;
use crate::theme;
use crate::ui::widgets::{
    settings_code, settings_danger_button, settings_dl_row, settings_h3, settings_hint,
    settings_list_item, settings_p,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

fn render_settings_category<E: IntoElement>(
    title: &'static str,
    subtitle: &'static str,
    content: E,
) -> impl IntoElement + use<E> {
    let t = theme::theme();
    let c = t.color;
    let sp = t.space;
    let ty = t.text;
    div()
        .flex()
        .flex_col()
        .gap(px(sp.s3))
        .rounded(px(t.radius.lg))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(theme::Palette::vn_card_border())
        .border_l_2()
        .p(px(sp.s4))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s1))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(crate::ui::widgets::icon("bunny_bow", 14.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_family(theme::FONT_CHARACTER)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::Palette::vn_pink_soft())
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.faint)
                        .child(subtitle),
                ),
        )
        .child(content)
}

impl UsamiApp {
    #[inline(never)]
    fn render_game_connectors_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
    fn render_vrm_avatar_card(
        &mut self,
        idx: usize,
        model_path: std::path::PathBuf,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;

        let lol_enabled = self.prefs.lol_enabled;
        let lol_role_pref = self.prefs.lol_preferred_role.clone();
        let lol_status = self.lol_status.clone();
        let lol_patch = self.lol_patch.clone();
        let lol_manual_overlay = self.lol_manual_overlay;
        let lol_click_through = self.lol_click_through;
        let lol_in_match = self.lol_in_match;

        let mut cards = div().flex().flex_col().gap(px(sp.s2_5));

        for descriptor in crate::lol::provider::default_game_connectors() {
            if descriptor.id == "lol" {
                let roles: [(&'static str, &'static str); 6] = [
                    ("auto", "Auto"),
                    ("top", "Top"),
                    ("jungle", "Jungle"),
                    ("mid", "Mid"),
                    ("adc", "ADC"),
                    ("support", "Suporte"),
                ];

                let mut role_chips = div().flex().flex_wrap().gap(px(sp.s1_5));
                for (idx, (key, label)) in roles.into_iter().enumerate() {
                    let is_selected = lol_role_pref == key;
                    role_chips = role_chips.child(
                        div()
                            .id(ElementId::NamedInteger("lol-role-chip".into(), idx as u64))
                            .cursor_pointer()
                            .px(px(10.0))
                            .py(px(5.0))
                            .rounded(px(r.sm))
                            .bg(if is_selected {
                                c.accent_tint
                            } else {
                                c.surface_soft
                            })
                            .border_1()
                            .border_color(if is_selected { c.primary } else { c.line })
                            .text_size(px(ty.xs))
                            .font_weight(if is_selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(c.pearl)
                            .hover(|s| s.border_color(c.primary))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.set_lol_preferred_role(key, cx);
                            }))
                            .child(label),
                    );
                }

                let patch_text = lol_patch
                    .as_deref()
                    .map(|p| format!("Patch DataDragon: {p}"))
                    .unwrap_or_else(|| "Patch DataDragon: sincronização automática".to_string());

                cards = cards.child(
                    div()
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(if lol_enabled { c.primary } else { c.line })
                        .flex()
                        .flex_col()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.primary)
                                        .child(descriptor.title),
                                )
                                .child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(if lol_enabled {
                                            theme::Palette::emerald_soft()
                                        } else {
                                            rgba(0x88888822)
                                        })
                                        .border_1()
                                        .border_color(if lol_enabled {
                                            theme::Palette::emerald_border()
                                        } else {
                                            rgba(0x88888866)
                                        })
                                        .text_size(px(ty.xs))
                                        .text_color(if lol_enabled {
                                            theme::Palette::emerald()
                                        } else {
                                            rgb(0x888888)
                                        })
                                        .child(if lol_in_match {
                                            "EM PARTIDA"
                                        } else if lol_enabled {
                                            "ATIVADO"
                                        } else {
                                            "DESATIVADO"
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(ty.sm))
                                .text_color(c.pearl)
                                .child(descriptor.subtitle),
                        )
                        .child(div().text_size(px(ty.xs)).text_color(c.muted).child(
                            SharedString::from(format!("Status: {} | {}", lol_status, patch_text)),
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div().text_size(px(ty.xs)).text_color(c.faint).child(
                                        "Role preferida (quando não detectada automaticamente)",
                                    ),
                                )
                                .child(role_chips),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(sp.s2))
                                .mt(px(sp.s1))
                                .child(
                                    div()
                                        .id("toggle-lol-connector-btn")
                                        .cursor_pointer()
                                        .px(px(12.0))
                                        .py(px(7.0))
                                        .rounded(px(r.sm))
                                        .bg(if lol_enabled {
                                            c.surface_soft
                                        } else {
                                            c.primary
                                        })
                                        .border_1()
                                        .border_color(if lol_enabled { c.line } else { c.primary })
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if lol_enabled { c.pearl } else { c.deep })
                                        .hover(|s| s.opacity(0.9))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_lol_connector(cx);
                                        }))
                                        .child(if lol_enabled {
                                            "Desativar conector"
                                        } else {
                                            "Ativar conector"
                                        }),
                                )
                                .child(
                                    div()
                                        .id("toggle-lol-overlay-preview-btn")
                                        .cursor_pointer()
                                        .px(px(12.0))
                                        .py(px(7.0))
                                        .rounded(px(r.sm))
                                        .bg(if lol_manual_overlay {
                                            c.accent_tint
                                        } else {
                                            c.surface_soft
                                        })
                                        .border_1()
                                        .border_color(if lol_manual_overlay {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_lol_manual_overlay(cx);
                                        }))
                                        .child(if lol_manual_overlay {
                                            "Fechar overlay de teste"
                                        } else {
                                            "Pré-visualizar overlay"
                                        }),
                                )
                                .child(
                                    div()
                                        .id("toggle-lol-clickthrough-btn")
                                        .cursor_pointer()
                                        .px(px(12.0))
                                        .py(px(7.0))
                                        .rounded(px(r.sm))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_lol_click_through(cx);
                                        }))
                                        .child(if lol_click_through {
                                            "Click-through: Ativo"
                                        } else {
                                            "Click-through: Interativo"
                                        }),
                                )
                                .child(
                                    div()
                                        .id("refresh-lol-ddragon-btn")
                                        .cursor_pointer()
                                        .px(px(12.0))
                                        .py(px(7.0))
                                        .rounded(px(r.sm))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.refresh_lol_ddragon(cx);
                                        }))
                                        .child("Atualizar DataDragon"),
                                ),
                        ),
                );
            } else {
                cards = cards.child(
                    div()
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.pearl)
                                        .child(descriptor.title),
                                )
                                .child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .text_size(px(10.0))
                                        .text_color(c.faint)
                                        .child("EM BREVE"),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .text_color(c.muted)
                                .child(descriptor.subtitle),
                        ),
        let display_name = crate::assets::vrm_display_name(&model_path);
        let file_name = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("modelo.vrm")
            .to_string();

        div()
            .id(ElementId::NamedInteger("vrm-card".into(), idx as u64))
            .cursor_pointer()
            .w(px(190.0))
            .p(px(sp.s3))
            .rounded(px(r.md))
            .bg(if is_active { c.accent_tint } else { c.card_bg })
            .border_1()
            .border_color(if is_active { c.primary } else { c.line_strong })
            .hover(|s| s.border_color(c.primary).bg(c.surface_soft))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !is_active && !this.vrm_loading {
                    this.select_vrm_model(model_path.clone(), cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(ty.sm))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.pearl)
                            .child(SharedString::from(display_name)),
                    )
                    .when(is_active, |row| {
                        row.child(
                            div()
                                .px(px(6.0))
                                .py(px(1.5))
                                .rounded(px(4.0))
                                .bg(c.primary)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.deep)
                                .child("ATIVO"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(ty.xs))
                    .text_color(c.faint)
                    .child(SharedString::from(file_name)),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn render_vrm_emotions_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let active_emotion = self.vrm_controller.active_emotion().to_ascii_lowercase();

        let emotions: [(&'static str, &'static str); 8] = [
            ("neutral", "Neutra"),
            ("happy", "Feliz"),
            ("tsundere", "Tsundere"),
            ("shy", "Tímida"),
            ("surprised", "Surpresa"),
            ("sad", "Triste"),
            ("sleepy", "Sonolenta"),
            ("angry", "Brava"),
        ];

        let mut row = div().flex().flex_wrap().gap(px(sp.s2)).mb(px(sp.s3));
        for (idx, (tag, label)) in emotions.into_iter().enumerate() {
            let is_selected = active_emotion == tag || (tag == "neutral" && active_emotion.is_empty());
            row = row.child(
                div()
                    .id(ElementId::NamedInteger("vrm-emotion-btn".into(), idx as u64))
                    .cursor_pointer()
                    .px(px(11.0))
                    .py(px(6.0))
                    .rounded(px(r.sm))
                    .bg(if is_selected { c.accent_tint } else { c.card_bg })
                    .border_1()
                    .border_color(if is_selected { c.primary } else { c.line_strong })
                    .text_size(px(ty.xs))
                    .font_weight(if is_selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(c.pearl)
                    .hover(|s| s.border_color(c.primary).bg(c.surface_soft))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.preview_vrm_emotion(tag, cx);
                    }))
                    .child(label),
            );
        }

        div()
            .flex()
            .flex_col()
            .child(settings_h3("TESTADOR DE EXPRESSÕES FACIAIS"))
            .child(settings_p(
                "Dispare uma expressão para validar os blendshapes do modelo ativo.",
            ))
            .child(row)
            .into_any_element()
    }

    #[inline(never)]
    fn render_vrm_animations_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let clips = self.available_vrmas.clone();
        let active_vrma = self.vrm_controller.active_vrma().to_ascii_lowercase();

        let mut chips = div().flex().flex_wrap().gap(px(sp.s2)).mb(px(sp.s3));
        if clips.is_empty() {
            chips = chips.child(settings_hint(
                "Nenhum arquivo .vrma encontrado na pasta de animações.",
            ));
        } else {
            for (idx, clip_path) in clips.into_iter().enumerate() {
                let label = crate::assets::vrma_display_name(&clip_path);
                let file_stem = clip_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let is_active = !active_vrma.is_empty()
                    && (active_vrma == label.to_ascii_lowercase()
                        || active_vrma == file_stem
                        || active_vrma.ends_with(&file_stem));
                chips = chips.child(
                    div()
                        .id(ElementId::NamedInteger("vrm-anim-btn".into(), idx as u64))
                        .cursor_pointer()
                        .px(px(11.0))
                        .py(px(6.0))
                        .rounded(px(r.sm))
                        .bg(if is_active { c.accent_tint } else { c.card_bg })
                        .border_1()
                        .border_color(if is_active { c.primary } else { c.line_strong })
                        .text_size(px(ty.xs))
                        .text_color(c.pearl)
                        .hover(|s| s.border_color(c.primary).bg(c.surface_soft))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.play_vrm_animation(clip_path.clone(), cx);
                        }))
                        .child(SharedString::from(label)),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .child(settings_h3("CONECTORES DE JOGOS (NO-CODE)"))
            .child(settings_p(
                "Ative ou configure integrações de jogos sem editar arquivos .env. Novos jogos seguem o contrato GameProvider.",
            ))
            .child(cards)
            .into_any_element()
    }
            .child(settings_h3("CATÁLOGO DE ANIMAÇÕES (.VRMA)"))
            .child(settings_p(
                "Clipes detectados automaticamente na pasta de animações. Clique para executar no avatar.",
            ))
            .child(chips)
            .into_any_element()
    }

    #[inline(never)]
    fn render_vrm_camera_and_arms_section(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let preset = self.vrm_camera_preset;
        let arm_deg = self.prefs.vrm_arm_clearance_deg;

        let presets: [(&'static str, &'static str); 3] = [
            ("closeup", "Close-up (Rosto)"),
            ("bust", "Busto"),
            ("full", "Corpo Inteiro"),
        ];

        let mut preset_row = div().flex().flex_wrap().gap(px(sp.s2)).mb(px(sp.s2_5));
        for (idx, (key, label)) in presets.into_iter().enumerate() {
            let is_active = preset == key;
            preset_row = preset_row.child(
                div()
                    .id(ElementId::NamedInteger("vrm-cam-preset".into(), idx as u64))
                    .cursor_pointer()
                    .px(px(11.0))
                    .py(px(6.0))
                    .rounded(px(r.sm))
                    .bg(if is_active { c.accent_tint } else { c.surface_soft })
                    .border_1()
                    .border_color(if is_active { c.primary } else { c.line })
                    .text_size(px(ty.xs))
                    .font_weight(if is_active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(c.pearl)
                    .hover(|s| s.border_color(c.primary))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.apply_vrm_camera_preset(key, cx);
                    }))
                    .child(label),
            );
        }

        let arm_presets: [f32; 5] = [0.0, 8.0, 16.0, 24.0, 32.0];
        let mut arm_row = div().flex().flex_wrap().items_center().gap(px(sp.s2));
        for (idx, deg) in arm_presets.into_iter().enumerate() {
            let is_active = (arm_deg - deg).abs() < 0.5;
            arm_row = arm_row.child(
                div()
                    .id(ElementId::NamedInteger("vrm-arm-deg".into(), idx as u64))
                    .cursor_pointer()
                    .px(px(10.0))
                    .py(px(5.0))
                    .rounded(px(r.sm))
                    .bg(if is_active { c.accent_tint } else { c.surface_soft })
                    .border_1()
                    .border_color(if is_active { c.primary } else { c.line })
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .hover(|s| s.border_color(c.primary))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_vrm_arm_clearance_deg(deg, cx);
                    }))
                    .child(SharedString::from(format!("{:.0}°", deg))),
            );
        }

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .p(px(sp.s3))
                    .mb(px(sp.s2))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .child(
                        div()
                            .text_size(px(ty.sm))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.pearl)
                            .child("Enquadramento da câmera"),
                    )
                    .child(preset_row),
            )
            .child(
                div()
                    .p(px(sp.s3))
                    .mb(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c.pearl)
                                    .child(SharedString::from(format!(
                                        "Abertura lateral dos braços ({:.0}°)",
                                        arm_deg
                                    ))),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.faint)
                                    .child("Evita que braços atravessem saias ou mangas largas em avatares customizados."),
                            ),
                    )
                    .child(arm_row),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn render_settings_avatar(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;

        let available_vrms = self.available_vrms.clone();
        let current_vrm_canon = self
            .current_vrm_path
            .as_ref()
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()));
        let current_model_label = self
            .current_vrm_path
            .as_ref()
            .map(|p| {
                let name = crate::assets::vrm_display_name(p);
                let file = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("desconhecido");
                format!("{name} ({file})")
            })
            .unwrap_or_else(|| "Nenhum modelo carregado".to_string());
        let vrm_loading = self.vrm_loading;
        let vrm_status = self.vrm_status.clone();
        let vrm_error = self.vrm_error.clone();
        let spring_bones_enabled = self.prefs.vrm_spring_bones;

        let mut cards_row = div().flex().flex_wrap().gap(px(sp.s2_5)).mb(px(sp.s3));
        if available_vrms.is_empty() {
            cards_row = cards_row.child(settings_hint(
                "Nenhum arquivo .vrm encontrado nas pastas padrão. Use o botão abaixo para carregar do disco.",
            ));
        } else {
            for (idx, model_path) in available_vrms.into_iter().enumerate() {
                let path_canon =
                    std::fs::canonicalize(&model_path).unwrap_or_else(|_| model_path.clone());
                let is_active = current_vrm_canon
                    .as_ref()
                    .is_some_and(|cur| cur == &path_canon);
                let card = self.render_vrm_avatar_card(idx, model_path, is_active, cx);
                cards_row = cards_row.child(card);
            }
        }

        let actions_bar = div()
            .flex()
            .flex_wrap()
            .gap(px(sp.s2))
            .mb(px(sp.s3))
            .child(
                div()
                    .id("vrm-pick-custom-btn")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(7.0))
                    .rounded(px(r.sm))
                    .bg(c.primary)
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.deep)
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if !this.vrm_loading {
                            this.pick_custom_vrm_prompt(cx);
                        }
                    }))
                    .child(if vrm_loading {
                        "Carregando modelo..."
                    } else {
                        "Carregar arquivo .vrm"
                    }),
            )
            .child(
                div()
                    .id("vrm-refresh-catalog-btn")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(7.0))
                    .rounded(px(r.sm))
                    .bg(c.surface_soft)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .hover(|s| s.bg(c.line_strong))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.refresh_vrm_catalog(cx);
                    }))
                    .child("Atualizar lista"),
            )
            .into_any_element();

        let physics_row = div()
            .flex()
            .items_center()
            .justify_between()
            .p(px(sp.s3))
            .mb(px(sp.s2))
            .rounded(px(r.md))
            .bg(c.card_bg)
            .border_1()
            .border_color(c.line)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(ty.sm))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.pearl)
                            .child("Física de cabelo e roupa (SpringBones)"),
                    )
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.faint)
                            .child("Simulação Verlet em tempo real para cabelos, fitas e saias."),
                    ),
            )
            .child(
                div()
                    .id("vrm-toggle-spring-bones-btn")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(r.sm))
                    .bg(if spring_bones_enabled {
                        c.accent_tint
                    } else {
                        c.surface_soft
                    })
                    .border_1()
                    .border_color(if spring_bones_enabled {
                        c.primary
                    } else {
                        c.line_strong
                    })
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.pearl)
                    .hover(|s| s.border_color(c.primary))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_vrm_spring_bones(cx);
                    }))
                    .child(if spring_bones_enabled {
                        "Ativada"
                    } else {
                        "Desativada"
                    }),
            )
            .into_any_element();

        let camera_row = div()
            .flex()
            .gap(px(sp.s2))
            .mb(px(sp.s3))
            .child(
                div()
                    .id("vrm-reset-camera-btn")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(r.sm))
                    .bg(c.surface_soft)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .hover(|s| s.bg(c.line_strong))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.reset_vrm_camera(cx);
                    }))
                    .child("Resetar enquadramento da câmera"),
            )
            .into_any_element();

        let mut view = div()
            .flex()
            .flex_col()
            .child(settings_h3("GALERIA DE AVATARES 3D (.VRM)"))
            .child(settings_p(
                "Clique em um avatar para trocar em tempo real no motor WeebUI ou carregue qualquer arquivo .vrm do seu computador.",
            ))
            .child(cards_row.into_any_element())
            .child(actions_bar);

        if let Some(status) = vrm_status {
            view = view.child(
                div()
                    .mb(px(sp.s2_5))
                    .p(px(sp.s2))
                    .rounded(px(r.sm))
                    .bg(c.surface_soft)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .child(status)
                    .into_any_element(),
            );
        }

        if let Some(err) = vrm_error {
            view = view.child(
                div()
                    .mb(px(sp.s2_5))
                    .p(px(sp.s2))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(rgb(0xff8a8a))
                    .child(err)
                    .into_any_element(),
            );
        }

        let emotions_section = self.render_vrm_emotions_section(cx);
        let animations_section = self.render_vrm_animations_section(cx);
        let camera_arms_section = self.render_vrm_camera_and_arms_section(cx);

        view.child(emotions_section)
            .child(animations_section)
            .child(settings_h3("FÍSICA E CÂMERA (WEEBUI)"))
            .child(physics_row)
            .child(camera_arms_section)
            .child(camera_row)
            .child(settings_h3("RENDERIZADOR 3D NATIVO"))
            .child(settings_dl_row("Motor", "WeebUI (gpui-vrm / WGPU 60 FPS)"))
            .child(settings_dl_row("Formato", "VRM 0.x / 1.0 e glTF 2.0"))
            .child(settings_dl_row("Modelo ativo", current_model_label))
            .child(settings_dl_row("Janela", "Transparência nativa + Drag"))
            .child(settings_hint(
                "A escolha do avatar e da física fica salva localmente e é restaurada ao abrir o app.",
            ))
    }

    fn render_settings_section(
        &mut self,
        tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        match tab {
            SettingsTab::Chat => {
                let base_url = self.api_client.base_url();
                let has_token = self.api_client.has_token();
                let proactive_enabled = self.proactive_enabled;

                div()
                    .flex()
                    .flex_col()
                    .child(settings_h3("CONEXÃO COM O BACKEND"))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("MOTOR LOCAL"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(if has_token { theme::Palette::emerald_soft() } else { rgba(0xf0ad4e22) })
                                            .border_1()
                                            .border_color(if has_token { theme::Palette::emerald_border() } else { rgba(0xf0ad4e66) })
                                            .text_size(px(ty.xs))
                                            .text_color(if has_token { theme::Palette::emerald() } else { rgb(0xf0ad4e) })
                                            .child(if has_token { "TOKEN ATIVO" } else { "SEM TOKEN" }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child(SharedString::from(format!("• URL: {}", base_url))),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1))
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(if has_token {
                                        "Autenticado via USAMI_API_TOKEN. Acesso a Memória e Jeito liberado."
                                    } else {
                                        "Defina USAMI_API_TOKEN no ambiente para liberar acesso a Memória e Jeito."
                                    }),
                            ),
                    )
                    .child(settings_h3("PROATIVIDADE E LEMBRETES"))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("EVENTOS PROATIVOS"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(if proactive_enabled { theme::Palette::emerald_soft() } else { rgba(0x88888822) })
                                            .border_1()
                                            .border_color(if proactive_enabled { theme::Palette::emerald_border() } else { rgba(0x88888866) })
                                            .text_size(px(ty.xs))
                                            .text_color(if proactive_enabled { theme::Palette::emerald() } else { rgb(0x888888) })
                                            .child(if proactive_enabled { "ATIVADO" } else { "DESATIVADO" }),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child("Recebe saudações por horário, lembretes do Obsidian e resumos automáticos via API local."),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s3))
                                    .flex()
                                    .child(
                                        div()
                                            .id("toggle-proactive-btn")
                                            .cursor_pointer()
                                            .px(px(14.0))
                                            .py(px(8.0))
                                            .rounded(px(r.sm))
                                            .bg(c.surface_soft)
                                            .border_1()
                                            .border_color(c.line)
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .hover(|s| s.bg(c.line_strong))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                let next = !this.proactive_enabled;
                                                this.set_proactive_enabled(next, cx);
                                            }))
                                            .child(if proactive_enabled { "Desativar proatividade" } else { "Ativar proatividade" }),
                                    ),
                            ),
                    )
                    .child(self.render_game_connectors_section(cx))
            }
            SettingsTab::Voice => {
                let has_token = self.api_client.has_token();
                div()
                    .flex()
                    .flex_col()
                    .child(settings_h3("VOZ OFICIAL"))
                    .child(settings_p(
                        "Gemini 3.8 Flash TTS com a voz desenhada para a Usami. A síntese é executada no servidor local.",
                    ))
                    .child(
                        div()
                            .p(px(sp.s3))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.primary)
                                    .child("TRANSCRIÇÃO DE VOZ (STT)"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s2))
                                    .text_size(px(ty.sm))
                                    .text_color(c.pearl)
                                    .child(if has_token {
                                        "• STT ativo: envio de áudio autenticado via x-usami-token."
                                    } else {
                                        "• STT requer token: defina USAMI_API_TOKEN para habilitar transcrição."
                                    }),
                            ),
                    )
                    .child(settings_h3("EXPRESSÕES SONORAS"))
                    .child(settings_p(
                        "O tom muda por emoção e o texto pode acionar sons curtos:",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(sp.s1_5))
                            .mb(px(sp.s3))
                            .child(settings_code("<laugh>"))
                            .child(settings_code("<chuckle>"))
                            .child(settings_code("<sigh>"))
                            .child(settings_code("<short pause>")),
                    )
                    .child(settings_p(
                        "A tag vira áudio no sintetizador e é ocultada no balão de fala.",
                    ))
                    .child(settings_hint(
                        "Para trocar voz ou modelo, ajuste no usami/.env (GEMINI_TTS_VOICE, GEMINI_TTS_MODEL).",
                    ))
                    .child(settings_h3("PRIVACIDADE"))
                    .child(settings_p(
                        "A chave da API Gemini permanece exclusivamente no servidor local.",
                    ))
            }
            SettingsTab::Avatar => self.render_settings_avatar(cx),
            SettingsTab::Memory => {
                let has_token = self.api_client.has_token();
                let loading = self.memory_loading;
                let error = self.memory_error.clone();
                let status = self.memory_status.clone();
                let confirm_active = self.memory_confirm_active;
                let memory_opt = self.memory_data.clone();
                let server_stage = self.server_stage.clone();
                let stage_str = server_stage.as_ref().map(|s| s.as_ref()).unwrap_or("Pendente");

                let mut view = div().flex().flex_col();

                view = view.child(settings_h3("MEMÓRIA DA USAMI"));

                if !has_token {
                    view = view.child(
                        div()
                            .p(px(sp.s3_5))
                            .rounded(px(r.md))
                            .bg(rgba(0xf0ad4e15))
                            .border_1()
                            .border_color(rgba(0xf0ad4e55))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf0ad4e))
                                    .child("AUTENTICAÇÃO NECESSÁRIA"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1_5))
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child("A leitura e exclusão de memórias exige o token do servidor. Configure a variável de ambiente USAMI_API_TOKEN e abra o app novamente."),
                            ),
                    );
                } else {
                    // Barra de ações de topo
                    let mut top_bar = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(sp.s3));

                    top_bar = top_bar.child(
                        div()
                            .id("reload-memory-btn")
                            .cursor_pointer()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .hover(|s| s.bg(c.line_strong))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_memory(cx);
                            }))
                            .child("Recarregar memória"),
                    );

                    view = view.child(top_bar);

                    if loading {
                        view = view.child(settings_hint("Carregando memórias do servidor..."));
                    }

                    if let Some(err) = error {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .text_color(c.danger_muted)
                                .child(SharedString::from(format!("Erro: {}", err))),
                        );
                    }

                    if let Some(st) = status {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(theme::Palette::emerald_soft())
                                .border_1()
                                .border_color(theme::Palette::emerald_border())
                                .text_size(px(ty.xs))
                                .text_color(theme::Palette::emerald())
                                .child(SharedString::from(st)),
                        );
                    }

                    if let Some(mem) = memory_opt {
                        view = view.child(
                            div()
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.primary)
                                        .child("RESUMO"),
                                )
                                .child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!(
                                            "• {} fato(s) durável(is) · {} recente(s) · {} resumo(s)",
                                            mem.durable_count(),
                                            mem.recent_facts.len(),
                                            mem.summaries.len()
                                        ))),
                                )
                                .child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child(SharedString::from(format!(
                                            "• Afinidade: {}/100 (estágio: {})",
                                            mem.affinity.as_ref().map(|a| a.score).unwrap_or(0),
                                            stage_str
                                        ))),
                                ),
                        );

                        // Espelho dos medidores: a mesma leitura do painel de
                        // Laços, guardada no memory.json pelo core.
                        if let Some(bond) = mem.bond.as_ref() {
                            let meters: [(&'static str, u32, u32, f32); 3] = [
                                ("Carinho", bond.intimacy.carinho, bond.meters.carinho.level, bond.meters.carinho.progress),
                                ("Humor", bond.intimacy.humor, bond.meters.humor.level, bond.meters.humor.progress),
                                ("Companhia", bond.intimacy.companhia, bond.meters.companhia.level, bond.meters.companhia.progress),
                            ];
                            view = view.child(
                                div()
                                    .mt(px(sp.s3))
                                    .p(px(sp.s3))
                                    .rounded(px(r.md))
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(SharedString::from(format!(
                                                "VÍNCULO (ESPELHO) · {}",
                                                bond.summary.name
                                            ))),
                                    )
                                    .children(meters.into_iter().map(|(label, xp, level, pct)| {
                                        div()
                                            .mt(px(sp.s2))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.pearl)
                                                            .child(label),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.muted)
                                                            .child(SharedString::from(format!(
                                                                "Nv.{} · {} XP",
                                                                level, xp
                                                            ))),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(px(6.0))
                                                    .rounded_full()
                                                    .bg(c.bond_track)
                                                    .overflow_hidden()
                                                    .child(
                                                        div()
                                                            .h_full()
                                                            .w(relative(pct.clamp(0.0, 1.0)))
                                                            .rounded_full()
                                                            .bg(c.primary),
                                                    ),
                                            )
                                    }))
                                    .child(
                                        div()
                                            .mt(px(sp.s2))
                                            .text_size(px(10.0))
                                            .text_color(c.faint)
                                            .child("Cópia dos medidores de Laços salva no memory.json."),
                                    ),
                            );
                        }

                        // Banner de liberação de exclusão
                        view = view.child(
                            div()
                                .mt(px(sp.s3))
                                .p(px(sp.s2_5))
                                .rounded(px(r.md))
                                .bg(if confirm_active { rgba(0xd9534f22) } else { c.surface_soft })
                                .border_1()
                                .border_color(if confirm_active { rgba(0xd9534f66) } else { c.line })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if confirm_active { rgb(0xd9534f) } else { c.pearl })
                                                .child(if confirm_active {
                                                    "MODO EXCLUSÃO ATIVO: exclusões liberadas."
                                                } else {
                                                    "Exclusões bloqueadas por segurança."
                                                }),
                                        )
                                        .child(
                                            div()
                                                .id("toggle-memory-confirm-btn")
                                                .cursor_pointer()
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .rounded(px(r.sm))
                                                .bg(if confirm_active { rgba(0xd9534f44) } else { c.card_bg })
                                                .border_1()
                                                .border_color(if confirm_active { rgb(0xd9534f) } else { c.line_strong })
                                                .text_size(px(ty.xs))
                                                .text_color(c.pearl)
                                                .hover(|s| s.bg(c.line_strong))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.memory_confirm_active = !this.memory_confirm_active;
                                                    cx.notify();
                                                }))
                                                .child(if confirm_active { "Bloquear" } else { "Liberar exclusões" }),
                                        ),
                                ),
                        );

                        // Renderização das seções de fatos
                        let sec_list: [(&'static str, &'static str, &Vec<String>); 6] = [
                            ("user", "IDENTIDADE DO USUÁRIO", &mem.sections.user),
                            ("preferences", "PREFERÊNCIAS", &mem.sections.preferences),
                            ("hobbies", "HOBBIES E INTERESSES", &mem.sections.hobbies),
                            ("relationship", "VÍNCULO E RELAÇÃO", &mem.sections.relationship),
                            ("dates", "DATAS E ROTINAS", &mem.sections.dates),
                            ("other", "OUTROS FATOS", &mem.sections.other),
                        ];

                        let mut has_any_fact = false;
                        let mut fact_idx: usize = 0;

                        for (sec_key, sec_title, facts) in sec_list {
                            if facts.is_empty() {
                                continue;
                            }
                            has_any_fact = true;

                            view = view.child(settings_h3(sec_title));

                            for fact in facts {
                                fact_idx += 1;
                                let fact_text = fact.clone();
                                let sec_name = sec_key.to_string();

                                let mut row = div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py(px(6.0))
                                    .border_b_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .child(SharedString::from(format!("• {}", fact_text))),
                                    );

                                if confirm_active {
                                    let del_fact = fact_text.clone();
                                    let del_sec = sec_name.clone();
                                    row = row.child(
                                        div()
                                            .id(ElementId::NamedInteger("del-fact".into(), fact_idx as u64))
                                            .cursor_pointer()
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(c.danger_muted_bg)
                                            .border_1()
                                            .border_color(c.danger_muted_border)
                                            .text_size(px(ty.xs))
                                            .text_color(c.danger_muted)
                                            .hover(|s| s.bg(rgba(0xd9534f33)))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.delete_memory_fact(del_sec.clone(), del_fact.clone(), cx);
                                            }))
                                            .child("Excluir"),
                                    );
                                }

                                view = view.child(row);
                            }
                        }

                        if !has_any_fact {
                            view = view.child(
                                div()
                                    .mt(px(sp.s3))
                                    .text_size(px(ty.sm))
                                    .text_color(c.muted)
                                    .child("Nenhum fato duradouro registrado ainda."),
                            );
                        }

                        // Ação perigosa geral
                        view = view.child(settings_h3("ZONA DE RISCO"));
                        view = view.child(settings_p(
                            "A exclusão total remove todos os fatos duráveis e o histórico salvo no memory.json do servidor local.",
                        ));

                        if confirm_active {
                            view = view.child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .id("delete-all-memory-btn")
                                            .cursor_pointer()
                                            .px(px(16.0))
                                            .py(px(8.0))
                                            .rounded(px(r.sm))
                                            .bg(rgb(0xd9534f))
                                            .text_size(px(ty.sm))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .hover(|s| s.bg(rgb(0xc9302c)))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.delete_memory_all(cx);
                                            }))
                                            .child("Apagar toda a memória"),
                                    ),
                            );
                        } else {
                            view = view.child(
                                div()
                                    .flex()
                                    .child(settings_danger_button("Apagar toda a memória (requer liberação)")),
                            );
                        }
                    }
                }

                view
            }
            SettingsTab::Personality => {
                let has_token = self.api_client.has_token();
                let loading = self.personality_loading;
                let error = self.personality_error.clone();
                let status = self.personality_status.clone();
                let p_opt = self.personality_data.clone();

                let mut view = div().flex().flex_col();

                view = view.child(settings_h3("JEITO DA USAMI"));

                if !has_token {
                    view = view.child(
                        div()
                            .p(px(sp.s3_5))
                            .rounded(px(r.md))
                            .bg(rgba(0xf0ad4e15))
                            .border_1()
                            .border_color(rgba(0xf0ad4e55))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf0ad4e))
                                    .child("AUTENTICAÇÃO NECESSÁRIA"),
                            )
                            .child(
                                div()
                                    .mt(px(sp.s1_5))
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child("O ajuste de traços e instruções da personalidade exige USAMI_API_TOKEN configurado."),
                            ),
                    );
                } else {
                    let mut top_bar = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(sp.s3));

                    top_bar = top_bar.child(
                        div()
                            .id("reload-personality-btn")
                            .cursor_pointer()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .hover(|s| s.bg(c.line_strong))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_personality(cx);
                            }))
                            .child("Recarregar do servidor"),
                    );

                    view = view.child(top_bar);

                    if loading {
                        view = view.child(settings_hint("Carregando traços do servidor..."));
                    }

                    if let Some(err) = error {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .text_color(c.danger_muted)
                                .child(SharedString::from(format!("Erro: {}", err))),
                        );
                    }

                    if let Some(st) = status {
                        view = view.child(
                            div()
                                .p(px(sp.s2))
                                .rounded(px(r.sm))
                                .bg(theme::Palette::emerald_soft())
                                .border_1()
                                .border_color(theme::Palette::emerald_border())
                                .text_size(px(ty.xs))
                                .text_color(theme::Palette::emerald())
                                .child(SharedString::from(st)),
                        );
                    }

                    if let Some(p) = p_opt {
                        view = view.child(settings_hint(
                            "50 é a personalidade padrão. Ajuste os traços de 5 em 5 pontos para calibrar o tom da Usami.",
                        ));

                        let traits_meta: [(&'static str, &'static str, u32); 4] = [
                            ("warmth", "Carinho", p.traits.warmth),
                            ("curiosity", "Curiosidade", p.traits.curiosity),
                            ("playfulness", "Brincadeira", p.traits.playfulness),
                            ("pride", "Orgulho (tsundere)", p.traits.pride),
                        ];

                        let mut traits_col = div().mt(px(sp.s3)).flex().flex_col().gap(px(12.0));

                        for (idx, (key, label, val)) in traits_meta.into_iter().enumerate() {
                            let trait_key_down = key;
                            let trait_key_up = key;
                            let bar_w = ((val as f32 / 100.0) * 320.0).clamp(0.0, 320.0);

                            let trait_box = div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(label)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(sp.s2))
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(c.primary)
                                                        .child(SharedString::from(format!("{}", val))),
                                                )
                                                .child(
                                                    div()
                                                        .id(ElementId::NamedInteger("trait-dec".into(), idx as u64))
                                                        .cursor_pointer()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(c.line)
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(c.line_strong))
                                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                            this.adjust_trait(trait_key_down, -5);
                                                            cx.notify();
                                                        }))
                                                        .child("-5"),
                                                )
                                                .child(
                                                    div()
                                                        .id(ElementId::NamedInteger("trait-inc".into(), idx as u64))
                                                        .cursor_pointer()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(c.line)
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(c.line_strong))
                                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                            this.adjust_trait(trait_key_up, 5);
                                                            cx.notify();
                                                        }))
                                                        .child("+5"),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .h(px(6.0))
                                        .w_full()
                                        .rounded_full()
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .child(
                                            div()
                                                .h_full()
                                                .w(px(bar_w))
                                                .rounded_full()
                                                .bg(c.primary),
                                        ),
                                );

                            traits_col = traits_col.child(trait_box);
                        }

                        view = view.child(traits_col);

                        // Instruções extras
                        view = view.child(
                            div()
                                .mt(px(sp.s4))
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(settings_h3("INSTRUÇÕES EXTRAS"))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.primary)
                                        .child(SharedString::from(format!(
                                            "{}/600",
                                            p.custom_prompt.chars().count()
                                        ))),
                                ),
                        );

                        let prompt_display: SharedString = if p.custom_prompt.trim().is_empty() {
                            "Nenhuma instrução extra configurada.".into()
                        } else {
                            p.custom_prompt.clone().into()
                        };

                        view = view.child(
                            div()
                                .p(px(sp.s2_5))
                                .rounded(px(r.md))
                                .bg(c.icon_bg)
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(ty.sm))
                                .text_color(c.pearl)
                                .child(prompt_display),
                        );

                        // Botões Salvar e Restaurar
                        view = view.child(
                            div()
                                .mt(px(sp.s4))
                                .flex()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .id("save-personality-btn")
                                        .cursor_pointer()
                                        .px(px(16.0))
                                        .py(px(8.0))
                                        .rounded(px(r.sm))
                                        .bg(c.primary)
                                        .text_size(px(ty.sm))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .hover(|s| s.opacity(0.85))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.save_personality(cx);
                                        }))
                                        .child("Salvar alterações"),
                                )
                                .child(
                                    div()
                                        .id("reset-personality-btn")
                                        .cursor_pointer()
                                        .px(px(16.0))
                                        .py(px(8.0))
                                        .rounded(px(r.sm))
                                        .bg(c.surface_soft)
                                        .border_1()
                                        .border_color(c.line)
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .hover(|s| s.bg(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.reset_personality(cx);
                                        }))
                                        .child("Restaurar padrão"),
                                ),
                        );
                    }
                }

                view
            }
            SettingsTab::About => div()
                .flex()
                .flex_col()
                .child(settings_h3("USAMI COMPANION"))
                .child(settings_p(
                    "Versão 0.5.1 — desktop companion nativo com avatar 3D/VRM e voz própria.",
                ))
                .child(settings_h3("MODO MASCOTE (DESKTOP MATE)"))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1))
                                .child(
                                    div()
                                        .text_size(px(ty.md))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.pearl)
                                        .child("Permitir modo mascote"),
                                )
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .text_color(c.muted)
                                        .child("Clique no botão de minimizar (–) vira avatar pequeno sempre no topo, fora da barra de tarefas. Ctrl+Alt+M alterna; duplo clique no avatar volta."),
                                ),
                        )
                        .child(
                            div()
                                .id("mascot-toggle")
                                .w(px(52.0))
                                .h(px(28.0))
                                .rounded(px(999.0))
                                .bg(if self.prefs.mascot_enabled { theme::Palette::emerald_soft() } else { rgba(0x88888822) })
                                .border_1()
                                .border_color(if self.prefs.mascot_enabled { theme::Palette::emerald_border() } else { rgba(0x88888866) })
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    let next = !this.prefs.mascot_enabled;
                                    this.prefs.mascot_enabled = next;
                                    crate::mascot::set_enabled(next);
                                    if !next && this.mascot_mode {
                                        this.exit_mascot(window, cx);
                                    }
                                    storage::save_prefs(&this.prefs);
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size_full()
                                        .child(
                                            div()
                                                .w(px(22.0))
                                                .h(px(22.0))
                                                .rounded(px(999.0))
                                                .bg(if self.prefs.mascot_enabled { theme::Palette::emerald() } else { rgb(0x888888) })
                                                .ml(if self.prefs.mascot_enabled { px(26.0) } else { px(4.0) }),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .mt(px(sp.s2))
                        .flex()
                        .justify_between()
                        .items_center()
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1))
                                .child(
                                    div()
                                        .text_size(px(ty.md))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.pearl)
                                        .child("Seguir o cursor"),
                                )
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .text_color(c.muted)
                                        .child("No modo mascote, a cabeça e os olhos acompanham o ponteiro pela tela inteira."),
                                ),
                        )
                        .child(
                            div()
                                .id("mascot-follow-toggle")
                                .w(px(52.0))
                                .h(px(28.0))
                                .rounded(px(999.0))
                                .bg(if self.prefs.mascot_follow { theme::Palette::emerald_soft() } else { rgba(0x88888822) })
                                .border_1()
                                .border_color(if self.prefs.mascot_follow { theme::Palette::emerald_border() } else { rgba(0x88888866) })
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    let next = !this.prefs.mascot_follow;
                                    this.prefs.mascot_follow = next;
                                    crate::mascot::set_follow(next);
                                    this.persist_prefs();
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size_full()
                                        .child(
                                            div()
                                                .w(px(22.0))
                                                .h(px(22.0))
                                                .rounded(px(999.0))
                                                .bg(if self.prefs.mascot_follow { theme::Palette::emerald() } else { rgb(0x888888) })
                                                .ml(if self.prefs.mascot_follow { px(26.0) } else { px(4.0) }),
                                        ),
                                ),
                        ),
                )
                .child(settings_h3("ARQUITETURA NATIVA"))
                .child(settings_list_item("Desktop: Rust + GPUI 0.2.2 + WGPU 60 FPS"))
                .child(settings_list_item("Avatar: 白ー (shiro) VRM 1.0 com renderização direta nativa"))
                .child(settings_list_item("Backend: Node.js (127.0.0.1:8787) com OpenRouter / NVIDIA (LLM) + Gemini (TTS)"))
                .child(settings_list_item("Presença Win32: Bandeja do sistema, atalho global (Alt+Espaço) e autostart"))
                .child(settings_h3("CRÉDITOS"))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1))
                        .mb(px(sp.s1))
                        .text_size(px(ty.md))
                        .line_height(px(21.0))
                        .text_color(c.pearl)
                        .child("•")
                        .child("Modelo 3D:")
                        .child(div().font_weight(FontWeight::BOLD).child("白ー (shiro)"))
                        .child("por Sweet_nya555"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1))
                        .mb(px(sp.s1))
                        .text_size(px(ty.md))
                        .line_height(px(21.0))
                        .text_color(c.pearl)
                        .child("•")
                        .child("Ícones: Lucide"),
                )
                .child(settings_h3("PRIVACIDADE"))
                .child(settings_p(
                    "Comunicação local em 127.0.0.1:8787. As credenciais e dados sensíveis residem unicamente no servidor local.",
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s1_5))
                        .child(settings_code("127.0.0.1:8787"))
                        .child(settings_code("USAMI_API_TOKEN"))
                        .child(settings_code("usami/.env")),
                ),
        }
    }

    fn render_settings_advanced(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        let msg_count = self.chat_history.len();
        let last_source = self.last_source.clone();
        let last_timing = self.last_timing.clone();
        let server_affinity = self.server_affinity;
        let server_stage = self.server_stage.clone();
        let source_str = last_source
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or("Nenhuma interação recente");
        let stage_str = server_stage.as_ref().map(|s| s.as_ref()).unwrap_or("—");

        div()
            .flex()
            .flex_col()
            .child(settings_h3("ÚLTIMO TURNO DE RESPOSTA"))
            .child(
                div()
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .font_family(theme::FONT_DISPLAY)
                            .font_weight(FontWeight::BOLD)
                            .text_color(c.primary)
                            .child("TELEMETRIA DA IA"),
                    )
                    .child(
                        div()
                            .mt(px(sp.s2))
                            .text_size(px(ty.sm))
                            .text_color(c.pearl)
                            .child(SharedString::from(format!("• Fonte: {}", source_str))),
                    )
                    .when_some(last_timing, |parent, timing| {
                        parent
                            .when_some(timing.llm_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!("• LLM: {} ms", ms))),
                                )
                            })
                            .when_some(timing.surface_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!(
                                            "• Superfície: {} ms",
                                            ms
                                        ))),
                                )
                            })
                            .when_some(timing.total_ms, |p, ms| {
                                p.child(
                                    div()
                                        .mt(px(sp.s1))
                                        .text_size(px(ty.sm))
                                        .text_color(c.pearl)
                                        .child(SharedString::from(format!("• Total: {} ms", ms))),
                                )
                            })
                    })
                    .child(
                        div()
                            .mt(px(sp.s1))
                            .text_size(px(ty.sm))
                            .text_color(c.pearl)
                            .child(SharedString::from(format!(
                                "• Vínculo no servidor: {} (estágio: {})",
                                server_affinity
                                    .map(|a| format!("{}/100", a))
                                    .unwrap_or_else(|| "—".into()),
                                stage_str
                            ))),
                    ),
            )
            .child(settings_h3("HISTÓRICO LOCAL"))
            .child(settings_p(SharedString::from(format!(
                "Total de {} mensagem(ns) salva(s) neste computador.",
                msg_count
            ))))
            .child(
                div().flex().child(
                    div()
                        .id("clear-session-history-btn")
                        .cursor_pointer()
                        .px(px(14.0))
                        .py(px(8.0))
                        .rounded(px(r.sm))
                        .bg(c.surface_soft)
                        .border_1()
                        .border_color(c.line)
                        .text_size(px(ty.sm))
                        .text_color(c.pearl)
                        .hover(|s| s.bg(c.line_strong))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.chat_history.clear();
                            this.persist_history();
                            this.show_bubble = false;
                            cx.notify();
                        }))
                        .child("Limpar histórico da conversa"),
                ),
            )
    }

    pub(crate) fn render_settings_page(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let voice = self.render_settings_section(SettingsTab::Voice, cx);
        let personality = self.render_settings_section(SettingsTab::Personality, cx);
        let memory = self.render_settings_section(SettingsTab::Memory, cx);
        let avatar = self.render_settings_section(SettingsTab::Avatar, cx);
        let integrations = self.render_settings_section(SettingsTab::Chat, cx);
        let system = self.render_settings_section(SettingsTab::About, cx);
        let advanced_open = self.settings_advanced_open;
        let advanced = if advanced_open {
            Some(self.render_settings_advanced(cx))
        } else {
            None
        };

        div()
            .id("vn-settings-page")
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(sp.s5))
            .p(px(sp.s6))
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s1))
                    .child(
                        div()
                            .text_size(px(26.0))
                            .font_family(theme::FONT_CHARACTER)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.pearl)
                            .child("Ajustes"),
                    )
                    .child(
                        div().text_size(px(ty.sm)).text_color(c.muted).child(
                            "Voz, memória, jeito de ser e integrações — tudo no mesmo lugar.",
                        ),
                    ),
            )
            .child(render_settings_category(
                "Voz e áudio",
                "Gemini Multimodal Live, TTS e privacidade da chave.",
                voice,
            ))
            .child(render_settings_category(
                "Personalidade e comportamento",
                "Traços, jeito de falar e tom das respostas.",
                personality,
            ))
            .child(render_settings_category(
                "Memória e privacidade",
                "O que a Usami lembra, exportação e apagamento.",
                memory,
            ))
            .child(render_settings_category(
                "Aparência e interface",
                "Renderizador 3D nativo e cenário.",
                avatar,
            ))
            .child(render_settings_category(
                "Integrações",
                "Conexão com o backend, proatividade e conectores de jogos.",
                integrations,
            ))
            .child(render_settings_category(
                "Sistema",
                "Versão, privacidade e diagnósticos locais.",
                system,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(t.radius.lg))
                    .border_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .id("vn-settings-advanced-toggle")
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(sp.s4))
                            .py(px(sp.s3))
                            .hover(|s| s.bg(c.surface_soft))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.settings_advanced_open = !this.settings_advanced_open;
                                this.poke(cx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(sp.s2))
                                    .child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.lavender)
                                            .child(if advanced_open { "▾" } else { "▸" }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ty.md))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c.pearl)
                                            .child("Avançado"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.faint)
                                    .child("Telemetria do último turno e histórico local"),
                            ),
                    )
                    .when_some(advanced, |el, content| {
                        el.child(
                            div()
                                .px(px(sp.s4))
                                .pb(px(sp.s4))
                                .flex()
                                .flex_col()
                                .child(content),
                        )
                    }),
            )
    }

    pub(crate) fn load_settings_data(&mut self, cx: &mut Context<Self>) {
        if !self.memory_loading {
            self.fetch_memory(cx);
        }
        if self.personality_data.is_none() && !self.personality_loading {
            self.fetch_personality(cx);
        }
    }

    pub(crate) fn render_settings_modal(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let active_tab = self.settings_tab;

        let tabs = [
            (SettingsTab::Chat, "Chat & IA", "settings-tab-chat"),
            (SettingsTab::Voice, "Voz & Áudio", "settings-tab-voice"),
            (SettingsTab::Avatar, "Avatar", "settings-tab-avatar"),
            (SettingsTab::Memory, "Memória", "settings-tab-memory"),
            (
                SettingsTab::Personality,
                "Personalidade",
                "settings-tab-personality",
            ),
            (SettingsTab::About, "Sistema", "settings-tab-about"),
        ];

        let content = self.render_settings_section(active_tab, cx);

        div()
            .id("settings-modal-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x0c0b0ed9))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("settings-modal-frame")
                    .relative()
                    .w(px(940.0))
                    .max_w(relative(0.92))
                    .h(px(620.0))
                    .max_h(relative(0.88))
                    .rounded(px(22.0))
                    .bg(t.vn_modal_bg())
                    .border_2()
                    .border_color(theme::Palette::silver())
                    .shadow(t.shadow.vn_modal.clone())
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Cantoneira ornamental superior esquerda
                    .child(
                        div()
                            .absolute()
                            .top(px(8.0))
                            .left(px(8.0))
                            .child(crate::ui::widgets::icon("vn_corner", 24.0)),
                    )
                    // Textura sutil de veludo no fundo
                    .child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .right(px(0.0))
                            .opacity(0.6)
                            .child(crate::ui::widgets::icon("velvet_pattern", 220.0)),
                    )
                    // Cabeçalho
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(sp.s6))
                            .pt(px(sp.s5))
                            .pb(px(sp.s4))
                            .border_b_1()
                            .border_color(theme::Palette::vn_card_border())
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(crate::ui::widgets::icon("bunny_ears", 24.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(22.0))
                                                    .font_family(theme::FONT_CHARACTER)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child("Configurações da Usami"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(ty.xs))
                                                    .text_color(theme::Palette::vn_pink_soft())
                                                    .child("Ajustes de voz, personalidade, memória e sistema"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("settings-modal-close")
                                    .cursor_pointer()
                                    .size(px(34.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(c.surface_soft)
                                    .border_1()
                                    .border_color(theme::Palette::vn_card_border())
                                    .hover(|s| {
                                        s.bg(theme::Palette::vn_pink_neon())
                                            .border_color(c.pearl)
                                    })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.active_drawer = crate::model::ActiveDrawer::None;
                                        this.poke(cx);
                                        cx.notify();
                                    }))
                                    .child(crate::ui::widgets::icon("close", 14.0)),
                            ),
                    )
                    // Barra horizontal de sub-abas
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(sp.s6))
                            .py(px(sp.s3))
                            .bg(rgba(0x14131699))
                            .border_b_1()
                            .border_color(theme::Palette::vn_card_border())
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .children(tabs.into_iter().map(|(tab, label, id)| {
                                let is_active = active_tab == tab;
                                div()
                                    .id(id)
                                    .cursor_pointer()
                                    .px(px(14.0))
                                    .py(px(7.0))
                                    .rounded(px(999.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .bg(if is_active {
                                        t.vn_tab_active_bg()
                                    } else {
                                        theme::Theme::gradient(
                                            90.0,
                                            rgba(0x27242c99),
                                            rgba(0x27242c99),
                                        )
                                    })
                                    .border_1()
                                    .border_color(if is_active {
                                        theme::Palette::vn_border_bright()
                                    } else {
                                        theme::Palette::vn_card_border()
                                    })
                                    .when(is_active, |el| {
                                        el.shadow(theme::glow(theme::Palette::vn_pink_soft(), 10.0))
                                    })
                                    .hover(|s| {
                                        if !is_active {
                                            s.bg(rgba(0x8e1b3355))
                                                .border_color(theme::Palette::vn_border_bright())
                                        } else {
                                            s
                                        }
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.settings_tab = tab;
                                        this.poke(cx);
                                        cx.notify();
                                    }))
                                    .when(is_active, |el| {
                                        el.child(crate::ui::widgets::icon("bunny_bow", 12.0))
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(if is_active {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_active { c.pearl } else { c.muted })
                                            .child(label),
                                    )
                            })),
                    )
                    // Corpo rolável com respiro
                    .child(
                        div()
                            .id("settings-modal-scroll")
                            .flex_1()
                            .min_h(px(0.0))
                            .p(px(sp.s6))
                            .overflow_y_scroll()
                            .child(content),
                    ),
            )
    }
}
