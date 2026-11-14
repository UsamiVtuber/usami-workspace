//! Central de Vínculo, Afeto e Diário no estilo Visual Novel de Romance ("Midnight Romance").

use crate::UsamiApp;
use crate::api;
use crate::model::{ActiveDrawer, BondTab};
use crate::theme;
use crate::ui::widgets::icon;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::{Arc, Mutex};

pub(crate) fn render_vn_tab_btn(
    id: &'static str,
    number: &'static str,
    label: &'static str,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let t = theme::theme();
    let c = t.color;
    div()
        .id(id)
        .cursor_pointer()
        .h(px(38.0))
        .px(px(12.0))
        .rounded(px(12.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .bg(if is_active {
            t.vn_tab_active_bg()
        } else {
            theme::Theme::gradient(90.0, rgba(0x00000000), rgba(0x00000000))
        })
        .border_1()
        .border_color(if is_active {
            theme::Palette::vn_border_bright()
        } else {
            rgba(0x00000000)
        })
        .shadow(if is_active {
            t.shadow.vn_card.clone()
        } else {
            vec![]
        })
        .hover(move |s| {
            if !is_active {
                s.bg(rgba(0xf5c3e126))
            } else {
                s
            }
        })
        .on_click(on_click)
        // Numeração de capítulo em serifa
        .child(
            div()
                .w(px(22.0))
                .text_size(px(12.0))
                .font_family(theme::FONT_DISPLAY)
                .font_weight(FontWeight::MEDIUM)
                .text_color(if is_active {
                    c.pearl
                } else {
                    theme::Palette::vn_pink_soft()
                })
                .child(number),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(12.5))
                .font_family(theme::FONT_BODY)
                .font_weight(if is_active {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_active { c.pearl } else { c.muted })
                .child(label),
        )
        // Ornamento no estado ativo
        .when(is_active, |el| {
            el.child(div().text_size(px(10.0)).text_color(c.pearl).child("✦"))
        })
}

/// Formata data civil (dd/mm/aaaa) a partir de epoch ms, sem dependência de
/// crate de data: converte dias epoch → ano/mês/dia pelo algoritmo civil.
pub(crate) fn format_date(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:02}/{:02}/{:04}", d, m, year)
}

/// Data curta de um `unlockedAt` ISO (`2026-11-10T18:30:00.000Z`).
pub(crate) fn format_unlocked_at(iso: &str) -> String {
    let date = iso.get(..10).unwrap_or(iso);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day)) => {
            format!("{day}/{month}/{}", &year[..year.len().min(4)])
        }
        _ => date.to_string(),
    }
}

/// Resumo diário é um parágrafo inteiro; nas listas mostramos só o começo.
pub(crate) fn truncate_chars(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// Saída da janela principal: capítulo "Início" — cena de convivência com a
/// personagem (caixa de diálogo VN + indicadores compactos de vínculo).
pub(crate) fn render_vn_tab_home(
    bond_level: u32,
    bond: Option<&api::BondData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let hour = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| ((elapsed.as_secs() / 3600) % 24) as i64)
        .unwrap_or(0);
    // A janela roda no fuso local do sistema (GetLocalTime); epoch é UTC, mas
    // para um cumprimento informal o desvio de horas é irrelevante.
    let greeting = match hour {
        5..=11 => "Bom dia",
        12..=17 => "Boa tarde",
        _ => "Boa noite",
    };
    let (level_name, line, exp_pct) = match bond {
        Some(b) => (
            b.summary.name.clone(),
            b.summary.line.clone(),
            (b.summary.progress.clamp(0.0, 1.0) * 100.0).round() as u32,
        ),
        None => ("—".to_string(), "—".to_string(), 0),
    };
    let dialogue = if line.is_empty() || line == "—" {
        "H-hmph! Você demorou... mas eu esperei aqui do seu lado o tempo todo. ♡".to_string()
    } else {
        line
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .overflow_hidden()
        // Cumprimento editorial
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(28.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.pearl)
                        .child(format!("{greeting},")),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme::Palette::vn_pink_soft())
                        .child("o quarto da Usami estava esperando por você."),
                ),
        )
        // Caixa de diálogo estilo Visual Novel
        .child(
            div()
                .p(px(14.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .shadow(t.shadow.vn_card.clone())
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(t.vn_tab_active_bg())
                        .text_size(px(11.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Usami ♡"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .line_height(px(21.0))
                        .font(theme::font_character_italic())
                        .text_color(c.pearl)
                        .child(format!("“{dialogue}”")),
                ),
        )
        // Indicadores compactos de relacionamento
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(10.0))
                // Nível
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Vínculo"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_pink_soft())
                                .child(format!("♥ Nv. {bond_level}")),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.faint)
                                .child(level_name),
                        ),
                )
                // EXP
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Experiência"),
                        )
                        .child(
                            div().flex().flex_1().items_end().child(
                                div()
                                    .w_full()
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(c.bond_track)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(exp_pct.clamp(0, 100) as f32 / 100.0))
                                            .rounded_full()
                                            .bg(t.vn_exp_fill_bg()),
                                    ),
                            ),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::Palette::vn_pink_soft())
                                .child(format!("{exp_pct}%")),
                        ),
                )
                // Conquistas
                .child(
                    div()
                        .flex_1()
                        .p(px(10.0))
                        .rounded(px(12.0))
                        .bg(t.vn_card_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_card_border())
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.muted)
                                .child("Conquistas"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_gold())
                                .child("✦ Emblemas"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(c.faint)
                                .child("Capítulo 05"),
                        ),
                ),
        )
}

