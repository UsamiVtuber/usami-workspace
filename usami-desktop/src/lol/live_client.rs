//! Cliente HTTP assíncrono passivo para a Live Client Data API da Riot (porta 2999)
//! com máquina de estados explícita (GameClosed -> LoadingScreen -> InMatch -> MatchEnded).

use crate::lol::provider::GameProvider;
use crate::lol::state_diff::GameSnapshot;
use crate::lol::types::AllGameData;
use reqwest::StatusCode;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

pub const LIVE_CLIENT_URL: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(400);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(900);
const MIN_BACKOFF: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(10);
const LOADING_INTERVAL: Duration = Duration::from_millis(1500);
const IN_MATCH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub enum PollerState {
    GameClosed {
        backoff: Duration,
    },
    LoadingScreen {
        started_at: Instant,
    },
    InMatch {
        champion_name: String,
        last_game_time: f64,
        snapshot: GameSnapshot,
    },
    MatchEnded,
}

impl Default for PollerState {
    fn default() -> Self {
        Self::GameClosed {
            backoff: MIN_BACKOFF,
        }
    }
}

impl PollerState {
    pub fn next_poll_delay(&self) -> Duration {
        match self {
            Self::GameClosed { backoff } => *backoff,
            Self::LoadingScreen { .. } => LOADING_INTERVAL,
            Self::InMatch { .. } => IN_MATCH_INTERVAL,
            Self::MatchEnded => Duration::from_millis(250),
        }
    }

    pub fn is_in_match(&self) -> bool {
        matches!(self, Self::InMatch { .. })
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::LoadingScreen { .. })
    }
}

#[derive(Debug)]
pub enum ProbeResult {
    /// Porta 2999 fechada / timeout / conexão recusada.
    Closed,
    /// Porta 2999 respondeu 404 ou payload ainda sem partida iniciada (`gameTime <= 0`).
    Loading,
    /// Partida ativa com payload completo.
    Active(Box<AllGameData>),
}

pub struct LiveClientPoller {
    client: reqwest::Client,
    url: String,
}

impl LiveClientPoller {
    /// Cria o cliente HTTP isolado exclusivamente para o certificado autoassinado
    /// local da Riot Games na porta 2999.
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            client,
            url: LIVE_CLIENT_URL.to_string(),
        })
    }

    pub async fn probe(&self) -> ProbeResult {
        let response = match self.client.get(&self.url).send().await {
            Ok(res) => res,
            Err(_) => return ProbeResult::Closed,
        };

        if response.status() == StatusCode::NOT_FOUND {
            return ProbeResult::Loading;
        }
        if !response.status().is_success() {
            return ProbeResult::Loading;
        }

        match response.json::<AllGameData>().await {
            Ok(data) if data.game_data.game_time > 0.0 && !data.all_players.is_empty() => {
                ProbeResult::Active(Box::new(data))
            }
            Ok(_) => ProbeResult::Loading,
            Err(_) => ProbeResult::Loading,
        }
    }
}

impl GameProvider for LiveClientPoller {
    fn name(&self) -> &'static str {
        "league-of-legends"
    }

    fn poll(&self) -> Pin<Box<dyn Future<Output = ProbeResult> + Send + '_>> {
        Box::pin(async { self.probe().await })
    }
}

/// Função pura de transição da máquina de estados para facilitar testes determinísticos.
pub fn advance_state(state: PollerState, probe: ProbeResult) -> (PollerState, Option<AllGameData>) {
    match (state, probe) {
        (PollerState::GameClosed { backoff }, ProbeResult::Closed) => {
            let next_backoff = (backoff * 2).min(MAX_BACKOFF);
            (
                PollerState::GameClosed {
                    backoff: next_backoff,
                },
                None,
            )
        }
        (PollerState::GameClosed { .. }, ProbeResult::Loading) => (
            PollerState::LoadingScreen {
                started_at: Instant::now(),
            },
            None,
        ),
        (PollerState::GameClosed { .. }, ProbeResult::Active(data)) => {
            let champion_name = crate::lol::state_diff::resolve_active_player(&data)
                .map(|p| p.champion_name.clone())
                .unwrap_or_default();
            let last_game_time = data.game_data.game_time;
            (
                PollerState::InMatch {
                    champion_name,
                    last_game_time,
                    snapshot: GameSnapshot::default(),
                },
                Some(*data),
            )
        }
        (PollerState::LoadingScreen { .. }, ProbeResult::Closed) => (
            PollerState::GameClosed {
                backoff: MIN_BACKOFF,
            },
            None,
        ),
        (PollerState::LoadingScreen { started_at }, ProbeResult::Loading) => {
            (PollerState::LoadingScreen { started_at }, None)
        }
        (PollerState::LoadingScreen { .. }, ProbeResult::Active(data)) => {
            let champion_name = crate::lol::state_diff::resolve_active_player(&data)
                .map(|p| p.champion_name.clone())
                .unwrap_or_default();
            let last_game_time = data.game_data.game_time;
            (
                PollerState::InMatch {
                    champion_name,
                    last_game_time,
                    snapshot: GameSnapshot::default(),
                },
                Some(*data),
            )
        }
        (PollerState::InMatch { .. }, ProbeResult::Closed) => (PollerState::MatchEnded, None),
        (
            PollerState::InMatch {
                champion_name,
                last_game_time,
                snapshot,
            },
            ProbeResult::Loading,
        ) => {
            // Se a API voltar a 404 após ter estado em partida, considera encerramento.
            let _ = (champion_name, last_game_time, snapshot);
            (PollerState::MatchEnded, None)
        }
        (
            PollerState::InMatch {
                mut champion_name,
                last_game_time: _,
                snapshot,
            },
            ProbeResult::Active(data),
        ) => {
            let game_ended_event = data.events.events.iter().any(|e| e.event_name == "GameEnd");
            if game_ended_event {
                return (PollerState::MatchEnded, Some(*data));
            }
            if champion_name.is_empty()
                && let Some(p) = crate::lol::state_diff::resolve_active_player(&data)
            {
                champion_name = p.champion_name.clone();
            }
            let last_game_time = data.game_data.game_time;
            (
                PollerState::InMatch {
                    champion_name,
                    last_game_time,
                    snapshot,
                },
                Some(*data),
            )
        }
        (PollerState::MatchEnded, _) => (
            PollerState::GameClosed {
                backoff: MIN_BACKOFF,
            },
            None,
        ),
    }
}

