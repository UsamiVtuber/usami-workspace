//! Persistência local do desktop: conversa, preferências e marcador de sessão.
//!
//! O app web guarda isso no `localStorage` do navegador. Aqui os mesmos dados
//! vivem em `%APPDATA%\Usami`, com escrita atômica (arquivo temporário + rename)
//! para não deixar JSON truncado se o processo morrer no meio da gravação.
//!
//! O vínculo (medidores de afeto) não mora aqui: ele é compartilhado por
//! dispositivo e pertence ao core local (frente D). Este módulo cobre apenas o
//! que é local à janela: histórico, preferências e o marcador de sessão usado
//! para saber se a última saída foi avisada.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const MAX_STORED_TURNS: usize = 100;
const MAX_TURN_CHARS: usize = 8_000;
const SESSION_HEARTBEAT_MS: u64 = 60_000;
const CONVERSATION_VERSION: u32 = 1;
const SESSION_VERSION: u32 = 1;

/// Metadados leves de um anexo (imagem ou documento) persistido em disco.
/// Os bytes não viajam no JSON: só o caminho relativo ao `data_dir()` é guardado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMedia {
    pub filename: String,
    pub mime_type: String,
    /// Caminho relativo a `data_dir()`, p. ex. `media/1699999999_arquivo.pdf`.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTurn {
    pub sender: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emotion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<StoredMedia>,
}

