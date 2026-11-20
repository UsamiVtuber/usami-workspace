//! Cérebro tático da Usami.GG: fila de prioridade, controle de cooldown anti-flood,
//! construção de prompt tsundere PT-BR e parser de resposta JSON estruturada.

use crate::lol::ddragon::DataDragonStore;
use crate::lol::types::{LolCompanionReply, LolEvent, LolExpression, PlayerRole};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const GLOBAL_SPEECH_COOLDOWN: Duration = Duration::from_secs(8);
const STALE_LOW_PRIORITY_AGE: Duration = Duration::from_secs(12);
const MAX_QUEUE_LEN: usize = 6;

#[derive(Debug, Clone)]
pub struct QueuedLolEvent {
    pub event: LolEvent,
    pub priority: u8,
    pub enqueued_at: Instant,
}

#[derive(Default)]
pub struct EventPriorityQueue {
    items: Vec<QueuedLolEvent>,
    last_global_emit: Option<Instant>,
    bucket_cooldowns: HashMap<&'static str, Instant>,
}

impl EventPriorityQueue {
    pub fn push(&mut self, event: LolEvent, now: Instant) {
        let priority = event.base_priority();
        let bucket = event.cooldown_bucket();

        // Verifica cooldown específico por categoria de evento (voz do usuário tem cooldown zero)
        let min_bucket_cd = bucket_cooldown_duration(bucket);
        if min_bucket_cd > Duration::ZERO
            && let Some(last) = self.bucket_cooldowns.get(bucket)
            && now.duration_since(*last) < min_bucket_cd
        {
            return;
        }

        // Se chegou evento crítico (>=4), descarta eventos antigos de prioridade baixa (<=2)
        if priority >= 4 {
            self.items.retain(|item| {
                item.priority >= 3 || now.duration_since(item.enqueued_at) < STALE_LOW_PRIORITY_AGE
            });
        }

        // Evita duplicar mesmo bucket na fila mantendo a maior prioridade
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|item| item.event.cooldown_bucket() == bucket)
        {
            if priority >= existing.priority {
                existing.event = event;
                existing.priority = priority;
                existing.enqueued_at = now;
            }
            return;
        }

        self.items.push(QueuedLolEvent {
            event,
            priority,
            enqueued_at: now,
        });

        // Ordena por maior prioridade e mais recente primeiro
        self.items.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.enqueued_at.cmp(&a.enqueued_at))
        });
        if self.items.len() > MAX_QUEUE_LEN {
            self.items.truncate(MAX_QUEUE_LEN);
        }
    }

    pub fn pop_ready(&mut self, now: Instant, is_speaking: bool) -> Option<QueuedLolEvent> {
        if is_speaking || self.items.is_empty() {
            return None;
        }

        // Se o próximo item for pergunta direta do usuário (priority >= 5), ele
        // não deve ser bloqueado pelo cooldown global de comentários passivos do jogo!
        let is_user_voice = self.items.first().map(|i| i.priority >= 5).unwrap_or(false);
        if !is_user_voice
            && let Some(last) = self.last_global_emit
            && now.duration_since(last) < GLOBAL_SPEECH_COOLDOWN
        {
            return None;
        }

        // Limpa itens de baixa prioridade que ficaram velhos durante uma teamfight
        self.items.retain(|item| {
            item.priority >= 4 || now.duration_since(item.enqueued_at) < Duration::from_secs(20)
        });

        if self.items.is_empty() {
            return None;
        }

        let item = self.items.remove(0);
        self.last_global_emit = Some(now);
        self.bucket_cooldowns
            .insert(item.event.cooldown_bucket(), now);
        Some(item)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

fn bucket_cooldown_duration(bucket: &str) -> Duration {
    match bucket {
        "user_voice" => Duration::ZERO,
        "cs" => Duration::from_secs(45),
        "item" => Duration::from_secs(20),
        "level" => Duration::from_secs(30),
        "turret" | "champion_kill" => Duration::from_secs(15),
        "player_died" => Duration::from_secs(8),
        _ => Duration::from_secs(5),
    }
}

/// Monta a mensagem contextualizada que será enviada ao core `127.0.0.1:8787` (`/api/chat`).
pub fn build_llm_prompt(
    event: &LolEvent,
    ddragon: &DataDragonStore,
    champion: &str,
    role: PlayerRole,
    enemies: &[String],
    recent_replies: &[String],
) -> String {
    let tactical_ctx = ddragon.build_tactical_context(champion, role, enemies);
    let event_desc = describe_event(event, ddragon);

    let recent_ctx = if recent_replies.is_empty() {
        String::new()
    } else {
        let list = recent_replies
            .iter()
            .map(|r| format!("- \"{r}\""))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\nÚltimas falas recentes que você disse nesta partida (REGRA DE OURO: NÃO repita as mesmas frases, ideias ou piadas!):\n{list}\n"
        )
    };

    format!(
        "[MODO USAMI.GG - COPILOTO DE LEAGUE OF LEGENDS]\n\
        Persona: Você é a Usami, copiloto especialista em LoL com personalidade tsundere em português brasileiro nativo. \
        Alterne entre análise tática afiada e reações irônicas/animadas quando o jogador errar ou fizer uma boa jogada. \
        Seja concisa (1 a 2 frases curtas, ideais para voz em tempo real no meio da partida).\n\n\
        Contexto Técnico:\n{tactical_ctx}\n\
        Evento Atual:\n{event_desc}\n\
        {recent_ctx}\n\
        Instrução Anti-Repetição: NUNCA repita o mesmo conselho ou frase. Seja dinâmica, reativa e reaja ao momento exato.\n\
        RESPONDA EXCLUSIVAMENTE EM JSON VÁLIDO NO FORMATO:\n\
        {{\"fala\": \"texto curto em pt-BR\", \"expressao\": \"smug|angry|disappointed|surprised|neutral\", \"prioridade\": 1}}"
    )
}

