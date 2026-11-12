pub mod api;
pub mod audio;
#[cfg(target_os = "windows")]
mod presence;
pub mod recorder;
mod storage;
mod theme;

use crate::api::UsamiApiClient;
use crate::audio::AudioPlayer;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_vrm::{VrmController, VrmView};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveDrawer {
    None,
    Chat,
    Bond,
    Settings,
    Vrma,
}

/// Abas do painel de configurações (mesma ordem do web).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Chat,
    Voice,
    Avatar,
    Memory,
    Personality,
    About,
}

/// Capítulos do menu estilo Visual Novel ("Midnight Romance").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BondTab {
    Home,
    Diary,
    Memories,
    Bonds,
    Achievements,
    Mood,
    Profile,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sender {
    User,
    Usami,
}

#[derive(Clone)]
struct ChatMessage {
    sender: Sender,
    text: String,
    emotion: Option<String>,
}

fn stored_turns(turns: &[ChatMessage]) -> Vec<storage::StoredTurn> {
    turns
        .iter()
        .map(|message| storage::StoredTurn {
            sender: match message.sender {
                Sender::User => "user".into(),
                Sender::Usami => "usami".into(),
            },
            text: message.text.clone(),
            emotion: message.emotion.clone(),
        })
        .collect()
}

fn chat_message_from_stored(turn: storage::StoredTurn) -> Option<ChatMessage> {
    let sender = match turn.sender.as_str() {
        "user" => Sender::User,
        "usami" => Sender::Usami,
        _ => return None,
    };
    if turn.text.is_empty() {
        return None;
    }
    Some(ChatMessage {
        sender,
        text: turn.text,
        emotion: turn.emotion,
    })
}

fn window_minimized_or_hidden() -> bool {
    #[cfg(target_os = "windows")]
    {
        presence::app_is_minimized_or_hidden()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

const CLOSE_FAREWELL_MIN: Duration = Duration::from_millis(1200);
const CLOSE_FAREWELL_MAX: Duration = Duration::from_secs(10);

/// A janela só sai depois da despedida: mínimo legível, áudio terminado e avatar
/// sem falar — ou o teto rígido, para nunca travar o fechamento.
fn farewell_can_close(elapsed: Duration, audio_done: bool, speaking: bool) -> bool {
    (elapsed >= CLOSE_FAREWELL_MIN && audio_done && !speaking) || elapsed >= CLOSE_FAREWELL_MAX
}

// Caminhos relativos à crate: o app roda de qualquer lugar do disco e continua
// achando ícones, fundo, VRM e VRMA. Variáveis abaixo permitem apontar outra
// instalação sem recompilar.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Pasta empacotada (release): `assets/` ao lado do executável. Em
/// desenvolvimento não existe, então cai para a pasta da crate.
fn asset_base_dir() -> &'static Path {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
            && dir.join("assets").is_dir()
        {
            return dir.to_path_buf();
        }
        manifest_dir()
    })
}

fn asset_path(relative: &str) -> PathBuf {
    asset_base_dir().join(relative)
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

fn first_existing(candidates: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|path| path.exists())
}

fn icon(name: &str, size_px: f32) -> impl IntoElement {
    img(asset_path(&format!("assets/icons/{name}.svg")))
        .size(px(size_px))
        .flex_shrink_0()
}

// ── Peças do painel de configurações (mesmos estilos de `h3`, `p`, `.settings-hint`,
// `code`, `.rig-actions` e `.upload` do web) ──

fn settings_h3(label: &'static str) -> impl IntoElement {
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

fn settings_p(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1_5))
        .mb(px(t.space.s3))
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.muted)
        .child(text.into())
}

fn settings_hint(text: impl Into<SharedString>) -> impl IntoElement {
    let t = theme::theme();
    div()
        .mt(px(t.space.s1))
        .text_size(px(t.text.xs))
        .line_height(px(16.5))
        .text_color(t.color.muted)
        .child(text.into())
}

fn settings_code(text: &'static str) -> impl IntoElement {
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

#[allow(dead_code)]
fn settings_disabled_button(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .px(px(15.0))
        .py(px(9.0))
        .rounded(px(t.radius.sm))
        .bg(t.color.card_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.sm))
        .text_color(t.color.pearl)
        .opacity(0.4)
        .child(label)
}

fn settings_danger_button(label: &'static str) -> impl IntoElement {
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

#[allow(dead_code)]
fn settings_disabled_input(placeholder: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .h(px(38.0))
        .w_full()
        .px(px(t.space.s2_5))
        .flex()
        .items_center()
        .rounded(px(8.0))
        .bg(t.color.icon_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.sm))
        .text_color(t.color.faint)
        .opacity(0.55)
        .child(placeholder)
}

fn settings_dl_row(key: &'static str, value: impl Into<SharedString>) -> impl IntoElement {
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

#[allow(dead_code)]
fn settings_disabled_slider() -> impl IntoElement {
    let t = theme::theme();
    div()
        .h(px(16.0))
        .w_full()
        .my(px(t.space.s2_5))
        .flex()
        .items_center()
        .opacity(0.4)
        .child(
            div()
                .h(px(6.0))
                .w_full()
                .rounded_full()
                .bg(t.color.surface_soft)
                .border_1()
                .border_color(t.color.line_strong),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .size(px(14.0))
                .rounded_full()
                .bg(t.color.primary),
        )
}

// `.trait-row` do web: rótulo à esquerda, valor em destaque à direita e o
// slider logo abaixo. O trilho repete `settings_disabled_slider` sem as margens
// verticais, que aqui vêm do `gap` da linha.
#[allow(dead_code)]
fn settings_trait_row(label: &'static str) -> impl IntoElement {
    let t = theme::theme();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .text_size(px(t.text.md))
                .text_color(t.color.pearl)
                .child(label)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.color.primary)
                        .child("50"),
                ),
        )
        .child(
            div()
                .h(px(16.0))
                .w_full()
                .flex()
                .items_center()
                .opacity(0.4)
                .child(
                    div()
                        .h(px(6.0))
                        .w_full()
                        .rounded_full()
                        .bg(t.color.surface_soft)
                        .border_1()
                        .border_color(t.color.line_strong),
                )
                .child(
                    // Traço neutro: o valor de referência é 50, então o pino
                    // fica no centro do trilho.
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().size(px(14.0)).rounded_full().bg(t.color.primary)),
                ),
        )
}

#[allow(dead_code)]
fn settings_disabled_textarea() -> impl IntoElement {
    let t = theme::theme();
    div()
        .min_h(px(88.0))
        .w_full()
        .p(px(t.space.s2_5))
        .rounded(px(10.0))
        .bg(t.color.icon_bg)
        .border_1()
        .border_color(t.color.line_strong)
        .text_size(px(t.text.md))
        .line_height(px(21.0))
        .text_color(t.color.faint)
        .opacity(0.55)
        .child("Ex.: me chame de capitão; fale mais de coelhos.")
}

// Item de lista do web (`aside ul`): marcador e texto na cor do painel.
fn settings_list_item(text: &'static str) -> impl IntoElement {
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

fn proactivity_config_path() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(|appdata| {
        let dir = PathBuf::from(appdata).join("Usami");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("proactivity_config.json")
    })
}

#[derive(Serialize, Deserialize)]
struct ProactivityConfig {
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

fn load_proactive_config_from_disk() -> bool {
    if let Some(path) = proactivity_config_path() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<ProactivityConfig>(&content) {
                return cfg.enabled;
            }
        }
    }
    true
}

fn save_proactive_config_to_disk(enabled: bool) {
    if let Some(path) = proactivity_config_path() {
        let cfg = ProactivityConfig { enabled };
        if let Ok(json) = serde_json::to_string(&cfg) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn proactivity_seen_path() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(|appdata| {
        let dir = PathBuf::from(appdata).join("Usami");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("proactivity_seen.json")
    })
}

fn load_proactive_seen_from_disk() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    if let Some(path) = proactivity_seen_path() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&content) {
                for id in list {
                    set.insert(id);
                }
            }
        }
    }
    set
}

