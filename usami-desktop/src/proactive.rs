//! Polling e apresentação de eventos proativos da Usami, além do controle de gavetas (drawers).

use crate::UsamiApp;
use crate::api::tokio_runtime;
use crate::model::{ActiveDrawer, ChatMessage, Sender, current_phone_time};
use crate::speech::sanitize_speech_text;
use crate::storage;
use gpui::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

impl UsamiApp {
    pub(crate) fn poll_proactive_events(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled {
            return;
        }
        let api = Arc::clone(&self.api_client);
        // Relógio de silêncio: enquanto a voz da Usami está tocando ou terminou há < 60s,
        // conta como atividade recente (OpenAmika 11c7c4e).
        let is_active = self.last_interaction.elapsed() < Duration::from_secs(60)
            || self.vrm_controller.is_speaking()
            || self.last_voice_ended.elapsed() < Duration::from_secs(60);
        let activity = if self.is_away {
            "away"
        } else if is_active {
            "active"
        } else {
            "idle"
        };

        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_proactive_events(activity).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                if let Ok(res) = res {
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, _cx| {
                                // Ausente: conversa social é consumida sem falar
                                // (não despeja atrasados ao voltar); lembrete fica
                                // pendente no servidor para não se perder.
                                if view.is_away {
                                    let acks: Vec<crate::api::ProactiveAckItem> = res
                                        .events
                                        .iter()
                                        .filter(|event| event.event_type != "reminder")
                                        .map(|event| crate::api::ProactiveAckItem {
                                            id: event.id.clone(),
                                            outcome: "skipped".to_string(),
                                        })
                                        .collect();
                                    if !acks.is_empty() {
                                        let api = Arc::clone(&view.api_client);
                                        crate::api::tokio_runtime().spawn(async move {
                                            let _ = api.ack_proactive_events(&acks).await;
                                        });
                                    }
                                    return;
                                }
                                for event in res.events {
                                    if let Some(dedupe_key) = &event.dedupe_key {
                                        if view.proactive_seen.contains(dedupe_key) {
                                            continue;
                                        }
                                    }
                                    if view.proactive_queue.iter().any(|e| e.id == event.id) {
                                        continue;
                                    }
                                    if view.proactive_queue.len() < 5 {
                                        view.proactive_queue.push(event);
                                    }
                                }
                            });
                        }
                    });
                }
            }
        })
        .detach();

        if !self.is_away && self.api_client.has_token() {
            let api = Arc::clone(&self.api_client);
            let (tx, rx) = oneshot::channel();
            tokio_runtime().spawn(async move {
                let resp = api.get_agent_suggestions(false, 20).await;
                let _ = tx.send(resp);
            });
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let resp = rx.await.expect("Tokio task dropped without sending result");
                    if let Ok(resp) = resp {
                        let _ = cx.update(|cx: &mut App| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |view, cx| {
                                    view.agent_suggestions = resp.suggestions;
                                    view.agent_suggestions_unanswered = resp.unanswered_count;
                                    view.agent_suggestions_loaded = true;
                                    view.agent_suggestions_error = None;
                                    cx.notify();
                                });
                            }
                        });
                    }
                }
            })
            .detach();
        }
    }

    pub(crate) fn present_next_proactive(&mut self, cx: &mut Context<Self>) {
        if !self.proactive_enabled || self.proactive_queue.is_empty() || self.is_away {
            return;
        }
        if self.is_busy
            || self.is_calling
            || self.is_listening
            || self.sleep_companion_started_at.is_some()
            || self.vrm_controller.is_speaking()
        {
            return;
        }

        let next_is_social = self.proactive_queue[0].event_type != "reminder";
        if next_is_social && self.last_proactive_speech.elapsed() < Duration::from_secs(300) {
            return;
        }

        let event = self.proactive_queue.remove(0);
        if let Some(dedupe_key) = &event.dedupe_key {
            self.proactive_seen.insert(dedupe_key.clone());
            storage::save_proactive_seen(&self.proactive_seen);
        }

        let emotion = event.emotion.clone().unwrap_or_else(|| "tsundere".into());
        self.last_reply = sanitize_speech_text(&event.text).into();
        self.last_emotion = emotion.clone().into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(&emotion, 5.0, false);
        if let Some(gesture) = &event.gesture {
            self.vrm_controller.play_vrma(gesture, false);
        }
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: event.text.clone(),
            emotion: Some(emotion.clone()),
            tool_badge: None,
            time_str: Some(current_phone_time()),
            media: None,
        });
        self.fetch_mood(cx);
        self.touch_ui(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        let audio = self.audio_player.clone();
        let event_id = event.id.clone();
        let text = event.text.clone();
        let is_muted = self.is_muted;
        let vrm_ctrl = self.vrm_controller.clone();

        self.last_proactive_speech = Instant::now();

        crate::api::tokio_runtime().spawn(async move {
            let mut delivered = false;
            if !is_muted && let Some(player) = audio {
                match api.stream_tts(&text, &emotion).await {
                    Ok(crate::api::TtsStream::Pcm {
                        sample_rate,
                        mut response,
                    }) => {
                        let mouth = vrm_ctrl.clone();
                        let mut sink = player.play_pcm_stream(sample_rate, move |level| {
                            mouth.set_speaking(true, level);
                        });
                        let mut started = false;
                        loop {
                            match response.chunk().await {
                                Ok(Some(chunk)) => {
                                    if sink.push_i16_le(&chunk) && !started {
                                        started = true;
                                        vrm_ctrl.begin_speech_stream();
                                    }
                                }
                                Ok(None) => break,
                                Err(error) => {
                                    eprintln!("[Proactive TTS] stream interrompido: {error}");
                                    break;
                                }
                            }
                        }
                        let tail = sink.finish();
                        if started {
                            let deadline = Instant::now() + Duration::from_secs(180);
                            while !tail.is_drained() && Instant::now() < deadline {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            vrm_ctrl.end_speech_stream();
                            delivered = true;
                        } else {
                            vrm_ctrl.speak(&text, None);
                        }
                    }
                    Ok(crate::api::TtsStream::Audio(bytes)) => {
                        vrm_ctrl.speak(&text, Some(bytes.clone()));
                        if let Err(error) = player.play_wav(bytes) {
                            eprintln!("[Proactive TTS] falha ao tocar áudio: {error}");
                        } else {
                            delivered = true;
                        }
                    }
                    Err(e) => {
                        eprintln!("[Proactive TTS] falha: {e:?}");
                    }
                }
            }

            let outcome = if delivered { "spoken" } else { "shown" };
            let _ = api
                .ack_proactive_events(&[crate::api::ProactiveAckItem {
                    id: event_id,
                    outcome: outcome.to_string(),
                }])
                .await;
        });
    }

    pub(crate) fn toggle_drawer(&mut self, drawer: ActiveDrawer, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.active_drawer == drawer {
            self.active_drawer = ActiveDrawer::None;
        } else {
            self.active_drawer = drawer;
            if drawer == ActiveDrawer::Chat {
                // Abrir no fim da conversa: o scroll nasce no topo por padrão.
                self.phone_scroll.scroll_to_bottom();
            } else if drawer == ActiveDrawer::Bond {
                // Abrir o painel relê o core: o web (ou outra janela) pode ter mudado.
                self.fetch_bond(cx);
                self.fetch_mood(cx);
                // Diário, Álbum e Memória também acompanham os registros/espelho do core.
                if !self.memory_loading {
                    self.fetch_memory(cx);
                }
            } else if drawer == ActiveDrawer::Skills {
                self.fetch_agent_center(cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn set_proactive_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.proactive_enabled = enabled;
        storage::save_proactive_config(enabled);
        if !enabled {
            self.proactive_queue.clear();
        }
        cx.notify();
    }
}