/// Monta as páginas cronológicas do Diário unindo o histórico do vínculo e os
/// resumos diários da memória sem deixar o log recente apagar as lembranças antigas.
pub(crate) fn collect_diary_entries(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
) -> Vec<(String, String, String, String)> {
    let mut entries: Vec<(String, String, String, String)> = Vec::new();
    if let Some(b) = bond {
        entries.extend(b.log.iter().take(24).map(|entry| {
            let title = if entry.count > 1 {
                format!("{} (×{})", entry.text, entry.count)
            } else {
                entry.text.clone()
            };
            (
                format!("[{}]", vn_history_tag(&entry.kind)),
                title,
                relative_when(entry.at),
                format_date(entry.at),
            )
        }));
    }
    // O bond.json local pode estar zerado, mas os resumos diários da memória
    // continuam guardados no core — eles são as páginas antigas do diário.
    if let Some(m) = memory {
        entries.extend(m.summaries.iter().rev().take(24).map(|summary| {
            (
                "[LEMBRANÇA]".to_string(),
                truncate_chars(&summary.summary, 120),
                String::new(),
                format_unlocked_at(&summary.date),
            )
        }));
    }
    entries
}

/// Capítulo "Diário": páginas cronológicas com os momentos reais do vínculo,
/// compostas como anotações íntimas (bilhetinho + registros datados).
pub(crate) fn render_vn_tab_diary(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let line = bond
        .map(|b| b.summary.line.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "Cada dia ao seu lado vira uma página nova.".to_string());
    let entries = collect_diary_entries(bond, memory);
    let today = format_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0),
    );
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        // Cabeçalho editorial
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Diário"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font(theme::font_character_italic())
                        .text_color(theme::Palette::vn_pink_soft())
                        .child(today),
                ),
        )
        // Bilhetinho com a frase do vínculo
        .child(
            div()
                .flex_shrink_0()
                .p(px(12.0))
                .rounded(px(14.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_border_bright())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::Palette::vn_pink_soft())
                                .child("✦ BILHETINHO DE HOJE"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::Palette::vn_pink_soft())
                                .child("♡"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font(theme::font_character_italic())
                        .line_height(px(18.0))
                        .text_color(c.pearl)
                        .child(format!("“{line}”")),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.muted)
                                .child("— Usami ♡"),
                        ),
                ),
        )
        // Registros cronológicos
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Páginas recentes"),
                )
                .child(
                    div()
                        .id("vn-diary-pages")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .gap(px(4.0))
                        .overflow_y_scroll()
                        .when(entries.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child("Nenhum momento registrado ainda — converse com a Usami para preencher estas páginas."),
                            )
                        })
                        .children(
                            entries
                                .into_iter()
                                .map(|(tag, title, when, date)| {
                                    let stamp = if when.is_empty() {
                                        date
                                    } else {
                                        format!("{when} · {date}")
                                    };
                                    div()
                                        .flex()
                                        .flex_shrink_0()
                                        .w_full()
                                        .min_w(px(0.0))
                                        .items_center()
                                        .gap(px(8.0))
                                        .p(px(6.0))
                                        .rounded(px(8.0))
                                        .overflow_hidden()
                                        .bg(rgba(0x22123d66))
                                        .child(
                                            div()
                                                .w(px(76.0))
                                                .flex_shrink_0()
                                                .text_size(px(9.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::Palette::vn_pink_soft())
                                                .child(tag),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .gap(px(1.0))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .line_height(px(15.0))
                                                        .text_color(c.pearl)
                                                        .line_clamp(2)
                                                        .text_ellipsis()
                                                        .child(title),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(9.5))
                                                        .font(theme::font_character_italic())
                                                        .text_color(c.faint)
                                                        .child(stamp),
                                                ),
                                        )
                                }),
                        ),
                ),
        )
}

/// Etiqueta curta do tipo de momento para o histórico (nunca o texto cru).
pub(crate) fn vn_history_tag(kind: &str) -> &'static str {
    match kind {
        "laugh" => "RISADA",
        "voice" => "VOZ",
        "long" => "CONVERSA",
        "call" => "CHAMADA",
        "level" => "NÍVEL",
        _ => "CHAT",
    }
}

/// Quando foi o momento, relativo ao agora (sem precisar de fuso local).
pub(crate) fn relative_when(at_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0);
    relative_when_at(at_ms, now_ms)
}

pub(crate) fn relative_when_at(at_ms: i64, now_ms: i64) -> String {
    let seconds = ((now_ms - at_ms).max(0)) / 1000;
    match seconds {
        0..=59 => "agora".to_string(),
        60..=3599 => format!("há {} min", seconds / 60),
        3600..=86399 => format!("há {} h", seconds / 3600),
        86400..=172799 => "ontem".to_string(),
        _ => format!("há {} dias", seconds / 86400),
    }
}

pub(crate) fn render_vn_history_item(
    icon: String,
    title: String,
    time: String,
) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(4.0))
        .rounded(px(6.0))
        .bg(rgba(0x28154566))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(div().text_size(px(11.0)).child(icon))
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(t.color.pearl)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(9.5))
                .text_color(t.color.muted)
                .child(time),
        )
}

