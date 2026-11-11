"""Consolida o Ajustes do Usami Desktop em pagina unica no hub VN.

- Extrai os seis bracos do `match settings_tab` do drawer para
  `render_settings_section`; a telemetria do braco Chat vira
  `render_settings_advanced` (secao expansivel "Avancado").
- Adiciona `render_settings_page` (pagina de capitulo do hub),
  `render_settings_category` (free fn) e `load_settings_data`.
- Esvazia o drawer de Configuracoes (só VRMA continua la), retira o
  campo/metodos `settings_tab` e aponta a engrenagem e o capítulo
  "07 Ajustes" para o hub.

Uso: python scripts/consolidate_settings.py [--dry]
"""
import sys

PATH = "src/main.rs"
DRY = "--dry" in sys.argv

lines = open(PATH, encoding="utf-8").read().split("\n")


def slice_lines(lo, hi):
    return lines[lo - 1 : hi]


def dedent(block, amount):
    out = []
    for ln in block:
        removed = 0
        i = 0
        while i < len(ln) and ln[i] == " " and removed < amount:
            i += 1
            removed += 1
        out.append(ln[i:])
    return out


def check_dedent(block, amount, name):
    for ln in block:
        if ln.strip() and (len(ln) - len(ln.lstrip(" "))) < amount:
            raise SystemExit(f"dedent insuficiente em {name}: {ln!r}")


# --- fatias dos bracos (1-indexado, inclusivo), validadas no dry-run ---
CHAT_PART1 = slice_lines(5269, 5402)
CHAT_ADV = slice_lines(5403, 5500)
VOICE = slice_lines(5502, 5563)
AVATAR = slice_lines(5564, 5578)
MEMORY = slice_lines(5579, 5881)
PERSONALITY = slice_lines(5882, 6164)
ABOUT = slice_lines(6165, 6218)

for blk, amt, name in [
    (CHAT_PART1, 36, "ChatPart1"),
    (VOICE, 36, "Voice"),
    (AVATAR, 36, "Avatar"),
    (MEMORY, 36, "Memory"),
    (PERSONALITY, 36, "Personality"),
    (ABOUT, 36, "About"),
    (CHAT_ADV, 44, "ChatAdv"),
]:
    check_dedent(blk, amt, name)

part1 = dedent(CHAT_PART1, 36) + ["            }"]
voice = dedent(VOICE, 36)
avatar = dedent(AVATAR, 36)
memory = dedent(MEMORY, 36)
personality = dedent(PERSONALITY, 36)
about = dedent(ABOUT, 36)
advanced = dedent(CHAT_ADV, 44)

NEW_METHODS = '''
    fn render_settings_section(
        &mut self,
        tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        match tab {
''' + "\n".join(
    part1 + voice + avatar + memory + personality + about
) + '''
        }
    }

    fn render_settings_advanced(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
''' + "\n".join(advanced) + '''
    }

    fn render_settings_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let advanced_open = self.settings_advanced_open;
        let advanced = if advanced_open {
            Some(self.render_settings_advanced(cx))
        } else {
            None
        };

        div()
            .id("vn-settings-page")
            .flex_1()
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
                        div()
                            .text_size(px(ty.sm))
                            .text_color(c.muted)
                            .child(
                                "Voz, memória, jeito de ser e integrações — tudo no mesmo lugar.",
                            ),
                    ),
            )
            .child(render_settings_category(
                "Voz e áudio",
                "Gemini Multimodal Live, TTS e privacidade da chave.",
                self.render_settings_section(SettingsTab::Voice, cx),
            ))
            .child(render_settings_category(
                "Personalidade e comportamento",
                "Traços, jeito de falar e tom das respostas.",
                self.render_settings_section(SettingsTab::Personality, cx),
            ))
            .child(render_settings_category(
                "Memória e privacidade",
                "O que a Usami lembra, exportação e apagamento.",
                self.render_settings_section(SettingsTab::Memory, cx),
            ))
            .child(render_settings_category(
                "Aparência e interface",
                "Renderizador 3D nativo e cenário.",
                self.render_settings_section(SettingsTab::Avatar, cx),
            ))
            .child(render_settings_category(
                "Integrações",
                "Conexão com o backend e proatividade.",
                self.render_settings_section(SettingsTab::Chat, cx),
            ))
            .child(render_settings_category(
                "Sistema",
                "Versão, privacidade e diagnósticos locais.",
                self.render_settings_section(SettingsTab::About, cx),
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

    fn load_settings_data(&mut self, cx: &mut Context<Self>) {
        if self.memory_data.is_none() && !self.memory_loading {
            self.fetch_memory(cx);
        }
        if self.personality_data.is_none() && !self.personality_loading {
            self.fetch_personality(cx);
        }
    }

'''

