//! Motor de State Diffing: compara o snapshot anterior com o payload atual
//! da Live Client Data API para emitir eventos internos limpos sem floodar o LLM.

use crate::lol::types::{AllGameData, LolEvent, Player, PlayerRole};
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct GameSnapshot {
    pub initialized: bool,
    pub champion: String,
    pub role: PlayerRole,
    pub player_team: String,
    pub level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub creep_score: i32,
    pub owned_items: HashSet<i64>,
    pub seen_event_ids: HashSet<i64>,
    pub last_cs_minute_mark: u32,
}

impl GameSnapshot {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Compara `data` contra o estado anterior e retorna apenas eventos novos e relevantes.
    pub fn diff(&mut self, data: &AllGameData) -> Vec<LolEvent> {
        let mut events = Vec::new();
        let game_time = data.game_data.game_time;

        let Some(me) = resolve_active_player(data) else {
            return events;
        };

        let my_name_candidates = player_identifiers(me);
        let my_team = me.team.clone();
        let champion = if me.champion_name.is_empty() {
            "Desconhecido".to_string()
        } else {
            me.champion_name.clone()
        };

        let detected_role = detect_player_role(me);

        if !self.initialized {
            self.initialized = true;
            self.champion = champion.clone();
            self.role = detected_role;
            self.player_team = my_team.clone();
            self.level = me.level.max(data.active_player.level);
            self.kills = me.scores.kills;
            self.deaths = me.scores.deaths;
            self.assists = me.scores.assists;
            self.creep_score = me.scores.creep_score;
            self.owned_items = me.items.iter().map(|i| i.item_id).collect();
            self.seen_event_ids = data.events.events.iter().map(|e| e.event_id).collect();
            self.last_cs_minute_mark = (game_time / 300.0).floor() as u32 * 5;

            let enemy_laners = data
                .all_players
                .iter()
                .filter(|p| !p.team.is_empty() && p.team != my_team)
                .map(|p| p.champion_name.clone())
                .filter(|n| !n.is_empty())
                .collect();

            events.push(LolEvent::MatchStarted {
                champion,
                role: self.role,
                game_mode: data.game_data.game_mode.clone(),
                enemy_laners,
            });
            return events;
        }

        // Atualiza a role se o jogador comprou item de suporte/jungle no primeiro recall
        if self.role == PlayerRole::Unknown && detected_role != PlayerRole::Unknown {
            self.role = detected_role;
        } else if detected_role == PlayerRole::Support && self.role != PlayerRole::Support {
            self.role = PlayerRole::Support;
        }

        // 1. Eventos da timeline oficial (`events.Events`)
        let ally_names: HashSet<String> = data
            .all_players
            .iter()
            .filter(|p| p.team == self.player_team)
            .flat_map(player_identifiers)
            .collect();

        for ev in &data.events.events {
            if !self.seen_event_ids.insert(ev.event_id) {
                continue;
            }
            match ev.event_name.as_str() {
                "FirstBlood" => {
                    let killer = ev.killer_name.clone().unwrap_or_default();
                    let is_player_team = ally_names.contains(&killer);
                    events.push(LolEvent::FirstBlood {
                        killer,
                        is_player_team,
                        game_time: ev.event_time,
                    });
                }
                "DragonKill" => {
                    let killer = ev.killer_name.as_deref().unwrap_or("");
                    let is_player_team = ally_names.contains(killer);
                    events.push(LolEvent::DragonKilled {
                        dragon_type: ev
                            .dragon_type
                            .clone()
                            .unwrap_or_else(|| "Elemental".to_string()),
                        is_player_team,
                        game_time: ev.event_time,
                    });
                }
                "BaronKill" => {
                    let killer = ev.killer_name.as_deref().unwrap_or("");
                    let is_player_team = ally_names.contains(killer);
                    events.push(LolEvent::BaronKilled {
                        is_player_team,
                        game_time: ev.event_time,
                    });
                }
                "HeraldKill" => {
                    let killer = ev.killer_name.as_deref().unwrap_or("");
                    let is_player_team = ally_names.contains(killer);
                    events.push(LolEvent::HeraldKilled {
                        is_player_team,
                        game_time: ev.event_time,
                    });
                }
                "TurretKilled" => {
                    let killer = ev.killer_name.as_deref().unwrap_or("");
                    let is_player_team = ally_names.contains(killer);
                    events.push(LolEvent::TurretDestroyed {
                        is_player_team,
                        game_time: ev.event_time,
                    });
                }
                "ChampionKill" => {
                    let killer = ev.killer_name.clone().unwrap_or_default();
                    let victim = ev.victim_name.clone().unwrap_or_default();
                    if my_name_candidates.contains(&victim) {
                        events.push(LolEvent::PlayerDied {
                            champion: champion.clone(),
                            deaths_total: me.scores.deaths.max(self.deaths + 1),
                            killer: if killer.is_empty() {
                                None
                            } else {
                                Some(killer)
                            },
                            game_time: ev.event_time,
                        });
                    } else if my_name_candidates.contains(&killer) {
                        events.push(LolEvent::ChampionKill {
                            champion: champion.clone(),
                            kills_total: me.scores.kills.max(self.kills + 1),
                            victim: if victim.is_empty() {
                                None
                            } else {
                                Some(victim)
                            },
                            game_time: ev.event_time,
                        });
                    }
                }
                _ => {}
            }
        }

        // Fallback caso `deaths` suba sem evento ChampionKill correspondente
        if me.scores.deaths > self.deaths
            && !events
                .iter()
                .any(|e| matches!(e, LolEvent::PlayerDied { .. }))
        {
            events.push(LolEvent::PlayerDied {
                champion: champion.clone(),
                deaths_total: me.scores.deaths,
                killer: None,
                game_time,
            });
        }

        // 2. Compras de itens relevantes (ignora trinkets/consumíveis baratos < 400g)
        let current_items: HashSet<i64> = me.items.iter().map(|i| i.item_id).collect();
        for item in &me.items {
            if !self.owned_items.contains(&item.item_id)
                && is_notable_item(item.item_id, item.price)
            {
                events.push(LolEvent::ItemPurchased {
                    champion: champion.clone(),
                    item_id: item.item_id,
                    item_name: item.display_name.clone(),
                    game_time,
                });
            }
        }
        self.owned_items = current_items;

        // 3. Marcos de CS a cada 5 minutos (Apenas para laners/carries; NUNCA para Suporte!)
        if self.role != PlayerRole::Support {
            let current_mark = (game_time / 300.0).floor() as u32 * 5;
            if current_mark >= 5 && current_mark > self.last_cs_minute_mark {
                self.last_cs_minute_mark = current_mark;
                let target_cs = (current_mark as i32) * 8;
                events.push(LolEvent::MinionCSThreshold {
                    champion: champion.clone(),
                    minute_mark: current_mark,
                    cs: me.scores.creep_score,
                    target_cs,
                });
            }
        }

        // 4. Marcos de Level (6, 11, 16 - power spikes de ultimate)
        let new_level = me.level.max(data.active_player.level);
        if new_level > self.level && matches!(new_level, 6 | 11 | 16) {
            events.push(LolEvent::LevelUp {
                champion,
                level: new_level,
                game_time,
            });
        }

        self.level = new_level;
        self.kills = me.scores.kills;
        self.deaths = me.scores.deaths;
        self.assists = me.scores.assists;
        self.creep_score = me.scores.creep_score;

        events
    }
}