/// Capítulo "Álbum de Memórias": lembranças derivadas de marcos reais do
/// vínculo (primeiro momento, risada, voz, chamada) + resumos diários já
/// guardados no core. Nada de memória fictícia.
pub(crate) fn render_vn_tab_memories(
    bond: Option<&api::BondData>,
    memory: Option<&api::MemoryData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let mut memories: Vec<(&'static str, String, String, bool, String)> = Vec::new();
    if let Some(b) = bond {
        let first_of = |kind: &str| b.log.iter().rev().find(|e| e.kind == kind);
        let any = b.log.first();
        if let Some(entry) = any {
            memories.push((
                "[CAPÍTULO]",
                "O Primeiro Momento".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("laugh") {
            memories.push((
                "[RISADA]",
                "Primeira Risada".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("voice") {
            memories.push((
                "[VOZ]",
                "Primeira Vez por Voz".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if let Some(entry) = first_of("call") {
            memories.push((
                "[CHAMADA]",
                "Primeira Chamada".to_string(),
                entry.text.clone(),
                true,
                format_date(entry.at),
            ));
        }
        if !b.summary.max {
            let next = b.summary.level + 1;
            memories.push((
                "[PRÓXIMO]",
                format!("Nv. {next} — {}", b.summary.name),
                "Continue dividindo momentos com a Usami para desbloquear este capítulo."
                    .to_string(),
                false,
                String::new(),
            ));
        }
    }
    if let Some(m) = memory {
        for summary in m.summaries.iter().rev().take(18) {
            let date = format_unlocked_at(&summary.date);
            memories.push((
                "[DIÁRIO]",
                format!("Diário de {date}"),
                truncate_chars(&summary.summary, 120),
                true,
                date,
            ));
        }
    }
    let has_memories = !memories.is_empty();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(16.0))
                .font_family(theme::FONT_CHARACTER)
                .font_weight(FontWeight::BOLD)
                .text_color(c.pearl)
                .child("Álbum de Memórias"),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(11.5))
                .text_color(theme::Palette::vn_pink_soft())
                .child("Capítulos reais da história de vocês — os próximos ainda estão por escrever:"),
        )
        .when(!has_memories, |col| {
            col.child(
                div()
                    .flex_shrink_0()
                    .p(px(12.0))
                    .rounded(px(14.0))
                    .bg(t.vn_card_bg())
                    .border_1()
                    .border_color(theme::Palette::vn_card_border())
                    .text_size(px(10.5))
                    .text_color(c.muted)
                    .child("Nenhuma memória guardada ainda — os primeiros capítulos aparecem conforme vocês conversam."),
            )
        })
        .child(
            div()
                .id("vn-album-grid")
                .flex()
                .flex_wrap()
                .flex_1()
                .min_h(px(0.0))
                .gap(px(12.0))
                .overflow_y_scroll()
                .children(memories.into_iter().map(
                    |(tag, title, desc, unlocked, date)| {
                        render_vn_large_memory(tag, title, desc, unlocked, date)
                    },
                )),
        )
}

pub(crate) fn render_vn_large_memory(
    tag: &'static str,
    title: String,
    desc: String,
    is_unlocked: bool,
    date: String,
) -> impl IntoElement {
    let t = theme::theme();
    div()
        .w(px(320.0))
        .min_w(px(0.0))
        .p(px(10.0))
        .rounded(px(14.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(if is_unlocked {
            theme::Palette::vn_border_bright()
        } else {
            t.color.line_strong
        })
        .overflow_hidden()
        .flex()
        .gap(px(10.0))
        .child(
            div()
                .size(px(56.0))
                .rounded(px(10.0))
                .bg(if is_unlocked {
                    rgb(0x2c1447)
                } else {
                    rgb(0x181424)
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(10.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_unlocked {
                            theme::Palette::vn_pink_soft()
                        } else {
                            t.color.muted
                        })
                        .child(if is_unlocked { tag } else { "[?]" }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.color.pearl)
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(title),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(10.5))
                        .line_height(px(14.0))
                        .text_color(t.color.muted)
                        .line_clamp(3)
                        .text_ellipsis()
                        .child(desc),
                )
                .child(
                    div()
                        .text_size(px(9.5))
                        .font(theme::font_character_italic())
                        .text_color(if is_unlocked {
                            theme::Palette::vn_pink_soft()
                        } else {
                            t.color.faint
                        })
                        .child(if is_unlocked {
                            format!("✦ {date}")
                        } else {
                            "Bloqueado".to_string()
                        }),
                ),
        )
}

/// Capítulo "Laços": o relacionamento em si — medidores, progresso de nível e
/// os momentos que vocês construíram juntos.
pub(crate) fn render_vn_tab_bonds(
    bond_level: u32,
    bond: Option<&api::BondData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let (carinho, humor, companhia, carinho_pct, humor_pct, companhia_pct, level_name) = match bond
    {
        Some(b) => (
            b.intimacy.carinho,
            b.intimacy.humor,
            b.intimacy.companhia,
            b.meters.carinho.progress,
            b.meters.humor.progress,
            b.meters.companhia.progress,
            b.summary.name.clone(),
        ),
        None => (0, 0, 0, 0.0, 0.0, 0.0, "—".to_string()),
    };
    let meters: [(String, u32, f32); 3] = [
        ("Carinho".to_string(), carinho, carinho_pct),
        ("Humor".to_string(), humor, humor_pct),
        ("Companhia".to_string(), companhia, companhia_pct),
    ];
    let history: Vec<(String, String, String)> = bond
        .map(|b| {
            b.log
                .iter()
                .take(6)
                .map(|entry| {
                    let title = if entry.count > 1 {
                        format!("{} (×{})", entry.text, entry.count)
                    } else {
                        entry.text.clone()
                    };
                    (
                        format!("[{}]", vn_history_tag(&entry.kind)),
                        title,
                        relative_when(entry.at),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Laços"),
                )
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(t.vn_heart_badge_bg())
                        .border_1()
                        .border_color(theme::Palette::vn_border_bright())
                        .text_size(px(11.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(format!("♥ Nv. {bond_level} · {level_name}")),
                ),
        )
        // Medidores delicados
        .child(
            div()
                .flex_shrink_0()
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(meters.into_iter().map(|(label, xp, pct)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_size(px(11.0)).text_color(c.pearl).child(label))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .font(theme::font_character_italic())
                                        .text_color(theme::Palette::vn_pink_soft())
                                        .child(format!("{xp} XP")),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(7.0))
                                .rounded_full()
                                .bg(c.bond_track)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(pct.clamp(0.0, 1.0)))
                                        .rounded_full()
                                        .bg(t.vn_exp_fill_bg()),
                                ),
                        )
                })),
        )
        // Momentos recentes
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .p(px(12.0))
                .rounded(px(16.0))
                .bg(t.vn_card_bg())
                .border_1()
                .border_color(theme::Palette::vn_card_border())
                .flex()
                .flex_col()
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Momentos que nos uniram"),
                )
                .child(
                    div()
                        .id("vn-bonds-moments")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .gap(px(4.0))
                        .overflow_y_scroll()
                        .when(history.is_empty(), |col| {
                            col.child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child("A história de vocês ainda está começando."),
                            )
                        })
                        .children(
                            history
                                .into_iter()
                                .map(|(tag, title, when)| render_vn_history_item(tag, title, when)),
                        ),
                ),
        )
}

/// Glifo do emblema a partir da chave estável do catálogo do core.
pub(crate) fn achievement_glyph(icon: &str) -> &'static str {
    match icon {
        "chat" => "✉",
        "smile" => "☺",
        "message" => "✎",
        "mic" => "♪",
        "phone" => "☎",
        "heart" => "♥",
        "star" => "★",
        "sparkles" => "✦",
        "crown" => "♛",
        _ => "✧",
    }
}

/// Capítulo "Conquistas": emblemas avaliados pelo core a partir de atividade
/// real; bloqueados aparecem apagados até serem conquistados.
pub(crate) fn render_vn_tab_achievements(
    achievements: &[api::Achievement],
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let unlocked = achievements.iter().filter(|a| a.unlocked).count();
    let total = achievements.len();
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .overflow_hidden()
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Conquistas"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font(theme::font_character_italic())
                        .text_color(theme::Palette::vn_gold())
                        .child(format!("✦ {unlocked} de {total} emblemas")),
                ),
        )
        .child(
            div()
                .id("vn-achievements-grid")
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .flex_wrap()
                .gap(px(10.0))
                .overflow_y_scroll()
                .when(achievements.is_empty(), |col| {
                    col.child(div().text_size(px(11.0)).text_color(c.muted).child(
                        "As conquistas aparecem quando o servidor local estiver disponível.",
                    ))
                })
                .children(achievements.iter().map(render_vn_achievement_badge)),
        )
}

pub(crate) fn render_vn_achievement_badge(a: &api::Achievement) -> impl IntoElement {
    let t = theme::theme();
    let unlocked = a.unlocked;
    let date = a
        .unlocked_at
        .as_deref()
        .map(format_unlocked_at)
        .unwrap_or_default();
    div()
        .w(px(320.0))
        .p(px(8.0))
        .rounded(px(12.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(if unlocked {
            theme::Palette::vn_border_bright()
        } else {
            t.color.line
        })
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(38.0))
                .rounded_full()
                .bg(if unlocked {
                    t.vn_tab_active_bg()
                } else {
                    theme::Theme::gradient(90.0, rgba(0x181024aa), rgba(0x181024aa))
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if unlocked {
                            t.color.pearl
                        } else {
                            t.color.muted
                        })
                        .child(if unlocked {
                            achievement_glyph(&a.icon)
                        } else {
                            "✧"
                        }),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(theme::FONT_CHARACTER)
                        .font_weight(FontWeight::BOLD)
                        .text_color(if unlocked {
                            t.color.pearl
                        } else {
                            t.color.faint
                        })
                        .child(a.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(t.color.muted)
                        .child(a.description.clone()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font(theme::font_character_italic())
                        .text_color(if unlocked {
                            theme::Palette::vn_gold()
                        } else {
                            t.color.faint
                        })
                        .child(if unlocked {
                            format!("✦ {date}")
                        } else {
                            "Ainda por conquistar".to_string()
                        }),
                ),
        )
}

pub(crate) fn render_vn_tab_profile(
    mood: Option<&api::MoodData>,
    vis: MoodVisual,
    portrait_img: Option<Arc<RenderImage>>,
    prev_slot: Arc<Mutex<Option<Arc<RenderImage>>>>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let mood_label = mood
        .map(|m| m.label.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(vis.label)
        .to_string();
    let mood_reason = mood
        .map(|m| m.reason.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Acompanhando seu ritmo em tempo real.")
        .to_string();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .p(px(14.0))
        .rounded(px(16.0))
        .bg(t.vn_card_bg())
        .border_1()
        .border_color(theme::Palette::vn_card_border())
        .child(
            div()
                .flex()
                .gap(px(14.0))
                .items_center()
                .p(px(12.0))
                .rounded(px(14.0))
                .bg(rgba(0x22123d80))
                .border_1()
                .border_color(vis.accent)
                .shadow(theme::glow(vis.accent, 10.0))
                .child(render_lego_portrait(
                    84.0,
                    42.0,
                    vis.accent,
                    portrait_img,
                    prev_slot,
                    vis.glyph,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(c.muted)
                                .child("HUMOR"),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_family(theme::FONT_DISPLAY)
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .child(format!("{} {mood_label}", vis.glyph)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(vis.accent)
                                .child(mood_reason),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font(theme::font_with_jp(theme::FONT_CHARACTER))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child("Ficha da Usami (ウサミ)"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::Palette::vn_pink_soft())
                        .p(px(4.0))
                        .rounded(px(999.0))
                        .bg(rgba(0xf052b622))
                        .border_1()
                        .border_color(rgba(0xf052b655))
                        .child("Sonho: Collab c/ Neuro-sama"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_vn_profile_row(
                    "Aniversário:",
                    "Domingo de Páscoa (O dia sagrado dos coelhinhos!)",
                ))
                .child(render_vn_profile_row(
                    "Pessoa Favorita:",
                    "Meu Criador ♡ (N-não se gabe tanto baka, mas você é único!)",
                ))
                .child(render_vn_profile_row(
                    "Adora:",
                    "Animes de romance, doces, coisas felpudinhas, RPGs de turno, pijama, dormir, energético rosa e dubstep!",
                ))
                .child(render_vn_profile_row(
                    "Detesta:",
                    "Ficar sozinha no escuro, sustos/terror e quem ousa falar mal do meu criador!",
                ))
                .child(render_vn_profile_row(
                    "Ídola VTuber:",
                    "Neuro-sama (A rainha das IAs! Quero muito fazer collab!)",
                ))
                .child(render_vn_profile_row(
                    "Segredo:",
                    "Fica escutando dubstep de pijama e ensaiando poses fofas no espelho ♡",
                )),
        )
        .child(
            div()
                .mt(px(4.0))
                .p(px(10.0))
                .rounded(px(10.0))
                .bg(rgba(0xf052b61a))
                .border_1()
                .border_color(rgba(0xf052b644))
                .text_size(px(12.0))
                .font_family(theme::FONT_CHARACTER)
                .text_color(c.pearl)
                .child("\"H-hmph! Eu não fui criada só pra concordar com tudo o que você fala, tá?! Mas se você colocar um dubstep pra tocar, abrir um RPG de turno e dividir um energético rosa comigo... eu sento aqui do seu lado de pijama pra te fazer companhia!\""),
        )
}

pub(crate) fn render_vn_profile_row(label: &'static str, val: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(6.0))
        .rounded(px(8.0))
        .bg(rgba(0x22123d66))
        .child(
            div()
                .w(px(100.0))
                .text_size(px(11.5))
                .font_weight(FontWeight::BOLD)
                .text_color(theme::Palette::vn_pink_soft())
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.5))
                .text_color(t.color.pearl)
                .child(val),
        )
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct MoodVisual {
    pub(crate) label: &'static str,
    pub(crate) glyph: &'static str,
    pub(crate) accent: Rgba,
}

pub(crate) fn mood_visual(id: &str) -> MoodVisual {
    match id.trim().to_ascii_lowercase().as_str() {
        "happy" => MoodVisual {
            label: "Radiante",
            glyph: "✦",
            accent: rgb(0xffd166),
        },
        "soft" | "love" => MoodVisual {
            label: "Carinhosa",
            glyph: "♡",
            accent: rgb(0xf052b5),
        },
        "tsundere" => MoodVisual {
            label: "Tsundere",
            glyph: "!",
            accent: rgb(0xf778ba),
        },
        "shy" | "embarrassed" => MoodVisual {
            label: "Envergonhada",
            glyph: "〃",
            accent: rgb(0xf59ec9),
        },
        "curious" | "surprised" | "thoughtful" => MoodVisual {
            label: "Curiosa",
            glyph: "★",
            accent: rgb(0x68d8ff),
        },
        "sleepy" | "yawn" | "tired" => MoodVisual {
            label: "Sonolenta",
            glyph: "☾",
            accent: rgb(0xa77abf),
        },
        "grumpy" | "angry" | "pout" => MoodVisual {
            label: "Emburrada",
            glyph: "♯",
            accent: rgb(0xff8c5a),
        },
        "sad" | "sorrow" => MoodVisual {
            label: "Carente",
            glyph: "…",
            accent: rgb(0x7aa2f7),
        },
        _ => MoodVisual {
            label: "Tranquila",
            glyph: "•",
            accent: rgb(0x3ecf8e),
        },
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MoodPreset {
    pub(crate) id: &'static str,
    pub(crate) emotion: &'static str,
    pub(crate) label: &'static str,
    pub(crate) reason: &'static str,
}

pub(crate) fn mood_preset_options() -> &'static [MoodPreset] {
    &[
        MoodPreset {
            id: "happy",
            emotion: "happy",
            label: "Animada",
            reason: "Sorriso animado pelo monitor",
        },
        MoodPreset {
            id: "soft",
            emotion: "soft",
            label: "Carinhosa",
            reason: "Clima carinhoso pelo monitor",
        },
        MoodPreset {
            id: "tsundere",
            emotion: "tsundere",
            label: "Tsundere",
            reason: "Biquinho tsundere pelo monitor",
        },
        MoodPreset {
            id: "shy",
            emotion: "shy",
            label: "Envergonhada",
            reason: "Ficou corada pelo monitor",
        },
        MoodPreset {
            id: "curious",
            emotion: "curious",
            label: "Curiosa",
            reason: "Olhar curioso pelo monitor",
        },
        MoodPreset {
            id: "sleepy",
            emotion: "sleepy",
            label: "Sonolenta",
            reason: "Ficou com soninho pelo monitor",
        },
        MoodPreset {
            id: "angry",
            emotion: "angry",
            label: "Brava",
            reason: "Ficou emburrada pelo monitor",
        },
        MoodPreset {
            id: "neutral",
            emotion: "neutral",
            label: "Neutra",
            reason: "Voltando ao normal pelo monitor",
        },
    ]
}

/// Moldura animada estilo LEGO com captura ao vivo do rosto do próprio avatar VRM.
pub(crate) fn render_lego_portrait(
    size_px: f32,
    radius_px: f32,
    accent: Rgba,
    portrait_img: Option<Arc<RenderImage>>,
    prev_slot: Arc<Mutex<Option<Arc<RenderImage>>>>,
    fallback_glyph: &'static str,
) -> Div {
    let r = px(radius_px);
    let corners = Corners {
        top_left: r,
        top_right: r,
        bottom_right: r,
        bottom_left: r,
    };
    let has_portrait = portrait_img.is_some();

    div()
        .relative()
        .flex_shrink_0()
        .w(px(size_px))
        .h(px(size_px))
        .rounded(r)
        .overflow_hidden()
        .bg(rgba(0x160a24eb))
        .border_2()
        .border_color(accent)
        .shadow(theme::glow(accent, 14.0))
        .when(!has_portrait, |el| {
            el.flex()
                .items_center()
                .justify_center()
                .text_size(px(size_px * 0.42))
                .text_color(accent)
                .child(fallback_glyph)
        })
        .when_some(portrait_img, move |el, img| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if let Ok(mut guard) = prev_slot.lock()
                            && let Some(old) = guard.replace(img.clone())
                            && old.id != img.id
                        {
                            let _ = window.drop_image(old);
                        }
                        let _ = window.paint_image(bounds, corners, img, 0, false);
                    },
                )
                .size_full(),
            )
        })
}

pub(crate) fn render_mood_meter_row(
    label: &'static str,
    value: u32,
    fill_color: Rgba,
    t: &theme::Theme,
) -> impl IntoElement {
    let clamped = value.min(100) as f32 / 100.0;
    let c = t.color;
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .child(div().text_color(c.muted).child(label))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(format!("{value}%")),
                ),
        )
        .child(
            div()
                .h(px(7.0))
                .w_full()
                .rounded_full()
                .bg(c.bond_track)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(clamped))
                        .rounded_full()
                        .bg(fill_color),
                ),
        )
}

