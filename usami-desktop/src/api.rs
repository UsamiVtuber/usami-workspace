use std::sync::OnceLock;
use std::time::Duration;
use serde::{Deserialize, Serialize};

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

pub struct UsamiApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl UsamiApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: normalize_base_url(Some(base_url.into())),
            client: build_http_client(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(api_base_url_from_env())
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
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
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;

        if is_pcm_stream(&response) {
            let sample_rate = pcm_sample_rate(&response);
            Ok(TtsStream::Pcm {
                sample_rate,
                response,
            })
        } else {
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
            Ok(TtsStream::Audio(bytes.to_vec()))
        }
    }

    pub async fn transcribe(&self, wav: Vec<u8>) -> Result<String, String> {
        let client = self.client.clone();
        let url = format!("{}/api/stt", self.base_url);

        let handle = tokio_runtime().spawn(async move {
            let part = reqwest::multipart::Part::bytes(wav)
                .file_name("mic.wav")
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?;
            let form = reqwest::multipart::Form::new().part("audio", part);
            let response = client
                .post(url)
                .multipart(form)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json::<SttResponse>()
                .await
                .map_err(|e| e.to_string())?;

            Ok::<String, String>(response.text)
        });

        match handle.await {
            Ok(res) => res,
            Err(e) => Err(format!("Erro interno na tarefa Tokio: {e}")),
        }
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
        assert_eq!(normalize_base_url(Some("   ".to_string())), DEFAULT_BASE_URL);
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
        assert!(request
            .content_type
            .unwrap_or_default()
            .starts_with("application/json"));
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
        let (base_url, requests) = spawn_server(
            "200 OK",
            "application/json",
            "",
            br#"{"text":"bom dia"}"#,
        );
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
        assert!(request
            .content_type
            .unwrap_or_default()
            .starts_with("multipart/form-data"));
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
}