pub fn detect_player_role(player: &Player) -> PlayerRole {
    // 1. Verificação por itens de suporte da season 14 (World Atlas / Atlas Mundial e evoluções)
    let has_support_item = player.items.iter().any(|i| {
        matches!(
            i.item_id,
            3865 | 3866 | 3867 | 3869 | 3870 | 3871 | 3876 | 3877
        )
    });
    if has_support_item {
        return PlayerRole::Support;
    }

    // 2. Verificação por itens de selva
    let has_jungle_item = player
        .items
        .iter()
        .any(|i| matches!(i.item_id, 1101 | 1102 | 1103));
    if has_jungle_item {
        return PlayerRole::Jungle;
    }

    // 3. Posição declarada pela Riot API
    match player.position.trim().to_ascii_uppercase().as_str() {
        "UTILITY" | "SUPPORT" => PlayerRole::Support,
        "MIDDLE" | "MID" => PlayerRole::Mid,
        "TOP" => PlayerRole::Top,
        "JUNGLE" => PlayerRole::Jungle,
        "BOTTOM" | "BOT" | "ADC" => PlayerRole::Adc,
        _ => PlayerRole::Unknown,
    }
}

fn is_notable_item(item_id: i64, price: i32) -> bool {
    // Trinkets (3340 Stealth Ward, 3363 Farsight, 3364 Oracle) e poções/botas iniciais
    if matches!(item_id, 2003 | 2031 | 2055 | 3340 | 3363 | 3364) {
        return false;
    }
    price >= 800 || price == 0
}

fn player_identifiers(player: &Player) -> HashSet<String> {
    let mut ids = HashSet::new();
    if !player.summoner_name.is_empty() {
        ids.insert(player.summoner_name.clone());
    }
    if let Some(ref riot_id) = player.riot_id {
        if !riot_id.is_empty() {
            ids.insert(riot_id.clone());
        }
    }
    if let Some(ref game_name) = player.riot_id_game_name {
        if !game_name.is_empty() {
            ids.insert(game_name.clone());
        }
    }
    ids
}

