//! Controle de voz: microfone, gravação STT, chamadas de áudio e modo sono/companhia.

use crate::recorder;
use crate::speech::is_duplicate_utterance;
use crate::UsamiApp;
use gpui::*;
use std::sync::Arc;
use std::time::Instant;

impl UsamiApp {
    pub(crate) fn start_call(&mut self, cx: &mut Context<Self>) {
        self.is_calling = true;
        self.call_started_at = Some(Instant::now());
        self.call_seconds = 0;
        self.status_text = "SUA VEZ · 00:00".into();
        self.start_listening(cx);
    }

    pub(crate) fn end_call(&mut self, cx: &mut Context<Self>) {
        let minutes = self
            .call_started_at
            .map(|started| (started.elapsed().as_secs() / 60) as u32)
            .unwrap_or(0);
        self.stop_listening();
        self.is_calling = false;
        self.call_started_at = None;
        self.call_seconds = 0;
        if minutes > 0 {
            self.award_bond_call(minutes, cx);
        }
        self.status_text = if self.is_companion_mode {
            "MODO COMPANION"
        } else {
            "COM VOCÊ"
        }
        .into();
    }

    pub(crate) fn toggle_sleep_companion(&mut self) {
        if self.sleep_companion_started_at.is_some() {
            self.stop_sleep_companion();
        } else {
            self.is_companion_mode = true;
            self.sleep_companion_started_at = Some(Instant::now());
            self.sleep_companion_minutes_left = 30;
            self.last_emotion = "sleepy".into();
            self.status_text = "MODO COMPANION".into();
            self.vrm_controller.set_emotion("sleepy");
        }
    }

    pub(crate) fn stop_sleep_companion(&mut self) {
        self.sleep_companion_started_at = None;
        self.sleep_companion_minutes_left = 30;
        self.is_companion_mode = false;
        self.last_emotion = "tsundere".into();
        self.status_text = if self.is_calling {
            "SUA VEZ"
        } else {
            "COM VOCÊ"
        }
        .into();
        self.vrm_controller
            .set_emotion_with_options("tsundere", 4.0, false);
        self.vrm_controller.play_idle();
    }

    /// Fecha a escuta enquanto a Usami pensa/fala e quando o usuário muta.
    pub(crate) fn sync_mic_gate(&self) {
        self.mic_gate
            .set_user(self.is_muted || self.is_busy || !self.is_listening);
    }