fn describe_event(event: &LolEvent, ddragon: &DataDragonStore) -> String {
    match event {
        LolEvent::UserVoiceQuery {
            query,
            champion,
            role,
            game_time,
        } => format!(
            "O jogador falou diretamente com você pelo microfone aos {:.1} min: \"{query}\". \
            Responda diretamente à pergunta ou comentário dele com base na partida atual ({champion} na posição {}).",
            game_time / 60.0,
            role.label_pt()
        ),
        LolEvent::MatchLoading { champion } => format!(
            "Tela de carregamento iniciada. Campeão detectado: {}. Dê uma dica rápida pré-jogo.",
            champion.as_deref().unwrap_or("ainda carregando")
        ),
        LolEvent::MatchStarted {
            champion,
            role,
            game_mode,
            enemy_laners,
        } => format!(
            "Partida começou ({game_mode}) com {champion} na posição {}. Inimigos: {}.",
            role.label_pt(),
            enemy_laners.join(", ")
        ),
        LolEvent::MatchEnded {
            champion,
            role,
            duration_secs,
            kills,
            deaths,
            assists,
        } => format!(
            "Partida encerrada com {champion} ({}) após {:.0} min. Placar final: {kills}/{deaths}/{assists}.",
            role.label_pt(),
            duration_secs / 60.0
        ),
        LolEvent::PlayerDied {
            champion,
            deaths_total,
            killer,
            game_time,
        } => format!(
            "O jogador morreu de {champion} aos {:.1} min (total de mortes: {deaths_total}). Abatido por: {}.",
            game_time / 60.0,
            killer.as_deref().unwrap_or("inimigo")
        ),
        LolEvent::ChampionKill {
            champion,
            kills_total,
            victim,
            game_time,
        } => format!(
            "O jogador conseguiu um abate de {champion} aos {:.1} min (total kills: {kills_total}) em {}.",
            game_time / 60.0,
            victim.as_deref().unwrap_or("inimigo")
        ),
        LolEvent::FirstBlood {
            killer,
            is_player_team,
            game_time,
        } => format!(
            "First Blood aos {:.1} min por {killer} ({}).",
            game_time / 60.0,
            if *is_player_team {
                "time aliado"
            } else {
                "time inimigo"
            }
        ),
        LolEvent::DragonKilled {
            dragon_type,
            is_player_team,
            game_time,
        } => format!(
            "Dragão ({dragon_type}) abatido aos {:.1} min pelo {}.",
            game_time / 60.0,
            if *is_player_team {
                "time aliado"
            } else {
                "time inimigo"
            }
        ),
        LolEvent::BaronKilled {
            is_player_team,
            game_time,
        } => format!(
            "Barão Na'Shor abatido aos {:.1} min pelo {}.",
            game_time / 60.0,
            if *is_player_team {
                "time aliado"
            } else {
                "time inimigo"
            }
        ),
        LolEvent::HeraldKilled {
            is_player_team,
            game_time,
        } => format!(
            "Arauto do Vale abatido aos {:.1} min pelo {}.",
            game_time / 60.0,
            if *is_player_team {
                "time aliado"
            } else {
                "time inimigo"
            }
        ),
        LolEvent::TurretDestroyed {
            is_player_team,
            game_time,
        } => format!(
            "Torre destruída aos {:.1} min ({}).",
            game_time / 60.0,
            if *is_player_team {
                "a favor"
            } else {
                "perdida"
            }
        ),
        LolEvent::ItemPurchased {
            champion,
            item_id,
            item_name,
            game_time,
        } => {
            let resolved = ddragon
                .lookup_item(*item_id)
                .map(|i| format!("{} ({})", i.name, i.plaintext))
                .unwrap_or_else(|| item_name.clone());
            format!(
                "{champion} comprou o item {resolved} aos {:.1} min.",
                game_time / 60.0
            )
        }
        LolEvent::MinionCSThreshold {
            champion,
            minute_mark,
            cs,
            target_cs,
        } => format!(
            "Marco de {minute_mark} min com {champion}: {cs} CS (meta ideal ~{target_cs} CS)."
        ),
        LolEvent::LevelUp {
            champion,
            level,
            game_time,
        } => format!(
            "{champion} atingiu nível {level} (power spike de ultimate) aos {:.1} min.",
            game_time / 60.0
        ),
    }
}

