//! Tokens visuais do Usami Desktop.
//!
//! Fonte de verdade: `usami/src/usami.css` (paleta oficial pós-migração D1,
//! 2026-11-08). Nenhum widget deve declarar cor, tamanho de fonte, raio ou
//! sombra solto — tudo passa por `theme()`.

use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use gpui::*;

pub const FONT_BODY: &str = "DM Sans";
pub const FONT_DISPLAY: &str = "Rajdhani";
pub const FONT_CHARACTER: &str = "Fraunces";

/// Arquivos embutidos em `assets/fonts` (SIL Open Font License 1.1).
pub const FONT_FILES: &[&str] = &[
    "assets/fonts/DMSans-Regular.ttf",
    "assets/fonts/DMSans-Medium.ttf",
    "assets/fonts/DMSans-Bold.ttf",
    "assets/fonts/Fraunces-Regular.ttf",
    "assets/fonts/Fraunces-Medium.ttf",
    "assets/fonts/Fraunces-SemiBold.ttf",
    "assets/fonts/Fraunces-Italic.ttf",
    "assets/fonts/Rajdhani-SemiBold.ttf",
    "assets/fonts/Rajdhani-Bold.ttf",
];

/// Registra as fontes OFL que acompanham o pacote. Ausência de arquivo não
/// derruba o app: o GPUI cai para a fonte padrão do sistema.
pub fn load_fonts(cx: &App, base_dir: &Path) {
    let mut fonts: Vec<Cow<'static, [u8]>> = Vec::new();
    for relative in FONT_FILES {
        match std::fs::read(base_dir.join(relative)) {
            Ok(bytes) => fonts.push(Cow::Owned(bytes)),
            Err(error) => eprintln!("[Usami Fonts] {relative} indisponível: {error}"),
        }
    }
    if fonts.is_empty() {
        return;
    }
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("[Usami Fonts] falha ao registrar fontes: {error}");
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    /// Fundo profundo do palco (`--deep`).
    pub deep: Rgba,
    /// Painéis (`--panel`).
    pub panel: Rgba,
    /// Rosa de destaque (`--primary`).
    pub primary: Rgba,
    /// Rosa claro (`--lavender`).
    pub lavender: Rgba,
    /// Texto principal (`--pearl`).
    pub pearl: Rgba,
    /// Ação vibrante (`--accent`).
    pub accent: Rgba,
    pub ok: Rgba,
    pub warn: Rgba,
    pub err: Rgba,
    /// `--line` (12%).
    pub line: Rgba,
    /// `--line-strong` (22%).
    pub line_strong: Rgba,
    /// `--line-glow` (35%).
    pub line_glow: Rgba,
    /// `--muted` (66%).
    pub muted: Rgba,
    /// `--faint` (55%).
    pub faint: Rgba,

    pub composer_bg: Rgba,
    pub icon_bg: Rgba,
    pub icon_bg_hover: Rgba,
    pub icon_bg_active: Rgba,
    pub status_bg: Rgba,
    pub bubble_bg: Rgba,
    pub chat_bg: Rgba,
    pub chat_turn_bg: Rgba,
    pub chat_user_border: Rgba,
    pub overlay_top: Rgba,
    pub overlay_bottom: Rgba,
    pub companion_overlay: Rgba,
    /// `rgba(7,13,31,.6)` — véu atrás de painéis modais.
    pub scrim: Rgba,
    /// `rgba(249,249,251,.08)` — botão neutro.
    pub surface_soft: Rgba,
    /// `rgba(249,249,251,.16)` — botão neutro em hover.
    pub surface_hover: Rgba,
    /// `rgba(255,94,126,.25)`.
    pub accent_tint: Rgba,
    pub danger_muted_bg: Rgba,
    pub danger_muted_border: Rgba,
    pub danger_muted: Rgba,
    pub success_border: Rgba,
    /// `rgba(249,249,251,.05)` — botão de chamada neutro.
    pub call_bg: Rgba,
    /// `rgba(249,249,251,.12)` — botão de chamada em hover.
    pub call_bg_hover: Rgba,
    /// `rgba(255,94,126,.5)` — borda de botão ativo.
    pub accent_border: Rgba,
    /// `rgba(40,45,66,.75)` — cartão dentro de painel.
    pub card_bg: Rgba,
    /// `rgba(255,166,201,.45)` — moldura do cartão de vínculo.
    pub bond_border: Rgba,
    /// `#D1485A` — hangup em hover.
    pub err_dark: Rgba,
    /// `rgba(249,249,251,.15)`.
    pub bond_track: Rgba,
    pub bond_fill_from: Rgba,
    pub bond_fill_to: Rgba,
}

