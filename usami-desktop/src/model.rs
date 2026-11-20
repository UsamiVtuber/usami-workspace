//! Modelos de dados e enums da interface do Usami Desktop.

use crate::api;
use crate::media::AttachedMedia;
use crate::storage;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ActiveDrawer {
    None,
    Chat,
    Bond,
    Settings,
    Vrma,
    Skills,
}

/// Abas do painel de configurações (mesma ordem do web).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SettingsTab {
    Chat,
    Voice,
    Avatar,
    Memory,
    Personality,
    About,
}

/// Capítulos do menu estilo Visual Novel ("Midnight Romance").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BondTab {
    Home,
    Diary,
    Memories,
    Bonds,
    Achievements,
    Mood,
    Profile,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sender {
    User,
    Usami,
}

#[derive(Clone)]
pub(crate) struct ChatMessage {
    pub(crate) sender: Sender,
    pub(crate) text: String,
    pub(crate) emotion: Option<String>,
    pub(crate) tool_badge: Option<String>,
    pub(crate) time_str: Option<String>,
    pub(crate) media: Option<AttachedMedia>,
}

pub(crate) fn current_phone_time() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let local_secs = total_secs.saturating_sub(3 * 3600);
    let hours = (local_secs / 3600) % 24;
    let mins = (local_secs % 3600) / 60;
    format!("{:02}:{:02}", hours, mins)
}

pub(crate) fn format_tool_badge(agent_turn: Option<&api::AgentTurnSummary>) -> Option<String> {
    let turn = agent_turn?;
    let status_suffix = match turn.status.as_deref() {
        Some("executed") => "✓",
        Some("awaiting_confirmation") => "⏳",
        _ => "✗",
    };
    if turn.tools.len() > 1 {
        let joined = turn.tools.join(" + ");
        Some(format!("{joined} {status_suffix}"))
    } else if let Some(tool) = turn.tool.as_deref() {
        Some(format!("{tool} {status_suffix}"))
    } else {
        Some("sem skill".into())
    }
}

pub(crate) fn stored_turns(turns: &[ChatMessage]) -> Vec<storage::StoredTurn> {
    turns
        .iter()
        .map(|message| storage::StoredTurn {
            sender: match message.sender {
                Sender::User => "user".into(),
                Sender::Usami => "usami".into(),
            },
            text: message.text.clone(),
            emotion: message.emotion.clone(),
            media: message.media.as_ref().map(|m| storage::StoredMedia {
                filename: m.filename.clone(),
                mime_type: m.mime_type.clone(),
                path: m.stored_path.clone().unwrap_or_default(),
            }),
        })
        .collect()
}

pub(crate) fn chat_message_from_stored(turn: storage::StoredTurn) -> Option<ChatMessage> {
    let sender = match turn.sender.as_str() {
        "user" => Sender::User,
        "usami" => Sender::Usami,
        _ => return None,
    };
    if turn.text.is_empty() {
        return None;
    }
    let media = turn.media.as_ref().and_then(|stored| {
        let bytes = std::fs::read(storage::media_path(&stored.path)).ok()?;
        let format = match stored.mime_type.as_str() {
            "image/jpeg" | "image/jpg" => gpui::ImageFormat::Jpeg,
            "image/webp" => gpui::ImageFormat::Webp,
            "image/gif" => gpui::ImageFormat::Gif,
            "image/bmp" => gpui::ImageFormat::Bmp,
            _ => gpui::ImageFormat::Png,
        };
        let preview = Arc::new(gpui::Image::from_bytes(format, bytes.clone()));
        Some(AttachedMedia {
            filename: stored.filename.clone(),
            mime_type: stored.mime_type.clone(),
            data_base64: api::base64_encode(&bytes),
            preview,
            stored_path: Some(stored.path.clone()),
        })
    });
    Some(ChatMessage {
        sender,
        text: turn.text,
        emotion: turn.emotion,
        tool_badge: None,
        time_str: Some(current_phone_time()),
        media,
    })
}
