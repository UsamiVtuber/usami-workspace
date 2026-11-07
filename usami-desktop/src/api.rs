use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRequest {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
    #[serde(rename = "wakeUpJolt", skip_serializing_if = "Option::is_none")]
    pub wake_up_jolt: Option<bool>,
    #[serde(rename = "sleepCompanionActive", skip_serializing_if = "Option::is_none")]
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
pub struct ChatResponse {
    pub reply: String,
    pub gesture: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
    pub source: Option<String>,
    #[serde(rename = "speechTags", default)]
    pub speech_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsRequest {
    pub text: String,
    #[serde(rename = "styleId")]
    pub style_id: String,
    pub stream: bool,
}

pub struct UsamiApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl UsamiApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn send_chat(&self, message: &str) -> Result<ChatResponse, reqwest::Error> {
        let payload = ChatMessageRequest {
            message: message.to_string(),
            capabilities: None,
            wake_up_jolt: None,
            sleep_companion_active: None,
        };

        let res = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        Ok(res)
    }

    pub async fn fetch_tts_audio(&self, text: &str, style_id: &str) -> Result<Vec<u8>, reqwest::Error> {
        let payload = TtsRequest {
            text: text.to_string(),
            style_id: style_id.to_string(),
            stream: false,
        };

        let bytes = self
            .client
            .post(format!("{}/api/tts", self.base_url))
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        Ok(bytes.to_vec())
    }
}
