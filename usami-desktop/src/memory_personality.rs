//! Gerenciamento assíncrono de Memória e Personalidade com o core Usami.

use crate::UsamiApp;
use crate::api;
use crate::api::tokio_runtime;
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

    pub(crate) fn delete_memory_fact(
        &mut self,
        section: String,
        fact: String,
        cx: &mut Context<Self>,
    ) {
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

    pub(crate) fn apply_personality_preset(
        &mut self,
        preset_id: &'static str,
        preset_label: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            cx.notify();
            return;
        }
        self.personality_loading = true;
        self.personality_status = None;
        self.personality_error = None;
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.apply_personality_preset(preset_id).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.personality_data = Some(resp.personality);
                                    view.personality_status =
                                        Some(format!("Preset aplicado: {preset_label}.").into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error =
                                        Some(format!("Falha ao aplicar preset: {err}").into());
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

    pub(crate) fn pick_character_card_prompt(&mut self, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            cx.notify();
            return;
        }
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Selecione um Cartão de Companheira (.png ou .json)".into()),
        });

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(path) = paths.into_iter().next() {
                        let _ = cx.update(|cx: &mut App| {
                            if let Some(this) = this.upgrade() {
                                this.update(cx, |view, cx| {
                                    view.import_character_card_from_path(path, cx);
                                });
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn import_character_card_from_path(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if ext != "png" && ext != "json" {
            self.personality_error = Some("Selecione um arquivo de cartão .png ou .json.".into());
            self.personality_status = None;
            cx.notify();
            return;
        }

        let is_png = ext == "png";
        self.personality_loading = true;
        self.personality_error = None;
        self.personality_status = Some("Importando cartão de companheira...".into());
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let bytes_res = cx
                    .background_executor()
                    .spawn(async move { std::fs::read(&path).map_err(|e| e.to_string()) })
                    .await;
                let res = match bytes_res {
                    Ok(bytes) => api.import_character_card(bytes, is_png, true).await,
                    Err(err) => Err(format!("Falha ao ler arquivo: {err}")),
                };

                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match res {
                                Ok(imported) => {
                                    view.personality_status = Some(
                                        format!(
                                            "Cartão '{}' importado ({}) e ativado!",
                                            imported.character_id, imported.format
                                        )
                                        .into(),
                                    );
                                    view.personality_error = None;
                                    view.fetch_personality(cx);
                                }
                                Err(err) => {
                                    view.personality_error = Some(err.into());
                                    view.personality_status = None;
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

    pub(crate) fn export_active_character_card(
        &mut self,
        format: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.touch_ui(cx);
        if !self.api_client.has_token() {
            self.personality_error = Some("USAMI_API_TOKEN não configurado no ambiente".into());
            cx.notify();
            return;
        }
        self.personality_loading = true;
        self.personality_error = None;
        self.personality_status = Some(format!("Exportando cartão ({format})...").into());
        cx.notify();

        let api = Arc::clone(&self.api_client);
        let char_id = self.api_client.character_id();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.export_character_card(&char_id, format).await;
                let write_res = match res {
                    Ok(bytes) => {
                        let out_dir = std::env::var_os("USERPROFILE")
                            .map(|p| std::path::PathBuf::from(p).join("Downloads"))
                            .filter(|p| p.exists())
                            .unwrap_or_else(std::env::temp_dir);
                        let filename = format!("{char_id}.card.{format}");
                        let out_path = out_dir.join(filename);
                        cx.background_executor()
                            .spawn(async move {
                                std::fs::write(&out_path, bytes)
                                    .map(|_| out_path.display().to_string())
                                    .map_err(|e| format!("Falha ao gravar arquivo: {e}"))
                            })
                            .await
                    }
                    Err(err) => Err(err),
                };

                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.personality_loading = false;
                            match write_res {
                                Ok(saved_path) => {
                                    view.personality_status =
                                        Some(format!("Cartão exportado em: {saved_path}").into());
                                    view.personality_error = None;
                                }
                                Err(err) => {
                                    view.personality_error = Some(err.into());
                                    view.personality_status = None;
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
