use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8787";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Base do backend: `USAMI_API_BASE` quando definida e não vazia, senão o
/// endereço local padrão. O resultado nunca termina com `/`.
pub fn normalize_base_url(value: Option<String>) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub fn api_base_url_from_env() -> String {
    normalize_base_url(std::env::var("USAMI_API_BASE").ok())
}

fn read_token_from_env_file(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.starts_with("USAMI_API_TOKEN") {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=') {
            if key.trim() == "USAMI_API_TOKEN" {
                let token = val.trim().trim_matches('"').trim_matches('\'').trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

pub fn api_token_from_env() -> Option<String> {
    if let Some(token) = std::env::var("USAMI_API_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Some(token);
    }

    // Fallback de desenvolvimento e desktop local: busca `.env` próximo
    let candidates = [
        PathBuf::from(".env"),
        PathBuf::from("../.env"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(".env")))
            .unwrap_or_default(),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            if let Some(token) = read_token_from_env_file(&candidate) {
                return Some(token);
            }
        }
    }

    None
}

/// Erros de rede chegam em inglês técnico (`error sending request for url ...`);
/// a UI é pt-BR e não deve vazar isso para quem usa o app.
fn map_request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return "O servidor local demorou para responder.".to_string();
    }
    if error.is_connect() {
        return "Servidor local indisponível — inicie o core em 127.0.0.1:8787.".to_string();
    }
    if let Some(status) = error.status() {
        return match status.as_u16() {
            401 => "Token recusado pelo servidor local (USAMI_API_TOKEN).".to_string(),
            403 => "Origem não autorizada pelo servidor local.".to_string(),
            503 => "Servidor local sem token configurado (USAMI_API_TOKEN).".to_string(),
            code => format!("O servidor local respondeu com erro {code}."),
        };
    }
    "Falha ao falar com o servidor local.".to_string()
}

// Sem timeout, um backend desligado deixava o app em "PENSANDO" até o TCP
// desistir. O teto de 90 s cobre o turno de chat mais lento + TTS.
fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .unwrap_or_else(|error| {
            eprintln!("[Usami API] Falha ao configurar timeouts ({error}); usando cliente padrão.");
            reqwest::Client::new()
        })
}