fn save_proactive_seen_to_disk(seen: &std::collections::HashSet<String>) {
    if let Some(path) = proactivity_seen_path() {
        let list: Vec<&String> = seen.iter().take(200).collect();
        if let Ok(json) = serde_json::to_string(&list) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[derive(Clone, Debug)]
struct AttachedMedia {
    filename: String,
    mime_type: String,
    data_base64: String,
    preview: Arc<gpui::Image>,
}

impl AttachedMedia {
    fn from_bytes(filename: String, mime_type: &str, bytes: Vec<u8>) -> Self {
        let format = match mime_type {
            "image/jpeg" | "image/jpg" => gpui::ImageFormat::Jpeg,
            "image/webp" => gpui::ImageFormat::Webp,
            "image/gif" => gpui::ImageFormat::Gif,
            "image/bmp" => gpui::ImageFormat::Bmp,
            _ => gpui::ImageFormat::Png,
        };
        let data_base64 = api::base64_encode(&bytes);
        let preview = Arc::new(gpui::Image::from_bytes(format, bytes));
        Self {
            filename,
            mime_type: mime_type.into(),
            data_base64,
            preview,
        }
    }
}

fn is_dead_accent_char(c: char) -> bool {
    matches!(c, '´' | '`' | '^' | '~' | '¨')
}

fn compose_dead_accent(accent: char, base: char) -> Option<char> {
    match (accent, base) {
        ('´', 'a') => Some('á'),
        ('´', 'e') => Some('é'),
        ('´', 'i') => Some('í'),
        ('´', 'o') => Some('ó'),
        ('´', 'u') => Some('ú'),
        ('´', 'y') => Some('ý'),
        ('´', 'c') => Some('ç'),
        ('´', 'A') => Some('Á'),
        ('´', 'E') => Some('É'),
        ('´', 'I') => Some('Í'),
        ('´', 'O') => Some('Ó'),
        ('´', 'U') => Some('Ú'),
        ('´', 'Y') => Some('Ý'),
        ('´', 'C') => Some('Ç'),
        ('`', 'a') => Some('à'),
        ('`', 'e') => Some('è'),
        ('`', 'i') => Some('ì'),
        ('`', 'o') => Some('ò'),
        ('`', 'u') => Some('ù'),
        ('`', 'A') => Some('À'),
        ('`', 'E') => Some('È'),
        ('`', 'I') => Some('Ì'),
        ('`', 'O') => Some('Ò'),
        ('`', 'U') => Some('Ù'),
        ('^', 'a') => Some('â'),
        ('^', 'e') => Some('ê'),
        ('^', 'i') => Some('î'),
        ('^', 'o') => Some('ô'),
        ('^', 'u') => Some('û'),
        ('^', 'A') => Some('Â'),
        ('^', 'E') => Some('Ê'),
        ('^', 'I') => Some('Î'),
        ('^', 'O') => Some('Ô'),
        ('^', 'U') => Some('Û'),
        ('~', 'a') => Some('ã'),
        ('~', 'o') => Some('õ'),
        ('~', 'n') => Some('ñ'),
        ('~', 'A') => Some('Ã'),
        ('~', 'O') => Some('Õ'),
        ('~', 'N') => Some('Ñ'),
        ('¨', 'a') => Some('ä'),
        ('¨', 'e') => Some('ë'),
        ('¨', 'i') => Some('ï'),
        ('¨', 'o') => Some('ö'),
        ('¨', 'u') => Some('ü'),
        ('¨', 'y') => Some('ÿ'),
        ('¨', 'A') => Some('Ä'),
        ('¨', 'E') => Some('Ë'),
        ('¨', 'I') => Some('Ï'),
        ('¨', 'O') => Some('Ö'),
        ('¨', 'U') => Some('Ü'),
        _ => None,
    }
}

fn push_key_char_with_dead_key(
    input_text: &mut String,
    pending_dead_key: &mut Option<char>,
    ch: &str,
) {
    let mut chars = ch.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return,
    };

    // Caso 1: é apenas um caractere (caso mais comum ao digitar)
    if chars.next().is_none() {
        if let Some(dead) = pending_dead_key.take() {
            if first == dead {
                // Pressionou o mesmo acento duas vezes seguidas -> emite o acento literal
                input_text.push(dead);
            } else if let Some(composed) = compose_dead_accent(dead, first) {
                // O Windows enviou a vogal crua após a dead key -> compomos manualmente
                input_text.push(composed);
            } else if !first.is_ascii() {
                // O kernel do Windows (ToUnicode com flag 1<<2) já compôs o caractere
                // na segunda tecla (ex: enviou '´' na dead key e 'í' na vogal) ->
                // descartamos o acento pendente duplicado e inserimos apenas 'í'.
                input_text.push(first);
            } else if first == ' ' {
                input_text.push(dead);
            } else {
                // Consoante/símbolo que não combina com o acento -> preserva ambos
                input_text.push(dead);
                input_text.push(first);
            }
            return;
        }

        if is_dead_accent_char(first) {
            *pending_dead_key = Some(first);
            return;
        }

        input_text.push(first);
        return;
    }

    // Caso 2: múltiplos caracteres no mesmo evento (ex: dead key + consoante vindo do OS)
    if let Some(dead) = pending_dead_key.take() {
        if !ch.starts_with(dead) {
            input_text.push(dead);
        }
    }
    input_text.push_str(ch);
}

fn normalize_utterance_key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_duplicate_utterance(
    last_commit: &mut Option<(String, Instant)>,
    text: &str,
    now: Instant,
) -> bool {
    let key = normalize_utterance_key(text);
    if key.is_empty() {
        return true;
    }
    if let Some((prev_key, prev_at)) = last_commit {
        if *prev_key == key && now.saturating_duration_since(*prev_at) < Duration::from_secs(5) {
            return true;
        }
    }
    *last_commit = Some((key, now));
    false
}

/// Sanitiza o texto de fala da Usami para a interface de diálogo,
/// garantindo que tags técnicas (`[intimate_shy]`, `[blush]`, etc.)
/// e sobras de animações não vazem no balão de fala da Visual Novel.
fn sanitize_speech_text(text: &str) -> String {
    let mut cleaned = String::new();
    let mut in_bracket = false;
    for ch in text.chars() {
        if ch == '[' {
            in_bracket = true;
            continue;
        }
        if ch == ']' {
            in_bracket = false;
            continue;
        }
        if !in_bracket {
            cleaned.push(ch);
        }
    }
    if let Some(pos) = cleaned.find("-prefab_") {
        if let Some(start) =
            cleaned[..pos].rfind(|c: char| c.is_ascii_digit() || c == ' ' || c == ',')
        {
            cleaned.truncate(start);
        }
    }
    cleaned.trim().to_string()
}

struct UsamiApp {
    focus_handle: FocusHandle,
    input_text: String,
    pending_dead_key: Option<char>,
    input_active: bool,
    caret_on: bool,
    status_text: SharedString,
    last_reply: SharedString,
    last_emotion: SharedString,
    show_bubble: bool,
    is_busy: bool,
    is_calling: bool,
    call_started_at: Option<Instant>,
    call_seconds: u64,
    is_listening: bool,
    listener: Option<recorder::VoiceListener>,
    mic_gate: Arc<recorder::MicGate>,
    is_companion_mode: bool,
    sleep_companion_started_at: Option<Instant>,
    sleep_companion_minutes_left: u32,
    is_muted: bool,
    is_ui_idle: bool,
    last_interaction: Instant,
    is_away: bool,
    away_since: Option<Instant>,
    window_focused: bool,
    last_user_activity: Instant,
    idle_prods: u8,
    last_return_greeting: Instant,
    was_maximized: bool,
    last_maximize_line: Instant,
    /// Despedida ao fechar: a primeira tentativa segura a janela, a segunda
    /// libera; o tick fecha sozinho quando o áudio acaba (ou estoura o teto).
    farewell_started_at: Option<Instant>,
    farewell_audio_done: Arc<AtomicBool>,
    close_allowed: bool,
    prefs: storage::UiPrefs,
    session: storage::SessionFile,
    last_prefs_save: Instant,
    window_subscriptions: Vec<Subscription>,
    close_subscription: Option<Subscription>,
    active_drawer: ActiveDrawer,
    settings_advanced_open: bool,
    bond_tab: BondTab,
    chat_history: Vec<ChatMessage>,
    /// Conquistas avaliadas pelo core (`GET /api/achievements`); lista vazia
    /// até a primeira resposta.
    achievements: Vec<api::Achievement>,

    /// Estado do vínculo vindo do core (`GET /api/bond`); None até a primeira
    /// resposta. `bond_level`/`bond_exp_pct` acompanham o summary para o pill.
    bond: Option<api::BondData>,
    /// Monitor de humor contínuo vindo do core (`GET /api/mood` / `/api/chat`).
    mood: Option<api::MoodData>,
    mood_loading: bool,
    prev_portrait_image: Arc<Mutex<Option<Arc<RenderImage>>>>,
    bond_level: u32,
    bond_exp_pct: u32,
    bg_asset_path: Option<PathBuf>,
    api_client: Arc<UsamiApiClient>,
    audio_player: Option<Arc<AudioPlayer>>,
    vrm_controller: VrmController,
    vrm_view: Option<Entity<VrmView>>,
    last_source: Option<SharedString>,
    last_timing: Option<api::ChatTiming>,
    server_affinity: Option<i32>,
    server_stage: Option<SharedString>,
    memory_data: Option<api::MemoryData>,
    memory_loading: bool,
    memory_error: Option<SharedString>,
    memory_confirm_active: bool,
    memory_status: Option<SharedString>,
    personality_data: Option<api::PersonalityData>,
    personality_loading: bool,
    personality_error: Option<SharedString>,
    personality_status: Option<SharedString>,
    proactive_enabled: bool,
    proactive_seen: std::collections::HashSet<String>,
    proactive_queue: Vec<api::ProactiveEvent>,
    last_proactive_speech: Instant,
    last_voice_ended: Instant,
    last_voice_commit: Option<(String, Instant)>,
    /// Prévia do STT enquanto a pessoa ainda fala (fatias parciais).
    live_transcript: SharedString,
    /// Identifica a sessão de escuta atual; prévias atrasadas de uma sessão
    /// anterior não podem reaparecer depois de parar ou reiniciar o microfone.
    listen_session: u64,
    /// Último turno cujo texto final já foi aceito; prévias desse turno ou de
    /// turnos anteriores são descartadas ao chegar atrasadas.
    voice_turn_closed: u64,
    attached_media: Option<AttachedMedia>,
    #[cfg(target_os = "windows")]
    presence_subscription: Option<Subscription>,
}

impl Drop for UsamiApp {
    fn drop(&mut self) {
        // Mantém as assinaturas de janela vivas até o app encerrar.
        self.close_subscription.take();
        #[cfg(target_os = "windows")]
        self.presence_subscription.take();
        // A gravação no fechamento captura a geometria final (o observador de
        // bounds economiza escrita enquanto a janela é arrastada).
        if self.prefs.window.is_some() {
            self.prefs.muted = self.is_muted;
            storage::save_prefs(&self.prefs);
        }
    }
}

impl UsamiApp {
    fn new(cx: &mut Context<Self>, prefs: storage::UiPrefs) -> Self {
        let player = match AudioPlayer::try_new() {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                eprintln!("[Usami Audio] Dispositivo de som indisponível: {:?}", e);
                None
            }
        };

        let bg_asset_path = first_existing([
            env_path("USAMI_BACKGROUND"),
            Some(asset_path("assets/background.png")),
            Some(asset_path("../usami/public/background.png")),
        ]);

        let vrm_path = first_existing([
            env_path("USAMI_VRM_PATH"),
            Some(asset_path("../usami/public/models/shiro.vrm")),
            Some(asset_path("assets/shiro.vrm")),
        ]);

        let vrma_dir = first_existing([
            env_path("USAMI_VRMA_DIR"),
            Some(asset_path("../usami/public/animations/vrma")),
            Some(asset_path("assets/vrma")),
        ]);

        let vrm_controller = VrmController::new();
        vrm_controller.set_emotion_with_options("tsundere", 4.5, false);

        // A escuta segue a boca do avatar: 100 ms entre o TTS começar e o
        // microfone fechar — bem abaixo dos 120 ms que abrem uma fala na VAD,
        // então a própria voz da Usami nunca vira transcrição.
        let mic_gate = Arc::new(recorder::MicGate::new());
        let gate = Arc::clone(&mic_gate);
        let speaking_controller = vrm_controller.clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let mut was_speaking = false;
                loop {
                    let speaking = speaking_controller.is_speaking();
                    gate.set_agent(speaking);
                    if was_speaking && !speaking {
                        let _ = cx.update(|cx: &mut App| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |view, _cx| {
                                    // O relógio de silêncio começa quando a VOZ termina (OpenAmika 11c7c4e).
                                    view.last_voice_ended = Instant::now();
                                });
                            }
                        });
                    }
                    was_speaking = speaking;
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                }
            }
        })
        .detach();

        let vrm_view =
            vrm_path.and_then(
                |path| match VrmView::new(&path, vrma_dir, vrm_controller.clone()) {
                    Ok(v) => Some(cx.new(|_cx| v)),
                    Err(e) => {
                        eprintln!("[gpui-vrm] Falha ao inicializar VrmView: {:?}", e);
                        None
                    }
                },
            );

        // O marcador de sessão precisa ser lido antes de gravar o novo: só assim
        // a abertura seguinte sabe se o processo anterior morreu sem fechar.
        let previous_session = storage::load_session();
        let session = storage::SessionFile::start();
        storage::save_session(&session);
        let abrupt_previous = previous_session.as_ref().is_some_and(|previous| {
            !previous.clean_shutdown
                && storage::now_ms().saturating_sub(previous.last_seen_ms) < 24 * 60 * 60 * 1000
        });

        let mut chat_history: Vec<ChatMessage> = storage::load_conversation()
            .into_iter()
            .filter_map(chat_message_from_stored)
            .collect();
        if chat_history.is_empty() {
            chat_history.push(ChatMessage {
                sender: Sender::Usami,
                text:
                    "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!"
                        .into(),
                emotion: Some("tsundere".into()),
            });
        }
        if abrupt_previous {
            chat_history.push(ChatMessage {
                sender: Sender::Usami,
                text: "Você fechou o app sem se despedir da última vez... eu fiquei aqui esperando, sabia?".into(),
                emotion: Some("tsundere".into()),
            });
            storage::save_conversation(&stored_turns(&chat_history));
        }
        let last_assistant = chat_history
            .iter()
            .rev()
            .find(|message| matches!(message.sender, Sender::Usami))
            .cloned();
        let last_reply: SharedString = last_assistant
            .as_ref()
            .map(|message| message.text.clone())
            .unwrap_or_else(|| {
                "Hmph... não é como se eu estivesse esperando você abrir o app nem nada, baka!"
                    .into()
            })
            .into();
        let last_emotion: SharedString = last_assistant
            .and_then(|message| message.emotion)
            .unwrap_or_else(|| "tsundere".into())
            .into();
        let close_subscription = Some(cx.on_window_closed(|_| storage::mark_session_clean()));

        // Loop em background para detectar inatividade e suavizar a UI (ui-idle)
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let mut tick_counter: u64 = 0;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    tick_counter = tick_counter.wrapping_add(1);
                    let should_poll = tick_counter % 30 == 1; // a cada ~15s (e no primeiro tick)
                    let should_present = tick_counter % 5 == 0; // a cada ~2.5s
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                let mut changed = false;
                                if let Some(started) = view.call_started_at {
                                    let seconds = started.elapsed().as_secs();
                                    if view.call_seconds != seconds {
                                        view.call_seconds = seconds;
                                        changed = true;
                                    }
                                }
                                if let Some(started) = view.sleep_companion_started_at {
                                    let elapsed = started.elapsed();
                                    let remaining =
                                        30_u32.saturating_sub((elapsed.as_secs() / 60) as u32);
                                    if view.sleep_companion_minutes_left != remaining {
                                        view.sleep_companion_minutes_left = remaining;
                                        changed = true;
                                    }
                                    if elapsed >= Duration::from_secs(30 * 60) {
                                        view.stop_sleep_companion();
                                        changed = true;
                                    }
                                }
                                let has_drawer = view.active_drawer != ActiveDrawer::None;
                                let is_idle = !has_drawer
                                    && view.last_interaction.elapsed()
                                        > Duration::from_millis(4500);
                                if view.is_ui_idle != is_idle {
                                    view.is_ui_idle = is_idle;
                                    view.vrm_controller.set_portrait_enabled(!is_idle);
                                    changed = true;
                                }
                                // Caret pisca enquanto o composer está ativo (o mesmo
                                // timer já redesenha a UI, então não há custo extra).
                                if view.input_active {
                                    view.caret_on = !view.caret_on;
                                    changed = true;
                                }
                                if changed {
                                    cx.notify();
                                }

                                // Minimizar/esconder não gera evento de foco em
                                // todo caso, então o estado ausente é reavaliado
                                // no pulso do timer.
                                view.refresh_away_state(cx);
                                view.tick_close_request(cx);
                                view.maybe_run_idle_prods(cx);
                                if view.session.touch() {
                                    storage::save_session(&view.session);
                                }

                                if should_poll {
                                    view.poll_proactive_events(cx);
                                }
                                if should_present {
                                    view.present_next_proactive(cx);
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();

        vrm_controller.set_portrait_enabled(true);

        Self {
            focus_handle: cx.focus_handle(),
            input_text: String::new(),
            pending_dead_key: None,
            input_active: false,
            caret_on: true,
            status_text: "COM VOCÊ".into(),
            last_reply,
            last_emotion,
            show_bubble: true,
            is_busy: false,
            is_calling: false,
            call_started_at: None,
            call_seconds: 0,
            is_listening: false,
            listener: None,
            mic_gate,
            is_companion_mode: false,
            sleep_companion_started_at: None,
            sleep_companion_minutes_left: 30,
            is_muted: prefs.muted,
            is_ui_idle: false,
            last_interaction: Instant::now(),
            is_away: false,
            away_since: None,
            window_focused: true,
            last_user_activity: Instant::now(),
            idle_prods: 0,
            last_return_greeting: Instant::now() - Duration::from_secs(300),
            was_maximized: false,
            last_maximize_line: Instant::now() - Duration::from_secs(900),
            farewell_started_at: None,
            farewell_audio_done: Arc::new(AtomicBool::new(false)),
            close_allowed: false,
            prefs,
            session,
            last_prefs_save: Instant::now() - Duration::from_secs(10),
            window_subscriptions: Vec::new(),
            close_subscription,
            active_drawer: ActiveDrawer::None,
            settings_advanced_open: false,
            bond_tab: BondTab::Home,
            chat_history,
            bond: None,
            mood: None,
            mood_loading: false,
            prev_portrait_image: Arc::new(Mutex::new(None)),
            achievements: Vec::new(),
            bond_level: 1,
            bond_exp_pct: 0,
            bg_asset_path,
            api_client: Arc::new(UsamiApiClient::from_env()),
            audio_player: player,
            vrm_controller,
            vrm_view,
            last_source: None,
            last_timing: None,
            server_affinity: None,
            server_stage: None,
            memory_data: None,
            memory_loading: false,
            memory_error: None,
            memory_confirm_active: false,
            memory_status: None,
            personality_data: None,
            personality_loading: false,
            personality_error: None,
            personality_status: None,
            proactive_enabled: load_proactive_config_from_disk(),
            proactive_seen: load_proactive_seen_from_disk(),
            proactive_queue: Vec::new(),
            last_proactive_speech: Instant::now() - Duration::from_secs(300),
            last_voice_ended: Instant::now() - Duration::from_secs(300),
            last_voice_commit: None,
            live_transcript: SharedString::default(),
            listen_session: 0,
            voice_turn_closed: 0,
            attached_media: None,
            #[cfg(target_os = "windows")]
            presence_subscription: None,
        }
    }

    /// Sobe bandeja, atalho global e toggles. O HWND só existe depois de
    /// `open_window`, então isto roda uma vez na abertura da janela.
    #[cfg(target_os = "windows")]
    fn attach_presence(&mut self, window: &Window, cx: &mut Context<Self>) {
        presence::start(window);
        self.presence_subscription = Some(cx.on_window_closed(|_| presence::shutdown()));
    }

    fn poke(&mut self, cx: &mut Context<Self>) {
        self.last_interaction = Instant::now();
        if self.is_ui_idle {
            self.is_ui_idle = false;
            self.vrm_controller.set_portrait_enabled(true);
            cx.notify();
        }
    }

    /// Atividade causada pelo usuário: além do relógio da UI, reinicia a
    /// contagem de provocações por inatividade.
    fn touch_ui(&mut self, cx: &mut Context<Self>) {
        self.last_user_activity = Instant::now();
        self.idle_prods = 0;
        self.poke(cx);
    }

    fn push_history(&mut self, message: ChatMessage) {
        self.chat_history.push(message);
        if self.chat_history.len() > storage::MAX_STORED_TURNS {
            let overflow = self.chat_history.len() - storage::MAX_STORED_TURNS;
            self.chat_history.drain(0..overflow);
        }
        self.persist_history();
    }

    fn persist_history(&self) {
        storage::save_conversation(&stored_turns(&self.chat_history));
    }

    fn persist_prefs(&mut self) {
        self.prefs.muted = self.is_muted;
        storage::save_prefs(&self.prefs);
        self.last_prefs_save = Instant::now();
    }

    /// Observa foco e geometria da janela: o foco alimenta o estado ausente e a
    /// geometria é lembrada entre sessões (a gravação é adiada para não escrever
    /// um JSON a cada pixel arrastado).
    fn attach_window_observers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds_subscription = cx.observe_window_bounds(window, |view, window, cx| {
            let bounds = window.bounds();
            view.prefs.window = Some(storage::WindowPrefs {
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y),
                width: f32::from(bounds.size.width),
                height: f32::from(bounds.size.height),
            });
            if view.last_prefs_save.elapsed() >= Duration::from_secs(2) {
                view.persist_prefs();
            }
            let maximized = window.is_maximized();
            if maximized != view.was_maximized {
                view.was_maximized = maximized;
                if maximized
                    && !view.is_away
                    && view.last_maximize_line.elapsed() >= Duration::from_secs(900)
                {
                    view.last_maximize_line = Instant::now();
                    view.show_local_line(
                        "Tela grande, hein? Vou fingir que não percebi que você quis me ver maior.",
                        "tsundere",
                        cx,
                    );
                }
            }
        });
        let activation_subscription = cx.observe_window_activation(window, |view, window, cx| {
            view.window_focused = window.is_window_active();
            view.refresh_away_state(cx);
        });
        self.window_subscriptions.push(bounds_subscription);
        self.window_subscriptions.push(activation_subscription);

        // Fechar (X, Alt+F4 ou "Sair" da bandeja) pede a despedida antes de
        // sair; o próprio app conclui o fechamento quando ela termina.
        let close_view = cx.weak_entity();
        window.on_window_should_close(cx, move |_window, cx| {
            close_view
                .update(cx, |this, cx| this.request_close(cx))
                .unwrap_or(true)
        });
        // Sem um resize inicial o observador não dispara; guardar a geometria
        // atual já garante que o fechamento salve a janela do primeiro uso.
        let bounds = window.bounds();
        self.prefs.window = Some(storage::WindowPrefs {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        });
        self.window_focused = window.is_window_active();
        self.was_maximized = window.is_maximized();
    }

    fn default_status(&self) -> SharedString {
        if self.is_calling {
            "SUA VEZ".into()
        } else if self.is_companion_mode {
            "MODO COMPANION".into()
        } else {
            "COM VOCÊ".into()
        }
    }

    /// Ausência é imediata: minimizar, esconder ou perder o foco já marca
    /// AUSENTE. Chamada e modo companhia são exceções — nessas horas o app pode
    /// ficar em segundo plano sem que ela "sinta" a ausência.
    fn refresh_away_state(&mut self, cx: &mut Context<Self>) {
        let exempt = self.is_calling || self.sleep_companion_started_at.is_some();
        let away = !exempt && (window_minimized_or_hidden() || !self.window_focused);
        if away == self.is_away {
            return;
        }
        self.is_away = away;
        if away {
            self.away_since = Some(Instant::now());
            self.status_text = "AUSENTE".into();
            // Eventos já buscados não devem falar quando ela voltar: o próximo
            // poll, com activity=away, consome e marca como pulados.
            self.proactive_queue.clear();
        } else {
            let away_for = self
                .away_since
                .take()
                .map(|since| since.elapsed())
                .unwrap_or_default();
            self.status_text = self.default_status();
            self.show_return_greeting(away_for, cx);
        }
        cx.notify();
    }

    fn show_return_greeting(&mut self, away_for: Duration, cx: &mut Context<Self>) {
        if away_for < Duration::from_secs(60)
            || self.last_return_greeting.elapsed() < Duration::from_secs(300)
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        let line = if away_for >= Duration::from_secs(30 * 60) {
            "Você ficou fora bastante tempo... eu fiquei aqui, sabia? Fico feliz que voltou."
        } else if away_for >= Duration::from_secs(5 * 60) {
            "Onde você foi? Não é como se eu estivesse esperando... mas pronto, você voltou."
        } else {
            "Você voltou rapidinho. Eu nem tive tempo de sentir sua falta."
        };
        self.last_return_greeting = Instant::now();
        self.show_local_line(line, "tsundere", cx);
    }

    /// Fala curta da própria Usami (sem backend): entrada no histórico, balão e
    /// emoção. Usada pelas despedidas/boas-vindas e pelas provocações de inatividade.
    fn show_local_line(
        &mut self,
        text: &'static str,
        emotion: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.last_reply = text.into();
        self.last_emotion = emotion.into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(emotion, 4.0, false);
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: text.to_string(),
            emotion: Some(emotion.to_string()),
        });
        self.poke(cx);
        cx.notify();
    }

    /// Primeira tentativa de fechar segura a janela para a despedida; a segunda
    /// (ou depois de liberado) deixa sair na hora — quem insiste consegue.
    fn request_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.close_allowed || self.farewell_started_at.is_some() {
            self.close_allowed = true;
            return true;
        }
        self.begin_farewell(cx);
        false
    }

    fn begin_farewell(&mut self, cx: &mut Context<Self>) {
        const LINE: &str = "Hmph... já vai embora? T-tá bom, vai... mas não demora pra voltar, tá?! Ainda tenho umas coisas pra te contar.";
        self.farewell_started_at = Some(Instant::now());
        self.show_local_line(LINE, "tsundere", cx);

        if self.is_muted {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        }
        let Some(player) = self.audio_player.clone() else {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        };

        let api = Arc::clone(&self.api_client);
        let vrm_ctrl = self.vrm_controller.clone();
        let done = Arc::clone(&self.farewell_audio_done);
        crate::api::tokio_runtime().spawn(async move {
            match api.stream_tts(LINE, "tsundere").await {
                Ok(crate::api::TtsStream::Pcm {
                    sample_rate,
                    mut response,
                }) => {
                    let mouth = vrm_ctrl.clone();
                    let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                        mouth.set_speaking(true, level);
                    });
                    let mut started = false;
                    loop {
                        match response.chunk().await {
                            Ok(Some(chunk)) => {
                                if sink.push_i16_le(&chunk) && !started {
                                    started = true;
                                    vrm_ctrl.begin_speech_stream();
                                }
                            }
                            Ok(None) => break,
                            Err(error) => {
                                eprintln!("[Despedida TTS] stream interrompido: {error}");
                                break;
                            }
                        }
                    }
                    let tail = sink.finish();
                    if started {
                        let deadline = Instant::now() + Duration::from_secs(15);
                        while !tail.is_drained() && Instant::now() < deadline {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        vrm_ctrl.end_speech_stream();
                    }
                }
                Ok(crate::api::TtsStream::Audio(bytes)) => {
                    vrm_ctrl.speak(LINE, Some(bytes.clone()));
                    if let Err(error) = player.play_wav(bytes) {
                        eprintln!("[Despedida TTS] falha ao tocar áudio: {error}");
                    }
                }
                Err(error) => eprintln!("[Despedida TTS] falha: {error:?}"),
            }
            done.store(true, Ordering::SeqCst);
        });
    }

    fn tick_close_request(&mut self, cx: &mut Context<Self>) {
        let Some(started) = self.farewell_started_at else {
            return;
        };
        let audio_done = self.farewell_audio_done.load(Ordering::SeqCst);
        if !farewell_can_close(
            started.elapsed(),
            audio_done,
            self.vrm_controller.is_speaking(),
        ) {
            return;
        }
        self.close_allowed = true;
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    /// Sem interação e sem fala há alguns minutos, ela chama atenção em vez de
    /// ficar em silêncio para sempre. Só roda com a janela em primeiro plano:
    /// com o app ausente quem fala é a despedida/boas-vindas.
    fn maybe_run_idle_prods(&mut self, cx: &mut Context<Self>) {
        if self.is_away
            || self.is_calling
            || self.sleep_companion_started_at.is_some()
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        const PRODS: [(u64, &str); 3] = [
            (
                210,
                "Ei... você tá aí? Pode ficar quieto, mas eu prefiro quando você fala comigo.",
            ),
            (
                420,
                "Hmph. Se você não tem nada pra dizer, eu também não vou ficar puxando assunto.",
            ),
            (600, "Já faz um tempinho... tá tudo bem aí?"),
        ];
        let idle = self.last_user_activity.elapsed().as_secs();
        let Some((_, line)) = PRODS
            .get(self.idle_prods as usize)
            .copied()
            .filter(|(threshold, _)| idle >= *threshold)
        else {
            return;
        };
        self.idle_prods += 1;
        self.show_local_line(line, "tsundere", cx);
    }

    fn start_call(&mut self, cx: &mut Context<Self>) {
        self.is_calling = true;
        self.call_started_at = Some(Instant::now());
        self.call_seconds = 0;
        self.status_text = "SUA VEZ · 00:00".into();
        self.start_listening(cx);
    }

    fn end_call(&mut self, cx: &mut Context<Self>) {
        let minutes = self
            .call_started_at
            .map(|started| (started.elapsed().as_secs() / 60) as u32)
            .unwrap_or(0);
        self.stop_listening();
        self.is_calling = false;
        self.call_started_at = None;
        self.call_seconds = 0;
        if minutes > 0 {
            self.award_bond_call(minutes, cx);
        }
        self.status_text = if self.is_companion_mode {
            "MODO COMPANION"
        } else {
            "COM VOCÊ"
        }
        .into();
    }

    fn toggle_sleep_companion(&mut self) {
        if self.sleep_companion_started_at.is_some() {
            self.stop_sleep_companion();
        } else {
            self.is_companion_mode = true;
            self.sleep_companion_started_at = Some(Instant::now());
            self.sleep_companion_minutes_left = 30;
            self.last_emotion = "sleepy".into();
            self.status_text = "MODO COMPANION".into();
            self.vrm_controller.set_emotion("sleepy");
        }
    }

    fn stop_sleep_companion(&mut self) {
        self.sleep_companion_started_at = None;
        self.sleep_companion_minutes_left = 30;
        self.is_companion_mode = false;
        self.last_emotion = "tsundere".into();
        self.status_text = if self.is_calling {
            "SUA VEZ"
        } else {
            "COM VOCÊ"
        }
        .into();
        self.vrm_controller
            .set_emotion_with_options("tsundere", 4.0, false);
        self.vrm_controller.play_idle();
    }

    /// Fecha a escuta enquanto a Usami pensa/fala e quando o usuário muta.
    fn sync_mic_gate(&self) {
        self.mic_gate
            .set_user(self.is_muted || self.is_busy || !self.is_listening);
    }

    fn start_listening(&mut self, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.is_listening {
            return;
        }
        match recorder::VoiceListener::start(Arc::clone(&self.mic_gate)) {
            Ok(mut listener) => {
                let Some(segments) = listener.take_segments() else {
                    listener.stop();
                    self.is_listening = false;
                    self.sync_mic_gate();
                    self.status_text = "MIC: canal de áudio indisponível.".into();
                    cx.notify();
                    return;
                };
                let partials = listener.take_partials();
                self.listener = Some(listener);
                self.is_listening = true;
                self.listen_session = self.listen_session.wrapping_add(1);
                let session = self.listen_session;
                self.last_voice_commit = None;
                self.live_transcript = SharedString::default();
                self.voice_turn_closed = 0;
                self.sync_mic_gate();
                self.status_text = "OUVINDO...".into();

                // Prévia ao vivo: fatias do áudio em andamento vão para o STT em
                // paralelo ao turno final. A latência só atrasa a legenda; parar
                // de falar continua encerrando o turno pelo caminho normal.
                if let Some(mut partials) = partials {
                    let api = Arc::clone(&self.api_client);
                    cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let cx = cx.clone();
                        async move {
                            while let Some(mut slice) = partials.recv().await {
                                // O STT local pode demorar mais que o intervalo das
                                // fatias; transcrever a mais recente evita fila parada.
                                while let Ok(newer) = partials.try_recv() {
                                    slice = newer;
                                }
                                let Ok(text) = api.transcribe(slice.wav).await else {
                                    continue;
                                };
                                let trimmed = text.trim().to_string();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let active = cx
                                    .update(|cx: &mut App| {
                                        if let Some(this) = this.upgrade() {
                                            this.update(cx, |view, cx| {
                                                if !view.is_listening
                                                    || !view.is_calling
                                                    || view.listen_session != session
                                                {
                                                    return false;
                                                }
                                                // Chegou depois do texto final: é só um
                                                // eco atrasado do turno já respondido.
                                                if slice.turn <= view.voice_turn_closed {
                                                    return true;
                                                }
                                                if view.live_transcript.as_ref() != trimmed {
                                                    view.live_transcript = trimmed.clone().into();
                                                    cx.notify();
                                                }
                                                true
                                            })
                                        } else {
                                            false
                                        }
                                    })
                                    .unwrap_or(false);
                                if !active {
                                    break;
                                }
                            }
                        }
                    })
                    .detach();
                }

                // Consome as falas detectadas pela VAD e manda para o STT. O loop
                // vive enquanto o listener existir; trocar de chamada recria tudo.
                let api = Arc::clone(&self.api_client);
                cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        let mut segments = segments;
                        while let Some(slice) = segments.recv().await {
                            // Fecha o turno antes de transcrever: prévias dele que
                            // ainda estejam na fila não podem reaparecer na legenda.
                            let accepted = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || !view.is_calling {
                                                return false;
                                            }
                                            view.voice_turn_closed =
                                                view.voice_turn_closed.max(slice.turn);
                                            view.live_transcript = SharedString::default();
                                            cx.notify();
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !accepted {
                                break;
                            }

                            let transcript = api.transcribe(slice.wav).await;
                            let active = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || !view.is_calling {
                                                return false;
                                            }
                                            match transcript {
                                                Ok(text) if !text.trim().is_empty() => {
                                                    let trimmed = text.trim().to_string();
                                                    if is_duplicate_utterance(
                                                        &mut view.last_voice_commit,
                                                        &trimmed,
                                                        Instant::now(),
                                                    ) {
                                                        return true;
                                                    }
                                                    view.input_text = trimmed;
                                                    if view.is_busy {
                                                        // Não fala por cima: guarda no campo.
                                                        view.status_text = "TEXTO PRONTO".into();
                                                        cx.notify();
                                                    } else {
                                                        view.submit(cx);
                                                    }
                                                }
                                                Ok(_) => {
                                                    view.status_text = "NÃO ENTENDI".into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                                Err(error) => {
                                                    view.status_text =
                                                        format!("ERRO STT: {error}").into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                            }
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !active {
                                break;
                            }
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                self.is_listening = false;
                self.live_transcript = SharedString::default();
                self.sync_mic_gate();
                self.status_text = format!("MIC: {error}").into();
            }
        }
        cx.notify();
    }

    fn stop_listening(&mut self) {
        self.is_listening = false;
        self.listen_session = self.listen_session.wrapping_add(1);
        self.live_transcript = SharedString::default();
        self.sync_mic_gate();
        if let Some(listener) = self.listener.take() {
            listener.stop();
        }
    }

    fn toggle_listening(&mut self, cx: &mut Context<Self>) {
        if self.is_listening {
            self.stop_listening();
            self.status_text = "MICROFONE OFF".into();
            cx.notify();
        } else {
            self.start_listening(cx);
        }
    }

    fn attach_file_prompt(&mut self, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Selecione uma imagem para a Usami".into()),
        });

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(path) = paths.into_iter().next() {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("imagem.png")
                                .to_string();
                            let mime = match path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_ascii_lowercase())
                                .as_deref()
                            {
                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                Some("png") => "image/png",
                                Some("webp") => "image/webp",
                                Some("gif") => "image/gif",
                                Some("bmp") => "image/bmp",
                                _ => "image/png",
                            };
                            let media = AttachedMedia::from_bytes(filename, mime, bytes);
                            let _ = cx.update(|cx: &mut App| {
                                if let Some(this) = this.upgrade() {
                                    this.update(cx, |view, cx| {
                                        view.attached_media = Some(media);
                                        view.input_active = true;
                                        cx.notify();
                                    });
                                }
                            });
                        }
                    }
                }
            }
        })
        .detach();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        let text = self.input_text.trim().to_string();
        let attached = self.attached_media.take();
        if (text.is_empty() && attached.is_none()) || self.is_busy {
            self.attached_media = attached;
            return;
        }

        self.input_text.clear();
        self.pending_dead_key = None;
        self.input_active = false;
        self.is_busy = true;
        self.sync_mic_gate();
        self.show_bubble = true;

        // Reação imediata de escuta e reflexão enquanto o LLM processa
        self.vrm_controller.play_vrma("thinking", false);
        // Limpa as expressões do turno anterior: sem isso elas se acumulam na cabeça.
        self.vrm_controller.clear_expressions();
        self.vrm_controller.set_expression("19-prefab_doubt", 0.45);

        if let Some(media) = attached {
            self.status_text = "ANALISANDO MÍDIA".into();
            let user_msg = if text.is_empty() {
                format!("[Mídia: {}]", media.filename)
            } else {
                format!("[Mídia: {}] {}", media.filename, text)
            };
            self.push_history(ChatMessage {
                sender: Sender::User,
                text: user_msg,
                emotion: None,
            });
            cx.notify();

            let api = Arc::clone(&self.api_client);
            let audio = self.audio_player.clone();
            let prompt = if text.is_empty() { None } else { Some(text) };

            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api
                        .analyze_media(
                            &media.mime_type,
                            &media.data_base64,
                            Some(&media.filename),
                            prompt.as_deref(),
                            None,
                        )
                        .await;

                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.is_busy = false;
                                view.sync_mic_gate();
                                view.poke(cx);
                                match res {
                                    Ok(response) => {
                                        let emotion = response.gesture.clone().unwrap_or_else(|| "tsundere".into());
                                        view.last_reply = sanitize_speech_text(&response.reply).into();
                                        view.last_emotion = emotion.clone().into();
                                        view.show_bubble = true;
                                        view.status_text = if view.is_calling {
                                            "EM CHAMADA".into()
                                        } else if view.is_companion_mode {
                                            "MODO COMPANION".into()
                                        } else {
                                            "COM VOCÊ".into()
                                        };

                                        view.vrm_controller.set_emotion_with_options(&emotion, 5.0, false);
                                        view.vrm_controller.clear_expressions();
                                        for action in &response.actions {
                                            if matches!(
                                                action.action_type.as_str(),
                                                "set_expression" | "expression" | "emote"
                                            ) && let Some(name) = &action.name
                                            {
                                                let weight = action.weight.or(action.value).unwrap_or(0.8);
                                                view.vrm_controller.set_expression(name, weight);
                                            }
                                        }

                                        let gesture = response
                                            .actions
                                            .iter()
                                            .find_map(|action| match action.action_type.as_str() {
                                                "gesture" | "play_clip" | "play_vrma" => action.name.clone(),
                                                _ => None,
                                            })
                                            .or_else(|| {
                                                response
                                                    .gesture
                                                    .clone()
                                                    .filter(|gesture| gesture != "neutral" && gesture != "idle")
                                            });
                                        if let Some(gesture) = gesture {
                                            view.vrm_controller.play_vrma(&gesture, false);
                                        }

                                        view.push_history(ChatMessage {
                                            sender: Sender::Usami,
                                            text: response.reply.clone(),
                                            emotion: Some(emotion.clone()),
                                        });

                                        cx.notify();

                                        if !view.is_muted
                                            && let Some(player) = audio
                                        {
                                            let api_tts = Arc::clone(&view.api_client);
                                            let tts_text = response.reply;
                                            let tts_style = emotion;
                                            let player = player.clone();
                                            let vrm_ctrl = view.vrm_controller.clone();
                                            crate::api::tokio_runtime().spawn(async move {
                                                match api_tts.stream_tts(&tts_text, &tts_style).await {
                                                    Ok(crate::api::TtsStream::Pcm {
                                                        sample_rate,
                                                        mut response,
                                                    }) => {
                                                        let mouth = vrm_ctrl.clone();
                                                        let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                                                            mouth.set_speaking(true, level);
                                                        });
                                                        let mut started = false;
                                                        loop {
                                                            match response.chunk().await {
                                                                Ok(Some(chunk)) => {
                                                                    if sink.push_i16_le(&chunk) && !started {
                                                                        started = true;
                                                                        vrm_ctrl.begin_speech_stream();
                                                                    }
                                                                }
                                                                Ok(None) => break,
                                                                Err(error) => {
                                                                    eprintln!("[TTS] stream interrompido: {error}");
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        let tail = sink.finish();
                                                        if started {
                                                            let deadline = Instant::now() + Duration::from_secs(180);
                                                            while !tail.is_drained() && Instant::now() < deadline {
                                                                tokio::time::sleep(Duration::from_millis(50)).await;
                                                            }
                                                            vrm_ctrl.end_speech_stream();
                                                        } else {
                                                            vrm_ctrl.speak(&tts_text, None);
                                                        }
                                                    }
                                                    Ok(crate::api::TtsStream::Audio(bytes)) => {
                                                        vrm_ctrl.speak(&tts_text, Some(bytes.clone()));
                                                        if let Err(error) = player.play_wav(bytes) {
                                                            eprintln!("[TTS] falha ao tocar áudio: {error}");
                                                        }
                                                    }
                                                    Err(error) => {
                                                        eprintln!("[TTS] streaming indisponível ({error}); fala só na legenda");
                                                        vrm_ctrl.speak(&tts_text, None);
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    Err(err) => {
                                        view.status_text = format!("ERRO: {}", err).into();
                                        cx.notify();
                                    }
                                }
                            });
                        }
                    });
                }
            })
            .detach();
            return;
        }

        self.push_history(ChatMessage {
            sender: Sender::User,
            text: text.clone(),
            emotion: None,
        });

        cx.notify();

        // Em chamada a mensagem veio pela voz; fora dela é texto digitado.
        self.award_bond_chat(text.clone(), self.is_calling, cx);

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();
        let sleep_companion_active = self.sleep_companion_started_at.is_some();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.send_chat(&text, sleep_companion_active).await;

                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.is_busy = false;
                            view.sync_mic_gate();
                            view.poke(cx);
                            match res {
                                Ok(response) => {
                                    let emotion = response.gesture.clone().unwrap_or_else(|| "tsundere".into());
                                    view.last_reply = sanitize_speech_text(&response.reply).into();
                                    view.last_emotion = emotion.clone().into();
                                    view.show_bubble = true;
                                    view.status_text = if view.is_calling {
                                        "EM CHAMADA".into()
                                    } else if view.is_companion_mode {
                                        "MODO COMPANION".into()
                                    } else {
                                        "COM VOCÊ".into()
                                    };

                                    // Emoção não dispara clipe: o gesto é escolhido uma vez, abaixo.
                                    view.vrm_controller.set_emotion_with_options(&emotion, 5.0, false);

                                    // Substitui (em vez de somar) as expressões do turno.
                                    view.vrm_controller.clear_expressions();
                                    for action in &response.actions {
                                        if matches!(
                                            action.action_type.as_str(),
                                            "set_expression" | "expression" | "emote"
                                        ) && let Some(name) = &action.name
                                        {
                                            let weight = action.weight.or(action.value).unwrap_or(0.8);
                                            view.vrm_controller.set_expression(name, weight);
                                        }
                                    }

                                    // Um único gesto por turno: a primeira ação de clipe; sem ação,
                                    // cai para o gesto do backend (nunca quando é só a emoção padrão).
                                    let gesture = response
                                        .actions
                                        .iter()
                                        .find_map(|action| match action.action_type.as_str() {
                                            "gesture" | "play_clip" | "play_vrma" => action.name.clone(),
                                            _ => None,
                                        })
                                        .or_else(|| {
                                            response
                                                .gesture
                                                .clone()
                                                .filter(|gesture| gesture != "neutral" && gesture != "idle")
                                        });
                                    if let Some(gesture) = gesture {
                                        view.vrm_controller.play_vrma(gesture, false);
                                    }

                                    view.push_history(ChatMessage {
                                        sender: Sender::Usami,
                                        text: response.reply.clone(),
                                        emotion: Some(emotion.clone()),
                                    });

                                    view.last_source = response.source.clone().map(Into::into);
                                    view.last_timing = response.timing.clone();
                                    // Nível/EXP do VN não saem mais da afinidade do
                                    // chat: quem manda é o /api/bond (abaixo, award).
                                    if let Some(aff) = response.affinity {
                                        view.server_affinity = Some(aff);
                                    }
                                    if let Some(stage) = response.stage.clone() {
                                        view.server_stage = Some(stage.into());
                                    }
                                    if let Some(mood) = response.mood.clone() {
                                        view.mood = Some(mood);
                                    }

                                    cx.notify();

                                    // Sem áudio não há boca animada: mudo mostra só as ações.
                                    if !view.is_muted
                                        && let Some(player) = audio
                                    {
                                        let api_tts = Arc::clone(&view.api_client);
                                        let tts_text = response.reply;
                                        let tts_style = emotion;
                                        let player = player.clone();
                                        let vrm_ctrl = view.vrm_controller.clone();
                                        crate::api::tokio_runtime().spawn(async move {
                                            match api_tts.stream_tts(&tts_text, &tts_style).await {
                                                Ok(crate::api::TtsStream::Pcm {
                                                    sample_rate,
                                                    mut response,
                                                }) => {
                                                    let mouth = vrm_ctrl.clone();
                                                    let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                                                        mouth.set_speaking(true, level);
                                                    });
                                                    let mut started = false;
                                                    loop {
                                                        match response.chunk().await {
                                                            Ok(Some(chunk)) => {
                                                                if sink.push_i16_le(&chunk) && !started {
                                                                    started = true;
                                                                    vrm_ctrl.begin_speech_stream();
                                                                }
                                                            }
                                                            Ok(None) => break,
                                                            Err(error) => {
                                                                eprintln!("[TTS] stream interrompido: {error}");
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    let tail = sink.finish();
                                                    if started {
                                                        // A boca acompanha o áudio até o buffer esvaziar,
                                                        // não até a rede fechar.
                                                        let deadline = Instant::now() + Duration::from_secs(180);
                                                        while !tail.is_drained() && Instant::now() < deadline {
                                                            tokio::time::sleep(Duration::from_millis(50)).await;
                                                        }
                                                        vrm_ctrl.end_speech_stream();
                                                    } else {
                                                        eprintln!("[TTS] stream sem áudio; fala só na legenda");
                                                        vrm_ctrl.speak(&tts_text, None);
                                                    }
                                                }
                                                Ok(crate::api::TtsStream::Audio(bytes)) => {
                                                    vrm_ctrl.speak(&tts_text, Some(bytes.clone()));
                                                    if let Err(error) = player.play_wav(bytes) {
                                                        eprintln!("[TTS] falha ao tocar áudio: {error}");
                                                    }
                                                }
                                                Err(error) => {
                                                    eprintln!("[TTS] streaming indisponível ({error}); fala só na legenda");
                                                    vrm_ctrl.speak(&tts_text, None);
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(err) => {
                                    view.status_text = format!("ERRO: {}", err).into();
                                    cx.notify();
                                }
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn poll_proactive_events(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled {
            return;
        }
        let api = Arc::clone(&self.api_client);
        // Relógio de silêncio: enquanto a voz da Usami está tocando ou terminou há < 60s,
        // conta como atividade recente (OpenAmika 11c7c4e).
        let is_active = self.last_interaction.elapsed() < Duration::from_secs(60)
            || self.vrm_controller.is_speaking()
            || self.last_voice_ended.elapsed() < Duration::from_secs(60);
        let activity = if self.is_away {
            "away"
        } else if is_active {
            "active"
        } else {
            "idle"
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Ok(res) = api.get_proactive_events(activity).await {
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, _cx| {
                                // Ausente: conversa social é consumida sem falar
                                // (não despeja atrasados ao voltar); lembrete fica
                                // pendente no servidor para não se perder.
                                if view.is_away {
                                    let acks: Vec<crate::api::ProactiveAckItem> = res
                                        .events
                                        .iter()
                                        .filter(|event| event.event_type != "reminder")
                                        .map(|event| crate::api::ProactiveAckItem {
                                            id: event.id.clone(),
                                            outcome: "skipped".to_string(),
                                        })
                                        .collect();
                                    if !acks.is_empty() {
                                        let api = Arc::clone(&view.api_client);
                                        crate::api::tokio_runtime().spawn(async move {
                                            let _ = api.ack_proactive_events(&acks).await;
                                        });
                                    }
                                    return;
                                }
                                for event in res.events {
                                    if let Some(dedupe_key) = &event.dedupe_key {
                                        if view.proactive_seen.contains(dedupe_key) {
                                            continue;
                                        }
                                    }
                                    if view.proactive_queue.iter().any(|e| e.id == event.id) {
                                        continue;
                                    }
                                    if view.proactive_queue.len() < 5 {
                                        view.proactive_queue.push(event);
                                    }
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();
    }

    fn present_next_proactive(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled || self.proactive_queue.is_empty() || self.is_away {
            return;
        }
        if self.is_busy
            || self.is_calling
            || self.is_listening
            || self.sleep_companion_started_at.is_some()
            || self.vrm_controller.is_speaking()
        {
            return;
        }

        let next_is_social = self.proactive_queue[0].event_type != "reminder";
        if next_is_social && self.last_proactive_speech.elapsed() < Duration::from_secs(300) {
            return;
        }

        let event = self.proactive_queue.remove(0);
        if let Some(dedupe_key) = &event.dedupe_key {
            self.proactive_seen.insert(dedupe_key.clone());
            save_proactive_seen_to_disk(&self.proactive_seen);
        }

        let emotion = event.emotion.clone().unwrap_or_else(|| "tsundere".into());
        self.last_reply = sanitize_speech_text(&event.text).into();
        self.last_emotion = emotion.clone().into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(&emotion, 5.0, false);
        if let Some(gesture) = &event.gesture {
            self.vrm_controller.play_vrma(gesture, false);
        }
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: event.text.clone(),
            emotion: Some(emotion.clone()),
        });
        self.fetch_mood(cx);
        self.touch_ui(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();
        let event_id = event.id.clone();
        let text = event.text.clone();
        let is_muted = self.is_muted;
        let vrm_ctrl = self.vrm_controller.clone();

        self.last_proactive_speech = Instant::now();

        crate::api::tokio_runtime().spawn(async move {
            let mut delivered = false;
            if !is_muted && let Some(player) = audio {
                match api.stream_tts(&text, &emotion).await {
                    Ok(crate::api::TtsStream::Pcm {
                        sample_rate,
                        mut response,
                    }) => {
                        let mouth = vrm_ctrl.clone();
                        let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                            mouth.set_speaking(true, level);
                        });
                        let mut started = false;
                        loop {
                            match response.chunk().await {
                                Ok(Some(chunk)) => {
                                    if sink.push_i16_le(&chunk) && !started {
                                        started = true;
                                        vrm_ctrl.begin_speech_stream();
                                    }
                                }
                                Ok(None) => break,
                                Err(error) => {
                                    eprintln!("[Proactive TTS] stream interrompido: {error}");
                                    break;
                                }
                            }
                        }
                        let tail = sink.finish();
                        if started {
                            let deadline = Instant::now() + Duration::from_secs(180);
                            while !tail.is_drained() && Instant::now() < deadline {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            vrm_ctrl.end_speech_stream();
                            delivered = true;
                        } else {
                            vrm_ctrl.speak(&text, None);
                        }
                    }
                    Ok(crate::api::TtsStream::Audio(bytes)) => {
                        vrm_ctrl.speak(&text, Some(bytes.clone()));
                        if let Err(error) = player.play_wav(bytes) {
                            eprintln!("[Proactive TTS] falha ao tocar áudio: {error}");
                        } else {
                            delivered = true;
                        }
                    }
                    Err(e) => {
                        eprintln!("[Proactive TTS] falha: {e:?}");
                    }
                }
            }

            let outcome = if delivered { "spoken" } else { "shown" };
            let _ = api
                .ack_proactive_events(&[crate::api::ProactiveAckItem {
                    id: event_id,
                    outcome: outcome.to_string(),
                }])
                .await;
        });
    }

    fn toggle_drawer(&mut self, drawer: ActiveDrawer, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.active_drawer == drawer {
            self.active_drawer = ActiveDrawer::None;
        } else {
            self.active_drawer = drawer;
            if drawer == ActiveDrawer::Bond {
                // Abrir o painel relê o core: o web (ou outra janela) pode ter mudado.
                self.fetch_bond(cx);
                self.fetch_mood(cx);
                // Diário, Álbum e Memória também acompanham os registros/espelho do core.
                if !self.memory_loading {
                    self.fetch_memory(cx);
                }
            }
        }
        cx.notify();
    }

    fn set_proactive_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.proactive_enabled = enabled;
        save_proactive_config_to_disk(enabled);
        if !enabled {
            self.proactive_queue.clear();
        }
        cx.notify();
    }


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
                                            .bg(if has_token { rgba(0x5cb85c22) } else { rgba(0xf0ad4e22) })
                                            .border_1()
                                            .border_color(if has_token { rgba(0x5cb85c66) } else { rgba(0xf0ad4e66) })
                                            .text_size(px(ty.xs))
                                            .text_color(if has_token { rgb(0x5cb85c) } else { rgb(0xf0ad4e) })
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
                                            .bg(if proactive_enabled { rgba(0x5cb85c22) } else { rgba(0x88888822) })
                                            .border_1()
                                            .border_color(if proactive_enabled { rgba(0x5cb85c66) } else { rgba(0x88888866) })
                                            .text_size(px(ty.xs))
                                            .text_color(if proactive_enabled { rgb(0x5cb85c) } else { rgb(0x888888) })
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
            SettingsTab::Avatar => div()
                .flex()
                .flex_col()
                .child(settings_h3("RENDERIZADOR 3D NATIVO"))
                .child(settings_dl_row("Formato", "VRM 1.0 (GLTF Estendido)"))
                .child(settings_dl_row("Modelo", "白ー (shiro)"))
                .child(settings_dl_row("Renderizador", "WGPU 60 FPS (Nativo)"))
                .child(settings_dl_row("Janela", "Transparência nativa + Drag"))
                .child(settings_hint(
                    "A renderização gráfica opera via WGPU com aceleração de hardware nativa.",
                ))
                .child(settings_h3("CENÁRIO E ANIMAÇÕES"))
                .child(settings_p(
                    "O app desktop foca na presença direta da Usami na área de trabalho. Cenários completos e controle de ossos pertencem à versão web.",
                )),
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
                                .bg(rgba(0x5cb85c22))
                                .border_1()
                                .border_color(rgba(0x5cb85c66))
                                .text_size(px(ty.xs))
                                .text_color(rgb(0x5cb85c))
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
                                .bg(rgba(0x5cb85c22))
                                .border_1()
                                .border_color(rgba(0x5cb85c66))
                                .text_size(px(ty.xs))
                                .text_color(rgb(0x5cb85c))
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
                            .child(SharedString::from(format!(
                                "• Fonte: {}",
                                source_str
                            ))),
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
                                        .child(SharedString::from(format!("• Superfície: {} ms", ms))),
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
                                server_affinity.map(|a| format!("{}/100", a)).unwrap_or_else(|| "—".into()),
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
                div()
                    .flex()
                    .child(
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

    fn render_settings_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                        div()
                            .text_size(px(ty.sm))
                            .text_color(c.muted)
                            .child(
                                "Voz, memória, jeito de ser e integrações — tudo no mesmo lugar.",
                            ),
                    ),
            )
            .child(Self::render_settings_category(
                "Voz e áudio",
                "Gemini Multimodal Live, TTS e privacidade da chave.",
                voice,
            ))
            .child(Self::render_settings_category(
                "Personalidade e comportamento",
                "Traços, jeito de falar e tom das respostas.",
                personality,
            ))
            .child(Self::render_settings_category(
                "Memória e privacidade",
                "O que a Usami lembra, exportação e apagamento.",
                memory,
            ))
            .child(Self::render_settings_category(
                "Aparência e interface",
                "Renderizador 3D nativo e cenário.",
                avatar,
            ))
            .child(Self::render_settings_category(
                "Integrações",
                "Conexão com o backend e proatividade.",
                integrations,
            ))
            .child(Self::render_settings_category(
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

    fn load_settings_data(&mut self, cx: &mut Context<Self>) {
        if !self.memory_loading {
            self.fetch_memory(cx);
        }
        if self.personality_data.is_none() && !self.personality_loading {
            self.fetch_personality(cx);
        }
    }

    /// O core é a única fonte do vínculo: nível, EXP e medidores vêm dele.
    fn apply_bond(&mut self, bond: api::BondData) {
        self.bond_level = bond.summary.level.max(1);
        self.bond_exp_pct = (bond.summary.progress.clamp(0.0, 1.0) * 100.0).round() as u32;
        self.bond = Some(bond);
    }

    fn fetch_bond(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_bond().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    // O core reavalia conquistas junto com o
                                    // vínculo; recarrega para refletir datas.
                                    view.fetch_achievements(cx);
                                }
                                Err(error) => eprintln!("[Vínculo] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_achievements(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_achievements().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.achievements = resp.achievements,
                                Err(error) => eprintln!("[Conquistas] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Busca o humor persistido no core (`GET /api/mood`) sem bloquear a UI.
    fn fetch_mood(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || self.mood_loading {
            return;
        }
        self.mood_loading = true;
        let had_mood = self.mood.is_some();
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_mood().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.mood_loading = false;
                            match res {
                                Ok(resp) => {
                                    if !had_mood && resp.mood.vrm_emotion != "neutral" {
                                        view.vrm_controller.set_emotion_with_options(
                                            &resp.mood.vrm_emotion,
                                            8.0,
                                            false,
                                        );
                                    }
                                    view.mood = Some(resp.mood);
                                }
                                Err(error) => eprintln!("[Humor] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Aplica uma expressão no avatar ao vivo e registra o ajuste de humor no core.
    fn trigger_mood_preview(
        &mut self,
        emotion: &'static str,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.vrm_controller
            .set_emotion_with_options(emotion, 8.0, false);
        self.last_emotion = emotion.into();
        self.poke(cx);
        cx.notify();

        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.post_mood_event(emotion, Some(reason)).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.mood = Some(resp.mood),
                                Err(error) => eprintln!("[Humor] falha ao atualizar: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Mesma regra do web: cada mensagem rende XP, mas quem soma é o core.
    fn award_bond_chat(&mut self, content: String, spoken: bool, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.award_bond_chat(&content, spoken).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    cx.notify();
                                }
                                Err(error) => eprintln!("[Vínculo] evento de chat falhou: {error}"),
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Chamada de voz rende companhia por minuto inteiro, como no web.
    fn award_bond_call(&mut self, minutes: u32, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || minutes == 0 {
            return;
        }
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.award_bond_call(minutes).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    cx.notify();
                                }
                                Err(error) => eprintln!("[Vínculo] evento de chamada falhou: {error}"),
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_memory(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.memory_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.memory_loading = true;
        self.memory_error = None;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_memory().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(ref aff) = resp.memory.affinity {
                                        view.server_affinity = Some(aff.score);
                                    }
                                    view.memory_data = Some(resp.memory);
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn delete_memory_fact(&mut self, section: String, fact: String, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "fact".into(),
            confirm: "APAGAR".into(),
            section: Some(section),
            fact: Some(fact),
            date: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.forget_memory(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    }
                                    view.memory_status = Some("Fato removido com sucesso.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao excluir: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn delete_memory_all(&mut self, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "all".into(),
            confirm: "APAGAR".into(),
            section: None,
            fact: None,
            date: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.forget_memory(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    } else {
                                        view.memory_data = Some(api::MemoryData::default());
                                    }
                                    view.memory_confirm_active = false;
                                    view.memory_status =
                                        Some("Toda a memória foi redefinida.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao apagar tudo: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn fetch_personality(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.personality_loading = true;
        self.personality_error = None;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_personality().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn adjust_trait(&mut self, trait_name: &'static str, delta: i32) {
        if let Some(ref mut data) = self.personality_data {
            let val = match trait_name {
                "warmth" => &mut data.traits.warmth,
                "curiosity" => &mut data.traits.curiosity,
                "playfulness" => &mut data.traits.playfulness,
                "pride" => &mut data.traits.pride,
                _ => return,
            };
            let new_val = (*val as i32 + delta).clamp(0, 100) as u32;
            *val = new_val;
        }
    }

    fn save_personality(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.personality_data.clone() else {
            return;
        };
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: Some(data.traits),
            custom_prompt: Some(data.custom_prompt),
            reset: None,
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.update_personality(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade salva no servidor!".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao salvar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn reset_personality(&mut self, cx: &mut Context<Self>) {
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: None,
            custom_prompt: None,
            reset: Some(true),
        };

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.update_personality(&payload).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade restaurada para o padrão (50).".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao restaurar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }
}

// ── COMPONENTES DO HUB DE VISUAL NOVEL DE ROMANCE ──

fn render_vn_tab_btn(
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
            el.child(
                div()
                    .text_size(px(10.0))
                    .text_color(c.pearl)
                    .child("✦"),
            )
        })
}

/// Formata data civil (dd/mm/aaaa) a partir de epoch ms, sem dependência de
/// crate de data: converte dias epoch → ano/mês/dia pelo algoritmo civil.
fn format_date(at_ms: i64) -> String {
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
fn format_unlocked_at(iso: &str) -> String {
    let date = iso.get(..10).unwrap_or(iso);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day)) => format!("{day}/{month}/{}", &year[..year.len().min(4)]),
        _ => date.to_string(),
    }
}

/// Resumo diário é um parágrafo inteiro; nas listas mostramos só o começo.
fn truncate_chars(text: &str, max: usize) -> String {
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
fn render_vn_tab_home(
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
                            div()
                                .flex()
                                .flex_1()
                                .items_end()
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
fn collect_diary_entries(
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
fn render_vn_tab_diary(
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
fn vn_history_tag(kind: &str) -> &'static str {
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
fn relative_when(at_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0);
    relative_when_at(at_ms, now_ms)
}

fn relative_when_at(at_ms: i64, now_ms: i64) -> String {
    let seconds = ((now_ms - at_ms).max(0)) / 1000;
    match seconds {
        0..=59 => "agora".to_string(),
        60..=3599 => format!("há {} min", seconds / 60),
        3600..=86399 => format!("há {} h", seconds / 3600),
        86400..=172799 => "ontem".to_string(),
        _ => format!("há {} dias", seconds / 86400),
    }
}

fn render_vn_history_item(icon: String, title: String, time: String) -> impl IntoElement {
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
fn render_vn_tab_memories(
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
                "Continue dividindo momentos com a Usami para desbloquear este capítulo.".to_string(),
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

fn render_vn_large_memory(
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
                        .text_color(if is_unlocked { theme::Palette::vn_pink_soft() } else { t.color.muted })
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
fn render_vn_tab_bonds(
    bond_level: u32,
    bond: Option<&api::BondData>,
    t: &theme::Theme,
) -> impl IntoElement {
    let c = t.color;
    let (carinho, humor, companhia, carinho_pct, humor_pct, companhia_pct, level_name) =
        match bond {
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
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(c.pearl)
                                        .child(label),
                                )
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
                                .map(|(tag, title, when)| {
                                    render_vn_history_item(tag, title, when)
                                }),
                        ),
                ),
        )
}

/// Glifo do emblema a partir da chave estável do catálogo do core.
fn achievement_glyph(icon: &str) -> &'static str {
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
fn render_vn_tab_achievements(
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
                    col.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(c.muted)
                            .child("As conquistas aparecem quando o servidor local estiver disponível."),
                    )
                })
                .children(
                    achievements
                        .iter()
                        .map(render_vn_achievement_badge),
                ),
        )
}

fn render_vn_achievement_badge(a: &api::Achievement) -> impl IntoElement {
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
                        .text_color(if unlocked { t.color.pearl } else { t.color.muted })
                        .child(if unlocked { achievement_glyph(&a.icon) } else { "✧" }),
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

fn render_vn_tab_profile(t: &theme::Theme) -> impl IntoElement {
    let c = t.color;
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

fn render_vn_profile_row(label: &'static str, val: &'static str) -> impl IntoElement {
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
struct MoodVisual {
    label: &'static str,
    glyph: &'static str,
    accent: Rgba,
}

fn mood_visual(id: &str) -> MoodVisual {
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
struct MoodPreset {
    id: &'static str,
    emotion: &'static str,
    label: &'static str,
    reason: &'static str,
}

fn mood_preset_options() -> &'static [MoodPreset] {
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
fn render_lego_portrait(
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

fn render_mood_meter_row(
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
    fn render_vn_tab_mood(
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
                    .child(
                        render_lego_portrait(
                            132.0,
                            66.0,
                            vis.accent,
                            portrait_img,
                            prev_slot,
                            vis.glyph,
                        ),
                    )
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
                                    .child(render_mood_meter_row("Bom Humor", valence, vis.accent, t))
                                    .child(render_mood_meter_row("Energia", energy, theme::Palette::vn_gold(), t))
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
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .children(mood_preset_options().iter().map(|preset| {
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
                                    .hover(|style| {
                                        style
                                            .bg(c.icon_bg_hover)
                                            .border_color(p_vis.accent)
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.trigger_mood_preview(emo, reason, cx);
                                    }))
                                    .child(format!("{} {}", p_vis.glyph, preset.label))
                            })),
                    ),
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
                                .child("As mudanças de humor aparecem aqui conforme vocês conversam."),
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
}

impl Render for UsamiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let r = t.radius;
        let ty = t.text;
        // O painel lateral é absoluto; sem altura explícita ele cresce com o conteúdo
        // e o `flex_1` das abas nunca encolhe. Amarra na altura da janela.
        let viewport_h = window.viewport_size().height;

        let is_busy = self.is_busy;
        let is_calling = self.is_calling;
        let is_companion = self.is_companion_mode;
        let is_muted = self.is_muted;
        let is_idle = self.is_ui_idle;
        let is_listening = self.is_listening;
        let active_drawer = self.active_drawer;
        let chat_open = active_drawer == ActiveDrawer::Chat;
        let side_drawer_open = active_drawer == ActiveDrawer::Vrma && !chat_open;
        let bond_level = self.bond_level;
        let bond_exp_pct = self.bond_exp_pct;
        let bond = self.bond.clone();
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
        let mood_label: String = mood_data
            .as_ref()
            .map(|m| m.label.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| mood_vis.label.to_string());
        let hud_portrait = if !is_idle && active_drawer != ActiveDrawer::Bond {
            self.vrm_controller.portrait_image()
        } else {
            None
        };
        let hud_prev_slot = self.prev_portrait_image.clone();
        let show_bubble = self.show_bubble;
        let live_transcript = self.live_transcript.clone();
        let show_live_caption = is_listening && !live_transcript.is_empty();
        let bg_asset = self.bg_asset_path.clone();
        let vrm_entity = self.vrm_view.clone();
        let last_reply = self.last_reply.clone();
        let input_text = self.input_text.clone();
        let input_active = self.input_active;
        let caret_on = self.caret_on;
        let caret_visible = input_active && caret_on;
        let has_media = self.attached_media.is_some();
        let attached_media = self.attached_media.clone();
        let send_enabled = !is_busy && (!input_text.trim().is_empty() || has_media);
        let turns = self.chat_history.clone();
        let no_turns = turns.is_empty();

        let status_label: SharedString = if is_calling {
            let state = if is_listening { "OUVINDO" } else { "MIC OFF" };
            format!(
                "{state} · {:02}:{:02}",
                self.call_seconds / 60,
                self.call_seconds % 60
            )
            .into()
        } else if is_busy {
            "PENSANDO".into()
        } else if is_companion {
            "MODO COMPANION".into()
        } else if self.is_away {
            "AUSENTE".into()
        } else {
            "COM VOCÊ".into()
        };
        let status_color = if is_calling {
            c.err
        } else if is_busy {
            c.warn
        } else if self.is_away {
            c.muted
        } else {
            c.ok
        };

        let top_mood_badge = if active_drawer != ActiveDrawer::Bond {
            Some(
                div()
                    .id("top-mood-portrait")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(sp.s2))
                    .pl(px(3.0))
                    .pr(px(sp.s3))
                    .py(px(3.0))
                    .rounded_full()
                    .bg(c.status_bg)
                    .border_1()
                    .border_color(mood_vis.accent)
                    .hover(|style| {
                        style
                            .bg(c.icon_bg_hover)
                            .border_color(c.primary)
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.bond_tab = BondTab::Mood;
                        this.active_drawer = ActiveDrawer::Bond;
                        this.fetch_bond(cx);
                        this.fetch_mood(cx);
                        if !this.memory_loading {
                            this.fetch_memory(cx);
                        }
                        this.poke(cx);
                        cx.notify();
                    }))
                    .child(render_lego_portrait(
                        48.0,
                        24.0,
                        mood_vis.accent,
                        hud_portrait,
                        hud_prev_slot,
                        mood_vis.glyph,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(c.muted)
                                    .child("HUMOR"),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(format!("{} {mood_label}", mood_vis.glyph)),
                            ),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(c.deep)
            .text_color(c.pearl)
            .font_family(theme::FONT_BODY)
            .track_focus(&self.focus_handle)
            .on_mouse_move(cx.listener(|this, _event: &MouseMoveEvent, _window, cx| {
                this.touch_ui(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.touch_ui(cx);
                let ks = &event.keystroke;
                let key = ks.key.as_str();

                // Atalhos com Control (ex: Ctrl+V para colar, Ctrl+Backspace / Ctrl+W para apagar palavra)
                if ks.modifiers.control && !ks.modifiers.alt {
                    if key.eq_ignore_ascii_case("v") {
                        if let Some(item) = cx.read_from_clipboard() {
                            let mut attached_image = false;
                            for entry in item.entries() {
                                if let gpui::ClipboardEntry::Image(image) = entry {
                                    let bytes = &image.bytes;
                                    if !bytes.is_empty() {
                                        let mime = if bytes.starts_with(b"\x89PNG") {
                                            "image/png"
                                        } else if bytes.starts_with(b"\xFF\xD8\xFF") {
                                            "image/jpeg"
                                        } else if bytes.starts_with(b"GIF") {
                                            "image/gif"
                                        } else if bytes.starts_with(b"RIFF")
                                            && bytes.len() > 12
                                            && &bytes[8..12] == b"WEBP"
                                        {
                                            "image/webp"
                                        } else {
                                            "image/png"
                                        };
                                        this.attached_media = Some(AttachedMedia::from_bytes(
                                            "screenshot.png".into(),
                                            mime,
                                            bytes.clone(),
                                        ));
                                        this.input_active = true;
                                        attached_image = true;
                                        cx.notify();
                                        break;
                                    }
                                }
                            }

                            if !attached_image {
                                if let Some(text) = item.text() {
                                    let trimmed = text.trim().trim_matches('"');
                                    let path = std::path::Path::new(trimmed);
                                    let is_image_ext = path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .map(|ext| {
                                            matches!(
                                                ext.to_ascii_lowercase().as_str(),
                                                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                                            )
                                        })
                                        .unwrap_or(false);

                                    if is_image_ext && path.is_file() {
                                        if let Ok(bytes) = std::fs::read(path) {
                                            let filename = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("imagem.png")
                                                .to_string();
                                            let mime = match path
                                                .extension()
                                                .and_then(|e| e.to_str())
                                                .map(|s| s.to_ascii_lowercase())
                                                .as_deref()
                                            {
                                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                                Some("png") => "image/png",
                                                Some("webp") => "image/webp",
                                                Some("gif") => "image/gif",
                                                Some("bmp") => "image/bmp",
                                                _ => "image/png",
                                            };
                                            this.attached_media = Some(AttachedMedia::from_bytes(
                                                filename,
                                                mime,
                                                bytes,
                                            ));
                                            this.input_active = true;
                                            cx.notify();
                                            return;
                                        }
                                    }

                                    this.input_active = true;
                                    this.pending_dead_key = None;
                                    this.input_text.push_str(&text);
                                    cx.notify();
                                }
                            }
                        }
                    } else if key.eq_ignore_ascii_case("w") || key == "backspace" {
                        this.input_active = true;
                        this.pending_dead_key = None;
                        if let Some(idx) = this.input_text.rfind(' ') {
                            this.input_text.truncate(idx);
                        } else {
                            this.input_text.clear();
                        }
                        cx.notify();
                    } else if key.eq_ignore_ascii_case("u") {
                        this.pending_dead_key = None;
                        this.input_text.clear();
                        cx.notify();
                    }
                    return;
                }

                // Ignora teclas com Alt isolado
                if ks.modifiers.alt && !ks.modifiers.control {
                    return;
                }

                if key == "enter" {
                    this.submit(cx);
                } else if key == "backspace" {
                    this.input_active = true;
                    if this.pending_dead_key.take().is_none() {
                        this.input_text.pop();
                    }
                    cx.notify();
                } else if key == "escape" {
                    this.pending_dead_key = None;
                    this.input_active = false;
                    this.active_drawer = ActiveDrawer::None;
                    cx.notify();
                } else if key == "space" || key == " " {
                    this.input_active = true;
                    if let Some(dead) = this.pending_dead_key.take() {
                        this.input_text.push(dead);
                    } else {
                        this.input_text.push(' ');
                    }
                    cx.notify();
                } else if let Some(ref ch) = ks.key_char {
                    if !ch.chars().any(|c| c.is_control()) {
                        this.input_active = true;
                        push_key_char_with_dead_key(&mut this.input_text, &mut this.pending_dead_key, ch);
                        cx.notify();
                    }
                } else if key.chars().count() == 1 {
                    let c = key.chars().next().unwrap();
                    if !c.is_control() {
                        this.input_active = true;
                        let mut buf = [0u8; 4];
                        let s = c.encode_utf8(&mut buf);
                        push_key_char_with_dead_key(&mut this.input_text, &mut this.pending_dead_key, s);
                        cx.notify();
                    }
                }
            }))
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
            // ── LAYER 3: LEGENDA DA USAMI (texto puro sobre a cena, sem balão) ──
            .when(show_bubble, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(150.0))
                        .flex()
                        .justify_center()
                        .px(px(sp.s6))
                        .child(
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
                        ),
                )
            })
            // ── LAYER 4: HEADER SUPERIOR (SOME NO IDLE) ──
            .when(!is_idle, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(sp.s5))
                        .left(px(sp.s6))
                        .right(px(sp.s6))
                        .flex()
                        .items_center()
                        .justify_between()
                        // Top Left: ações circulares em vidro
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .id("top-chat-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if chat_open { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if chat_open { c.primary } else { c.line })
                                        .when(chat_open, |el| el.shadow(theme::glow(c.primary, 14.0)))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Chat, cx);
                                        }))
                                        .child(icon("chat_bubble", 16.0)),
                                )
                                .child(
                                    div()
                                        .id("top-settings-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Bond && bond_tab == BondTab::Settings {
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
                                        }))
                                        .child(icon("gear", 16.0)),
                                )
                                .child(
                                    div()
                                        .id("top-recenter-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(c.icon_bg)
                                        .border_1()
                                        .border_color(c.line)
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.vrm_controller.reset_camera();
                                            this.vrm_controller.play_idle();
                                            cx.notify();
                                        }))
                                        .child(icon("recenter", 16.0)),
                                ),
                        )
                        // Top Center: pill com LED de estado
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .px(px(sp.s4))
                                .py(px(sp.s1_5))
                                .rounded_full()
                                .bg(c.status_bg)
                                .border_1()
                                .border_color(c.line)
                                .shadow(t.shadow.pill.clone())
                                .child({
                                    let dot = div()
                                        .size(px(7.0))
                                        .rounded_full()
                                        .bg(status_color)
                                        .shadow(theme::glow(status_color, 8.0));
                                    if is_busy {
                                        dot.with_animation(
                                            "status-pulse",
                                            Animation::new(Duration::from_millis(1000)).repeat(),
                                            |el, delta| el.opacity(0.4 + 0.6 * delta),
                                        )
                                        .into_any_element()
                                    } else {
                                        dot.into_any_element()
                                    }
                                })
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c.pearl)
                                        .child(status_label),
                                ),
                        )
                        // Top Right: VRMA + nível de vínculo + crédito
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(sp.s3))
                                .child(
                                    div()
                                        .id("top-vrma-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(38.0))
                                        .rounded_full()
                                        .bg(if active_drawer == ActiveDrawer::Vrma {
                                            c.icon_bg_active
                                        } else {
                                            c.icon_bg
                                        })
                                        .border_1()
                                        .border_color(if active_drawer == ActiveDrawer::Vrma {
                                            c.primary
                                        } else {
                                            c.line
                                        })
                                        .when(active_drawer == ActiveDrawer::Vrma, |el| {
                                            el.shadow(theme::glow(c.primary, 14.0))
                                        })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Vrma, cx);
                                        }))
                                        .child(icon("clapperboard", 16.0)),
                                )
                                .child(
                                    div()
                                        .id("top-bond-pill")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap(px(sp.s1_5))
                                        .px(px(sp.s3_5))
                                        .py(px(7.0))
                                        .rounded_full()
                                        .bg(t.bond_pill_bg())
                                        .border_1()
                                        .border_color(c.bond_border)
                                        .hover(|style| {
                                            style
                                                .bg(t.bond_pill_hover())
                                                .border_color(c.primary)
                                        })
                                        .text_size(px(ty.xs))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.toggle_drawer(ActiveDrawer::Bond, cx);
                                        }))
                                        .child(format!("♥ NÍVEL {bond_level}")),
                                )
                                .when_some(top_mood_badge, |row, badge| row.child(badge))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child("白ー (shiro) — Model by Sweet_nya555"),
                                ),
                        ),
                )
            })
            // ── LAYER 4b: PRÉVIA AO VIVO DO STT (o que já foi entendido) ──
            .when(show_live_caption, |parent| {
                parent.child(
                    div()
                        .absolute()
                        .top(px(94.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .px(px(sp.s6))
                        .child(
                            div()
                                .id("live-stt-caption")
                                .max_w(px(640.0))
                                .px(px(sp.s4))
                                .py(px(sp.s2))
                                .rounded(px(r.md))
                                .bg(rgba(0x120a20dd))
                                .border_1()
                                .border_color(theme::Palette::vn_border_bright())
                                .shadow(t.shadow.pill.clone())
                                .flex()
                                .items_center()
                                .gap(px(sp.s2))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.0))
                                        .text_color(c.lavender)
                                        .child("♪"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_size(px(ty.sm))
                                        .line_height(px(18.0))
                                        .text_color(c.pearl)
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(live_transcript),
                                ),
                        ),
                )
            })
            // ── LAYER 5: CONTROLES INFERIORES & COMPOSER FLUTUANTE ──
            .child(
                div()
                    .absolute()
                    .bottom(if is_idle { px(12.0) } else { px(sp.s5) })
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(9.0))
                    .opacity(if is_idle { 0.15 } else { 1.0 })
                    // Disclaimer (sempre presente; o idle esmaece junto, como na web)
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.faint)
                            .child("Mensagens usam os provedores configurados para responder."),
                    )
                    // Prévia de imagem anexada acima da barra de texto (estilo chat moderno)
                    .when_some(attached_media.as_ref(), |parent, media| {
                        parent.child(
                            div()
                                .id("composer-media-preview-row")
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
                            .w(px(720.0))
                            .max_w(px(760.0))
                            .min_h(px(46.0))
                            .flex()
                            .items_center()
                            .gap(px(sp.s1_5))
                            .pt(px(sp.s1_5))
                            .pb(px(sp.s1_5))
                            .pl(px(sp.s3))
                            .pr(px(sp.s2))
                            .rounded(px(r.composer))
                            .bg(c.composer_bg)
                            .border_1()
                            .border_color(if input_active { theme::Palette::vn_border_bright() } else { c.line_strong })
                            .shadow(if input_active {
                                t.shadow.composer_focus.clone()
                            } else {
                                t.shadow.composer.clone()
                            })
                            .on_mouse_move(cx.listener(|this, _, _, cx| {
                                this.poke(cx);
                            }))
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
                                    .bg(if has_media { c.accent_tint } else { gpui::rgba(0x00000000) })
                                    .text_color(if has_media { c.primary } else { c.muted })
                                    .hover(|style| style.bg(c.call_bg_hover).text_color(c.pearl))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.attach_file_prompt(cx);
                                    }))
                                    .child(icon("image", 18.0)),
                            )
                            // Campo de texto expansível
                            .child(
                                div()
                                    .id("input-box")
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .px(px(sp.s1_5))
                                    .py(px(sp.s1_5))
                                    .cursor_text()
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
                                            .w_full()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .items_center()
                                            .when(input_text.is_empty(), |row| {
                                                row.when(input_active, |row| {
                                                    row.child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .w(px(1.5))
                                                            .h(px(16.0))
                                                            .mr(px(1.5))
                                                            .bg(if caret_visible {
                                                                c.primary
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
                                                row.child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.0))
                                                        .max_h(px(90.0))
                                                        .overflow_hidden()
                                                        .text_size(px(ty.input))
                                                        .line_height(px(20.0))
                                                        .text_color(c.pearl)
                                                        .child(input_text.clone()),
                                                )
                                                .when(input_active, |row| {
                                                    row.child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .w(px(1.5))
                                                            .h(px(16.0))
                                                            .ml(px(2.0))
                                                            .bg(if caret_visible {
                                                                theme::Palette::vn_pink_neon()
                                                            } else {
                                                                gpui::rgba(0x00000000)
                                                            }),
                                                    )
                                                })
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
                                                .bg(if is_muted { c.danger_muted_bg } else { c.call_bg })
                                                .border_1()
                                                .border_color(if is_muted {
                                                    c.danger_muted_border
                                                } else {
                                                    c.line
                                                })
                                                .text_color(if is_muted { c.danger_muted } else { c.muted })
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
                                                .bg(if is_companion { c.accent_tint } else { c.call_bg })
                                                .border_1()
                                                .border_color(if is_companion { c.accent_border } else { c.line })
                                                .text_color(if is_companion { c.primary } else { c.muted })
                                                .when(is_companion, |el| el.shadow(theme::glow(c.primary, 10.0)))
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
                                        .border_color(if is_listening { c.success_border } else { c.line })
                                        .when(is_listening, |el| {
                                            el.shadow(theme::glow(c.ok, 14.0)).opacity(1.0)
                                        })
                                        .when(!is_listening, |el| el.opacity(0.6))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
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
                                        .bg(if chat_open { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if chat_open { c.primary } else { c.line })
                                        .when(chat_open, |el| el.shadow(theme::glow(c.primary, 14.0)))
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
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
                                        .hover(|style| style.bg(c.err_dark).shadow(theme::glow(c.err, 14.0)))
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
                                        .bg(if is_muted { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if is_muted { c.primary } else { c.line })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
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
                                        .bg(if is_companion { c.icon_bg_active } else { c.icon_bg })
                                        .border_1()
                                        .border_color(if is_companion { c.primary } else { c.line })
                                        .hover(|style| style.bg(c.icon_bg_hover).border_color(c.line_strong))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.poke(cx);
                                            this.toggle_sleep_companion();
                                            cx.notify();
                                        }))
                                        .child(icon("moon", 19.0)),
                                ),
                        )
                    }),
            )
            // ── LAYER 6: HISTÓRICO DA CONVERSA (PAINEL FLUTUANTE DA WEB) ──
            .when(chat_open, |parent| {
                parent.child(
                    div()
                        .id("chat-history")
                        .occlude()
                        .absolute()
                        .left(px(sp.s6))
                        .bottom(px(84.0))
                        .w(px(390.0))
                        .max_h(px(483.0))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(px(r.xl))
                        .bg(c.chat_bg)
                        .border_1()
                        .border_color(c.line_strong)
                        .shadow(t.shadow.panel.clone())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pt(px(sp.s3))
                                .pr(px(sp.s3_5))
                                .pb(px(sp.s2_5))
                                .pl(px(sp.s4))
                                .border_b_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(7.0))
                                        .text_size(px(11.5))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.lavender)
                                        .child(icon("chat_bubble", 14.0))
                                        .child("CONVERSA"),
                                )
                                .child(
                                    div()
                                        .id("drawer-close-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(28.0))
                                        .rounded_full()
                                        .text_color(c.muted)
                                        .hover(|style| style.bg(c.surface_soft).text_color(c.pearl))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::None;
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("close", 15.0)),
                                ),
                        )
                        .child(
                            div()
                                .id("chat-message-list")
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_col()
                                .justify_end()
                                .gap(px(sp.s2_5))
                                .p(px(sp.s3_5))
                                .overflow_y_scroll()
                                .when(no_turns, |col| {
                                    col.justify_center().child(
                                        div()
                                            .py(px(22.0))
                                            .px(px(sp.s3_5))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .child(
                                                div()
                                                    .mb(px(sp.s1_5))
                                                    .text_size(px(ty.empty_title))
                                                    .font_family(theme::FONT_CHARACTER)
                                                    .italic()
                                                    .text_color(c.pearl)
                                                    .child("Comece do seu jeito."),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(ty.sm))
                                                    .line_height(px(18.0))
                                                    .text_color(c.muted)
                                                    .child("Escreva abaixo ou inicie uma chamada com a Usami."),
                                            ),
                                    )
                                })
                                .children(turns.into_iter().map(|msg| {
                                    let is_user = msg.sender == Sender::User;
                                    let bubble = div()
                                        .flex()
                                        .flex_col()
                                        .max_w(px(343.0))
                                        .when(is_user, |el| el.items_end())
                                        .when(!is_user, |el| el.items_start())
                                        .child(
                                            div()
                                                .mx(px(7.0))
                                                .mb(px(sp.s1))
                                                .text_size(px(ty.xs))
                                                .font_family(theme::FONT_DISPLAY)
                                                .text_color(if is_user { c.primary } else { c.muted })
                                                .child(if is_user {
                                                    SharedString::from("VOCÊ")
                                                } else if let Some(emotion) = msg.emotion.as_deref() {
                                                    SharedString::from(format!("USAMI · {emotion}"))
                                                } else {
                                                    SharedString::from("USAMI")
                                                }),
                                        )
                                        .child(
                                            div()
                                                .max_w(px(343.0))
                                                .py(px(sp.s2_5))
                                                .px(px(sp.s3_5))
                                                .rounded(px(r.lg))
                                                .bg(c.chat_turn_bg)
                                                .border_1()
                                                .border_color(if is_user { c.chat_user_border } else { c.line })
                                                .text_size(px(ty.md))
                                                .line_height(px(19.0))
                                                .text_color(c.pearl)
                                                .child(msg.text),
                                        );
                                    div()
                                        .flex()
                                        .flex_shrink_0()
                                        .w_full()
                                        .when(is_user, |el| el.justify_end())
                                        .when(!is_user, |el| el.justify_start())
                                        .child(bubble)
                                }))
                                .when(is_busy, |col| {
                                    col.child(
                                        div()
                                            .flex()
                                            .flex_shrink_0()
                                            .w_full()
                                            .justify_start()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(5.0))
                                                    .py(px(sp.s2_5))
                                                    .px(px(sp.s3_5))
                                                    .rounded(px(r.lg))
                                                    .bg(c.chat_turn_bg)
                                                    .border_1()
                                                    .border_color(c.line)
                                                    .children((0..3usize).map(|idx| {
                                                        div()
                                                            .size(px(5.0))
                                                            .rounded_full()
                                                            .bg(c.lavender)
                                                            .with_animation(
                                                                ("typing", idx),
                                                                Animation::new(Duration::from_millis(1200)).repeat(),
                                                                move |el, delta| {
                                                                    let phase = (delta + idx as f32 * 0.15) % 1.0;
                                                                    let lift = if phase < 0.3 {
                                                                        phase / 0.3 * 3.0
                                                                    } else if phase < 0.6 {
                                                                        (1.0 - (phase - 0.3) / 0.3) * 3.0
                                                                    } else {
                                                                        0.0
                                                                    };
                                                                    let opacity = if phase < 0.6 {
                                                                        0.35 + phase / 0.6 * 0.65
                                                                    } else {
                                                                        1.0 - (phase - 0.6) / 0.4 * 0.65
                                                                    };
                                                                    el.opacity(opacity).mt(px(-lift))
                                                                },
                                                            )
                                                    })),
                                            ),
                                    )
                                }),
                        ),
                )
            })
            // ── LAYER 7: PAINÉIS LATERAIS (VÍNCULO / CONFIGURAÇÕES / VRMA) ──
            .when(side_drawer_open, |parent| {
                parent
                    .child(
                    div()
                        .id("side-drawer")
                        .occlude()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(420.0))
                        .h(viewport_h)
                        .flex()
                        .flex_col()
                        .border_l_1()
                        .border_color(c.line_strong)
                        .bg(c.panel)
                        .p(px(sp.s8))
                        .gap(px(sp.s4))
                        .shadow(t.shadow.drawer.clone())
                        .when(active_drawer != ActiveDrawer::Settings, |parent| {
                            parent.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb(px(sp.s3))
                                .border_b_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(theme::FONT_DISPLAY)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.lavender)
                                        .child(match active_drawer {
                                            ActiveDrawer::Chat => "",
                                            ActiveDrawer::Bond => "VÍNCULO COM A USAMI",
                                            ActiveDrawer::Settings => "CONFIGURAÇÕES",
                                            ActiveDrawer::Vrma => "ANIMAÇÕES VRMA",
                                            ActiveDrawer::None => "",
                                        }),
                                )
                                .child(
                                    div()
                                        .id("drawer-close-btn")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(28.0))
                                        .rounded_full()
                                        .text_color(c.muted)
                                        .hover(|style| style.bg(c.surface_soft).text_color(c.pearl))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.active_drawer = ActiveDrawer::None;
                                            this.poke(cx);
                                            cx.notify();
                                        }))
                                        .child(icon("close", 15.0)),
                                ),
                            )
                        })
                        .child(match active_drawer {
                            ActiveDrawer::Chat => div(),
                            ActiveDrawer::Bond => div(),
                            ActiveDrawer::Vrma => {
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(sp.s3))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child("TESTES DE ANIMAÇÃO"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap(px(sp.s2))
                                            .child(
                                                div()
                                                    .id("vrma-wave")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_vrma("goodbye", false);
                                                        cx.notify();
                                                    }))
                                                    .child("Acenar"),
                                            )
                                            .child(
                                                div()
                                                    .id("vrma-jump")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_vrma("jump", false);
                                                        cx.notify();
                                                    }))
                                                    .child("Pular"),
                                            )
                                            .child(
                                                div()
                                                    .id("vrma-idle")
                                                    .cursor_pointer()
                                                    .px(px(15.0))
                                                    .py(px(9.0))
                                                    .rounded(px(r.sm))
                                                    .bg(c.card_bg)
                                                    .border_1()
                                                    .border_color(c.line_strong)
                                                    .hover(|style| style.bg(c.accent_tint).border_color(c.primary))
                                                    .text_size(px(ty.sm))
                                                    .text_color(c.pearl)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.vrm_controller.play_idle();
                                                        cx.notify();
                                                    }))
                                                    .child("Idle Natural"),
                                            ),
                                    )
                            }
                            ActiveDrawer::Settings => div(),
                            ActiveDrawer::None => div(),
                        }),
                )
            })
            // ── LAYER 8: CENTRAL DE RELACIONAMENTO & DIÁRIO ESTILO VISUAL NOVEL ──
            .when(active_drawer == ActiveDrawer::Bond, |parent| {
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

                parent.child(
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
                                                BondTab::Profile => render_vn_tab_profile(t).into_any_element(),
                                                BondTab::Settings => self.render_settings_page(cx).into_any_element(),
                                            }
                                        ),
                                ),
                        ),
                )
            })
    }
}