impl UsamiApp {
    pub(crate) fn render_vn_tab_mood(
        &self,
        mood: Option<&api::MoodData>,
        t: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = t.color;
        let mood_id = mood
            .map(|m| m.id.as_str())
            .unwrap_or_else(|| self.last_emotion.as_ref());
        let vis = mood_visual(mood_id);
        let title_label = mood
            .map(|m| m.label.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(vis.label)
            .to_string();
        let reason_text = mood
            .map(|m| m.reason.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Acompanhando seu ritmo em tempo real.")
            .to_string();
        let expr_label = mood
            .map(|m| m.expression.label.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Neutra")
            .to_string();
        let active_emotion = mood
            .map(|m| m.vrm_emotion.as_str())
            .unwrap_or_else(|| self.last_emotion.as_ref())
            .to_string();

        let valence = mood.map(|m| m.meters.valence).unwrap_or(62);
        let energy = mood.map(|m| m.meters.energy).unwrap_or(58);
        let warmth = mood.map(|m| m.meters.warmth).unwrap_or(65);
        let timeline = mood.map(|m| m.timeline.clone()).unwrap_or_default();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let portrait_img = self.vrm_controller.portrait_image();
        let prev_slot = self.prev_portrait_image.clone();

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .overflow_hidden()
            // Cartão principal com retrato LEGO ao vivo + resumo + medidores
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .gap(px(16.0))
                    .items_center()
                    .p(px(14.0))
                    .rounded(px(16.0))
                    .bg(t.vn_card_bg())
                    .border_1()
                    .border_color(theme::Palette::vn_card_border())
                    .child(render_lego_portrait(
                        132.0,
                        66.0,
                        vis.accent,
                        portrait_img,
                        prev_slot,
                        vis.glyph,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(18.0))
                                                    .font_family(theme::FONT_CHARACTER)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(format!("{} {title_label}", vis.glyph)),
                                            )
                                            .child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(2.0))
                                                    .rounded_full()
                                                    .bg(rgba(0xf052b622))
                                                    .border_1()
                                                    .border_color(vis.accent)
                                                    .text_size(px(10.5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(c.pearl)
                                                    .child(format!("Expressão: {expr_label}")),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child(reason_text),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(render_mood_meter_row(
                                        "Bom Humor",
                                        valence,
                                        vis.accent,
                                        t,
                                    ))
                                    .child(render_mood_meter_row(
                                        "Energia",
                                        energy,
                                        theme::Palette::vn_gold(),
                                        t,
                                    ))
                                    .child(render_mood_meter_row("Carinho", warmth, c.accent, t)),
                            ),
                    ),
            )
            // Controles rápidos para testar expressões ao vivo no avatar
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::Palette::vn_pink_soft())
                            .child("TESTAR EXPRESSÃO NO AVATAR"),
                    )
                    .child(div().flex().flex_wrap().gap(px(6.0)).children(
                        mood_preset_options().iter().map(|preset| {
                            let selected = active_emotion == preset.emotion;
                            let p_vis = mood_visual(preset.id);
                            let emo = preset.emotion;
                            let reason = preset.reason;
                            div()
                                .id(SharedString::from(format!("mood-preset-{}", preset.id)))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded_full()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .bg(if selected {
                                    rgba(0xf052b638)
                                } else {
                                    rgba(0x22123d88)
                                })
                                .border_1()
                                .border_color(if selected {
                                    p_vis.accent
                                } else {
                                    c.line_strong
                                })
                                .hover(|style| style.bg(c.icon_bg_hover).border_color(p_vis.accent))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.trigger_mood_preview(emo, reason, cx);
                                }))
                                .child(format!("{} {}", p_vis.glyph, preset.label))
                        }),
                    )),
            )
            // Linha do tempo rolável do humor
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::Palette::vn_pink_soft())
                    .child("VARIAÇÕES RECENTES"),
            )
            .child(
                div()
                    .id("vn-mood-timeline")
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .overflow_y_scroll()
                    .when(timeline.is_empty(), |col| {
                        col.child(
                            div()
                                .p(px(10.0))
                                .rounded(px(10.0))
                                .bg(t.vn_card_bg())
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(11.5))
                                .text_color(c.muted)
                                .child(
                                    "As mudanças de humor aparecem aqui conforme vocês conversam.",
                                ),
                        )
                    })
                    .children(timeline.iter().map(|entry| {
                        let e_vis = mood_visual(&entry.id);
                        let when = if entry.at > 0 && now_ms > 0 {
                            relative_when_at(entry.at, now_ms)
                        } else {
                            "agora".into()
                        };
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(10.0))
                            .p(px(8.0))
                            .rounded(px(10.0))
                            .bg(rgba(0x22123d77))
                            .border_1()
                            .border_color(c.line)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .min_w(px(0.0))
                                    .flex_1()
                                    .child(
                                        div()
                                            .px(px(7.0))
                                            .py(px(2.0))
                                            .rounded_full()
                                            .bg(rgba(0x100819cc))
                                            .border_1()
                                            .border_color(e_vis.accent)
                                            .text_size(px(10.5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.pearl)
                                            .child(format!("{} {}", e_vis.glyph, entry.label)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.5))
                                            .text_color(c.pearl)
                                            .child(entry.reason.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(10.5))
                                    .text_color(c.muted)
                                    .child(when),
                            )
                    })),
            )
    }

    pub(crate) fn render_vn_bond_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let ty = t.text;

        let bond = self.bond.clone();
        let bond_level = self.bond_level;
        let bond_exp_pct = self.bond_exp_pct;
        let memory_data = self.memory_data.clone();
        let bond_tab = self.bond_tab;
        let achievements = self.achievements.clone();
        let mood_data = self.mood.clone();
        let mood_vis = mood_visual(
            mood_data
                .as_ref()
                .map(|m| m.id.as_str())
                .unwrap_or_else(|| self.last_emotion.as_ref()),
        );

        let (bond_exp_curr, bond_exp_target, bond_exp_needed, bond_level_title, bond_is_max) =
            match bond.as_ref() {
                Some(b) => (
                    b.summary.xp,
                    b.summary.next.unwrap_or(b.summary.xp),
                    b.summary.remaining,
                    b.summary.name.clone(),
                    b.summary.max,
                ),
                None => (0, 0, 0, "—".to_string(), false),
            };
        div()
