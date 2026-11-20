//! Tipos e contratos de dados para o módulo LoL Gamer Companion (Usami.GG).

use serde::{Deserialize, Serialize};

/// Expressões estruturadas retornadas pelo LLM para sincronizar VRM e HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LolExpression {
    Smug,
    Angry,
    Disappointed,
    Surprised,
    #[default]
    #[serde(other)]
    Neutral,
}

impl LolExpression {
    pub fn from_str_loose(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "smug" | "tsundere" | "playful" => Self::Smug,
            "angry" | "very_angry" | "mad" => Self::Angry,
            "disappointed" | "sad" | "sorrow" | "frustrated" => Self::Disappointed,
            "surprised" | "shocked" | "startled" => Self::Surprised,
            _ => Self::Neutral,
        }
    }

    /// Mapeia para os nomes de expressão suportados pelo `gpui-vrm`.
    pub fn to_vrm_emotion(self) -> &'static str {
        match self {
            Self::Smug => "playful",
            Self::Angry => "angry",
            Self::Disappointed => "sad",
            Self::Surprised => "surprised",
            Self::Neutral => "neutral",
        }
    }

    pub fn label_pt(self) -> &'static str {
        match self {
            Self::Smug => "Confiante",
            Self::Angry => "Irritada",
            Self::Disappointed => "Decepcionada",
            Self::Surprised => "Surpresa",
            Self::Neutral => "Analítica",
        }
    }
}

/// Resposta JSON estruturada esperada do LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LolCompanionReply {
    #[serde(alias = "reply", alias = "texto", alias = "message")]
    pub fala: String,
    #[serde(default, alias = "emotion", alias = "gesture")]
    pub expressao: LolExpression,
    #[serde(default = "default_priority", alias = "priority")]
    pub prioridade: u8,
}

fn default_priority() -> u8 {
    3
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlayerRole {
    Top,
    Jungle,
    Mid,
    Adc,
    Support,
    #[default]
    Unknown,
}

impl PlayerRole {
    pub fn label_pt(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Jungle => "Jungle",
            Self::Mid => "Mid",
            Self::Adc => "ADC",
            Self::Support => "Suporte",
            Self::Unknown => "Geral",
        }
    }
}

/// Eventos internos limpos emitidos pelo motor de State Diffing.
#[derive(Debug, Clone, PartialEq)]
pub enum LolEvent {
    MatchLoading {
        champion: Option<String>,
    },
    MatchStarted {
        champion: String,
        role: PlayerRole,
        game_mode: String,
        enemy_laners: Vec<String>,
    },
    UserVoiceQuery {
        query: String,
        champion: String,
        role: PlayerRole,
        game_time: f64,
    },
    MatchEnded {
        champion: String,
        role: PlayerRole,
        duration_secs: f64,
        kills: i32,
        deaths: i32,
        assists: i32,
    },
    PlayerDied {
        champion: String,
        deaths_total: i32,
        killer: Option<String>,
        game_time: f64,
    },
    ChampionKill {
        champion: String,
        kills_total: i32,
        victim: Option<String>,
        game_time: f64,
    },
    FirstBlood {
        killer: String,
        is_player_team: bool,
        game_time: f64,
    },
    DragonKilled {
        dragon_type: String,
        is_player_team: bool,
        game_time: f64,
    },
    BaronKilled {
        is_player_team: bool,
        game_time: f64,
    },
    HeraldKilled {
        is_player_team: bool,
        game_time: f64,
    },
    TurretDestroyed {
        is_player_team: bool,
        game_time: f64,
    },
    ItemPurchased {
        champion: String,
        item_id: i64,
        item_name: String,
        game_time: f64,
    },
    MinionCSThreshold {
        champion: String,
        minute_mark: u32,
        cs: i32,
        target_cs: i32,
    },
    LevelUp {
        champion: String,
        level: i32,
        game_time: f64,
    },
}