fn main() {
    println!("[Usami Desktop] Inicializando interface minimalista idêntica à print...");
    Application::new().run(|cx: &mut App| {
        // Fontes OFL locais (sem CDN em runtime). Fica antes de abrir a janela
        // para o primeiro frame já medir com as famílias certas.
        theme::load_fonts(cx, asset_base_dir());
        let prefs = storage::load_prefs();
        let bounds = prefs
            .window
            .filter(|window| window.is_sane())
            .map(|window| {
                Bounds::new(
                    point(px(window.x), px(window.y)),
                    size(px(window.width), px(window.height)),
                )
            })
            .unwrap_or_else(|| Bounds::centered(None, size(px(1440.0), px(900.0)), cx));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Usami".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| UsamiApp::new(cx, prefs));
                window.focus(&view.read(cx).focus_handle);
                view.update(cx, |this, cx| this.attach_window_observers(window, cx));
                view.update(cx, |this, cx| this.fetch_bond(cx));
                view.update(cx, |this, cx| this.fetch_achievements(cx));
                view.update(cx, |this, cx| this.fetch_memory(cx));
                view.update(cx, |this, cx| this.fetch_mood(cx));
                #[cfg(target_os = "windows")]
                view.update(cx, |this, cx| this.attach_presence(window, cx));
                view
            },
        )
        .unwrap();
    });
}