impl Palette {
    pub fn official() -> Self {
        Self {
            deep: rgb(0x070d1f),
            panel: rgb(0x131a33),
            primary: rgb(0xffa6c9),
            lavender: rgb(0xffb8d2),
            pearl: rgb(0xf9f9fb),
            accent: rgb(0xff5e7e),
            ok: rgb(0x3ecf8e),
            warn: rgb(0xf5c451),
            err: rgb(0xff6b7a),
            line: rgba(0xf9f9fb1f),
            line_strong: rgba(0xf9f9fb38),
            line_glow: rgba(0xffa6c959),
            muted: rgba(0xf9f9fba8),
            faint: rgba(0xf9f9fb8c),

            composer_bg: rgba(0x0b132bb8),
            icon_bg: rgba(0x131a33c7),
            icon_bg_hover: rgba(0xffa6c938),
            icon_bg_active: rgba(0xff5e7e59),
            status_bg: rgba(0x0b132bd1),
            bubble_bg: rgba(0x070d1fd9),
            chat_bg: rgba(0x0b132bd1),
            chat_turn_bg: rgba(0x282d42d9),
            chat_user_border: rgba(0xffa6c940),
            overlay_top: rgba(0x0b132b8c),
            overlay_bottom: rgba(0x070d1fb8),
            companion_overlay: rgba(0x070d1f9e),
            scrim: rgba(0x070d1f99),
            surface_soft: rgba(0xf9f9fb14),
            surface_hover: rgba(0xf9f9fb29),
            accent_tint: rgba(0xff5e7e40),
            danger_muted_bg: rgba(0xff8a961f),
            danger_muted_border: rgba(0xff8a9640),
            danger_muted: rgb(0xff8a96),
            success_border: rgba(0x3ecf8e80),
            call_bg: rgba(0xf9f9fb0d),
            call_bg_hover: rgba(0xf9f9fb1f),
            accent_border: rgba(0xff5e7e80),
            card_bg: rgba(0x282d42bf),
            bond_border: rgba(0xffa6c973),
            err_dark: rgb(0xd1485a),
            bond_track: rgba(0xf9f9fb26),
            bond_fill_from: rgb(0xffdce6),
            bond_fill_to: rgb(0xffb8d2),
        }
    }

    /// Cores exclusivas para o estilo Visual Novel de Romance
    pub fn vn_pink_neon() -> Rgba {
        rgb(0xff5e9e)
    }

    pub fn vn_pink_soft() -> Rgba {
        rgb(0xffa6c9)
    }

    pub fn vn_purple_glow() -> Rgba {
        rgb(0xb86bff)
    }

    pub fn vn_card_border() -> Rgba {
        rgba(0xffa6c94d)
    }

    pub fn vn_border_bright() -> Rgba {
        rgba(0xff79c680)
    }

    pub fn vn_gold() -> Rgba {
        rgb(0xf5c451)
    }
}

#[derive(Clone, Copy)]
pub struct Typography {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub h3: f32,
    pub tab: f32,
    pub input: f32,
    pub bubble: f32,
    pub empty_title: f32,
    pub h2: f32,
}

