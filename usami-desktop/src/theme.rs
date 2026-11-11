//! Tokens visuais do Usami Desktop — direção de arte "Midnight Romance".
//!
//! Paleta oficial do redesign (2026-11-11): fundo `#100819`, superfície
//! `#20102F`, rosa de destaque `#F052B5`, rosa claro `#F5C3E1`, lavanda
//! `#A77ABF`, texto `#F6EAF3`. Nenhum widget deve declarar cor, tamanho de
//! fonte, raio ou sombra solto — tudo passa por `theme()`.

use std::borrow::Cow;
use std::path::Path;
use std::sync::OnceLock;

use gpui::*;

pub const FONT_BODY: &str = "DM Sans";
pub const FONT_DISPLAY: &str = "Cormorant Garamond";
pub const FONT_CHARACTER: &str = "Cormorant Garamond";
pub const FONT_JP: &str = "Noto Sans JP";

/// Cormorant itálico com fallback explícito para símbolos.
///
/// O layout itálico da fonte embutida não resolve glifos fora do alfabeto
/// (♡, ♥, ★) pelo fallback automático e mostra tofu; declarar a família de
/// símbolos do Windows no estilo corrige sem trocar a fonte do texto.
pub fn font_character_italic() -> Font {
    let mut font = gpui::font(FONT_CHARACTER).italic();
    font.fallbacks = Some(FontFallbacks::from_fonts(vec!["Segoe UI Symbol".into()]));
    font
}

/// Japonês com fallback para símbolos vem em `font_with_jp` (texto misto);
/// o subset embutido cobre só kana/kanji usados na interface e glifos fora
/// dele caem para a família de símbolos.

/// Texto misto (latim + kana/kanji): mantém a família base e usa o subset
/// japonês como fallback para os glifos que ela não possui.
pub fn font_with_jp(family: &'static str) -> Font {
    let mut font = gpui::font(family);
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![FONT_JP.into(), "Segoe UI Symbol".into()]));
    font
}

/// Arquivos embutidos em `assets/fonts` (SIL Open Font License 1.1).
pub const FONT_FILES: &[&str] = &[
    "assets/fonts/DMSans-Regular.ttf",
    "assets/fonts/DMSans-Medium.ttf",
    "assets/fonts/DMSans-Bold.ttf",
    "assets/fonts/CormorantGaramond-Regular.ttf",
    "assets/fonts/CormorantGaramond-Medium.ttf",
    "assets/fonts/CormorantGaramond-SemiBold.ttf",
    "assets/fonts/CormorantGaramond-Bold.ttf",
    "assets/fonts/CormorantGaramond-Italic.ttf",
    "assets/fonts/CormorantGaramond-MediumItalic.ttf",
    "assets/fonts/NotoSansJP-Regular.ttf",
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
    /// Fundo principal Midnight Romance (`#100819`).
    pub deep: Rgba,
    /// Superfícies (`#20102F`).
    pub panel: Rgba,
    /// Rosa claro — texto/destaque suave (`#F5C3E1`).
    pub primary: Rgba,
    /// Lavanda (`#A77ABF`).
    pub lavender: Rgba,
    /// Texto claro (`#F6EAF3`).
    pub pearl: Rgba,
    /// Rosa principal — ação vibrante (`#F052B5`).
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
    /// `rgba(16,8,25,.6)` — véu atrás de painéis modais.
    pub scrim: Rgba,
    /// `rgba(246,234,243,.08)` — botão neutro.
    pub surface_soft: Rgba,
    /// `rgba(246,234,243,.16)` — botão neutro em hover.
    pub surface_hover: Rgba,
    /// `rgba(240,82,181,.25)`.
    pub accent_tint: Rgba,
    pub danger_muted_bg: Rgba,
    pub danger_muted_border: Rgba,
    pub danger_muted: Rgba,
    pub success_border: Rgba,
    /// `rgba(246,234,243,.05)` — botão de chamada neutro.
    pub call_bg: Rgba,
    /// `rgba(246,234,243,.12)` — botão de chamada em hover.
    pub call_bg_hover: Rgba,
    /// `rgba(240,82,181,.5)` — borda de botão ativo.
    pub accent_border: Rgba,
    /// `rgba(42,21,64,.75)` — cartão dentro de painel.
    pub card_bg: Rgba,
    /// `rgba(245,195,225,.45)` — moldura do cartão de vínculo.
    pub bond_border: Rgba,
    /// `#D1485A` — hangup em hover.
    pub err_dark: Rgba,
    /// `rgba(246,234,243,.15)`.
    pub bond_track: Rgba,
}