pub fn resolve_active_player(data: &AllGameData) -> Option<&Player> {
    let active_summoner = data.active_player.summoner_name.as_str();
    let active_riot_id = data.active_player.riot_id.as_deref().unwrap_or("");
    let active_game_name = data
        .active_player
        .riot_id_game_name
        .as_deref()
        .unwrap_or("");

    data.all_players
        .iter()
        .find(|p| {
            (!active_summoner.is_empty() && p.summoner_name == active_summoner)
                || (!active_riot_id.is_empty() && p.riot_id.as_deref() == Some(active_riot_id))
                || (!active_game_name.is_empty()
                    && (p.riot_id_game_name.as_deref() == Some(active_game_name)
                        || p.summoner_name == active_game_name))
        })
        .or_else(|| data.all_players.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lol::types::{
        ActivePlayer, EventsWrapper, GameData, GameEvent, PlayerItem, PlayerScores,
    };

    fn sample_game_data(game_time: f64) -> AllGameData {
        AllGameData {
            active_player: ActivePlayer {
                summoner_name: "UsamiPlayer".into(),
                riot_id: Some("UsamiPlayer#BR1".into()),
                riot_id_game_name: Some("UsamiPlayer".into()),
                level: 1,
                ..Default::default()
            },
            all_players: vec![
                Player {
                    summoner_name: "UsamiPlayer".into(),
                    riot_id: Some("UsamiPlayer#BR1".into()),
                    riot_id_game_name: Some("UsamiPlayer".into()),
                    champion_name: "Sett".into(),
                    team: "ORDER".into(),
                    position: "TOP".into(),
                    level: 1,
                    scores: PlayerScores {
                        kills: 0,
                        deaths: 0,
                        assists: 0,
                        creep_score: 0,
                        ward_score: 0.0,
                    },
                    items: vec![],
                    ..Default::default()
                },
                Player {
                    summoner_name: "EnemyTop".into(),
                    champion_name: "Darius".into(),
                    team: "CHAOS".into(),
                    ..Default::default()
                },
            ],
            events: EventsWrapper::default(),
            game_data: GameData {
                game_mode: "CLASSIC".into(),
                game_time,
                map_name: "Map11".into(),
            },
        }
    }

    #[test]
    fn emits_match_started_then_death_item_and_cs_threshold() {
        let mut snap = GameSnapshot::default();

        // Primeiro tick -> MatchStarted
        let d0 = sample_game_data(12.0);
        let ev0 = snap.diff(&d0);
        assert!(
            matches!(ev0.as_slice(), [LolEvent::MatchStarted { champion, .. }] if champion == "Sett")
        );

        // Segundo tick -> PlayerDied + ItemPurchased + CS 5 min
        let mut d1 = sample_game_data(305.0);
        d1.all_players[0].scores.deaths = 1;
        d1.all_players[0].scores.creep_score = 38;
        d1.all_players[0].items.push(PlayerItem {
            item_id: 6631,
            display_name: "Stridebreaker".into(),
            count: 1,
            price: 3300,
        });
        d1.events.events.push(GameEvent {
            event_id: 10,
            event_name: "ChampionKill".into(),
            event_time: 298.0,
            killer_name: Some("EnemyTop".into()),
            victim_name: Some("UsamiPlayer".into()),
            ..Default::default()
        });

        let ev1 = snap.diff(&d1);
        assert!(ev1.iter().any(|e| matches!(
            e,
            LolEvent::PlayerDied {
                deaths_total: 1,
                ..
            }
        )));
        assert!(
            ev1.iter()
                .any(|e| matches!(e, LolEvent::ItemPurchased { item_id: 6631, .. }))
        );
        assert!(ev1.iter().any(|e| matches!(
            e,
            LolEvent::MinionCSThreshold {
                minute_mark: 5,
                cs: 38,
                ..
            }
        )));

        // Terceiro tick idêntico não deve reemitir nada
        let ev2 = snap.diff(&d1);
        assert!(ev2.is_empty());
    }

    #[test]
    fn support_role_never_emits_cs_threshold() {
        let mut snap = GameSnapshot::default();
        let mut d0 = sample_game_data(10.0);
        d0.all_players[0].position = "UTILITY".into();
        snap.diff(&d0);

        let mut d1 = sample_game_data(310.0);
        d1.all_players[0].position = "UTILITY".into();
        d1.all_players[0].scores.creep_score = 12; // Pouco CS de suporte
        let ev1 = snap.diff(&d1);
        assert!(
            !ev1.iter()
                .any(|e| matches!(e, LolEvent::MinionCSThreshold { .. }))
        );
    }
}
