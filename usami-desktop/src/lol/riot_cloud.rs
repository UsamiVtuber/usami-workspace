//! Estrutura base para consumo passivo da Riot Games Cloud API (pré-jogo e histórico).

use crate::lol::types::{LeagueEntryDto, SummonerDto};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RiotCloudError {
    #[error("Chave da Riot API não configurada (RIOT_API_KEY)")]
    MissingApiKey,
    #[error("Rate limit da Riot API atingido")]
    RateLimited,
    #[error("Falha HTTP na Riot Cloud API: {0}")]
    Http(#[from] reqwest::Error),
}

pub struct RiotCloudClient {
    client: reqwest::Client,
    api_key: Option<String>,
    platform_host: String,
    regional_host: String,
}

impl RiotCloudClient {
    pub fn from_env() -> Self {
        let api_key = std::env::var("RIOT_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            api_key,
            platform_host: "https://br1.api.riotgames.com".to_string(),
            regional_host: "https://americas.api.riotgames.com".to_string(),
        }
    }

    pub fn set_api_key(&mut self, api_key: Option<String>) {
        self.api_key = api_key
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    pub async fn get_summoner_by_puuid(&self, puuid: &str) -> Result<SummonerDto, RiotCloudError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or(RiotCloudError::MissingApiKey)?;
        let url = format!("{}/lol/summoner/v4/summoners/by-puuid/{puuid}", self.platform_host);
        let res = self
            .client
            .get(url)
            .header("X-Riot-Token", key)
            .send()
            .await?;
        if res.status().as_u16() == 429 {
            return Err(RiotCloudError::RateLimited);
        }
        Ok(res.error_for_status()?.json::<SummonerDto>().await?)
    }

    pub async fn get_league_entries(
        &self,
        encrypted_summoner_id: &str,
    ) -> Result<Vec<LeagueEntryDto>, RiotCloudError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or(RiotCloudError::MissingApiKey)?;
        let url = format!(
            "{}/lol/league/v4/entries/by-summoner/{encrypted_summoner_id}",
            self.platform_host
        );
        let res = self
            .client
            .get(url)
            .header("X-Riot-Token", key)
            .send()
            .await?;
        if res.status().as_u16() == 429 {
            return Err(RiotCloudError::RateLimited);
        }
        Ok(res.error_for_status()?.json::<Vec<LeagueEntryDto>>().await?)
    }

    pub async fn get_recent_match_ids(
        &self,
        puuid: &str,
        count: usize,
    ) -> Result<Vec<String>, RiotCloudError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or(RiotCloudError::MissingApiKey)?;
        let url = format!(
            "{}/lol/match/v5/matches/by-puuid/{puuid}/ids?start=0&count={count}",
            self.regional_host
        );
        let res = self
            .client
            .get(url)
            .header("X-Riot-Token", key)
            .send()
            .await?;
        if res.status().as_u16() == 429 {
            return Err(RiotCloudError::RateLimited);
        }
        Ok(res.error_for_status()?.json::<Vec<String>>().await?)
    }
}

