use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

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

    pub async fn send_chat(
        &self,
        message: &str,
        sleep_companion_active: bool,
    ) -> Result<ChatResponse, String> {
        let capabilities = serde_json::json!({
            "format": "VRM",
            "gestures": ["tilt", "idle", "wave", "angry", "happy", "blush", "jump", "sad", "sleepy", "surprised", "clapping", "nod", "talking"],
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
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json::<ChatResponse>()
                .await
                .map_err(|e| e.to_string())?;

            Ok::<ChatResponse, String>(res)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }

    pub async fn fetch_tts_audio(&self, text: &str, style_id: &str) -> Result<Vec<u8>, String> {
        let payload = TtsRequest {
            text: text.to_string(),
            style_id: style_id.to_string(),
            stream: false,
        };

        let client = self.client.clone();
        let url = format!("{}/api/tts", self.base_url);

        let handle = tokio_runtime().spawn(async move {
            let bytes = client
                .post(url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;

            Ok::<Vec<u8>, String>(bytes.to_vec())
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
    }
}