pub fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("usami-tokio-worker")
            .build()
            .expect("Falha ao inicializar Tokio runtime")
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRequest {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
    #[serde(rename = "wakeUpJolt", skip_serializing_if = "Option::is_none")]
    pub wake_up_jolt: Option<bool>,
    #[serde(
        rename = "sleepCompanionActive",
        skip_serializing_if = "Option::is_none"
    )]
    pub sleep_companion_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: String,
    pub name: Option<String>,
    pub value: Option<f32>,
    pub weight: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTiming {
    #[serde(rename = "surfaceMs", default)]
    pub surface_ms: Option<u64>,
    #[serde(rename = "llmMs", default)]
    pub llm_ms: Option<u64>,
    #[serde(rename = "totalMs", default)]
    pub total_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentTurnSummary {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub route: Option<String>,
    pub status: Option<String>,
    pub tool: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub reply: String,
    pub gesture: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
    pub source: Option<String>,
    #[serde(rename = "speechTags", default)]
    pub speech_tags: Vec<String>,
    #[serde(default)]
    pub timing: Option<ChatTiming>,
    #[serde(default)]
    pub affinity: Option<i32>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub mood: Option<MoodData>,
    #[serde(rename = "agentTurn", default)]
    pub agent_turn: Option<AgentTurnSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryUser {
    pub name: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySections {
    #[serde(default)]
    pub user: Vec<String>,
    #[serde(default)]
    pub preferences: Vec<String>,
    #[serde(default)]
    pub hobbies: Vec<String>,
    #[serde(default)]
    pub relationship: Vec<String>,
    #[serde(default)]
    pub dates: Vec<String>,
    #[serde(default)]
    pub other: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFact {
    pub fact: String,
    pub section: String,
    pub date: Option<String>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySummary {
    pub date: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHistoryItem {
    pub date: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryAffinity {
    #[serde(default)]
    pub score: i32,
    #[serde(rename = "lastDailyGainDate")]
    pub last_daily_gain_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryData {
    pub version: Option<u32>,
    #[serde(default)]
    pub user: Option<MemoryUser>,
    #[serde(default)]
    pub sections: MemorySections,
    #[serde(rename = "recentFacts", default)]
    pub recent_facts: Vec<RecentFact>,
    #[serde(default)]
    pub summaries: Vec<MemorySummary>,
    #[serde(default)]
    pub history: Vec<MemoryHistoryItem>,
    #[serde(default)]
    pub affinity: Option<MemoryAffinity>,
    /// Cópia dos medidores do vínculo guardada no memory.json pelo core.
    #[serde(default)]
    pub bond: Option<BondData>,
}

impl MemoryData {
    pub fn durable_count(&self) -> usize {
        self.sections.user.len()
            + self.sections.preferences.len()
            + self.sections.hobbies.len()
            + self.sections.relationship.len()
            + self.sections.dates.len()
            + self.sections.other.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResponse {
    pub ok: bool,
    pub memory: MemoryData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryForgetRequest {
    pub kind: String,
    pub confirm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryForgetResponse {
    pub ok: bool,
    #[serde(default)]
    pub removed: Option<usize>,
    #[serde(default)]
    pub memory: Option<MemoryData>,
}

/// Medidores do vínculo (0–999). O core decide os ganhos; o desktop só lê.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondMeters {
    #[serde(default)]
    pub carinho: u32,
    #[serde(default)]
    pub humor: u32,
    #[serde(default)]
    pub companhia: u32,
}

/// Progresso de um medidor já calculado pelo core (barra sem repetir thresholds).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondMeterState {
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondMetersState {
    #[serde(default)]
    pub carinho: BondMeterState,
    #[serde(default)]
    pub humor: BondMeterState,
    #[serde(default)]
    pub companhia: BondMeterState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondLogEntry {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub gains: BondMeters,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondSummary {
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub line: String,
    #[serde(default)]
    pub next: Option<u32>,
    #[serde(default)]
    pub remaining: u32,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub max: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BondData {
    #[serde(default)]
    pub intimacy: BondMeters,
    #[serde(default)]
    pub meters: BondMetersState,
    #[serde(default)]
    pub log: Vec<BondLogEntry>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub summary: BondSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BondResponse {
    pub ok: bool,
    pub bond: BondData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoodMeters {
    #[serde(default)]
    pub valence: u32,
    #[serde(default)]
    pub energy: u32,
    #[serde(default)]
    pub warmth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoodExpression {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoodTimelineEntry {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub emotion: String,
    #[serde(default)]
    pub reason: String,
    pub at: i64,
}

/// Humor contínuo calculado pelo core (`GET /api/mood` e campo `mood` do chat).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoodData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "vrmEmotion", default)]
    pub vrm_emotion: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub meters: MoodMeters,
    #[serde(default)]
    pub expression: MoodExpression,
    #[serde(default)]
    pub timeline: Vec<MoodTimelineEntry>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoodResponse {
    pub ok: bool,
    pub mood: MoodData,
}

/// Conquista desbloqueável avaliada pelo core a partir de atividade real.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Chave estável do emblema; o desktop mapeia para um glifo próprio.
    pub icon: String,
    pub unlocked: bool,
    #[serde(rename = "unlockedAt", default)]
    pub unlocked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementsResponse {
    pub ok: bool,
    pub achievements: Vec<Achievement>,
}

fn default_trait_val() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonalityTraits {
    #[serde(default = "default_trait_val")]
    pub warmth: u32,
    #[serde(default = "default_trait_val")]
    pub curiosity: u32,
    #[serde(default = "default_trait_val")]
    pub playfulness: u32,
    #[serde(default = "default_trait_val")]
    pub pride: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonalityData {
    pub version: Option<u32>,
    #[serde(default)]
    pub traits: PersonalityTraits,
    #[serde(rename = "customPrompt", default)]
    pub custom_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitMeta {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityResponse {
    pub ok: bool,
    pub personality: PersonalityData,
    #[serde(default)]
    pub traits: Vec<TraitMeta>,
    #[serde(rename = "customPromptMax", default)]
    pub custom_prompt_max: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traits: Option<PersonalityTraits>,
    #[serde(rename = "customPrompt", skip_serializing_if = "Option::is_none")]
    pub custom_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub text: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: Option<String>,
    #[serde(rename = "notBefore", default)]
    pub not_before: Option<String>,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<String>,
    #[serde(rename = "dedupeKey", default)]
    pub dedupe_key: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default = "default_true")]
    pub speak: bool,
    #[serde(default)]
    pub emotion: Option<String>,
    #[serde(default)]
    pub gesture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveEventsResponse {
    pub events: Vec<ProactiveEvent>,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveAckItem {
    pub id: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveAckRequest {
    pub acks: Vec<ProactiveAckItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveAckResponse {
    pub ok: bool,
    #[serde(default)]
    pub acked: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillActivation {
    #[serde(rename = "canActivate", default)]
    pub can_activate: bool,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMissing {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSkill {
    pub id: String,
    #[serde(default)]
    pub valid: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(rename = "maxPermissionLevel", default)]
    pub max_permission_level: Option<u8>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub activation: SkillActivation,
    #[serde(default)]
    pub missing: SkillMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillsResponse {
    pub ok: bool,
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillToggleResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub skill: Option<AgentSkill>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentExecution {
    pub id: String,
    pub tool: String,
    pub status: String,
    #[serde(rename = "permissionLevel", default)]
    pub permission_level: Option<u8>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(rename = "startedAt", default)]
    pub started_at: String,
    #[serde(rename = "durationMs", default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(rename = "inputSummary", default)]
    pub input_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionsResponse {
    pub ok: bool,
    #[serde(default)]
    pub executions: Vec<AgentExecution>,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfToolSummary {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(rename = "permissionLevel", default)]
    pub permission_level: u8,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub revoked: bool,
    #[serde(rename = "revokedReason", default)]
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSelfToolsData {
    #[serde(default)]
    pub tools: Vec<SelfToolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSelfResponse {
    pub ok: bool,
    #[serde(rename = "self", default)]
    pub self_data: AgentSelfToolsData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRevocation {
    pub tool: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(rename = "revokedAt", default)]
    pub revoked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermissionsResponse {
    pub ok: bool,
    #[serde(default)]
    pub revocations: Vec<PermissionRevocation>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillProposal {
    pub id: String,
    #[serde(rename = "skillId")]
    pub skill_id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub benefit: String,
    #[serde(default)]
    pub risks: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<u8>,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillProposalsResponse {
    pub ok: bool,
    #[serde(default)]
    pub proposals: Vec<SkillProposal>,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuedConfirmation {
    pub token: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(rename = "permissionLevel", default)]
    pub permission_level: Option<u8>,
    #[serde(rename = "expiresAt", default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConfirmationResponse {
    pub ok: bool,
    pub confirmation: IssuedConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstallResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub installed: Option<bool>,
    #[serde(default)]
    pub skill: Option<AgentSkill>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPlanStep {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(rename = "permissionLevel", default)]
    pub permission_level: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPlan {
    pub id: String,
    pub goal: String,
    pub version: u32,
    pub status: String,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(default)]
    pub steps: Vec<AgentPlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlansResponse {
    pub ok: bool,
    #[serde(default)]
    pub plans: Vec<AgentPlan>,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSuggestion {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(rename = "recommendedGoal", default)]
    pub recommended_goal: String,
    pub status: String,
    #[serde(default)]
    pub relevance: f64,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "planId", default)]
    pub plan_id: Option<String>,
    #[serde(rename = "dismissReason", default)]
    pub dismiss_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSuggestionsResponse {
    pub ok: bool,
    #[serde(default)]
    pub suggestions: Vec<AgentSuggestion>,
    #[serde(default)]
    pub count: usize,
    #[serde(rename = "unansweredCount", default)]
    pub unanswered_count: usize,
    #[serde(default)]
    pub pending: Option<AgentSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionActionResponse {
    pub ok: bool,
    pub suggestion: AgentSuggestion,
    #[serde(default)]
    pub plan: Option<AgentPlan>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BriefingCounts {
    #[serde(default)]
    pub commitments: usize,
    #[serde(rename = "priorityTasks", default)]
    pub priority_tasks: usize,
    #[serde(default)]
    pub projects: usize,
    #[serde(default)]
    pub pending: usize,
    #[serde(rename = "pendingPlans", default)]
    pub pending_plans: usize,
    #[serde(default)]
    pub executions: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyBriefingData {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub empty: bool,
    #[serde(default)]
    pub text: String,
    #[serde(rename = "narrativeSource", default)]
    pub narrative_source: Option<String>,
    #[serde(default)]
    pub counts: BriefingCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBriefingResponse {
    pub ok: bool,
    pub briefing: DailyBriefingData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SttResponse {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsRequest {
    pub text: String,
    #[serde(rename = "styleId")]
    pub style_id: String,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaAnalyzeRequest {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaAnalyzeResponse {
    pub ok: bool,
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub gesture: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub ocr: Vec<String>,
    #[serde(default)]
    pub transcript: Option<String>,
    #[serde(rename = "crossReference", default)]
    pub cross_reference: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub struct UsamiApiClient {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl UsamiApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_token(base_url, None)
    }

    pub fn with_token(base_url: impl Into<String>, token: Option<String>) -> Self {
        Self {
            base_url: normalize_base_url(Some(base_url.into())),
            token,
            client: build_http_client(),
        }
    }

    pub fn from_env() -> Self {
        Self::with_token(api_base_url_from_env(), api_token_from_env())
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub async fn send_chat(
        &self,
        message: &str,
        sleep_companion_active: bool,
    ) -> Result<ChatResponse, String> {
        let capabilities = serde_json::json!({
            "format": "VRM",
            // Nomes reais de clipe no gpui-vrm e aceitos por GESTURE_NAMES no
            // servidor. "happy" saiu: é emoção (set_expression), não clipe.
            "gestures": ["tilt", "idle", "wave", "angry", "blush", "jump", "sad", "sleepy", "surprised", "clapping", "clap", "nod", "talking", "goodbye"],
            "expressions": [
                "happy", "angry", "sad", "relaxed", "surprised",
                "01-prefab_happy01", "02-prefab_happy02", "08-prefab_angry02", "12-prefab_sad01",
                "13-prefab_sad02", "15-prefab_awkward01", "16-prefab_awkward02", "19-prefab_doubt",
                "20-prefab_startled", "21-prefab_emoji"
            ],
            "sequences": true
        });

        let payload = ChatMessageRequest {
            message: message.to_string(),
            capabilities: Some(capabilities),
            wake_up_jolt: None,
            sleep_companion_active: Some(sleep_companion_active),
        };

        let client = self.client.clone();
        let url = format!("{}/api/chat", self.base_url);

        let handle = tokio_runtime().spawn(async move {
            let res = client
                .post(url)
                .json(&payload)
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<ChatResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<ChatResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    /// Pede o TTS em streaming. Deve ser chamado de dentro de `tokio_runtime`
    /// porque a resposta é lida em pedaços pelo chamador.
    pub async fn stream_tts(&self, text: &str, style_id: &str) -> Result<TtsStream, String> {
        let payload = TtsRequest {
            text: text.to_string(),
            style_id: style_id.to_string(),
            stream: true,
        };

        let response = self
            .client
            // O teto padrão de 90 s vale para o corpo inteiro; uma fala longa em
            // streaming passaria disso e seria cortada no meio.
            .post(format!("{}/api/tts", self.base_url))
            .timeout(Duration::from_secs(180))
            .json(&payload)
            .send()
            .await
            .map_err(map_request_error)?
            .error_for_status()
            .map_err(map_request_error)?;

        if is_pcm_stream(&response) {
            let sample_rate = pcm_sample_rate(&response);
            Ok(TtsStream::Pcm {
                sample_rate,
                response,
            })
        } else {
            let bytes = response.bytes().await.map_err(map_request_error)?;
            Ok(TtsStream::Audio(bytes.to_vec()))
        }
    }

    pub async fn transcribe(&self, wav: Vec<u8>) -> Result<String, String> {
        let client = self.client.clone();
        let url = format!("{}/api/stt", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let part = reqwest::multipart::Part::bytes(wav)
                .file_name("mic.wav")
                .mime_str("audio/wav")
                .map_err(map_request_error)?;
            let form = reqwest::multipart::Form::new().part("audio", part);
            let mut req = client.post(url).multipart(form);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let response = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<SttResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<String, String>(response.text)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_memory(&self) -> Result<MemoryResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/memory", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<MemoryResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<MemoryResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn forget_memory(
        &self,
        payload: &MemoryForgetRequest,
    ) -> Result<MemoryForgetResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/memory/forget", self.base_url);
        let token = self.token.clone();
        let payload = payload.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&payload);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<MemoryForgetResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<MemoryForgetResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_bond(&self) -> Result<BondResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/bond", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<BondResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<BondResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_achievements(&self) -> Result<AchievementsResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/achievements", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<AchievementsResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<AchievementsResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    /// Manda o turno para o core calcular o XP (a regra não vive no cliente).
    pub async fn award_bond_chat(
        &self,
        content: &str,
        spoken: bool,
    ) -> Result<BondResponse, String> {
        let payload = serde_json::json!({ "content": content, "spoken": spoken });
        self.post_bond_event(payload).await
    }

    pub async fn award_bond_call(&self, minutes: u32) -> Result<BondResponse, String> {
        let payload = serde_json::json!({ "kind": "call", "minutes": minutes });
        self.post_bond_event(payload).await
    }

    async fn post_bond_event(&self, payload: serde_json::Value) -> Result<BondResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/bond/event", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&payload);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<BondResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<BondResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_mood(&self) -> Result<MoodResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/mood", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<MoodResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<MoodResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn post_mood_event(
        &self,
        emotion: &str,
        reason: Option<&str>,
    ) -> Result<MoodResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/mood/event", self.base_url);
        let token = self.token.clone();
        let payload = serde_json::json!({
            "emotion": emotion,
            "reason": reason.unwrap_or(""),
        });

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&payload);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<MoodResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<MoodResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_personality(&self) -> Result<PersonalityResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/personality", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<PersonalityResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<PersonalityResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn update_personality(
        &self,
        payload: &PersonalityUpdateRequest,
    ) -> Result<PersonalityResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/personality", self.base_url);
        let token = self.token.clone();
        let payload = payload.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&payload);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req
                .send()
                .await
                .map_err(map_request_error)?
                .error_for_status()
                .map_err(map_request_error)?
                .json::<PersonalityResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<PersonalityResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_proactive_events(
        &self,
        activity: &str,
    ) -> Result<ProactiveEventsResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/proactive/events", self.base_url);
        let token = self.token.clone();
        let activity = activity.to_string();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client
                .get(url)
                .header("x-usami-client", "usami-desktop")
                .header("x-usami-activity", activity);

            if let Some(t) = token {
                req = req.header("x-usami-token", t);
            }

            let res = req.send().await.map_err(map_request_error)?;

            if !res.status().is_success() {
                return Err(format!("HTTP {}", res.status()));
            }

            let data = res
                .json::<ProactiveEventsResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<ProactiveEventsResponse, String>(data)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn ack_proactive_events(
        &self,
        acks: &[ProactiveAckItem],
    ) -> Result<ProactiveAckResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/proactive/events/ack", self.base_url);
        let token = self.token.clone();
        let payload = ProactiveAckRequest {
            acks: acks.to_vec(),
        };

        let handle = tokio_runtime().spawn(async move {
            let mut req = client
                .post(url)
                .json(&payload)
                .header("x-usami-client", "usami-desktop");

            if let Some(t) = token {
                req = req.header("x-usami-token", t);
            }

            let res = req.send().await.map_err(map_request_error)?;

            if !res.status().is_success() {
                return Err(format!("HTTP {}", res.status()));
            }

            let data = res
                .json::<ProactiveAckResponse>()
                .await
                .map_err(map_request_error)?;

            Ok::<ProactiveAckResponse, String>(data)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn analyze_media(
        &self,
        mime_type: &str,
        data_base64: &str,
        filename: Option<&str>,
        prompt: Option<&str>,
        capabilities: Option<serde_json::Value>,
    ) -> Result<MediaAnalyzeResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/media/analyze", self.base_url);
        let token = self.token.clone();
        let payload = MediaAnalyzeRequest {
            mime_type: mime_type.to_string(),
            data: data_base64.to_string(),
            filename: filename.map(ToString::to_string),
            prompt: prompt.map(ToString::to_string),
            capabilities,
        };

        let handle = tokio_runtime().spawn(async move {
            let mut req = client
                .post(url)
                .json(&payload)
                .header("x-usami-client", "usami-desktop");

            if let Some(t) = token {
                req = req.header("x-usami-token", t);
            }

            let res = req
                .send()
                .await
                .map_err(|e| format!("Falha de conexão com a API de análise de mídia: {e}"))?;

            let status = res.status();
            if !status.is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!(
                    "Erro HTTP {status} na análise de mídia: {err_text}"
                ));
            }

            let data = res
                .json::<MediaAnalyzeResponse>()
                .await
                .map_err(|e| format!("Resposta inválida da API de mídia: {e}"))?;

            Ok::<MediaAnalyzeResponse, String>(data)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_agent_skills(&self) -> Result<AgentSkillsResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/agent/skills", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req.send().await.map_err(map_request_error)?;
            let status = res.status();
            if !status.is_success() {
                if let Ok(err_json) = res.json::<serde_json::Value>().await {
                    if let Some(msg) = err_json.get("error").and_then(|v| v.as_str()) {
                        return Err(msg.to_string());
                    }
                }
                return Err(format!("Falha ao consultar skills (HTTP {status})."));
            }
            res.json::<AgentSkillsResponse>()
                .await
                .map_err(map_request_error)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_agent_executions(
        &self,
        limit: usize,
    ) -> Result<AgentExecutionsResponse, String> {
        let client = self.client.clone();
        let url = format!("{}/api/agent/executions?limit={limit}", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req.send().await.map_err(map_request_error)?;
            let status = res.status();
            if !status.is_success() {
                if let Ok(err_json) = res.json::<serde_json::Value>().await {
                    if let Some(msg) = err_json.get("error").and_then(|v| v.as_str()) {
                        return Err(msg.to_string());
                    }
                }
                return Err(format!("Falha ao consultar execuções (HTTP {status})."));
            }
            res.json::<AgentExecutionsResponse>()
                .await
                .map_err(map_request_error)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn toggle_skill_enabled(
        &self,
        skill_id: &str,
        enable: bool,
    ) -> Result<SkillToggleResponse, String> {
        let client = self.client.clone();
        let action = if enable { "enable" } else { "disable" };
        let url = format!("{}/api/agent/skills/{skill_id}/{action}", self.base_url);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&serde_json::json!({}));
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req.send().await.map_err(map_request_error)?;
            let status = res.status();
            if !status.is_success() {
                if let Ok(err_json) = res.json::<serde_json::Value>().await {
                    if let Some(msg) = err_json.get("error").and_then(|v| v.as_str()) {
                        return Err(msg.to_string());
                    }
                }
                return Err(format!("Falha ao atualizar skill (HTTP {status})."));
            }
            res.json::<SkillToggleResponse>()
                .await
                .map_err(map_request_error)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    async fn send_agent_get<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: String,
        fallback_err: &'static str,
    ) -> Result<T, String> {
        let client = self.client.clone();
        let url = format!("{}{}", self.base_url, path);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.get(url);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req.send().await.map_err(map_request_error)?;
            let status = res.status();
            if !status.is_success() {
                if let Ok(err_json) = res.json::<serde_json::Value>().await {
                    if let Some(msg) = err_json.get("error").and_then(|v| v.as_str()) {
                        return Err(msg.to_string());
                    }
                }
                return Err(format!("{fallback_err} (HTTP {status})."));
            }
            res.json::<T>().await.map_err(map_request_error)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    async fn send_agent_post<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: String,
        payload: serde_json::Value,
        fallback_err: &'static str,
    ) -> Result<T, String> {
        let client = self.client.clone();
        let url = format!("{}{}", self.base_url, path);
        let token = self.token.clone();

        let handle = tokio_runtime().spawn(async move {
            let mut req = client.post(url).json(&payload);
            if let Some(ref t) = token {
                req = req.header("x-usami-token", t);
            }
            let res = req.send().await.map_err(map_request_error)?;
            let status = res.status();
            if !status.is_success() {
                if let Ok(err_json) = res.json::<serde_json::Value>().await {
                    if let Some(msg) = err_json.get("error").and_then(|v| v.as_str()) {
                        return Err(msg.to_string());
                    }
                }
                return Err(format!("{fallback_err} (HTTP {status})."));
            }
            res.json::<T>().await.map_err(map_request_error)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn get_agent_self_tools(&self) -> Result<AgentSelfResponse, String> {
        self.send_agent_get(
            "/api/agent/self?focus=tools".into(),
            "Falha ao consultar ferramentas do core",
        )
        .await
    }

    pub async fn get_agent_permissions(&self) -> Result<AgentPermissionsResponse, String> {
        self.send_agent_get(
            "/api/agent/permissions".into(),
            "Falha ao consultar permissões",
        )
        .await
    }

    pub async fn set_tool_permission(
        &self,
        tool: &str,
        action: &str,
        reason: Option<&str>,
    ) -> Result<AgentPermissionsResponse, String> {
        let mut payload = serde_json::json!({ "tool": tool, "action": action });
        if let Some(r) = reason.filter(|s| !s.trim().is_empty()) {
            payload["reason"] = serde_json::json!(r.trim());
        }
        self.send_agent_post(
            "/api/agent/permissions".into(),
            payload,
            "Falha ao atualizar permissão",
        )
        .await
    }

    pub async fn get_skill_proposals(&self) -> Result<SkillProposalsResponse, String> {
        self.send_agent_get(
            "/api/agent/skills/proposals".into(),
            "Falha ao consultar propostas de skills",
        )
        .await
    }

    pub async fn create_tool_confirmation(
        &self,
        tool: &str,
    ) -> Result<CreateConfirmationResponse, String> {
        self.send_agent_post(
            "/api/agent/confirmations".into(),
            serde_json::json!({ "tool": tool, "confirm": "CONFIRMAR" }),
            "Falha ao emitir confirmação de uso único",
        )
        .await
    }

    pub async fn create_skill_confirmation(
        &self,
        skill_id: &str,
    ) -> Result<CreateConfirmationResponse, String> {
        self.send_agent_post(
            "/api/agent/confirmations".into(),
            serde_json::json!({ "skill": skill_id, "confirm": "CONFIRMAR" }),
            "Falha ao emitir confirmação para instalar skill",
        )
        .await
    }

    pub async fn install_skill_with_confirmation(
        &self,
        skill_id: &str,
        confirmation_token: &str,
    ) -> Result<SkillInstallResponse, String> {
        self.send_agent_post(
            format!("/api/agent/skills/{skill_id}/install"),
            serde_json::json!({ "confirmation": confirmation_token }),
            "Falha ao instalar skill confirmada",
        )
        .await
    }

    pub async fn get_agent_plans(&self, limit: usize) -> Result<AgentPlansResponse, String> {
        self.send_agent_get(
            format!("/api/agent/plans?limit={limit}"),
            "Falha ao consultar planos",
        )
        .await
    }

    pub async fn get_agent_suggestions(
        &self,
        evaluate: bool,
        limit: usize,
    ) -> Result<AgentSuggestionsResponse, String> {
        let query = if evaluate {
            format!("/api/agent/suggestions?evaluate=1&limit={limit}")
        } else {
            format!("/api/agent/suggestions?limit={limit}")
        };
        self.send_agent_get(query, "Falha ao consultar sugestões")
            .await
    }

    pub async fn accept_agent_suggestion(
        &self,
        suggestion_id: &str,
    ) -> Result<SuggestionActionResponse, String> {
        self.send_agent_post(
            format!("/api/agent/suggestions/{suggestion_id}/accept"),
            serde_json::json!({}),
            "Falha ao aceitar sugestão",
        )
        .await
    }

    pub async fn dismiss_agent_suggestion(
        &self,
        suggestion_id: &str,
        reason: Option<&str>,
    ) -> Result<SuggestionActionResponse, String> {
        let payload = match reason.filter(|s| !s.trim().is_empty()) {
            Some(r) => serde_json::json!({ "reason": r.trim() }),
            None => serde_json::json!({}),
        };
        self.send_agent_post(
            format!("/api/agent/suggestions/{suggestion_id}/dismiss"),
            payload,
            "Falha ao descartar sugestão",
        )
        .await
    }

    pub async fn get_daily_briefing(&self) -> Result<DailyBriefingResponse, String> {
        self.send_agent_get(
            "/api/agent/briefing".into(),
            "Falha ao gerar briefing diário",
        )
        .await
    }
}

/// Resposta do `/api/tts` com `stream:true`: PCM cru quando o provedor entrega
/// em pedaços, WAV/blob inteiro no cache HIT ou no fallback do servidor.
pub enum TtsStream {
    Pcm {
        sample_rate: u32,
        response: reqwest::Response,
    },
    Audio(Vec<u8>),
}

fn is_pcm_stream(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get("x-tts-stream")
        .and_then(|value| value.to_str().ok())
        == Some("1")
}

fn pcm_sample_rate(response: &reqwest::Response) -> u32 {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(';').find_map(|part| {
                part.trim()
                    .strip_prefix("rate=")
                    .and_then(|rate| rate.trim().parse::<u32>().ok())
            })
        })
        .filter(|rate| (8_000..=192_000).contains(rate))
        .unwrap_or(24_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    struct CapturedRequest {
        method: String,
        path: String,
        content_type: Option<String>,
        body: String,
    }

    // Servidor HTTP mínimo: aceita uma requisição, captura método/caminho/corpo
    // e responde o que o teste pedir. Sem dependência nova e sem porta fixa.
    fn spawn_server(
        status: &'static str,
        response_content_type: &'static str,
        extra_headers: &'static str,
        response_body: &'static [u8],
    ) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind do servidor de teste");
        let address = listener.local_addr().expect("endereço local do servidor");
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];
            while !buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            let header_end = buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| position + 4)
                .unwrap_or(buffer.len());
            let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
            let content_length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let mut body = buffer[header_end..].to_vec();
            while body.len() < content_length {
                let read = stream.read(&mut chunk).unwrap_or(0);
                if read == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..read]);
            }
            let request_line = head.lines().next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            let content_type = head.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-type") {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            });
            let _ = sender.send(CapturedRequest {
                method,
                path,
                content_type,
                body: String::from_utf8_lossy(&body).to_string(),
            });
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {response_content_type}\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(response_body);
        });
        (format!("http://{address}"), receiver)
    }

    #[test]
    fn base_url_is_trimmed_and_defaulted() {
        assert_eq!(normalize_base_url(None), DEFAULT_BASE_URL);
        assert_eq!(
            normalize_base_url(Some("   ".to_string())),
            DEFAULT_BASE_URL
        );
        assert_eq!(
            normalize_base_url(Some("http://127.0.0.1:9000/".to_string())),
            "http://127.0.0.1:9000"
        );
    }

    #[tokio::test]
    async fn chat_posts_contract_and_parses_reply() {
        let (base_url, requests) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"reply":"oi","gesture":"tilt","actions":[{"type":"gesture","name":"wave"}],"speechTags":["<laugh>"]}"#,
        );
        let client = UsamiApiClient::new(format!("{base_url}/"));
        let response = client.send_chat("olá", true).await.expect("chat responde");

        assert_eq!(response.reply, "oi");
        assert_eq!(response.gesture.as_deref(), Some("tilt"));
        assert_eq!(response.actions.len(), 1);
        assert_eq!(response.speech_tags, vec!["<laugh>"]);

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/chat");
        assert!(
            request
                .content_type
                .unwrap_or_default()
                .starts_with("application/json")
        );
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("corpo JSON do chat");
        assert_eq!(body["message"], "olá");
        assert_eq!(body["sleepCompanionActive"], true);
        assert_eq!(body["capabilities"]["format"], "VRM");
        assert_eq!(body["capabilities"]["sequences"], true);
        let gestures = body["capabilities"]["gestures"]
            .as_array()
            .expect("gestos anunciados");
        for expected in ["tilt", "sleepy", "clapping", "goodbye"] {
            assert!(
                gestures.iter().any(|gesture| gesture == expected),
                "gesto {expected} ausente do catálogo"
            );
        }
        assert!(
            !gestures.iter().any(|gesture| gesture == "happy"),
            "'happy' é emoção, não clipe"
        );
    }

    #[tokio::test]
    async fn tts_stream_posts_stream_true_and_returns_pcm_chunks() {
        let (base_url, requests) = spawn_server(
            "200 OK",
            "audio/l16; codec=pcm; rate=24000",
            "X-TTS-Stream: 1\r\n",
            b"\x00\x01\x00\x02",
        );
        let client = UsamiApiClient::new(base_url);
        let TtsStream::Pcm {
            sample_rate,
            mut response,
        } = client
            .stream_tts("bom dia", "happy")
            .await
            .expect("tts responde")
        else {
            panic!("esperava PCM em streaming");
        };
        assert_eq!(sample_rate, 24_000);

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.expect("pedaço do stream") {
            body.extend_from_slice(&chunk);
        }
        assert_eq!(body, b"\x00\x01\x00\x02");

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/tts");
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("corpo JSON do tts");
        assert_eq!(body["text"], "bom dia");
        assert_eq!(body["styleId"], "happy");
        assert_eq!(body["stream"], true);
    }

    #[tokio::test]
    async fn tts_without_stream_header_falls_back_to_full_audio() {
        let (base_url, _requests) = spawn_server("200 OK", "audio/wav", "", b"RIFF----WAVE");
        let client = UsamiApiClient::new(base_url);
        let TtsStream::Audio(bytes) = client
            .stream_tts("bom dia", "happy")
            .await
            .expect("tts responde")
        else {
            panic!("cache HIT/devem voltar como áudio inteiro");
        };
        assert_eq!(bytes, b"RIFF----WAVE");
    }

    #[tokio::test]
    async fn stt_posts_multipart_audio_and_returns_text() {
        let (base_url, requests) =
            spawn_server("200 OK", "application/json", "", br#"{"text":"bom dia"}"#);
        let client = UsamiApiClient::new(base_url);
        let text = client
            .transcribe(b"RIFF----WAVE".to_vec())
            .await
            .expect("stt responde");
        assert_eq!(text, "bom dia");

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/stt");
        assert!(
            request
                .content_type
                .unwrap_or_default()
                .starts_with("multipart/form-data")
        );
        assert!(request.body.contains("name=\"audio\""));
        assert!(request.body.contains("filename=\"mic.wav\""));
        assert!(request.body.contains("RIFF"));
    }

    #[tokio::test]
    async fn http_error_status_becomes_err() {
        let (base_url, _requests) = spawn_server(
            "500 Internal Server Error",
            "application/json",
            "",
            br#"{"error":"falhou"}"#,
        );
        let client = UsamiApiClient::new(base_url);
        assert!(client.send_chat("oi", false).await.is_err());
    }

    const BOND_JSON: &str = r#"{
        "ok": true,
        "bond": {
            "intimacy": {"carinho": 41, "humor": 5, "companhia": 10},
            "meters": {
                "carinho": {"xp": 41, "level": 2, "progress": 0.457},
                "humor": {"xp": 5, "level": 1, "progress": 0.2},
                "companhia": {"xp": 10, "level": 1, "progress": 0.4}
            },
            "log": [
                {"kind": "laugh", "text": "Vocês riram juntos", "at": 1794365589048, "count": 2, "gains": {"carinho": 2, "humor": 4}},
                {"kind": "call", "text": "Chamada de voz · 4 min", "at": 1794279021215, "count": 1, "gains": {"companhia": 4}}
            ],
            "updatedAt": "2026-11-11T02:49:55.880Z",
            "summary": {"xp": 41, "level": 2, "name": "Amigos Próximos", "line": "Você me deixa muito feliz! ♡", "next": 60, "remaining": 19, "progress": 0.457, "max": false}
        }
    }"#;

    #[tokio::test]
    async fn bond_get_parses_meters_log_and_summary() {
        let (base_url, requests) =
            spawn_server("200 OK", "application/json", "", BOND_JSON.as_bytes());
        let client = UsamiApiClient::with_token(base_url, Some("token-bond".into()));
        let res = client.get_bond().await.expect("busca vínculo");

        assert!(res.ok);
        let bond = res.bond;
        assert_eq!(bond.intimacy.carinho, 41);
        assert_eq!(bond.meters.carinho.level, 2);
        assert!((bond.meters.carinho.progress - 0.457).abs() < 1e-6);
        assert_eq!(bond.log.len(), 2);
        assert_eq!(bond.log[0].kind, "laugh");
        assert_eq!(bond.log[0].count, 2);
        assert_eq!(bond.summary.level, 2);
        assert_eq!(bond.summary.name, "Amigos Próximos");
        assert_eq!(bond.summary.next, Some(60));
        assert_eq!(bond.summary.remaining, 19);
        assert!(!bond.summary.max);

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/bond");
    }

    #[tokio::test]
    async fn bond_chat_event_posts_content_and_spoken() {
        let (base_url, requests) =
            spawn_server("200 OK", "application/json", "", BOND_JSON.as_bytes());
        let client = UsamiApiClient::new(base_url);
        let res = client
            .award_bond_chat("kkkk amei isso", true)
            .await
            .expect("evento de vínculo");

        assert_eq!(res.bond.summary.xp, 41);
        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/bond/event");
        let body: serde_json::Value = serde_json::from_str(&request.body).expect("corpo json");
        assert_eq!(body["content"], "kkkk amei isso");
        assert_eq!(body["spoken"], true);
        assert!(body.get("kind").is_none(), "chat manda content, não kind");
    }

    #[tokio::test]
    async fn bond_call_event_posts_minutes() {
        let (base_url, requests) =
            spawn_server("200 OK", "application/json", "", BOND_JSON.as_bytes());
        let client = UsamiApiClient::new(base_url);
        client.award_bond_call(4).await.expect("evento de chamada");

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.path, "/api/bond/event");
        let body: serde_json::Value = serde_json::from_str(&request.body).expect("corpo json");
        assert_eq!(body["kind"], "call");
        assert_eq!(body["minutes"], 4);
    }

    #[test]
    fn memory_response_parses_and_counts_durable_facts() {
        let json = r#"{
            "ok": true,
            "memory": {
                "version": 1,
                "sections": {
                    "user": ["Capitão"],
                    "preferences": ["Gosta de chá verde", "Prefere temas escuros"],
                    "hobbies": ["Desenvolvimento de software"],
                    "relationship": ["Amigos próximos"],
                    "dates": ["Aniversário em maio"],
                    "other": []
                },
                "recentFacts": [
                    {"fact": "Trabalhou até tarde ontem", "section": "daily", "expiresAt": null, "expired": false}
                ],
                "summaries": [
                    {"date": "2026-11-08", "summary": "Conversamos sobre Rust e GPUI."}
                ],
                "affinity": {
                    "score": 65,
                    "lastDailyGainDate": "2026-11-08"
                },
                "bond": {
                    "intimacy": {"carinho": 31, "humor": 0, "companhia": 26},
                    "meters": {
                        "carinho": {"xp": 31, "level": 2, "progress": 0.1714},
                        "humor": {"xp": 0, "level": 1, "progress": 0.0},
                        "companhia": {"xp": 26, "level": 2, "progress": 0.0286}
                    },
                    "summary": {"xp": 31, "level": 2, "name": "Amigos Próximos", "line": "", "next": 60, "remaining": 29, "progress": 0.1714, "max": false},
                    "updatedAt": "2026-11-10T20:00:00.000Z"
                }
            }
        }"#;

        let res: MemoryResponse = serde_json::from_str(json).expect("deserializa memory json");
        assert!(res.ok);
        let data = res.memory;
        assert_eq!(data.durable_count(), 6);
        assert_eq!(data.recent_facts.len(), 1);
        assert_eq!(data.summaries.len(), 1);
        assert_eq!(data.affinity.map(|a| a.score), Some(65));
        let bond = data.bond.expect("espelho do vínculo");
        assert_eq!(bond.intimacy.carinho, 31);
        assert_eq!(bond.meters.carinho.level, 2);
        assert_eq!(bond.summary.name, "Amigos Próximos");
    }

    #[test]
    fn personality_response_parses_traits_and_prompt() {
        let json = r#"{
            "ok": true,
            "personality": {
                "traits": {
                    "warmth": 75,
                    "curiosity": 60,
                    "playfulness": 45,
                    "pride": 80
                },
                "customPrompt": "Lembre de me chamar de capitão."
            },
            "traits": [
                {"key": "warmth", "label": "Carinho"}
            ]
        }"#;

        let res: PersonalityResponse =
            serde_json::from_str(json).expect("deserializa personality json");
        assert!(res.ok);
        let p = res.personality;
        assert_eq!(p.traits.warmth, 75);
        assert_eq!(p.traits.curiosity, 60);
        assert_eq!(p.traits.playfulness, 45);
        assert_eq!(p.traits.pride, 80);
        assert_eq!(p.custom_prompt, "Lembre de me chamar de capitão.");
    }

    #[tokio::test]
    async fn api_client_attaches_token_header_when_present() {
        let (base_url, requests) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"personality":{"traits":{"warmth":50,"curiosity":50,"playfulness":50,"pride":50},"customPrompt":""}}"#,
        );
        let client = UsamiApiClient::with_token(base_url, Some("meu-token-secreto".into()));
        assert!(client.has_token());
        assert_eq!(client.token(), Some("meu-token-secreto"));

        let res = client.get_personality().await.expect("busca personalidade");
        assert!(res.ok);

        let request = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("requisição capturada");
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/personality");
    }

    #[test]
    fn proactive_event_deserializes_and_defaults_speak_true() {
        let json = r#"{
            "events": [
                {
                    "id": "evt_abc123",
                    "type": "reminder",
                    "text": "Hora do remédio!",
                    "priority": "high",
                    "expiresAt": "2026-11-09T18:00:00Z"
                }
            ],
            "serverTime": "2026-11-09T14:00:00Z"
        }"#;

        let res: ProactiveEventsResponse =
            serde_json::from_str(json).expect("deserializa proactive json");
        assert_eq!(res.events.len(), 1);
        let ev = &res.events[0];
        assert_eq!(ev.id, "evt_abc123");
        assert_eq!(ev.event_type, "reminder");
        assert_eq!(ev.text, "Hora do remédio!");
        assert_eq!(ev.priority.as_deref(), Some("high"));
        assert_eq!(ev.speak, true);
    }

    #[tokio::test]
    async fn proactive_events_and_ack_round_trip() {
        let (base_url, requests) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"events":[{"id":"evt_1","type":"greeting","text":"Oi capitao!","speak":true}],"serverTime":"2026-11-09T14:00:00Z"}"#,
        );
        let client = UsamiApiClient::with_token(base_url, Some("token-proactive".into()));
        let res = client
            .get_proactive_events("active")
            .await
            .expect("busca eventos");
        assert_eq!(res.events.len(), 1);
        assert_eq!(res.events[0].text, "Oi capitao!");

        let req1 = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("recebe get");
        assert_eq!(req1.method, "GET");
        assert_eq!(req1.path, "/api/proactive/events");
    }

    #[test]
    fn read_token_from_env_file_parses_cleanly() {
        let temp_dir = std::env::temp_dir().join(format!("usami_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join(".env");
        let content = "# comentario\nUSAMI_API_TOKEN=\"segredo-123\"\nOUTRA_VAR=teste\n";
        std::fs::write(&file_path, content).expect("escreve .env temporario");

        let token = read_token_from_env_file(&file_path);
        assert_eq!(token.as_deref(), Some("segredo-123"));

        let _ = std::fs::remove_file(file_path);
        let _ = std::fs::remove_dir(temp_dir);
    }

    #[test]
    fn test_base64_encode_matches_standard() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[tokio::test]
    async fn media_analyze_posts_payload_and_parses_response() {
        let (base_url, requests) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"reply":"Que imagem bonita!","gesture":"nod","actions":[{"type":"gesture","name":"nod"}]}"#,
        );
        let client = UsamiApiClient::with_token(base_url, Some("token-media".into()));
        let res = client
            .analyze_media(
                "image/png",
                "Zm9v",
                Some("foto.png"),
                Some("olha isso"),
                None,
            )
            .await
            .expect("analisa midia");

        assert!(res.ok);
        assert_eq!(res.reply, "Que imagem bonita!");
        assert_eq!(res.gesture.as_deref(), Some("nod"));
        assert_eq!(res.actions.len(), 1);

        let req = requests
            .recv_timeout(Duration::from_secs(5))
            .expect("recebe post");
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/api/media/analyze");
        let body: serde_json::Value = serde_json::from_str(&req.body).expect("corpo json");
        assert_eq!(body["mimeType"], "image/png");
        assert_eq!(body["data"], "Zm9v");
        assert_eq!(body["filename"], "foto.png");
        assert_eq!(body["prompt"], "olha isso");
    }

    #[tokio::test]
    async fn mood_get_and_event_parse_meters_and_timeline() {
        let payload = br#"{
            "ok": true,
            "mood": {
                "id": "shy",
                "label": "Envergonhada",
                "vrmEmotion": "shy",
                "reason": "Ficou corada com o elogio.",
                "meters": { "valence": 72, "energy": 64, "warmth": 78 },
                "expression": { "name": "16-prefab_awkward02", "label": "Constrangida" },
                "timeline": [
                    { "id": "shy", "label": "Envergonhada", "emotion": "shy", "reason": "Ficou corada com o elogio.", "at": 1700000000000 }
                ]
            }
        }"#;
        let (base_url, requests) = spawn_server("200 OK", "application/json", "", payload);
        let client = UsamiApiClient::with_token(base_url, Some("token-mood".into()));

        let res = client.get_mood().await.expect("busca humor");
        assert!(res.ok);
        assert_eq!(res.mood.id, "shy");
        assert_eq!(res.mood.label, "Envergonhada");
        assert_eq!(res.mood.meters.warmth, 78);
        assert_eq!(res.mood.expression.label, "Constrangida");
        assert_eq!(res.mood.timeline.len(), 1);

        let req = requests.recv_timeout(Duration::from_secs(5)).expect("get");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/mood");
    }

    #[tokio::test]
    async fn agent_skills_get_and_toggle_send_token_and_parse_response() {
        let payload = br#"{
            "ok": true,
            "count": 1,
            "skills": [
                {
                    "id": "daily-briefing",
                    "valid": true,
                    "name": "Daily Briefing",
                    "version": "1.0.0",
                    "description": "Resumo do dia",
                    "state": "installed",
                    "origin": "dev",
                    "maxPermissionLevel": 1,
                    "activation": { "canActivate": true, "reasons": [] },
                    "missing": { "tools": [], "dependencies": [] }
                }
            ]
        }"#;
        let (base_url, requests) = spawn_server("200 OK", "application/json", "", payload);
        let client = UsamiApiClient::with_token(base_url, Some("token-skills".into()));

        let res = client.get_agent_skills().await.expect("consulta skills");
        assert!(res.ok);
        assert_eq!(res.skills.len(), 1);
        assert_eq!(res.skills[0].id, "daily-briefing");
        assert_eq!(res.skills[0].state.as_deref(), Some("installed"));
        assert!(res.skills[0].activation.can_activate);

        let req = requests.recv_timeout(Duration::from_secs(5)).expect("get skills");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/agent/skills");

        let (toggle_url, toggle_reqs) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"skill":{"id":"daily-briefing","valid":true,"state":"disabled","activation":{"canActivate":false,"reasons":["skill desativada"]},"missing":{"tools":[],"dependencies":[]}}}"#,
        );
        let toggle_client = UsamiApiClient::with_token(toggle_url, Some("token-skills".into()));
        let toggled = toggle_client
            .toggle_skill_enabled("daily-briefing", false)
            .await
            .expect("desativa skill");
        assert!(toggled.ok);
        assert_eq!(
            toggled.skill.and_then(|s| s.state).as_deref(),
            Some("disabled")
        );

        let treq = toggle_reqs
            .recv_timeout(Duration::from_secs(5))
            .expect("post disable");
        assert_eq!(treq.method, "POST");
        assert_eq!(treq.path, "/api/agent/skills/daily-briefing/disable");
    }

    #[tokio::test]
    async fn agent_executions_get_and_api_error_message_round_trip() {
        let payload = br#"{
            "ok": true,
            "count": 1,
            "executions": [
                {
                    "id": "ex-0123456789ab",
                    "tool": "briefing.build",
                    "status": "failed",
                    "permissionLevel": 1,
                    "origin": "agent",
                    "startedAt": "2026-11-11T22:10:05.000Z",
                    "durationMs": 14,
                    "verified": false,
                    "reason": "resultado reprovado por verify()",
                    "inputSummary": "args: date"
                }
            ]
        }"#;
        let (base_url, requests) = spawn_server("200 OK", "application/json", "", payload);
        let client = UsamiApiClient::with_token(base_url, Some("token-exec".into()));

        let res = client
            .get_agent_executions(50)
            .await
            .expect("consulta execuções");
        assert!(res.ok);
        assert_eq!(res.executions.len(), 1);
        assert_eq!(res.executions[0].tool, "briefing.build");
        assert!(!res.executions[0].verified);
        assert_eq!(
            res.executions[0].reason.as_deref(),
            Some("resultado reprovado por verify()")
        );

        let req = requests.recv_timeout(Duration::from_secs(5)).expect("get execs");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/agent/executions?limit=50");

        let (err_url, _) = spawn_server(
            "409 Conflict",
            "application/json",
            "",
            br#"{"error":"Skill precisa estar instalada."}"#,
        );
        let err_client = UsamiApiClient::with_token(err_url, Some("token-exec".into()));
        let err = err_client
            .toggle_skill_enabled("x-skill", true)
            .await
            .expect_err("retorna mensagem de erro");
        assert_eq!(err, "Skill precisa estar instalada.");
    }

    #[tokio::test]
    async fn agent_permissions_tools_and_revoke_round_trip() {
        let (self_url, self_reqs) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"self":{"tools":[{"name":"briefing.build","version":"1.0.0","description":"Monta briefing","origin":"skill","permissionLevel":1,"active":true,"revoked":false}]}}"#,
        );
        let client = UsamiApiClient::with_token(self_url, Some("tok".into()));
        let tools_res = client.get_agent_self_tools().await.expect("self tools");
        assert_eq!(tools_res.self_data.tools.len(), 1);
        assert_eq!(tools_res.self_data.tools[0].name, "briefing.build");
        assert_eq!(tools_res.self_data.tools[0].permission_level, 1);
        let r = self_reqs.recv_timeout(Duration::from_secs(5)).expect("req");
        assert_eq!(r.path, "/api/agent/self?focus=tools");

        let (perm_url, perm_reqs) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"revocations":[{"tool":"briefing.*","reason":"bloqueada","revokedAt":"2026-11-11T22:00:00.000Z"}]}"#,
        );
        let perm_client = UsamiApiClient::with_token(perm_url, Some("tok".into()));
        let perm_res = perm_client
            .set_tool_permission("briefing.*", "revoke", Some("bloqueada"))
            .await
            .expect("revoke");
        assert_eq!(perm_res.revocations.len(), 1);
        assert_eq!(perm_res.revocations[0].tool, "briefing.*");
        let preq = perm_reqs.recv_timeout(Duration::from_secs(5)).expect("post");
        assert_eq!(preq.method, "POST");
        assert_eq!(preq.path, "/api/agent/permissions");
        let body: serde_json::Value = serde_json::from_str(&preq.body).expect("json");
        assert_eq!(body["tool"], "briefing.*");
        assert_eq!(body["action"], "revoke");
        assert_eq!(body["reason"], "bloqueada");
    }

    #[tokio::test]
    async fn agent_confirmations_suggestions_and_briefing_round_trip() {
        let (conf_url, conf_reqs) = spawn_server(
            "201 Created",
            "application/json",
            "",
            br#"{"ok":true,"confirmation":{"token":"singleuse123","skill":"extra-skill","expiresAt":"2026-11-11T23:00:00.000Z"}}"#,
        );
        let client = UsamiApiClient::with_token(conf_url, Some("tok".into()));
        let conf = client
            .create_skill_confirmation("extra-skill")
            .await
            .expect("confirmacao");
        assert_eq!(conf.confirmation.token, "singleuse123");
        let creq = conf_reqs.recv_timeout(Duration::from_secs(5)).expect("post");
        assert_eq!(creq.path, "/api/agent/confirmations");
        let cbody: serde_json::Value = serde_json::from_str(&creq.body).expect("json");
        assert_eq!(cbody["skill"], "extra-skill");
        assert_eq!(cbody["confirm"], "CONFIRMAR");

        let (sug_url, sug_reqs) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"count":1,"unansweredCount":1,"suggestions":[{"id":"sg-0123456789ab","kind":"overdue_task","title":"Revisar entrega","summary":"Tarefa atrasada","recommendedGoal":"Revisar entrega hoje","status":"suggested","relevance":0.9,"createdAt":"2026-11-11T22:00:00.000Z"}]}"#,
        );
        let sclient = UsamiApiClient::with_token(sug_url, Some("tok".into()));
        let sres = sclient
            .get_agent_suggestions(true, 50)
            .await
            .expect("sugestoes");
        assert_eq!(sres.unanswered_count, 1);
        assert_eq!(sres.suggestions[0].kind, "overdue_task");
        let sreq = sug_reqs.recv_timeout(Duration::from_secs(5)).expect("get");
        assert_eq!(sreq.path, "/api/agent/suggestions?evaluate=1&limit=50");

        let (br_url, br_reqs) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"ok":true,"briefing":{"date":"2026-11-11","empty":true,"text":"Briefing vazio","narrativeSource":"deterministic","counts":{"commitments":0,"priorityTasks":0,"projects":0,"pending":0,"pendingPlans":0,"executions":0}}}"#,
        );
        let bclient = UsamiApiClient::with_token(br_url, Some("tok".into()));
        let bres = bclient.get_daily_briefing().await.expect("briefing");
        assert!(bres.briefing.empty);
        assert_eq!(bres.briefing.date, "2026-11-11");
        let breq = br_reqs.recv_timeout(Duration::from_secs(5)).expect("get");
        assert_eq!(breq.path, "/api/agent/briefing");
    }
}