impl LolEvent {
    /// Prioridade base (1-5) usada pela fila de controle antes do envio ao LLM.
    pub fn base_priority(&self) -> u8 {
        match self {
            Self::UserVoiceQuery { .. } => 5,
            Self::PlayerDied { .. } | Self::FirstBlood { .. } | Self::BaronKilled { .. } => 5,
            Self::MatchStarted { .. }
            | Self::MatchEnded { .. }
            | Self::DragonKilled { .. }
            | Self::ChampionKill { .. } => 4,
            Self::ItemPurchased { .. }
            | Self::HeraldKilled { .. }
            | Self::TurretDestroyed { .. } => 3,
            Self::MinionCSThreshold { .. } | Self::MatchLoading { .. } => 2,
            Self::LevelUp { .. } => 1,
        }
    }

    /// Categoria de cooldown para evitar spam de eventos repetidos.
    pub fn cooldown_bucket(&self) -> &'static str {
        match self {
            Self::UserVoiceQuery { .. } => "user_voice",
            Self::MatchLoading { .. } => "match_loading",
            Self::MatchStarted { .. } => "match_started",
            Self::MatchEnded { .. } => "match_ended",
            Self::PlayerDied { .. } => "player_died",
            Self::ChampionKill { .. } => "champion_kill",
            Self::FirstBlood { .. } => "first_blood",
            Self::DragonKilled { .. } => "dragon",
            Self::BaronKilled { .. } => "baron",
            Self::HeraldKilled { .. } => "herald",
            Self::TurretDestroyed { .. } => "turret",
            Self::ItemPurchased { .. } => "item",
            Self::MinionCSThreshold { .. } => "cs",
            Self::LevelUp { .. } => "level",
        }
    }
}

// ── Live Client Data API (https://127.0.0.1:2999/liveclientdata/allgamedata) ──

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllGameData {
    #[serde(default)]
    pub active_player: ActivePlayer,
    #[serde(default)]
    pub all_players: Vec<Player>,
    #[serde(default)]
    pub events: EventsWrapper,
    #[serde(default)]
    pub game_data: GameData,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePlayer {
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub riot_id: Option<String>,
    #[serde(default)]
    pub riot_id_game_name: Option<String>,
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub current_gold: f64,
    #[serde(default)]
    pub champion_stats: ChampionStats,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionStats {
    #[serde(default)]
    pub ability_haste: f64,
    #[serde(default)]
    pub ability_power: f64,
    #[serde(default)]
    pub armor: f64,
    #[serde(default)]
    pub attack_damage: f64,
    #[serde(default)]
    pub current_health: f64,
    #[serde(default)]
    pub max_health: f64,
    #[serde(default)]
    pub magic_resist: f64,
    #[serde(default)]
    pub move_speed: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub riot_id: Option<String>,
    #[serde(default)]
    pub riot_id_game_name: Option<String>,
    #[serde(default)]
    pub champion_name: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub position: String,
    #[serde(default)]
    pub level: i32,
    #[serde(default)]
    pub is_dead: bool,
    #[serde(default)]
    pub scores: PlayerScores,
    #[serde(default)]
    pub items: Vec<PlayerItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScores {
    #[serde(default)]
    pub kills: i32,
    #[serde(default)]
    pub deaths: i32,
    #[serde(default)]
    pub assists: i32,
    #[serde(default)]
    pub creep_score: i32,
    #[serde(default)]
    pub ward_score: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerItem {
    #[serde(rename = "itemID", default)]
    pub item_id: i64,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub price: i32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EventsWrapper {
    #[serde(default)]
    pub events: Vec<GameEvent>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GameEvent {
    #[serde(rename = "EventID", default)]
    pub event_id: i64,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_time: f64,
    #[serde(default)]
    pub killer_name: Option<String>,
    #[serde(default)]
    pub victim_name: Option<String>,
    #[serde(default)]
    pub dragon_type: Option<String>,
    #[serde(default)]
    pub turret_killed: Option<String>,
    #[serde(default)]
    pub stolen: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    #[serde(default)]
    pub game_mode: String,
    #[serde(default)]
    pub game_time: f64,
    #[serde(default)]
    pub map_name: String,
}

// ── Riot Games Cloud API (Base para pré-jogo e histórico) ──

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerDto {
    pub id: String,
    pub account_id: String,
    pub puuid: String,
    pub profile_icon_id: i32,
    pub summoner_level: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeagueEntryDto {
    pub queue_type: String,
    pub tier: String,
    pub rank: String,
    pub league_points: i32,
    pub wins: i32,
    pub losses: i32,
}