/// Circuit breaker para o poller da Live Client API.
///
/// Depois de `FAILURE_THRESHOLD` falhas consecutivas na porta 2999, o
/// circuito abre: `should_poll` retorna falso até `retry_after` expirar,
/// evitando que o worker bata numa API morta. Qualquer semente de sucesso
/// (`Loading` ou `Active`) fecha o circuito e reseta o contador.
pub struct CircuitBreaker {
    failures: u32,
    retry_after: Option<Instant>,
}

const FAILURE_THRESHOLD: u32 = 5;
const INITIAL_BACKOFF: Duration = Duration::from_secs(10);

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            failures: 0,
            retry_after: None,
        }
    }

    /// Retorna falso se o circuito estiver aberto e o tempo de retry nao
    /// expirou. Em caso afirmativo, o caller deve pular o poll deste ciclo.
    pub fn should_poll(&self, now: Instant) -> bool {
        !matches!(self.retry_after, Some(deadline) if now < deadline)
    }

    /// Registra uma falha na sonda. Abre o circuito quando o limite e
    /// atingido, com backoff exponencial.
    pub fn record_failure(&mut self, now: Instant) {
        self.failures += 1;
        if self.failures >= FAILURE_THRESHOLD {
            // Expoente limitado: 2u32.pow(32) estoura em debug e o backoff já
            // é limitado por MAX_BACKOFF de qualquer forma.
            let exponent = self.failures.saturating_sub(FAILURE_THRESHOLD).min(5);
            let backoff = (INITIAL_BACKOFF * 2u32.pow(exponent)).min(MAX_BACKOFF);
            self.retry_after = Some(now + backoff);
            eprintln!(
                "[Usami.GG] Circuit breaker aberto apos {} falhas; retry em {:?}",
                self.failures, backoff
            );
        }
    }

    /// Registra sucesso na sonda: fecha o circuito e reseta o contador.
    pub fn record_success(&mut self) {
        self.failures = 0;
        self.retry_after = None;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lol::types::{GameData, Player};

    fn active_payload() -> AllGameData {
        AllGameData {
            all_players: vec![Player {
                champion_name: "Malzahar".into(),
                ..Default::default()
            }],
            game_data: GameData {
                game_time: 42.0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn state_machine_transitions_cleanly() {
        let s0 = PollerState::default();
        let (s1, _) = advance_state(s0, ProbeResult::Closed);
        assert!(
            matches!(s1, PollerState::GameClosed { backoff } if backoff == Duration::from_secs(4))
        );

        let (s2, _) = advance_state(s1, ProbeResult::Loading);
        assert!(s2.is_loading());

        let (s3, data) = advance_state(s2, ProbeResult::Active(Box::new(active_payload())));
        assert!(s3.is_in_match());
        assert!(data.is_some());

        let (s4, _) = advance_state(s3, ProbeResult::Closed);
        assert!(matches!(s4, PollerState::MatchEnded));

        let (s5, _) = advance_state(s4, ProbeResult::Closed);
        assert!(matches!(s5, PollerState::GameClosed { backoff } if backoff == MIN_BACKOFF));
    }

    #[test]
    fn circuit_breaker_survives_many_consecutive_failures() {
        let start = Instant::now();
        let mut breaker = CircuitBreaker::new();
        for _ in 0..100 {
            breaker.record_failure(start);
        }
        assert!(!breaker.should_poll(start));
        assert!(breaker.should_poll(start + MAX_BACKOFF + Duration::from_millis(1)));
        breaker.record_success();
        assert!(breaker.should_poll(start));
    }
}