#[cfg(test)]
mod tests {
    // `use gpui::*` no topo do crate traz o atributo `test` do GPUI; importar
    // só os helpers evita que `#[test]` resolva para ele e entre em recursão.
    use super::{
        ProactivityConfig, asset_path, farewell_can_close, first_existing, is_duplicate_utterance,
        manifest_dir, mood_preset_options, mood_visual, normalize_utterance_key,
        push_key_char_with_dead_key, relative_when_at, vn_history_tag,
    };
    use std::time::{Duration, Instant};

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
    fn farewell_holds_close_until_audio_ends_or_timeout() {
        use super::{CLOSE_FAREWELL_MAX, CLOSE_FAREWELL_MIN};
        assert!(!farewell_can_close(Duration::ZERO, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, true, true));
        assert!(farewell_can_close(CLOSE_FAREWELL_MIN, true, false));
        assert!(farewell_can_close(CLOSE_FAREWELL_MAX, false, true));
    }

    #[test]
    fn dead_key_accent_does_not_duplicate_when_os_sends_precomposed_vowel() {
        let mut text = String::from("olha voce a");
        let mut pending = None;

        // No GPUI Windows, pressionar '´' envia Some("´") e pressionar 'i' em seguida
        // envia Some("í") já composto pelo kernel (ToUnicode flag 1<<2).
        push_key_char_with_dead_key(&mut text, &mut pending, "´");
        assert_eq!(text, "olha voce a");
        assert_eq!(pending, Some('´'));

        push_key_char_with_dead_key(&mut text, &mut pending, "í");
        assert_eq!(text, "olha voce aí");
        assert_eq!(pending, None);
    }