fn is_operational_or_system_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("code.propose")
        || lower.contains("memory.search")
        || lower.contains("excedia 500")
        || lower.contains("tentei usar a ferramenta")
        || lower.contains("não consegui executar")
        || lower.contains("falhou ao executar")
        || lower.contains("validation: function")
        || lower.contains("nvidia api http")
        || lower.contains("bad request")
}

/// Faz o parse da resposta do LLM aceitando JSON puro, bloco ```json ou texto livre com fallback.
pub fn parse_companion_reply(
    raw: &str,
    fallback_priority: u8,
    fallback_expr: LolExpression,
) -> LolCompanionReply {
    let trimmed = raw.trim();
    if is_operational_or_system_error(trimmed) {
        return LolCompanionReply {
            fala: String::new(),
            expressao: fallback_expr,
            prioridade: 1,
        };
    }

    let candidate = extract_json_slice(trimmed).unwrap_or(trimmed);
    if is_operational_or_system_error(candidate) {
        return LolCompanionReply {
            fala: String::new(),
            expressao: fallback_expr,
            prioridade: 1,
        };
    }

    if let Ok(mut parsed) = serde_json::from_str::<LolCompanionReply>(candidate) {
        parsed.prioridade = parsed.prioridade.clamp(1, 5);
        if !parsed.fala.trim().is_empty() && !is_operational_or_system_error(&parsed.fala) {
            return parsed;
        }
    }

    // Fallback seguro se o LLM retornar texto puro sem envelope JSON
    let clean_text = trimmed.trim_matches('`').trim().to_string();
    LolCompanionReply {
        fala: if clean_text.is_empty() || is_operational_or_system_error(&clean_text) {
            String::new()
        } else {
            clean_text
        },
        expressao: fallback_expr,
        prioridade: fallback_priority.clamp(1, 5),
    }
}

fn extract_json_slice(input: &str) -> Option<&str> {
    let start = input.find('{')?;
    let end = input.rfind('}')?;
    if end > start {
        Some(&input[start..=end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_queue_prefers_higher_priority_and_respects_cooldown() {
        let mut q = EventPriorityQueue::default();
        let t0 = Instant::now();

        q.push(
            LolEvent::LevelUp {
                champion: "Sett".into(),
                level: 6,
                game_time: 360.0,
            },
            t0,
        );
        q.push(
            LolEvent::PlayerDied {
                champion: "Sett".into(),
                deaths_total: 1,
                killer: Some("Darius".into()),
                game_time: 362.0,
            },
            t0,
        );

        let first = q.pop_ready(t0, false).expect("deve retornar PlayerDied");
        assert!(matches!(first.event, LolEvent::PlayerDied { .. }));

        // Cooldown global ativo logo após
        assert!(q.pop_ready(t0 + Duration::from_secs(2), false).is_none());

        // Após cooldown global de 8s, libera o próximo
        let second = q.pop_ready(t0 + Duration::from_secs(9), false);
        assert!(second.is_some());
    }

    #[test]
    fn parses_json_inside_markdown_fences() {
        let raw = "```json\n{\"fala\": \"Olha o mapa!\", \"expressao\": \"angry\", \"prioridade\": 5}\n```";
        let reply = parse_companion_reply(raw, 3, LolExpression::Neutral);
        assert_eq!(reply.fala, "Olha o mapa!");
        assert_eq!(reply.expressao, LolExpression::Angry);
        assert_eq!(reply.prioridade, 5);
    }

    #[test]
    fn parses_reply_alias_from_core() {
        let raw = "{\"reply\": \"Faz Rylai logo!\", \"gesture\": \"nod\"}";
        let reply = parse_companion_reply(raw, 3, LolExpression::Neutral);
        assert_eq!(reply.fala, "Faz Rylai logo!");
    }

    #[test]
    fn user_voice_bypasses_global_speech_cooldown() {
        let mut q = EventPriorityQueue::default();
        let t0 = Instant::now();

        q.push(
            LolEvent::LevelUp {
                champion: "Malzahar".into(),
                level: 6,
                game_time: 360.0,
            },
            t0,
        );
        let first = q.pop_ready(t0, false).expect("deve retornar LevelUp");
        assert!(matches!(first.event, LolEvent::LevelUp { .. }));

        // 2 segundos depois o jogador pergunta algo
        q.push(
            LolEvent::UserVoiceQuery {
                query: "o que eu compro?".into(),
                champion: "Malzahar".into(),
                role: PlayerRole::Support,
                game_time: 362.0,
            },
            t0 + Duration::from_secs(2),
        );

        // Deve retornar imediatamente sem esperar 8 segundos!
        let voice = q
            .pop_ready(t0 + Duration::from_secs(2), false)
            .expect("pergunta do usuário não deve esperar cooldown");
        assert!(matches!(voice.event, LolEvent::UserVoiceQuery { .. }));
    }
}
