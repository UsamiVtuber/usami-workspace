//! Ciclo de vida da aplicacao: observadores de janela, ausencia/retorno, despedida e ociosidade.

use crate::UsamiApp;
use crate::model::{ActiveDrawer, ChatMessage, Sender, current_phone_time};
use crate::storage;
use gpui::*;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn window_minimized_or_hidden() -> bool {
    #[cfg(target_os = "windows")]
    {
        crate::presence::app_is_minimized_or_hidden()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(crate) const CLOSE_FAREWELL_MIN: Duration = Duration::from_millis(1200);
pub(crate) const CLOSE_FAREWELL_MAX: Duration = Duration::from_secs(10);

/// A janela so sai depois da despedida: minimo legivel, audio terminado e avatar
/// sem falar -- ou o teto rigido, para nunca travar o fechamento.
pub(crate) fn farewell_can_close(elapsed: Duration, audio_done: bool, speaking: bool) -> bool {
    (elapsed >= CLOSE_FAREWELL_MIN && audio_done && !speaking) || elapsed >= CLOSE_FAREWELL_MAX
}

impl UsamiApp {
    /// Observa foco e geometria da janela: o foco alimenta o estado ausente e a
    /// geometria é lembrada entre sessões (a gravação é adiada para não escrever
    /// um JSON a cada pixel arrastado).
    pub(crate) fn attach_window_observers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds_subscription = cx.observe_window_bounds(window, |view, window, cx| {
            let bounds = window.bounds();
            let width: f32 = bounds.size.width.into();
            let height: f32 = bounds.size.height.into();
            // O modo mascote tem geometria própria: ela não deve virar a
            // "janela lembrada" da próxima abertura.
            if view.mascot_mode {
                return;
            }
            // Encolher abaixo do mínimo utilizável é o segundo gatilho do modo
            // mascote (o primeiro é bandeja/atalho).
            if crate::mascot::is_enabled()
                && crate::mascot::should_snap(width, height)
                && !view.lol_overlay_active
            {
                view.enter_mascot(window, cx);
                return;
            }
            view.prefs.window = Some(storage::WindowPrefs {
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y),
                width,
                height,
            });
            if let Some(rect) = crate::mascot::current_rect() {
                view.last_normal_rect = Some(rect);
            }
            if view.last_prefs_save.elapsed() >= Duration::from_secs(2) {
                view.persist_prefs();
            }
            let maximized = window.is_maximized();
            if maximized != view.was_maximized {
                view.was_maximized = maximized;
                if maximized
                    && !view.is_away
                    && view.last_maximize_line.elapsed() >= Duration::from_secs(900)
                {
                    view.last_maximize_line = Instant::now();
                    view.show_local_line(
                        "Tela grande, hein? Vou fingir que não percebi que você quis me ver maior.",
                        "tsundere",
                        cx,
                    );
                }
            }
        });
        let activation_subscription = cx.observe_window_activation(window, |view, window, cx| {
            view.window_focused = window.is_window_active();
            view.refresh_away_state(cx);
        });
        self.window_subscriptions.push(bounds_subscription);
        self.window_subscriptions.push(activation_subscription);

        // Fechar (X, Alt+F4 ou "Sair" da bandeja) pede a despedida antes de
        // sair; o próprio app conclui o fechamento quando ela termina.
        let close_view = cx.weak_entity();
        window.on_window_should_close(cx, move |_window, cx| {
            close_view
                .update(cx, |this, cx| this.request_close(cx))
                .unwrap_or(true)
        });
        // Sem um resize inicial o observador não dispara; guardar a geometria
        // atual já garante que o fechamento salve a janela do primeiro uso.
        let bounds = window.bounds();
        self.prefs.window = Some(storage::WindowPrefs {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        });
        if let Some(rect) = crate::mascot::current_rect() {
            self.last_normal_rect = Some(rect);
        }
        self.window_focused = window.is_window_active();
        self.was_maximized = window.is_maximized();
    }

    /// Alterna entre o app completo e a janelinha de mascote.
    pub(crate) fn toggle_mascot(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        source: crate::mascot::ToggleRequest,
    ) {
        use crate::mascot::ToggleRequest;
        if self.mascot_mode {
            self.exit_mascot(window, cx);
        } else if self.lol_overlay_active {
            // Com a sobreposição do LoL ativa o mascote não entra. O clique em
            // minimizar mantém o comportamento padrão; o atalho/bandeja não
            // mexe na janela.
            if source == ToggleRequest::Minimize {
                window.minimize_window();
            }
        } else {
            self.enter_mascot(window, cx);
        }
    }

    pub(crate) fn enter_mascot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mascot_mode || self.lol_overlay_active {
            return;
        }
        // Marcado antes de mexer na janela: o observador de bounds reentra.
        self.mascot_mode = true;
        self.mascot_grab = None;
        self.mascot_menu_at = None;
        self.mascot_settings_open = false;
        self.active_drawer = ActiveDrawer::None;
        self.is_away = false;
        self.away_since = None;
        self.status_text = self.default_status();
        #[cfg(target_os = "windows")]
        {
            if let Some(restore) =
                crate::mascot::enter(window.scale_factor(), self.last_normal_rect)
            {
                self.last_normal_rect = Some(restore);
            }
            crate::mascot::set_active(true);
            // O SetWindowPos aconteceu dentro deste update: o bounds_changed do
            // GPUI foi descartado (App emprestado). Este resize é adiado pelo
            // GPUI e reemite WM_SIZE fora do empréstimo, ressincronizando o
            // viewport com a janelinha.
            window.resize(size(
                px(crate::mascot::MASCOT_WIDTH),
                px(crate::mascot::MASCOT_HEIGHT),
            ));
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window;
        self.vrm_controller.set_portrait_enabled(true);
        self.prefs.mascot = true;
        self.persist_prefs();
        cx.notify();
    }

    pub(crate) fn exit_mascot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.mascot_mode {
            return;
        }
        self.mascot_mode = false;
        self.mascot_grab = None;
        self.mascot_menu_at = None;
        self.mascot_settings_open = false;
        // Fora do mascote os laços de STT encerram; sem isso `is_listening`
        // ficaria preso em `true` e o microfone nunca fecharia de novo.
        if self.is_listening && !self.is_calling && !self.lol_overlay_active {
            self.stop_listening();
        }
        #[cfg(target_os = "windows")]
        {
            let restored_maximized = crate::mascot::exit();
            crate::mascot::set_active(false);
            // Mesmo motivo do enter: ressincroniza o viewport depois de
            // devolver estilo e geometria da janela. No caso maximizado o
            // maximize é assíncrono e emite o próprio WM_SIZE.
            if !restored_maximized {
                if let Some(saved) = self.prefs.window {
                    window.resize(size(px(saved.width), px(saved.height)));
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window;
        // O laço de olhar só reseta quando percebe a troca; zerar aqui evita
        // que o avatar fique vesgo até o próximo ciclo lento.
        self.vrm_controller.look_at_normalized(0.0, 0.0);
        self.is_ui_idle = false;
        self.vrm_controller.set_portrait_enabled(true);
        self.prefs.mascot = false;
        self.persist_prefs();
        self.is_away = false;
        self.away_since = None;
        self.status_text = self.default_status();
        cx.notify();
    }

    pub(crate) fn mascot_open_menu(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.mascot_menu_at = Some(at);
        self.mascot_grab = None;
        self.touch_ui(cx);
        cx.notify();
    }

    pub(crate) fn mascot_close_menu(&mut self, cx: &mut Context<Self>) {
        if self.mascot_menu_at.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn mascot_toggle_follow(&mut self, cx: &mut Context<Self>) {
        let next = !self.prefs.mascot_follow;
        self.prefs.mascot_follow = next;
        crate::mascot::set_follow(next);
        self.persist_prefs();
        let line = if next {
            "Hmph... vou ficar de olho onde você estiver. Não me julga."
        } else {
            "Tá, parei de te seguir. Mas eu ainda sei onde você está."
        };
        self.show_local_line(line, "tsundere", cx);
    }

    pub(crate) fn mascot_open_settings(&mut self, cx: &mut Context<Self>) {
        self.mascot_menu_at = None;
        self.mascot_settings_open = true;
        self.load_settings_data(cx);
        self.touch_ui(cx);
        cx.notify();
    }

    pub(crate) fn mascot_close_settings(&mut self, cx: &mut Context<Self>) {
        self.mascot_settings_open = false;
        self.touch_ui(cx);
        cx.notify();
    }

    /// Sai do mascote já com o app completo na aba de ajustes.
    pub(crate) fn mascot_open_settings_in_app(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mascot_settings_open = false;
        self.exit_mascot(window, cx);
        self.load_settings_data(cx);
        self.active_drawer = ActiveDrawer::Settings;
        cx.notify();
    }

    pub(crate) fn mascot_mouse_down(&mut self) {
        self.mascot_grab = crate::mascot::grab_offset();
    }

    pub(crate) fn mascot_mouse_move(&mut self, event: &MouseMoveEvent) {
        if !self.mascot_mode || event.pressed_button != Some(MouseButton::Left) {
            self.mascot_grab = None;
            return;
        }
        if let Some(grab) = self.mascot_grab {
            crate::mascot::move_with_cursor(grab);
        }
    }

    pub(crate) fn mascot_click(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mascot_grab = None;
        if self.mascot_settings_open {
            return;
        }
        if self.mascot_menu_at.is_some() {
            self.mascot_close_menu(cx);
            return;
        }
        if event.click_count() >= 2 {
            self.exit_mascot(window, cx);
            return;
        }
        const LINES: [&str; 3] = [
            "Ei! Não fica me cutucando assim, baka.",
            "Tô aqui, tá? Pode continuar no seu jogo.",
            "Hmph. Se quiser conversar de verdade, duplo clique em mim.",
        ];
        let line = LINES[self.mascot_pokes as usize % LINES.len()];
        self.mascot_pokes = self.mascot_pokes.wrapping_add(1);
        self.touch_ui(cx);
        self.show_local_line(line, "tsundere", cx);
    }

    pub(crate) fn default_status(&self) -> SharedString {
        if self.is_calling {
            "SUA VEZ".into()
        } else if self.is_companion_mode {
            "MODO COMPANION".into()
        } else {
            "COM VOCÊ".into()
        }
    }

    /// Ausência é imediata: minimizar, esconder ou perder o foco já marca
    /// AUSENTE. Chamada e modo companhia são exceções — nessas horas o app pode
    /// ficar em segundo plano sem que ela "sinta" a ausência.
    pub(crate) fn refresh_away_state(&mut self, cx: &mut Context<Self>) {
        // Modo mascote é presença: ela está no desktop mesmo sem foco.
        let exempt =
            self.is_calling || self.sleep_companion_started_at.is_some() || self.mascot_mode;
        let away = !exempt && (window_minimized_or_hidden() || !self.window_focused);
        if away == self.is_away {
            return;
        }
        self.is_away = away;
        if away {
            self.away_since = Some(Instant::now());
            self.status_text = "AUSENTE".into();
            // Eventos já buscados não devem falar quando ela voltar: o próximo
            // poll, com activity=away, consome e marca como pulados.
            self.proactive_queue.clear();
        } else {
            let away_for = self
                .away_since
                .take()
                .map(|since| since.elapsed())
                .unwrap_or_default();
            self.status_text = self.default_status();
            self.show_return_greeting(away_for, cx);
        }
        cx.notify();
    }

    pub(crate) fn show_return_greeting(&mut self, away_for: Duration, cx: &mut Context<Self>) {
        if away_for < Duration::from_secs(60)
            || self.last_return_greeting.elapsed() < Duration::from_secs(300)
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        let line = if away_for >= Duration::from_secs(30 * 60) {
            "Você ficou fora bastante tempo... eu fiquei aqui, sabia? Fico feliz que voltou."
        } else if away_for >= Duration::from_secs(5 * 60) {
            "Onde você foi? Não é como se eu estivesse esperando... mas pronto, você voltou."
        } else {
            "Você voltou rapidinho. Eu nem tive tempo de sentir sua falta."
        };
        self.last_return_greeting = Instant::now();
        self.show_local_line(line, "tsundere", cx);
    }

    /// Fala curta da própria Usami (sem backend): entrada no histórico, balão e
    /// emoção. Usada pelas despedidas/boas-vindas e pelas provocações de inatividade.
    pub(crate) fn show_local_line(
        &mut self,
        text: &'static str,
        emotion: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.last_reply = text.into();
        self.last_emotion = emotion.into();
        self.show_bubble = true;
        self.vrm_controller
            .set_emotion_with_options(emotion, 4.0, false);
        self.push_history(ChatMessage {
            sender: Sender::Usami,
            text: text.to_string(),
            emotion: Some(emotion.to_string()),
            tool_badge: None,
            time_str: Some(current_phone_time()),
            media: None,
        });
        self.poke(cx);
        cx.notify();
    }

    /// Primeira tentativa de fechar segura a janela para a despedida; a segunda
    /// (ou depois de liberado) deixa sair na hora — quem insiste consegue.
    pub(crate) fn request_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.close_allowed || self.farewell_started_at.is_some() {
            self.close_allowed = true;
            return true;
        }
        self.begin_farewell(cx);
        false
    }

    pub(crate) fn begin_farewell(&mut self, cx: &mut Context<Self>) {
        const LINE: &str = "Hmph... já vai embora? T-tá bom, vai... mas não demora pra voltar, tá?! Ainda tenho umas coisas pra te contar.";
        self.farewell_started_at = Some(Instant::now());
        self.show_local_line(LINE, "tsundere", cx);

        if self.is_muted {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        }
        let Some(player) = self.audio_player.clone() else {
            self.farewell_audio_done.store(true, Ordering::SeqCst);
            return;
        };

        let api = Arc::clone(&self.api_client);
        let vrm_ctrl = self.vrm_controller.clone();
        let done = Arc::clone(&self.farewell_audio_done);
        crate::api::tokio_runtime().spawn(async move {
            match api.stream_tts(LINE, "tsundere").await {
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
                                eprintln!("[Despedida TTS] stream interrompido: {error}");
                                break;
                            }
                        }
                    }
                    let tail = sink.finish();
                    if started {
                        let deadline = Instant::now() + Duration::from_secs(15);
                        while !tail.is_drained() && Instant::now() < deadline {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        vrm_ctrl.end_speech_stream();
                    }
                }
                Ok(crate::api::TtsStream::Audio(bytes)) => {
                    vrm_ctrl.speak(LINE, Some(bytes.clone()));
                    if let Err(error) = player.play_wav(bytes) {
                        eprintln!("[Despedida TTS] falha ao tocar áudio: {error}");
                    }
                }
                Err(error) => eprintln!("[Despedida TTS] falha: {error:?}"),
            }
            done.store(true, Ordering::SeqCst);
        });
    }

    pub(crate) fn tick_close_request(&mut self, cx: &mut Context<Self>) {
        let Some(started) = self.farewell_started_at else {
            return;
        };
        let audio_done = self.farewell_audio_done.load(Ordering::SeqCst);
        if !farewell_can_close(
            started.elapsed(),
            audio_done,
            self.vrm_controller.is_speaking(),
        ) {
            return;
        }
        self.close_allowed = true;
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    /// Sem interação e sem fala há alguns minutos, ela chama atenção em vez de
    /// ficar em silêncio para sempre. Só roda com a janela em primeiro plano:
    /// com o app ausente quem fala é a despedida/boas-vindas.
    pub(crate) fn maybe_run_idle_prods(&mut self, cx: &mut Context<Self>) {
        if self.is_away
            || self.is_calling
            || self.sleep_companion_started_at.is_some()
            || self.is_busy
            || self.vrm_controller.is_speaking()
        {
            return;
        }
        const PRODS: [(u64, &str); 3] = [
            (
                210,
                "Ei... você tá aí? Pode ficar quieto, mas eu prefiro quando você fala comigo.",
            ),
            (
                420,
                "Hmph. Se você não tem nada pra dizer, eu também não vou ficar puxando assunto.",
            ),
            (600, "Já faz um tempinho... tá tudo bem aí?"),
        ];
        let idle = self.last_user_activity.elapsed().as_secs();
        let Some((_, line)) = PRODS
            .get(self.idle_prods as usize)
            .copied()
            .filter(|(threshold, _)| idle >= *threshold)
        else {
            return;
        };
        self.idle_prods += 1;
        self.show_local_line(line, "tsundere", cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use std::time::Duration;

    #[test]
    fn farewell_holds_close_until_audio_ends_or_timeout() {
        assert!(!farewell_can_close(Duration::ZERO, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, false, false));
        assert!(!farewell_can_close(CLOSE_FAREWELL_MIN, true, true));
        assert!(farewell_can_close(CLOSE_FAREWELL_MIN, true, false));
        assert!(farewell_can_close(CLOSE_FAREWELL_MAX, false, true));
    }
}