    #[test]
    fn dead_key_accent_composes_when_os_sends_raw_ascii_vowel() {
        let mut text = String::new();
        let mut pending = None;

        push_key_char_with_dead_key(&mut text, &mut pending, "~");
        push_key_char_with_dead_key(&mut text, &mut pending, "a");
        push_key_char_with_dead_key(&mut text, &mut pending, "o");
        assert_eq!(text, "ão");
        assert_eq!(pending, None);
    }

    #[test]
    fn dead_key_accent_preserves_accent_before_non_composable_consonant() {
        let mut text = String::new();
        let mut pending = None;

        push_key_char_with_dead_key(&mut text, &mut pending, "´");
        push_key_char_with_dead_key(&mut text, &mut pending, "x");
        assert_eq!(text, "´x");
        assert_eq!(pending, None);
    }

    #[test]
    fn icon_paths_resolve_from_crate_dir() {
        let path = asset_path("assets/icons/chat.svg");
        assert!(path.is_absolute());
        assert!(path.starts_with(manifest_dir()));
        assert!(path.exists(), "ícone esperado em {}", path.display());
    }

    #[test]
    fn first_existing_skips_missing_candidates() {
        let dir =
            std::env::temp_dir().join(format!("usami-desktop-path-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("cria diretório temporário");
        let missing = dir.join("missing.png");
        let present = dir.join("present.png");
        std::fs::write(&present, b"x").expect("cria arquivo de teste");

        assert_eq!(
            first_existing([Some(missing.clone()), Some(present.clone())]),
            Some(present)
        );
        assert_eq!(first_existing([Some(missing), None]), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn proactivity_config_serializes_and_defaults_enabled() {
        let json = r#"{"enabled":false}"#;
        let cfg: ProactivityConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.enabled);

        let default_cfg: ProactivityConfig = serde_json::from_str("{}").unwrap();
        assert!(default_cfg.enabled);

        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(serialized.contains(r#""enabled":false"#));
    }

    #[test]
    fn duplicate_utterance_within_five_seconds_is_dropped() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(!is_duplicate_utterance(&mut last, "Oi, tudo bem?", t0));
        assert_eq!(normalize_utterance_key("Oi, tudo bem?"), "oitudobem");
        // Mesmas palavras sem pontuação 3s depois -> descartado
        assert!(is_duplicate_utterance(
            &mut last,
            "oi tudo bem",
            t0 + Duration::from_secs(3)
        ));
        // Palavras diferentes -> aceito
        assert!(!is_duplicate_utterance(
            &mut last,
            "Oi, tudo bem sim!",
            t0 + Duration::from_secs(3)
        ));
        // Mesmas palavras 6s depois -> aceito novamente
        assert!(!is_duplicate_utterance(
            &mut last,
            "oi tudo bem sim",
            t0 + Duration::from_secs(9)
        ));
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