impl Palette {
    pub fn official() -> Self {
        Self {
            deep: rgb(0x100819),
            panel: rgb(0x20102f),
            primary: rgb(0xf5c3e1),
            lavender: rgb(0xa77abf),
            pearl: rgb(0xf6eaf3),
            accent: rgb(0xf052b5),
            ok: rgb(0x3ecf8e),
            warn: rgb(0xf5c451),
            err: rgb(0xff6b7a),
            line: rgba(0xf6eaf31f),
            line_strong: rgba(0xf6eaf338),
            line_glow: rgba(0xf052b559),
            muted: rgba(0xf6eaf3a8),
            faint: rgba(0xf6eaf38c),

            composer_bg: rgba(0x100819b8),
            icon_bg: rgba(0x20102fc7),
            icon_bg_hover: rgba(0xf052b538),
            icon_bg_active: rgba(0xf052b559),
            status_bg: rgba(0x100819d1),
            bubble_bg: rgba(0x100819d9),
            chat_bg: rgba(0x100819d1),
            chat_turn_bg: rgba(0x2a1540d9),
            chat_user_border: rgba(0xf5c3e140),
            overlay_top: rgba(0x1008198c),
            overlay_bottom: rgba(0x100819b8),
            companion_overlay: rgba(0x1008199e),
            scrim: rgba(0x10081999),
            surface_soft: rgba(0xf6eaf314),
            surface_hover: rgba(0xf6eaf329),
            accent_tint: rgba(0xf052b540),
            danger_muted_bg: rgba(0xff8a961f),
            danger_muted_border: rgba(0xff8a9640),
            danger_muted: rgb(0xff8a96),
            success_border: rgba(0x3ecf8e80),
            call_bg: rgba(0xf6eaf30d),
            call_bg_hover: rgba(0xf6eaf31f),
            accent_border: rgba(0xf052b580),
            card_bg: rgba(0x2a1540bf),
            bond_border: rgba(0xf5c3e173),
            err_dark: rgb(0xd1485a),
            bond_track: rgba(0xf6eaf326),
        }
    }

    /// Cores exclusivas para o estilo Visual Novel de Romance
    pub fn vn_pink_neon() -> Rgba {
        rgb(0xf052b5)
    }

    pub fn vn_pink_soft() -> Rgba {
        rgb(0xf5c3e1)
    }

    /// Lavanda clareada para gradientes e brilhos.
    pub fn vn_purple_glow() -> Rgba {
        rgb(0xc49dd4)
    }

    /// Lavanda intermediária (par de gradiente do rosa principal).
    pub fn vn_lavender_bright() -> Rgba {
        rgb(0xb78cd1)
    }

    pub fn vn_card_border() -> Rgba {
        rgba(0xf5c3e14d)
    }

    pub fn vn_border_bright() -> Rgba {
        rgba(0xf052b680)
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

    /// `linear-gradient(135deg, rgba(240,82,181,.45), rgba(245,195,225,.3))`
    pub fn bond_pill_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0xf052b573), rgba(0xf5c3e14d))
    }

    pub fn bond_pill_hover(&self) -> Background {
        Self::gradient(135.0, rgba(0xf052b5a6), rgba(0xf5c3e173))
    }

    pub fn send_bg(&self) -> Background {
        Self::gradient(135.0, self.color.accent, self.color.lavender)
    }

    pub fn bond_sheet_bg(&self) -> Background {
        Self::gradient(160.0, rgba(0x2a1540f0), rgba(0x100819f5))
    }

    pub fn stage_overlay(&self) -> Background {
        Self::gradient(180.0, self.color.overlay_top, self.color.overlay_bottom)
    }

    /// `linear-gradient(90deg, transparent 30%, rgba(16,8,25,.55) 70%)`
    pub fn bond_scrim_bg(&self) -> Background {
        Self::gradient(90.0, rgba(0x10081900), rgba(0x1008198c))
    }

    /// Fundo com gradiente ametista profundo característico de Visual Novel de Romance
    pub fn vn_modal_bg(&self) -> Background {
        Self::gradient(145.0, rgba(0x2a1540f8), rgba(0x100819fa))
    }

    pub fn vn_sidebar_bg(&self) -> Background {
        Self::gradient(180.0, rgba(0x1d0e2ee6), rgba(0x12071cee))
    }

    pub fn vn_card_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0x2a1540e0), rgba(0x1c0d2cdc))
    }

    pub fn vn_tab_active_bg(&self) -> Background {
        Self::gradient(90.0, Palette::vn_pink_neon(), Palette::vn_lavender_bright())
    }

    pub fn vn_heart_badge_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0xf052b5f0), rgba(0xa77abff0))
    }

    pub fn vn_exp_fill_bg(&self) -> Background {
        Self::gradient(90.0, Palette::vn_pink_neon(), Palette::vn_lavender_bright())
    }

    pub fn vn_bubble_tag_bg(&self) -> Background {
        Self::gradient(90.0, rgba(0xf052b573), rgba(0xa77abf4d))
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
