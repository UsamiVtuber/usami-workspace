//! Gerenciamento assíncrono de Memória e Personalidade com o core Usami.

use crate::api;
use crate::api::tokio_runtime;
use crate::UsamiApp;
use gpui::*;
use std::sync::Arc;
use tokio::sync::oneshot;

impl UsamiApp {
    pub(crate) fn fetch_memory(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.memory_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.memory_loading = true;
        self.memory_error = None;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_memory().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(ref aff) = resp.memory.affinity {
                                        view.server_affinity = Some(aff.score);
                                    }
                                    view.memory_data = Some(resp.memory);
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn delete_memory_fact(&mut self, section: String, fact: String, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "fact".into(),
            confirm: "APAGAR".into(),
            section: Some(section),
            fact: Some(fact),
            date: None,
        };
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.forget_memory(&payload).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    }
                                    view.memory_status = Some("Fato removido com sucesso.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao excluir: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn delete_memory_all(&mut self, cx: &mut Context<Self>) {
        if !self.memory_confirm_active {
            return;
        }
        self.memory_loading = true;
        self.memory_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::MemoryForgetRequest {
            kind: "all".into(),
            confirm: "APAGAR".into(),
            section: None,
            fact: None,
            date: None,
        };
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.forget_memory(&payload).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.memory_loading = false;
                            match res {
                                Ok(resp) => {
                                    if let Some(mem) = resp.memory {
                                        view.memory_data = Some(mem);
                                    } else {
                                        view.memory_data = Some(api::MemoryData::default());
                                    }
                                    view.memory_confirm_active = false;
                                    view.memory_status =
                                        Some("Toda a memória foi redefinida.".into());
                                    view.memory_error = None;
                                }
                                Err(err) => {
                                    view.memory_error =
                                        Some(format!("Falha ao apagar tudo: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn fetch_personality(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            return;
        }
        self.personality_loading = true;
        self.personality_error = None;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.get_personality().await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error = Some(err.into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn adjust_trait(&mut self, trait_name: &'static str, delta: i32) {
        if let Some(ref mut data) = self.personality_data {
            let val = match trait_name {
                "warmth" => &mut data.traits.warmth,
                "curiosity" => &mut data.traits.curiosity,
                "playfulness" => &mut data.traits.playfulness,
                "pride" => &mut data.traits.pride,
                _ => return,
            };
            let new_val = (*val as i32 + delta).clamp(0, 100) as u32;
            *val = new_val;
        }
    }

    pub(crate) fn save_personality(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.personality_data.clone() else {
            return;
        };
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: Some(data.traits),
            custom_prompt: Some(data.custom_prompt),
            reset: None,
        };
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.update_personality(&payload).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade salva no servidor!".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao salvar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn reset_personality(&mut self, cx: &mut Context<Self>) {
        self.personality_loading = true;
        self.personality_status = None;
        let api = Arc::clone(&self.api_client);
        let payload = api::PersonalityUpdateRequest {
            traits: None,
            custom_prompt: None,
            reset: Some(true),
        };
        let (tx, rx) = oneshot::channel();
        tokio_runtime().spawn(async move {
            let res = api.update_personality(&payload).await;
            let _ = tx.send(res);
        });
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = rx.await.expect("Tokio task dropped without sending result");
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some("Personalidade restaurada para o padrão (50).".into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao restaurar: {err}").into());
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }
}
