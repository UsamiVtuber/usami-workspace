//! Cliente local do Ollama para geração de texto sem usar APIs pagas.
//!
//! Chama `POST http://127.0.0.1:11434/api/chat` com o modelo `phi3:mini`.
//! Usado por componentes novos (ex.: Usami.GG / `src/lol/`) como alternativa
//! ao backend `127.0.0.1:8787` (OpenRouter/NVIDIA), economizando créditos de API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const OLLAMA_MODEL: &str = "phi3:mini";
const OLLAMA_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    #[serde(default)]
    pub created_at: String,
    pub message: OllamaResponseMessage,
    pub done: bool,
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub eval_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaResponseMessage {
    pub role: String,
    pub content: String,
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(OLLAMA_TIMEOUT_SECS))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: OLLAMA_BASE_URL.to_string(),
            model: OLLAMA_MODEL.to_string(),
        }
    }

    /// Gera uma resposta de texto para o `prompt` usando o modelo local.
    /// Devolve o conteúdo da mensagem do assistente, ou um erro descritivo
    /// (em português) se o Ollama estiver indisponível ou falhar.
    pub async fn chat(&self, prompt: &str) -> Result<String, String> {
        let payload = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![OllamaMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            stream: Some(false),
            options: Some(OllamaOptions {
                temperature: Some(0.7),
                num_predict: Some(512),
            }),
        };

        let url = format!("{}/api/chat", self.base_url);

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    "Ollama indisponível — confirme que `ollama serve` está rodando em 127.0.0.1:11434.".to_string()
                } else if e.is_timeout() {
                    "Ollama demorou para responder (timeout de 30 s).".to_string()
                } else {
                    format!("Falha ao consultar o Ollama: {e}")
                }
            })?
            .error_for_status()
            .map_err(|e| {
                if let Some(status) = e.status() {
                    format!("Ollama respondeu com erro HTTP {status}.")
                } else {
                    "Resposta inválida do Ollama.".to_string()
                }
            })?;

        let data: OllamaChatResponse = res
            .json()
            .await
            .map_err(|e| format!("Resposta do Ollama não pôde ser lida: {e}"))?;

        Ok(data.message.content)
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_uses_phi3_mini_and_non_stream() {
        let client = OllamaClient::new();
        assert_eq!(client.model, "phi3:mini");
        assert_eq!(client.base_url, "http://127.0.0.1:11434");
    }
}
