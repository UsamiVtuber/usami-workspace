//! Tokens visuais do Usami Desktop — direção de arte "Velvet Bunny" (Visual Novel).
//!
//! Paleta oficial inspirada na modelo da Usami: cinza carvão profundo `#141316`,
//! superfície grafite `#1E1C22`, vermelho bordô `#A82440`, rosa claro `#F5D0D8`,
//! prata `#C8CAD4`, branco rosado `#F9F2F4` e detalhes em verde esmeralda `#10B981`.
//! Nenhum widget deve declarar cor, tamanho de fonte, raio ou sombra solto —
//! tudo passa por `theme()`.

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
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        FONT_JP.into(),
        "Segoe UI Symbol".into(),
    ]));
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
    /// `--muted` (66%).
    pub muted: Rgba,
    /// `--faint` (55%).
    pub faint: Rgba,

    pub composer_bg: Rgba,
    pub icon_bg: Rgba,
    pub icon_bg_hover: Rgba,
    pub icon_bg_active: Rgba,
    pub status_bg: Rgba,
    pub overlay_top: Rgba,
    pub overlay_bottom: Rgba,
    pub companion_overlay: Rgba,
    /// `rgba(246,234,243,.08)` — botão neutro.
    pub surface_soft: Rgba,
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
            deep: rgb(0x141316),
            panel: rgb(0x1e1c22),
            primary: rgb(0xf5d0d8),
            lavender: rgb(0xc8cad4),
            pearl: rgb(0xf9f2f4),
            accent: rgb(0xa82440),
            ok: rgb(0x10b981),
            warn: rgb(0xf5c451),
            err: rgb(0xe11d48),
            line: rgba(0xc8cad424),
            line_strong: rgba(0xc8cad44d),
            muted: rgba(0xf9f2f4a8),
            faint: rgba(0xf9f2f47a),

            composer_bg: rgba(0x141316cc),
            icon_bg: rgba(0x1e1c22d9),
            icon_bg_hover: rgba(0xa8244042),
            icon_bg_active: rgba(0xa824406b),
            status_bg: rgba(0x141316d9),
            overlay_top: rgba(0x14131685),
            overlay_bottom: rgba(0x141316b8),
            companion_overlay: rgba(0x141316a6),
            surface_soft: rgba(0xc8cad414),
            accent_tint: rgba(0xa8244040),
            danger_muted_bg: rgba(0xe11d4822),
            danger_muted_border: rgba(0xe11d4855),
            danger_muted: rgb(0xfda4af),
            success_border: rgba(0x10b98180),
            call_bg: rgba(0xc8cad40f),
            call_bg_hover: rgba(0xc8cad424),
            accent_border: rgba(0xf5d0d873),
            card_bg: rgba(0x27242ceb),
            bond_border: rgba(0xc8cad459),
            err_dark: rgb(0x8e1b33),
            bond_track: rgba(0xc8cad426),
        }
    }

    /// Vermelho bordô vibrante (fitas e destaques principais)
    pub fn vn_pink_neon() -> Rgba {
        rgb(0xa82440)
    }

    /// Rosa claro sakura (brilhos, rubor e textos suaves)
    pub fn vn_pink_soft() -> Rgba {
        rgb(0xf5d0d8)
    }

    /// Prata luminosa para contornos e ornamentos
    pub fn vn_purple_glow() -> Rgba {
        rgb(0xc8cad4)
    }

    /// Bordô profundo para gradientes
    pub fn vn_lavender_bright() -> Rgba {
        rgb(0x8e1b33)
    }

    pub fn vn_card_border() -> Rgba {
        rgba(0xc8cad438)
    }

    pub fn vn_border_bright() -> Rgba {
        rgba(0xf5d0d880)
    }

    /// Verde esmeralda (joia central da Usami — detalhes pontuais)
    pub fn emerald() -> Rgba {
        rgb(0x10b981)
    }

    pub fn emerald_soft() -> Rgba {
        rgba(0x10b98126)
    }

    pub fn emerald_border() -> Rgba {
        rgba(0x10b98170)
    }

    pub fn silver() -> Rgba {
        rgb(0xc8cad4)
    }

    pub fn silver_bright() -> Rgba {
        rgb(0xe5e7f0)
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
    pub input: f32,
    pub bubble: f32,
}

impl Typography {
    pub fn official() -> Self {
        Self {
            xs: 11.0,
            sm: 12.0,
            md: 13.0,
            h3: 13.5,
            input: 14.5,
            bubble: 16.0,
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
    pub composer: f32,
}

impl Radii {
    pub fn official() -> Self {
        Self {
            sm: 9.0,
            md: 12.0,
            lg: 16.0,
            composer: 28.0,
        }
    }
}

#[derive(Clone)]
pub struct Shadows {
    pub composer: Vec<BoxShadow>,
    pub composer_focus: Vec<BoxShadow>,
    pub pill: Vec<BoxShadow>,
    pub drawer: Vec<BoxShadow>,
    pub send: Vec<BoxShadow>,
    pub vn_modal: Vec<BoxShadow>,
    pub vn_card: Vec<BoxShadow>,
}

impl Shadows {
    fn official(palette: &Palette) -> Self {
        let pink = palette.primary;
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
            pill: vec![shadow_of(black, 0.35, 0.0, 8.0, 24.0)],
            drawer: vec![shadow_of(black, 0.75, -20.0, 0.0, 70.0)],
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

    /// `linear-gradient(135deg, rgba(168,36,64,.55), rgba(245,208,216,.3))`
    pub fn bond_pill_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0xa824408c), rgba(0xf5d0d84d))
    }

    pub fn bond_pill_hover(&self) -> Background {
        Self::gradient(135.0, rgba(0xa82440bf), rgba(0xf5d0d873))
    }

    pub fn send_bg(&self) -> Background {
        Self::gradient(135.0, self.color.accent, self.color.err_dark)
    }

    pub fn stage_overlay(&self) -> Background {
        Self::gradient(180.0, self.color.overlay_top, self.color.overlay_bottom)
    }

    /// Fundo cinza grafite profundo característico do tema Velvet Bunny
    pub fn vn_modal_bg(&self) -> Background {
        Self::gradient(145.0, rgba(0x221f26f8), rgba(0x141316fa))
    }

    pub fn vn_sidebar_bg(&self) -> Background {
        Self::gradient(180.0, rgba(0x1b191ff2), rgba(0x121114f5))
    }

    pub fn vn_card_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0x2a2630e8), rgba(0x1d1b22e6))
    }

    pub fn vn_tab_active_bg(&self) -> Background {
        Self::gradient(90.0, rgb(0x8e1b33), rgb(0xb8324f))
    }

    pub fn vn_heart_badge_bg(&self) -> Background {
        Self::gradient(135.0, rgba(0x8e1b33f5), rgba(0xc23b5af0))
    }

    pub fn vn_exp_fill_bg(&self) -> Background {
        Self::gradient(90.0, rgb(0xa82440), rgb(0xf5d0d8))
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