impl Typography {
    pub fn official() -> Self {
        Self {
            xs: 11.0,
            sm: 12.0,
            md: 13.0,
            h3: 13.5,
            tab: 11.5,
            input: 14.5,
            bubble: 16.0,
            empty_title: 20.0,
            h2: 32.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub s1: f32,
    pub s1_5: f32,
    pub s2: f32,
    pub s2_5: f32,
    pub s3: f32,
    pub s3_5: f32,
    pub s4: f32,
    pub s5: f32,
    pub s6: f32,
    pub s8: f32,
}

impl Spacing {
    pub fn official() -> Self {
        Self {
            s1: 4.0,
            s1_5: 6.0,
            s2: 8.0,
            s2_5: 10.0,
            s3: 12.0,
            s3_5: 14.0,
            s4: 16.0,
            s5: 20.0,
            s6: 24.0,
            s8: 32.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Radii {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub sheet: f32,
    pub composer: f32,
}

impl Radii {
    pub fn official() -> Self {
        Self {
            sm: 9.0,
            md: 12.0,
            lg: 16.0,
            xl: 20.0,
            sheet: 26.0,
            composer: 28.0,
        }
    }
}

#[derive(Clone)]
pub struct Shadows {
    pub composer: Vec<BoxShadow>,
    pub composer_focus: Vec<BoxShadow>,
    pub bubble: Vec<BoxShadow>,
    pub panel: Vec<BoxShadow>,
    pub pill: Vec<BoxShadow>,
    pub drawer: Vec<BoxShadow>,
    pub sheet: Vec<BoxShadow>,
    pub send: Vec<BoxShadow>,
    pub vn_modal: Vec<BoxShadow>,
    pub vn_card: Vec<BoxShadow>,
}

impl Shadows {
    fn official(palette: &Palette) -> Self {
        let pink = palette.primary;
        let accent = palette.accent;
        let black = rgb(0x000000);
        Self {
            composer: vec![
                shadow_of(black, 0.45, 0.0, 16.0, 45.0),
                shadow_of(pink, 0.08, 0.0, 0.0, 16.0),
            ],
            composer_focus: vec![
                shadow_of(black, 0.6, 0.0, 16.0, 45.0),
                shadow_of(pink, 0.2, 0.0, 0.0, 20.0),
            ],
            bubble: vec![
                shadow_of(black, 0.55, 0.0, 20.0, 50.0),
                shadow_of(pink, 0.14, 0.0, 0.0, 30.0),
            ],
            panel: vec![
                shadow_of(black, 0.7, 0.0, 24.0, 60.0),
                shadow_of(pink, 0.12, 0.0, 0.0, 25.0),
            ],
            pill: vec![shadow_of(black, 0.35, 0.0, 8.0, 24.0)],
            drawer: vec![shadow_of(black, 0.75, -20.0, 0.0, 70.0)],
            sheet: vec![
                shadow_of(black, 0.6, 0.0, 30.0, 80.0),
                shadow_of(accent, 0.22, 0.0, 0.0, 40.0),
            ],
            send: vec![shadow_of(pink, 0.4, 0.0, 0.0, 14.0)],
            vn_modal: vec![
                shadow_of(black, 0.75, 0.0, 32.0, 90.0),
                shadow_of(Palette::vn_pink_neon(), 0.35, 0.0, 0.0, 45.0),
                shadow_of(Palette::vn_purple_glow(), 0.2, 0.0, 0.0, 80.0),
            ],
            vn_card: vec![
                shadow_of(black, 0.4, 0.0, 10.0, 25.0),
                shadow_of(Palette::vn_pink_soft(), 0.1, 0.0, 0.0, 12.0),
            ],
        }
    }
}

/// Brilho pontual (`box-shadow: 0 0 Npx <cor>`) usado por LEDs e selos.
pub fn glow(color: Rgba, blur: f32) -> Vec<BoxShadow> {
    vec![shadow_of(color, 1.0, 0.0, 0.0, blur)]
}

pub struct Theme {
    pub color: Palette,
    pub text: Typography,
    pub space: Spacing,
    pub radius: Radii,
    pub shadow: Shadows,
}

impl Theme {
    fn official() -> Self {
        let color = Palette::official();
        Self {
            color,
            text: Typography::official(),
            space: Spacing::official(),
            radius: Radii::official(),
            shadow: Shadows::official(&color),
        }
    }

    pub fn gradient(angle: f32, from: Rgba, to: Rgba) -> Background {
        linear_gradient(
            angle,
            linear_color_stop(from, 0.0),
            linear_color_stop(to, 1.0),
        )
    }

    /// `linear-gradient(135deg, rgba(255,94,126,.45), rgba(255,166,201,.3))`
    pub fn bond_pill_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0xff5e7e73), rgba(0xffa6c94d))
    }

    pub fn bond_pill_hover(&self) -> Background {
        Self::gradient(135.0, rgba(0xff5e7ea6), rgba(0xffa6c973))
    }

    pub fn send_bg(&self) -> Background {
        Self::gradient(135.0, self.color.accent, self.color.lavender)
    }

    pub fn bond_sheet_bg(&self) -> Background {
        Self::gradient(160.0, rgba(0x282d42f0), rgba(0x0b132bf5))
    }

    pub fn bond_fill_bg(&self) -> Background {
        Self::gradient(90.0, self.color.bond_fill_from, self.color.bond_fill_to)
    }

    pub fn stage_overlay(&self) -> Background {
        Self::gradient(180.0, self.color.overlay_top, self.color.overlay_bottom)
    }

    /// `linear-gradient(90deg, transparent 30%, rgba(7,13,31,.55) 70%)`
    pub fn bond_scrim_bg(&self) -> Background {
        Self::gradient(90.0, rgba(0x070d1f00), rgba(0x070d1f8c))
    }

    /// Fundo com gradiente ametista profundo característico de Visual Novel de Romance
    pub fn vn_modal_bg(&self) -> Background {
        Self::gradient(145.0, rgba(0x1a0c2ef8), rgba(0x0b0517fa))
    }

    pub fn vn_sidebar_bg(&self) -> Background {
        Self::gradient(180.0, rgba(0x140a26e6), rgba(0x0e061cee))
    }

    pub fn vn_card_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0x23113de0), rgba(0x180c2cdc))
    }

    pub fn vn_tab_active_bg(&self) -> Background {
        Self::gradient(90.0, Palette::vn_pink_neon(), rgb(0xdb5ed9))
    }

    pub fn vn_heart_badge_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0xff5e9ef0), rgba(0x9d4eddf0))
    }

    pub fn vn_exp_fill_bg(&self) -> Background {
        Self::gradient(90.0, Palette::vn_pink_neon(), rgb(0xdb68fa))
    }

    pub fn vn_bubble_tag_bg(&self) -> Background {
        Self::gradient(90.0, rgba(0xff5e9e73), rgba(0xb86bff4d))
    }
}

pub fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(Theme::official)
}

fn shadow_of(color: Rgba, alpha: f32, x: f32, y: f32, blur: f32) -> BoxShadow {
    let color: Hsla = color.into();
    BoxShadow {
        color: color.alpha(alpha),
        offset: point(px(x), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
    }
}