CATEGORY_FN = '''
fn render_settings_category(
    title: &'static str,
    subtitle: &'static str,
    content: impl IntoElement,
) -> impl IntoElement {
    let t = theme::theme();
    let c = t.color;
    let sp = t.space;
    let ty = t.text;
    div()
        .flex()
        .flex_col()
        .gap(px(sp.s3))
        .rounded(px(t.radius.lg))
        .border_1()
        .border_color(c.line)
        .p(px(sp.s4))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s1))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::Palette::vn_pink_soft())
                        .child(title),
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
'''

ANCHOR = "    /// O core é a única fonte do vínculo: nível, EXP e medidores vêm dele."

GEAR_NEW = '''                                        .bg(if active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::Bond;
                                            this.bond_tab = BondTab::Settings;
                                            this.load_settings_data(cx);
                                            this.poke(cx);
                                            cx.notify();
                                        }))'''

SIDEBAR_SETTINGS_NEW = '''                                                        .child(render_vn_tab_btn("vn-tab-settings", "07", "Ajustes", bond_tab == BondTab::Settings, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Settings;
                                                            this.load_settings_data(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))),'''

# (lo, hi, replacement_lines) aplicadas de baixo para cima; 1-indexado inclusivo.
EDITS = [
    (6552, 6557, ['                                                BondTab::Settings => self.render_settings_page(cx).into_any_element(),']),
    (6372, 6376, [SIDEBAR_SETTINGS_NEW]),
    (5138, 6221, ['                            ActiveDrawer::Settings => div(),']),
    (4987, 4997, None),
    (4202, 4219, [GEAR_NEW]),
    (3808, 3809, ['        let side_drawer_open = active_drawer == ActiveDrawer::Vrma && !chat_open;']),
    (3806, 3806, None),
    (3713, 3787, None),
    (2134, 2149, None),
    (2119, 2124, None),
    (2108, 2110, None),
    (996, 996, ['            settings_advanced_open: false,']),
    (698, 698, ['    settings_advanced_open: bool,']),
]


def expect(lo, text):
    actual = lines[lo - 1]
    if actual != text:
        raise SystemExit(f"ancora invalida na linha {lo}:\n  esperado: {text!r}\n  atual:    {actual!r}")


# verificacao das ancoras antes de qualquer alteracao
expect(698, "    settings_tab: SettingsTab,")
expect(996, "            settings_tab: SettingsTab::Chat,")
expect(2108, "            if drawer == ActiveDrawer::Settings {")
expect(2119, "    fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {")
expect(2134, "    fn load_active_settings_tab(&mut self, cx: &mut Context<Self>) {")
expect(3713, "fn render_vn_tab_settings(")
expect(3806, "        let settings_tab = self.settings_tab;")
expect(3808, "        let side_drawer_open =")
expect(4987, "                    .when(active_drawer == ActiveDrawer::Settings, |parent| {")
expect(5138, "                            ActiveDrawer::Settings => {")
expect(6372, '                                                        .child(render_vn_tab_btn("vn-tab-settings", "07", "Ajustes", bond_tab == BondTab::Settings, cx.listener(|this, _: &ClickEvent, _, cx| {')
expect(6552, "                                                BondTab::Settings => render_vn_tab_settings(&t, cx.listener(|this, _: &ClickEvent, _, cx| {")

if DRY:
    print("ancoras OK")
    sys.exit(0)

# edicao por linha, de baixo para cima (numeros do arquivo original)
for lo, hi, repl in sorted(EDITS, key=lambda e: e[0], reverse=True):
    if repl is None:
        del lines[lo - 1 : hi]
    else:
        lines[lo - 1 : hi] = repl

# insercao dos metodos por ancora textual, depois das edicoes numeradas
count = 0
for i, ln in enumerate(lines):
    if ln == ANCHOR:
        lines[i:i] = [CATEGORY_FN.rstrip("\n"), ""] + NEW_METHODS.rstrip("\n").split("\n") + [""]
        count += 1
if count != 1:
    raise SystemExit(f"ancora de insercao encontrada {count}x")

out = "\n".join(lines)

for gone in [
    "settings_tab",
    "set_settings_tab",
    "load_active_settings_tab",
    "render_vn_tab_settings",
    "Painel de Ajustes Detalhados",
]:
    if gone in out:
        raise SystemExit(f"referencia residual apos transformacao: {gone}")

open(PATH, "w", encoding="utf-8", newline="\n").write(out)
print("transformacao aplicada")