#[derive(Serialize, Deserialize)]
struct ConversationFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    turns: Vec<StoredTurn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPrefs {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl WindowPrefs {
    /// Rejeita geometria absurda (arquivo editado à mão, tela desconectada).
    /// Coordenadas negativas são válidas em multi-monitor.
    pub fn is_sane(&self) -> bool {
        let finite = self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite();
        finite
            && (640.0..=10_000.0).contains(&self.width)
            && (480.0..=10_000.0).contains(&self.height)
            && (-30_000.0..=30_000.0).contains(&self.x)
            && (-30_000.0..=30_000.0).contains(&self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiPrefs {
    #[serde(default)]
    pub muted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowPrefs>,
    /// Abrir já no modo mascote (janela pequena sem moldura).
    #[serde(default)]
    pub mascot: bool,
    /// Botão minimizar (–) entra no modo mascote. Ligado por padrão;
    /// as Configurações podem desligar.
    #[serde(default = "default_true")]
    pub mascot_enabled: bool,
    /// No modo mascote, a cabeça e os olhos acompanham o cursor pela tela.
    #[serde(default)]
    pub mascot_follow: bool,
    /// Caminho do modelo `.vrm` selecionado pelo usuário (None = padrão).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrm_path: Option<String>,
    /// Simulação de física de cabelo/roupa (SpringBones) no motor WeebUI.
    #[serde(default = "default_true")]
    pub vrm_spring_bones: bool,
    /// Abertura lateral dos braços em graus (0..=45) para evitar colisão com saias/mangas.
    #[serde(default = "default_arm_clearance_deg")]
    pub vrm_arm_clearance_deg: f32,
}

fn default_arm_clearance_deg() -> f32 {
    8.0
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            muted: false,
            window: None,
            mascot: false,
            mascot_enabled: true,
            mascot_follow: false,
            vrm_path: None,
            vrm_spring_bones: true,
            vrm_arm_clearance_deg: default_arm_clearance_deg(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionFile {
    #[serde(default)]
    version: u32,
    pub started_at_ms: u64,
    pub last_seen_ms: u64,
    pub clean_shutdown: bool,
}

impl SessionFile {
    pub fn start() -> Self {
        let now = now_ms();
        Self {
            version: SESSION_VERSION,
            started_at_ms: now,
            last_seen_ms: now,
            clean_shutdown: false,
        }
    }

    /// Atualiza o batimento se passou pelo menos 1 minuto; devolve `true`
    /// quando o arquivo precisa ser regravado.
    pub fn touch(&mut self) -> bool {
        let now = now_ms();
        if now.saturating_sub(self.last_seen_ms) < SESSION_HEARTBEAT_MS {
            return false;
        }
        self.last_seen_ms = now;
        true
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|dir| dir.join("Usami"))
        .unwrap_or_else(|| std::env::temp_dir().join("usami-desktop"))
}

fn conversation_path() -> PathBuf {
    data_dir().join("conversation.json")
}

fn media_dir() -> PathBuf {
    data_dir().join("media")
}

/// Resolve um caminho relativo de mídia (guardado no `StoredMedia.path`)
/// de volta ao caminho absoluto em disco.
pub fn media_path(relative: &str) -> PathBuf {
    data_dir().join(relative)
}

/// Grava os bytes de um anexo em `data_dir()/media/` com um nome de arquivo
/// seguro (timestamp + nome original sanitizado) e devolve o caminho relativo
/// a ser guardado no `StoredMedia.path`. Devolve `None` em caso de falha.
pub fn save_media_file(filename: &str, _mime_type: &str, bytes: &[u8]) -> Option<String> {
    let dir = media_dir();
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let safe_name = filename
        .rsplit('/')
        .next()
        .and_then(|n| n.rsplit('\\').next())
        .unwrap_or("anexo")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = format!("{ts}_{safe_name}");
    let path = dir.join(&file_name);
    match fs::write(&path, bytes) {
        Ok(()) => Some(format!("media/{file_name}")),
        Err(error) => {
            eprintln!(
                "[Usami Storage] falha ao gravar mídia {}: {error}",
                path.display()
            );
            None
        }
    }
}

fn prefs_path() -> PathBuf {
    data_dir().join("ui.json")
}

fn session_path() -> PathBuf {
    data_dir().join("session.json")
}

/// Registra falhas/panics locais em `%APPDATA%\Usami\crash.log` para diagnóstico
/// offline mesmo quando o backend não está acessível.
pub fn append_crash_log(entry: &str) {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("crash.log");
    use std::io::Write;
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}] {}", now_ms(), entry);
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(bytes) = serde_json::to_vec_pretty(value) else {
        eprintln!("[Usami Storage] falha ao serializar {}", path.display());
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = fs::write(&tmp, bytes) {
        eprintln!("[Usami Storage] falha ao gravar {}: {error}", tmp.display());
        return;
    }
    if let Err(error) = fs::rename(&tmp, path) {
        eprintln!(
            "[Usami Storage] falha ao publicar {}: {error}",
            path.display()
        );
        let _ = fs::remove_file(&tmp);
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn sanitize_turns(turns: Vec<StoredTurn>) -> Vec<StoredTurn> {
    turns
        .into_iter()
        .filter_map(|turn| {
            let sender = turn.sender.trim().to_lowercase();
            if sender != "user" && sender != "usami" {
                return None;
            }
            let text = turn.text.trim();
            if text.is_empty() {
                return None;
            }
            let mut text = text.to_string();
            if text.chars().count() > MAX_TURN_CHARS {
                text = text.chars().take(MAX_TURN_CHARS).collect();
            }
            let emotion = turn
                .emotion
                .map(|value| value.trim().chars().take(64).collect::<String>())
                .filter(|value| !value.is_empty());
            let media = turn.media.filter(|m| !m.path.is_empty());
            Some(StoredTurn {
                sender,
                text,
                emotion,
                media,
            })
        })
        .collect()
}

fn trim_turns(mut turns: Vec<StoredTurn>) -> Vec<StoredTurn> {
    if turns.len() > MAX_STORED_TURNS {
        let overflow = turns.len() - MAX_STORED_TURNS;
        turns.drain(0..overflow);
    }
    turns
}

pub fn load_conversation() -> Vec<StoredTurn> {
    load_conversation_from(&conversation_path())
}

fn load_conversation_from(path: &Path) -> Vec<StoredTurn> {
    let Some(file) = read_json::<ConversationFile>(path) else {
        return Vec::new();
    };
    trim_turns(sanitize_turns(file.turns))
}

pub fn save_conversation(turns: &[StoredTurn]) {
    save_conversation_to(&conversation_path(), turns);
}

fn save_conversation_to(path: &Path, turns: &[StoredTurn]) {
    let file = ConversationFile {
        version: CONVERSATION_VERSION,
        turns: trim_turns(sanitize_turns(turns.to_vec())),
    };
    write_json(path, &file);
}

pub fn load_prefs() -> UiPrefs {
    load_prefs_from(&prefs_path())
}

fn load_prefs_from(path: &Path) -> UiPrefs {
    let mut prefs = read_json::<UiPrefs>(path).unwrap_or_default();
    if prefs.window.is_some_and(|window| !window.is_sane()) {
        prefs.window = None;
    }
    prefs
}

pub fn save_prefs(prefs: &UiPrefs) {
    save_prefs_to(&prefs_path(), prefs);
}

fn save_prefs_to(path: &Path, prefs: &UiPrefs) {
    let mut prefs = prefs.clone();
    if prefs.window.is_some_and(|window| !window.is_sane()) {
        prefs.window = None;
    }
    write_json(path, &prefs);
}

pub fn load_session() -> Option<SessionFile> {
    load_session_from(&session_path())
}

fn load_session_from(path: &Path) -> Option<SessionFile> {
    read_json::<SessionFile>(path)
}

pub fn save_session(session: &SessionFile) {
    write_json(&session_path(), session);
}

/// Marca a sessão como encerrada normalmente. Chamado quando a janela fecha;
/// um kill abrupto deixa o marcador em `false` para a próxima abertura notar.
pub fn mark_session_clean() {
    let path = session_path();
    let Some(mut session) = load_session_from(&path) else {
        return;
    };
    session.clean_shutdown = true;
    session.last_seen_ms = now_ms();
    write_json(&path, &session);
}

// ── Proatividade (persistência de configuração e eventos vistos) ──

fn proactivity_config_path() -> PathBuf {
    data_dir().join("proactivity_config.json")
}

#[derive(Serialize, Deserialize)]
pub struct ProactivityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn load_proactive_config() -> bool {
    let path = proactivity_config_path();
    if let Some(cfg) = read_json::<ProactivityConfig>(&path) {
        return cfg.enabled;
    }
    true
}

pub fn save_proactive_config(enabled: bool) {
    let path = proactivity_config_path();
    let cfg = ProactivityConfig { enabled };
    write_json(&path, &cfg);
}

fn proactivity_seen_path() -> PathBuf {
    data_dir().join("proactivity_seen.json")
}

pub fn load_proactive_seen() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let path = proactivity_seen_path();
    if let Some(list) = read_json::<Vec<String>>(&path) {
        for id in list {
            set.insert(id);
        }
    }
    set
}

pub fn save_proactive_seen(seen: &std::collections::HashSet<String>) {
    let path = proactivity_seen_path();
    let list: Vec<&String> = seen.iter().take(200).collect();
    write_json(&path, &list);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("usami-storage-test-{}-{name}", std::process::id()))
    }

    fn turn(sender: &str, text: &str) -> StoredTurn {
        StoredTurn {
            sender: sender.into(),
            text: text.into(),
            emotion: None,
            media: None,
        }
    }

    #[test]
    fn conversation_round_trip_keeps_last_hundred_turns() {
        let path = temp_path("conversation").with_extension("json");
        let turns: Vec<StoredTurn> = (0..120)
            .map(|index| turn("user", &format!("mensagem {index}")))
            .collect();

        save_conversation_to(&path, &turns);
        let loaded = load_conversation_from(&path);

        assert_eq!(loaded.len(), MAX_STORED_TURNS);
        assert_eq!(loaded[0].text, "mensagem 20");
        assert_eq!(loaded[MAX_STORED_TURNS - 1].text, "mensagem 119");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn conversation_drops_unknown_sender_and_empty_text() {
        let path = temp_path("conversation-filter").with_extension("json");
        let turns = vec![
            turn("user", "  oi  "),
            turn("system", "não deveria entrar"),
            turn("usami", "   "),
            turn("USAMI", "tudo bem"),
        ];

        save_conversation_to(&path, &turns);
        let loaded = load_conversation_from(&path);

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "oi");
        assert_eq!(loaded[0].sender, "user");
        assert_eq!(loaded[1].sender, "usami");
        assert_eq!(loaded[1].text, "tudo bem");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn session_abrupt_is_detected_until_marked_clean() {
        let path = temp_path("session").with_extension("json");
        write_json(&path, &SessionFile::start());
        let previous = load_session_from(&path).expect("sessão gravada");
        assert!(!previous.clean_shutdown);

        let mut clean = previous.clone();
        clean.clean_shutdown = true;
        write_json(&path, &clean);
        assert!(
            load_session_from(&path)
                .expect("sessão gravada")
                .clean_shutdown
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn window_prefs_reject_absurd_geometry() {
        let sane = WindowPrefs {
            x: -1200.0,
            y: 40.0,
            width: 1440.0,
            height: 900.0,
        };
        assert!(sane.is_sane());

        let tiny = WindowPrefs {
            width: 10.0,
            ..sane
        };
        assert!(!tiny.is_sane());

        let not_finite = WindowPrefs {
            width: f32::NAN,
            ..sane
        };
        assert!(!not_finite.is_sane());
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
}
