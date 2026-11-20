//! Gerenciamento assíncrono de Vínculo, Afeto, Humor e Atividade de Chat com o core.

use crate::UsamiApp;
use crate::api;
use crate::api::tokio_runtime;
use gpui::*;
use std::sync::Arc;
use tokio::sync::oneshot;

impl UsamiApp {
    pub(crate) fn apply_bond(&mut self, bond: api::BondData) {
        self.bond_level = bond.summary.level.max(1);
        self.bond_exp_pct = (bond.summary.progress.clamp(0.0, 1.0) * 100.0).round() as u32;
        self.bond = Some(bond);
    }

    pub(crate) fn fetch_bond(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_bond().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => {
                                    view.apply_bond(resp.bond);
                                    view.fetch_achievements(cx);
                                }
                                Err(error) => eprintln!("[Vínculo] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Lê a atividade ao vivo do turno em andamento.
    ///
    /// Só faz sentido enquanto `is_busy`: é o intervalo de 10-20 s em que o
    /// usuário não sabe se a Usami está pensando, executando uma ferramenta
    /// ou travada. `active_tool` carrega o nome da ferramenta em execução.
    pub(crate) fn fetch_chat_activity(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || self.activity_loading {
            return;
        }
        self.activity_loading = true;
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_chat_activity().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.activity_loading = false;
                            if let Ok(activity) = res {
                                let tool = activity.tool.clone().filter(|t| !t.is_empty());
                                let phase = activity.phase.clone();
                                let changed = view.active_tool != tool
                                    || view.chat_activity.as_ref().and_then(|a| a.phase.clone())
                                        != phase;
                                view.active_tool = tool;
                                view.chat_activity = if activity.active {
                                    Some(activity)
                                } else {
                                    None
                                };
                                if changed {
                                    cx.notify();
                                }
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn fetch_achievements(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_achievements().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.achievements = resp.achievements,
                                Err(error) => eprintln!("[Conquistas] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Busca o humor persistido no core (`GET /api/mood`) sem bloquear a UI.
    pub(crate) fn fetch_mood(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || self.mood_loading {
            return;
        }
        self.mood_loading = true;
        let had_mood = self.mood.is_some();
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_mood().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.mood_loading = false;
                            match res {
                                Ok(resp) => {
                                    if !had_mood && resp.mood.vrm_emotion != "neutral" {
                                        view.vrm_controller.set_emotion_with_options(
                                            &resp.mood.vrm_emotion,
                                            8.0,
                                            false,
                                        );
                                    }
                                    #[cfg(target_os = "windows")]
                                    crate::presence::set_emotion_icon(&resp.mood.vrm_emotion);
                                    view.mood = Some(resp.mood);
                                }
                                Err(error) => eprintln!("[Humor] indisponível: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Aplica uma expressão no avatar ao vivo e registra o ajuste de humor no core.
    pub(crate) fn trigger_mood_preview(
        &mut self,
        emotion: &'static str,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.vrm_controller
            .set_emotion_with_options(emotion, 8.0, false);
        self.last_emotion = emotion.into();
        #[cfg(target_os = "windows")]
        crate::presence::set_emotion_icon(emotion);
        self.poke(cx);
        cx.notify();

        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.post_mood_event(emotion, Some(reason)).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            match res {
                                Ok(resp) => view.mood = Some(resp.mood),
                                Err(error) => eprintln!("[Humor] falha ao atualizar: {error}"),
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Mesma regra do web: cada mensagem rende XP, mas quem soma é o core.
    pub(crate) fn award_bond_chat(
        &mut self,
        content: String,
        spoken: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.api_client.has_token() {
            return;
        }
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.award_bond_chat(&content, spoken).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| match res {
                            Ok(resp) => {
                                view.apply_bond(resp.bond);
                                cx.notify();
                            }
                            Err(error) => eprintln!("[Vínculo] evento de chat falhou: {error}"),
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Chamada de voz rende companhia por minuto inteiro, como no web.
    pub(crate) fn award_bond_call(&mut self, minutes: u32, cx: &mut Context<Self>) {
        if !self.api_client.has_token() || minutes == 0 {
            return;
        }
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.award_bond_call(minutes).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| match res {
                            Ok(resp) => {
                                view.apply_bond(resp.bond);
                                cx.notify();
                            }
                            Err(error) => eprintln!("[Vínculo] evento de chamada falhou: {error}"),
                        });
                    }
                });
            }
        })
        .detach();
    }
}