    pub(crate) fn start_listening(&mut self, cx: &mut Context<Self>) {
        self.poke(cx);
        if self.is_listening {
            return;
        }
        match recorder::VoiceListener::start(Arc::clone(&self.mic_gate)) {
            Ok(mut listener) => {
                let Some(segments) = listener.take_segments() else {
                    listener.stop();
                    self.is_listening = false;
                    self.sync_mic_gate();
                    self.status_text = "MIC: canal de áudio indisponível.".into();
                    cx.notify();
                    return;
                };
                let partials = listener.take_partials();
                self.listener = Some(listener);
                self.is_listening = true;
                self.listen_session = self.listen_session.wrapping_add(1);
                let session = self.listen_session;
                self.last_voice_commit = None;
                self.live_transcript = SharedString::default();
                self.voice_turn_closed = 0;
                self.sync_mic_gate();
                self.status_text = "OUVINDO...".into();

                // Prévia ao vivo: fatias do áudio em andamento vão para o STT em
                // paralelo ao turno final. A latência só atrasa a legenda; parar
                // de falar continua encerrando o turno pelo caminho normal.
                if let Some(mut partials) = partials {
                    let api = Arc::clone(&self.api_client);
                    cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let cx = cx.clone();
                        async move {
                            while let Some(mut slice) = partials.recv().await {
                                // O STT local pode demorar mais que o intervalo das
                                // fatias; transcrever a mais recente evita fila parada.
                                while let Ok(newer) = partials.try_recv() {
                                    slice = newer;
                                }
                                let Ok(text) = api.transcribe(slice.wav).await else {
                                    continue;
                                };
                                let trimmed = text.trim().to_string();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let active = cx
                                    .update(|cx: &mut App| {
                                        if let Some(this) = this.upgrade() {
                                            this.update(cx, |view, cx| {
                                                if !view.is_listening
                                                    || !view.is_calling
                                                    || view.listen_session != session
                                                {
                                                    return false;
                                                }
                                                // Chegou depois do texto final: é só um
                                                // eco atrasado do turno já respondido.
                                                if slice.turn <= view.voice_turn_closed {
                                                    return true;
                                                }
                                                if view.live_transcript.as_ref() != trimmed {
                                                    view.live_transcript = trimmed.clone().into();
                                                    cx.notify();
                                                }
                                                true
                                            })
                                        } else {
                                            false
                                        }
                                    })
                                    .unwrap_or(false);
                                if !active {
                                    break;
                                }
                            }
                        }
                    })
                    .detach();
                }

                // Consome as falas detectadas pela VAD e manda para o STT. O loop
                // vive enquanto o listener existir; trocar de chamada recria tudo.
                let api = Arc::clone(&self.api_client);
                cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        let mut segments = segments;
                        while let Some(slice) = segments.recv().await {
                            // Fecha o turno antes de transcrever: prévias dele que
                            // ainda estejam na fila não podem reaparecer na legenda.
                            let accepted = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || (!view.is_calling && !view.lol_overlay_active) {
                                                return false;
                                            }
                                            view.voice_turn_closed =
                                                view.voice_turn_closed.max(slice.turn);
                                            view.live_transcript = SharedString::default();
                                            cx.notify();
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !accepted {
                                break;
                            }

                            let transcript = api.transcribe(slice.wav).await;
                            let active = cx
                                .update(|cx: &mut App| {
                                    if let Some(this) = this.upgrade() {
                                        this.update(cx, |view, cx| {
                                            if !view.is_listening || (!view.is_calling && !view.lol_overlay_active) {
                                                return false;
                                            }
                                            match transcript {
                                                Ok(text) if !text.trim().is_empty() => {
                                                    let trimmed = text.trim().to_string();
                                                    if is_duplicate_utterance(
                                                        &mut view.last_voice_commit,
                                                        &trimmed,
                                                        Instant::now(),
                                                    ) {
                                                        return true;
                                                    }
                                                    if view.lol_overlay_active {
                                                        if let Some(ref handle) = view.lol_handle {
                                                            handle.send_control(crate::lol::ipc::LolControlMsg::UserVoice(trimmed));
                                                        }
                                                        return true;
                                                    }
                                                    view.cursor_char_idx = trimmed.chars().count();
                                                    view.selection_anchor = None;
                                                    view.input_text = trimmed;
                                                    if view.is_busy {
                                                        // Não fala por cima: guarda no campo.
                                                        view.status_text = "TEXTO PRONTO".into();
                                                        cx.notify();
                                                    } else {
                                                        view.submit(cx);
                                                    }
                                                }
                                                Ok(_) => {
                                                    view.status_text = "NÃO ENTENDI".into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                                Err(error) => {
                                                    view.status_text =
                                                        format!("ERRO STT: {error}").into();
                                                    view.poke(cx);
                                                    cx.notify();
                                                }
                                            }
                                            true
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !active {
                                break;
                            }
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                self.is_listening = false;
                self.live_transcript = SharedString::default();
                self.sync_mic_gate();
                self.status_text = format!("MIC: {error}").into();
            }
        }
        cx.notify();
    }

    pub(crate) fn stop_listening(&mut self) {
        self.is_listening = false;
        self.listen_session = self.listen_session.wrapping_add(1);
        self.live_transcript = SharedString::default();
        self.sync_mic_gate();
        if let Some(listener) = self.listener.take() {
            listener.stop();
        }
    }

    pub(crate) fn toggle_listening(&mut self, cx: &mut Context<Self>) {
        if self.is_listening {
            self.stop_listening();
            self.status_text = "MICROFONE OFF".into();
            cx.notify();
        } else {
            self.start_listening(cx);
        }
    }
}