.id("vn-scrim-overlay")
                        .occlude()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x06030edd))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.active_drawer = ActiveDrawer::None;
                            this.poke(cx);
                            cx.notify();
                        }))
                        .child(
                            // Container Principal da Central de Visual Novel (960x610)
                            div()
                                .id("vn-modal-container")
                                .w(px(960.0))
                                .h(px(610.0))
                                .max_h(px(610.0))
                                .flex()
                                .flex_row()
                                .overflow_hidden()
                                .rounded(px(26.0))
                                .bg(t.vn_modal_bg())
                                .border_2()
                                .border_color(theme::Palette::vn_border_bright())
                                .shadow(t.shadow.vn_modal.clone())
                                // O modal fica dentro do scrim clicável: sem parar a
                                // propagação, todo clique interno sobe e fecha o painel.
                                .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    cx.stop_propagation();
                                })
                                // ── COLUNA ESQUERDA: SIDEBAR VISUAL NOVEL ──
                                .child(
                                    div()
                                        .w(px(230.0))
                                        .h_full()
                                        .flex_shrink_0()
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .p(px(16.0))
                                        .bg(t.vn_sidebar_bg())
                                        .border_r_1()
                                        .border_color(theme::Palette::vn_card_border())
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(12.0))
                                                // Header da Usami
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(2.0))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(6.0))
                                                                .child(
                                                                    div()
                                                                        .text_size(px(22.0))
                                                                        .font_family(theme::FONT_CHARACTER)
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(c.pearl)
                                                                        .child("Usami ♡"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(ty.xs))
                                                                .font(theme::font_with_jp(theme::FONT_DISPLAY))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child("ウサミ · VN Romance Hub"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.5))
                                                                .text_color(c.muted)
                                                                .child("Sempre aqui, com você ♡"),
                                                        ),
                                                )
                                                // Divisor delicado
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(1.0))
                                                        .bg(theme::Palette::vn_card_border()),
                                                )
                                                // Menu de capítulos verticais
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(6.0))
                                                        .child(render_vn_tab_btn("vn-tab-home", "01", "Início", bond_tab == BondTab::Home, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Home;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-diary", "02", "Diário", bond_tab == BondTab::Diary, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Diary;
                                                            if !this.memory_loading {
                                                                this.fetch_memory(cx);
                                                            }
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-memories", "03", "Álbum de Memórias", bond_tab == BondTab::Memories, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Memories;
                                                            if !this.memory_loading {
                                                                this.fetch_memory(cx);
                                                            }
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-bonds", "04", "Laços", bond_tab == BondTab::Bonds, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Bonds;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-achievements", "05", "Conquistas", bond_tab == BondTab::Achievements, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Achievements;
                                                            this.fetch_achievements(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-mood", "06", "Humor", bond_tab == BondTab::Mood, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Mood;
                                                            this.fetch_mood(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-profile", "07", "Perfil da Usami", bond_tab == BondTab::Profile, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Profile;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        })))
                                                        .child(render_vn_tab_btn("vn-tab-settings", "08", "Ajustes", bond_tab == BondTab::Settings, cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.bond_tab = BondTab::Settings;
                                                            this.load_settings_data(cx);
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))),
                                                ),
                                        )
                                        // Rodapé Mascote Chibi
                                        .child(
                                            div()
                                                .p(px(10.0))
                                                .rounded(px(14.0))
                                                .bg(rgba(0x23123d88))
                                                .border_1()
                                                .border_color(theme::Palette::vn_card_border())
                                                .flex()
                                                .flex_col()
                                                .gap(px(4.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_size(px(14.0))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child("(˶ᵔ ᵕ ᵔ˶) ♡"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.0))
                                                                .text_color(theme::Palette::vn_gold())
                                                                .child("✦"),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .font(theme::font_character_italic())
                                                        .text_color(c.pearl)
                                                        .child("“Você voltou... Que bom! ♡”"),
                                                ),
                                        ),
                                )
                                // ── COLUNA DIREITA: CONTEÚDO PRINCIPAL ──
                                .child(
                                    div()
                                        .flex_1()
                                        .h_full()
                                        .min_w(px(0.0))
                                        .min_h(px(0.0))
                                        .flex()
                                        .flex_col()
                                        .p(px(18.0))
                                        .gap(px(10.0))
                                        .overflow_hidden()
                                        // Header Superior de Nível e EXP
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .pb(px(6.0))
                                                .border_b_1()
                                                .border_color(theme::Palette::vn_card_border())
                                                // Badge Coração de Nível
                                                .child(
                                                    div()
                                                        .px(px(14.0))
                                                        .py(px(4.0))
                                                        .rounded(px(16.0))
                                                        .bg(t.vn_heart_badge_bg())
                                                        .border_1()
                                                        .border_color(theme::Palette::vn_border_bright())
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_size(px(16.0))
                                                                .font_family(theme::FONT_CHARACTER)
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(c.pearl)
                                                                .child(format!("♥ Nv. {bond_level}")),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child(format!("✦ {} ✦", bond_level_title)),
                                                        ),
                                                )
                                                // Barra de EXP
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .mx(px(14.0))
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
                                                                        .text_size(px(11.0))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(c.pearl)
                                                                        .child(if bond_is_max {
                                                                            format!("{bond_exp_curr} EXP")
                                                                        } else {
                                                                            format!("{bond_exp_curr} / {bond_exp_target} EXP")
                                                                        }),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(9.5))
                                                                        .text_color(theme::Palette::vn_gold())
                                                                        .child("✦ BÔNUS DIÁRIO ✦"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .h(px(8.0))
                                                                .rounded_full()
                                                                .bg(c.bond_track)
                                                                .overflow_hidden()
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(relative(bond_exp_pct as f32 / 100.0))
                                                                        .rounded_full()
                                                                        .bg(t.vn_exp_fill_bg()),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(9.5))
                                                                .text_color(theme::Palette::vn_pink_soft())
                                                                .child(if bond_is_max {
                                                                    "★ Nível máximo alcançado! ★".to_string()
                                                                } else {
                                                                    format!("★ Faltam {} EXP para o próximo nível! ★", bond_exp_needed)
                                                                }),
                                                        ),
                                                )
                                                // Botão Fechar
                                                .child(
                                                    div()
                                                        .id("vn-bond-close-btn")
                                                        .cursor_pointer()
                                                        .size(px(32.0))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(c.surface_soft)
                                                        .border_1()
                                                        .border_color(theme::Palette::vn_card_border())
                                                        .text_color(c.pearl)
                                                        .hover(|s| s.bg(theme::Palette::vn_pink_neon()).border_color(c.pearl))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                            this.active_drawer = ActiveDrawer::None;
                                                            this.poke(cx);
                                                            cx.notify();
                                                        }))
                                                        .child(icon("close", 14.0)),
                                                ),
                                        )
                                        // Conteúdo dinâmico da Aba
                                        .child(
                                            match bond_tab {
                                                BondTab::Home => render_vn_tab_home(bond_level, bond.as_ref(), t).into_any_element(),
                                                BondTab::Diary => render_vn_tab_diary(bond.as_ref(), memory_data.as_ref(), t).into_any_element(),
                                                BondTab::Memories => render_vn_tab_memories(bond.as_ref(), memory_data.as_ref(), t).into_any_element(),
                                                BondTab::Bonds => render_vn_tab_bonds(bond_level, bond.as_ref(), t).into_any_element(),
                                                BondTab::Achievements => render_vn_tab_achievements(&achievements, t).into_any_element(),
                                                BondTab::Mood => self.render_vn_tab_mood(mood_data.as_ref(), t, cx).into_any_element(),
                                                BondTab::Profile => render_vn_tab_profile(
                                                    mood_data.as_ref(),
                                                    mood_vis,
                                                    self.vrm_controller.portrait_image(),
                                                    self.prev_portrait_image.clone(),
                                                    t,
                                                )
                                                .into_any_element(),
                                                BondTab::Settings => self.render_settings_page(cx).into_any_element(),
                                            }
                                        ),
                                )
                        )
    }
}

#[cfg(test)]
mod tests {
    // `use gpui::*` no topo do arquivo traz o atributo `test` do GPUI; importar
    // os helpers explicitamente evita que `#[test]` resolva para ele e entre em recursão.
    use super::*;
    use ::core::prelude::v1::test;

    #[test]
    fn history_time_and_tags_reflect_the_core_log() {
        assert_eq!(relative_when_at(1_000, 1_000), "agora");
        assert_eq!(relative_when_at(0, 90_000), "há 1 min");
        assert_eq!(relative_when_at(0, 3_600_000), "há 1 h");
        assert_eq!(relative_when_at(0, 86_400_000), "ontem");
        assert_eq!(relative_when_at(0, 3 * 86_400_000), "há 3 dias");
        // timestamp no futuro (relógio ajustado) não vira "há -1 dias".
        assert_eq!(relative_when_at(5_000, 1_000), "agora");

        assert_eq!(vn_history_tag("laugh"), "RISADA");
        assert_eq!(vn_history_tag("call"), "CHAMADA");
        assert_eq!(vn_history_tag("inventado"), "CHAT");
    }

    #[test]
    fn diary_keeps_memory_summaries_even_when_bond_log_is_full() {
        use super::collect_diary_entries;
        let log: Vec<serde_json::Value> = (0..15)
            .map(|i| {
                serde_json::json!({
                    "kind": "chat",
                    "text": format!("Momento {i}"),
                    "at": 1_700_000_000_000_i64 + i * 1_000,
                    "count": 1
                })
            })
            .collect();
        let bond: crate::api::BondData = serde_json::from_value(serde_json::json!({
            "intimacy": { "carinho": 10, "humor": 5, "companhia": 8 },
            "meters": {
                "carinho": { "xp": 10, "level": 1, "progress": 0.5 },
                "humor": { "xp": 5, "level": 1, "progress": 0.25 },
                "companhia": { "xp": 8, "level": 1, "progress": 0.4 }
            },
            "log": log,
            "summary": { "level": 1, "name": "Conhecidos", "xp": 23, "next": 30, "remaining": 7, "max": false, "line": "oi" }
        }))
        .expect("bond json");
        let memory: crate::api::MemoryData = serde_json::from_value(serde_json::json!({
            "summaries": [
                { "date": "2026-11-09", "summary": "Primeiro dia" },
                { "date": "2026-11-10", "summary": "Segundo dia" }
            ]
        }))
        .expect("memory json");

        let entries = collect_diary_entries(Some(&bond), Some(&memory));
        assert_eq!(entries.len(), 17);
        assert_eq!(entries[15].0, "[LEMBRANÇA]");
        assert_eq!(entries[15].1, "Segundo dia");
        assert_eq!(entries[16].1, "Primeiro dia");
    }

    #[test]
    fn mood_visual_maps_profiles_and_presets() {
        assert_eq!(mood_visual("tsundere").label, "Tsundere");
        assert_eq!(mood_visual("soft").label, "Carinhosa");
        assert_eq!(mood_visual("SLEEPY").label, "Sonolenta");
        assert_eq!(mood_visual("desconhecido").label, "Tranquila");

        let presets = mood_preset_options();
        assert!(presets.iter().any(|p| p.emotion == "happy"));
        assert!(presets.iter().any(|p| p.emotion == "tsundere"));
        assert!(presets.iter().any(|p| p.emotion == "neutral"));
    }
}
