//! Central do Agente (Central de Inteligência): visualização e operações.
//!
//! Reúne abas de Skills, Execuções, Permissões, Confirmações e Sugestões.

use crate::agent_center_model::{
    classify_execution_status, collect_skill_reasons, format_duration_ms, format_execution_time,
    format_relevance_pct, group_skills_by_state, group_tools_by_namespace, is_local_quiet_hours_now,
    origin_label, pending_confirmations_count, pending_suggestions_count, permission_level_label,
    state_label, suggestion_kind_label, suggestion_status_label, summarize_executions,
    AgentCenterTab, ExecFilter, ExecutionTone, SkillFilter,
};
use crate::api;
use crate::theme;
use crate::ui::widgets::settings_hint;
use crate::UsamiApp;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;

impl UsamiApp {
    /// Consulta estado completo da Central do Agente (`/api/agent/*`) sem bloquear a UI.
    pub(crate) fn fetch_agent_center(&mut self, cx: &mut Context<Self>) {
        if !self.api_client.has_token() {
            self.agent_skills_loading = false;
            self.agent_executions_loading = false;
            self.agent_permissions_loading = false;
            self.agent_suggestions_loading = false;
            self.agent_skills_loaded = true;
            self.agent_executions_loaded = true;
            self.agent_permissions_loaded = true;
            self.agent_suggestions_loaded = true;
            let msg: SharedString =
                "Token ausente (USAMI_API_TOKEN) para consultar o core.".into();
            self.agent_skills_error = Some(msg.clone());
            self.agent_executions_error = Some(msg.clone());
            self.agent_permissions_error = Some(msg.clone());
            self.agent_suggestions_error = Some(msg);
            cx.notify();
            return;
        }
        if !self.agent_skills_loading {
            self.agent_skills_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_skills().await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_skills_loading = false;
                                view.agent_skills_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_skills = resp.skills;
                                        view.agent_skills_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_skills_error = Some(err.into());
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
        if !self.agent_executions_loading {
            self.agent_executions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_executions(50).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_executions_loading = false;
                                view.agent_executions_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_executions = resp.executions;
                                        view.agent_executions_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_executions_error = Some(err.into());
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
        if !self.agent_permissions_loading {
            self.agent_permissions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let tools_res = api.get_agent_self_tools().await;
                    let perms_res = api.get_agent_permissions().await;
                    let props_res = api.get_skill_proposals().await;
                    let plans_res = api.get_agent_plans(20).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_permissions_loading = false;
                                view.agent_permissions_loaded = true;
                                let mut first_err: Option<String> = None;
                                match tools_res {
                                    Ok(resp) => view.agent_tools = resp.self_data.tools,
                                    Err(err) => first_err = Some(err),
                                }
                                match perms_res {
                                    Ok(resp) => view.agent_permissions = resp.revocations,
                                    Err(err) if first_err.is_none() => first_err = Some(err),
                                    Err(_) => {}
                                }
                                if let Ok(resp) = props_res {
                                    view.agent_proposals = resp.proposals;
                                }
                                if let Ok(resp) = plans_res {
                                    view.agent_plans = resp.plans;
                                }
                                view.agent_permissions_error = first_err.map(Into::into);
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        if !self.agent_suggestions_loading {
            self.agent_suggestions_loading = true;
            let api = Arc::clone(&self.api_client);
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let res = api.get_agent_suggestions(false, 50).await;
                    let _ = cx.update(|cx: &mut App| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |view, cx| {
                                view.agent_suggestions_loading = false;
                                view.agent_suggestions_loaded = true;
                                match res {
                                    Ok(resp) => {
                                        view.agent_suggestions = resp.suggestions;
                                        view.agent_suggestions_unanswered = resp.unanswered_count;
                                        view.agent_suggestions_error = None;
                                    }
                                    Err(err) => {
                                        view.agent_suggestions_error = Some(err.into());
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
        cx.notify();
    }

    fn revoke_or_restore_permission(
        &mut self,
        tool: String,
        revoke: bool,
        reason: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.agent_permission_busy.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para alterar permissões.".into());
            cx.notify();
            return;
        }
        self.agent_permission_busy = Some(tool.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let action = if revoke { "revoke" } else { "restore" };
        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api
                    .set_tool_permission(&tool, action, reason.as_deref())
                    .await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_permission_busy = None;
                            match res {
                                Ok(resp) => {
                                    view.agent_permissions = resp.revocations;
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Permissão \"{tool}\" {}.",
                                            if revoke { "revogada" } else { "restaurada" }
                                        )
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
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

    fn approve_pending_skill(&mut self, skill_id: String, cx: &mut Context<Self>) {
        if self.agent_confirm_busy.is_some() || !self.agent_confirm_armed {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para confirmar instalação.".into());
            cx.notify();
            return;
        }
        // Exige novo armamento explícito para a próxima confirmação e mantém o
        // token de uso único estritamente efêmero dentro desta tarefa assíncrona.
        self.agent_confirm_armed = false;
        self.agent_confirm_busy = Some(skill_id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let step = match api.create_skill_confirmation(&skill_id).await {
                    Ok(conf) => {
                        api.install_skill_with_confirmation(&skill_id, &conf.confirmation.token)
                            .await
                    }
                    Err(err) => Err(err),
                };
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_confirm_busy = None;
                            match step {
                                Ok(resp) => {
                                    let label = resp
                                        .skill
                                        .as_ref()
                                        .and_then(|s| s.name.as_deref())
                                        .unwrap_or(&skill_id);
                                    view.agent_action_notice = Some(
                                        format!("Skill \"{label}\" aprovada e instalada.").into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
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

    fn deny_pending_skill(&mut self, skill_id: String, cx: &mut Context<Self>) {
        self.agent_confirm_armed = false;
        self.revoke_or_restore_permission(
            format!("skill.{skill_id}"),
            true,
            Some("Negada na fila de confirmações".to_string()),
            cx,
        );
    }

    fn request_level4_tool_confirmation(&mut self, tool_name: String, cx: &mut Context<Self>) {
        if self.agent_confirm_busy.is_some() || !self.agent_confirm_armed {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para emitir confirmação.".into());
            cx.notify();
            return;
        }
        self.agent_confirm_armed = false;
        self.agent_confirm_busy = Some(tool_name.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.create_tool_confirmation(&tool_name).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_confirm_busy = None;
                            match res {
                                Ok(_) => {
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Confirmação de uso único emitida no core para \"{tool_name}\" (token mantido oculto)."
                                        )
                                        .into(),
                                    );
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
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

    fn evaluate_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.agent_suggestions_loading {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para verificar sugestões.".into());
            cx.notify();
            return;
        }
        self.agent_suggestions_loading = true;
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_agent_suggestions(true, 50).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_suggestions_loading = false;
                            view.agent_suggestions_loaded = true;
                            match res {
                                Ok(resp) => {
                                    let pending_now =
                                        pending_suggestions_count(&resp.suggestions);
                                    view.agent_suggestions = resp.suggestions;
                                    view.agent_suggestions_unanswered = resp.unanswered_count;
                                    view.agent_suggestions_error = None;
                                    view.agent_action_notice = Some(
                                        if pending_now > 0 {
                                            format!("{pending_now} sugestão(ões) pendente(s).")
                                        } else {
                                            "Nenhuma nova sugestão contextual no momento (respeitando DND/cadência)."
                                                .to_string()
                                        }
                                        .into(),
                                    );
                                }
                                Err(err) => {
                                    view.agent_suggestions_error = Some(err.into());
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

    fn respond_suggestion(&mut self, id: String, accept: bool, cx: &mut Context<Self>) {
        if self.agent_suggestion_busy.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para responder sugestão.".into());
            cx.notify();
            return;
        }
        self.agent_suggestion_busy = Some(id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = if accept {
                    api.accept_agent_suggestion(&id).await
                } else {
                    api.dismiss_agent_suggestion(&id, Some("Descartada pela interface"))
                        .await
                };
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_suggestion_busy = None;
                            match res {
                                Ok(resp) => {
                                    view.agent_action_notice = Some(
                                        if accept {
                                            match resp.plan {
                                                Some(plan) => format!(
                                                    "Sugestão aceita — plano criado ({}).",
                                                    plan.status
                                                ),
                                                None => "Sugestão aceita.".to_string(),
                                            }
                                        } else {
                                            "Sugestão descartada.".to_string()
                                        }
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
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

    fn fetch_daily_briefing(&mut self, cx: &mut Context<Self>) {
        if self.agent_briefing_loading {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_briefing_error =
                Some("Token ausente (USAMI_API_TOKEN) para gerar o briefing.".into());
            cx.notify();
            return;
        }
        self.agent_briefing_loading = true;
        self.agent_briefing_error = None;
        self.agent_action_notice = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.get_daily_briefing().await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_briefing_loading = false;
                            match res {
                                Ok(resp) => {
                                    view.agent_briefing = Some(resp.briefing);
                                    view.agent_briefing_error = None;
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_briefing_error = Some(err.into());
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

    fn toggle_agent_skill(&mut self, skill_id: String, enable: bool, cx: &mut Context<Self>) {
        if self.agent_skill_busy_id.is_some() {
            return;
        }
        if !self.api_client.has_token() {
            self.agent_action_error =
                Some("Token ausente (USAMI_API_TOKEN) para alterar skills.".into());
            cx.notify();
            return;
        }
        self.agent_skill_busy_id = Some(skill_id.clone());
        self.agent_action_notice = None;
        self.agent_action_error = None;
        self.poke(cx);
        cx.notify();

        let api = Arc::clone(&self.api_client);
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let res = api.toggle_skill_enabled(&skill_id, enable).await;
                let _ = cx.update(|cx: &mut App| {
                    if let Some(this) = this.upgrade() {
                        this.update(cx, |view, cx| {
                            view.agent_skill_busy_id = None;
                            match res {
                                Ok(resp) => {
                                    let label = resp
                                        .skill
                                        .as_ref()
                                        .and_then(|s| s.name.as_deref())
                                        .unwrap_or(&skill_id);
                                    view.agent_action_notice = Some(
                                        format!(
                                            "Skill \"{label}\" {}.",
                                            if enable { "ativada" } else { "desativada" }
                                        )
                                        .into(),
                                    );
                                    view.fetch_agent_center(cx);
                                }
                                Err(err) => {
                                    view.agent_action_error = Some(err.into());
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

    fn push_agent_feedback_banners(&self, mut section: Div) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        if let Some(err) = self.agent_action_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }
        if let Some(notice) = self.agent_action_notice.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.surface_soft)
                    .border_1()
                    .border_color(c.success_border)
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .child(notice),
            );
        }
        section
    }

    fn render_agent_permissions_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        if let Some(err) = self.agent_permissions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if self.agent_permissions_loading && !self.agent_permissions_loaded {
            return section.child(
                div()
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Carregando permissões e ferramentas…"),
            );
        }

        if !self.agent_permissions.is_empty() {
            let mut rev_box = div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(c.danger_muted_border)
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.err)
                        .child(format!(
                            "REVOGAÇÕES ATIVAS ({})",
                            self.agent_permissions.len()
                        )),
                );

            for rev in self.agent_permissions.clone() {
                let target = rev.tool.clone();
                let is_busy = self.agent_permission_busy.as_deref() == Some(target.as_str());
                let when = format_execution_time(&rev.revoked_at);
                let mut detail = Vec::new();
                if let Some(reason) = rev.reason.filter(|s| !s.trim().is_empty()) {
                    detail.push(reason);
                }
                if !when.is_empty() {
                    detail.push(when);
                }
                let target_click = target.clone();
                rev_box = rev_box.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .p(px(sp.s2))
                        .rounded(px(r.sm))
                        .bg(c.deep)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(target.clone()),
                                )
                                .when(!detail.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(detail.join(" · ")),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-perm-restore-{target}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(c.success_border)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.ok)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.revoke_or_restore_permission(
                                        target_click.clone(),
                                        false,
                                        None,
                                        cx,
                                    );
                                }))
                                .child(if is_busy { "…" } else { "Restaurar" }),
                        ),
                );
            }
            section = section.child(rev_box);
        }

        let groups = group_tools_by_namespace(&self.agent_tools);
        if groups.is_empty() && self.agent_permissions_loaded {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma ferramenta registrada no core no momento."),
            );
        }

        for group in groups {
            let wildcard = format!("{}.*", group.namespace);
            let ns_revoked = self
                .agent_permissions
                .iter()
                .any(|rev| rev.tool == wildcard);
            let ns_busy = self.agent_permission_busy.as_deref() == Some(wildcard.as_str());
            let wildcard_click = wildcard.clone();
            let next_ns_revoke = !ns_revoked;

            let mut group_card = div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if ns_revoked {
                    c.danger_muted_border
                } else {
                    c.line
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.lavender)
                                .child(format!(
                                    "INTEGRAÇÃO: {} ({})",
                                    group.namespace.to_uppercase(),
                                    group.tools.len()
                                )),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-ns-toggle-{wildcard}")))
                                .cursor_pointer()
                                .px(px(9.0))
                                .py(px(3.5))
                                .rounded(px(r.sm))
                                .bg(c.deep)
                                .border_1()
                                .border_color(if ns_revoked {
                                    c.success_border
                                } else {
                                    c.danger_muted_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if ns_revoked { c.ok } else { c.danger_muted })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let reason = if next_ns_revoke {
                                        Some(format!("Integração {wildcard_click} revogada no desktop"))
                                    } else {
                                        None
                                    };
                                    this.revoke_or_restore_permission(
                                        wildcard_click.clone(),
                                        next_ns_revoke,
                                        reason,
                                        cx,
                                    );
                                }))
                                .child(if ns_busy {
                                    "…".to_string()
                                } else if ns_revoked {
                                    format!("Restaurar {wildcard}")
                                } else {
                                    format!("Revogar {wildcard}")
                                }),
                        ),
                );

            for tool in group.tools {
                let tname = tool.name.clone();
                let is_busy = self.agent_permission_busy.as_deref() == Some(tname.as_str());
                let level_label = permission_level_label(tool.permission_level);
                let origin = origin_label(tool.origin.as_deref());
                let (status_text, status_color, status_border) = if tool.revoked {
                    ("Revogada", c.err, c.danger_muted_border)
                } else if tool.active {
                    ("Ativa", c.ok, c.success_border)
                } else {
                    ("Inativa", c.warn, c.accent_border)
                };
                let tname_click = tname.clone();
                let next_revoke = !tool.revoked;

                let mut item = div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s1_5))
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.deep)
                    .border_1()
                    .border_color(if tool.revoked {
                        c.danger_muted_border
                    } else {
                        c.line
                    })
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.pearl)
                                            .child(tname.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(format!("{origin} · {level_label}")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(status_border)
                                    .text_size(px(ty.xs))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(status_color)
                                    .child(status_text),
                            ),
                    );

                if !tool.description.trim().is_empty() {
                    item = item.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(tool.description),
                    );
                }
                if let Some(reason) = tool.revoked_reason.filter(|s| !s.trim().is_empty()) {
                    item = item.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(format!("Motivo: {reason}")),
                    );
                }

                item = item.child(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-tool-perm-{tname}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(if tool.revoked {
                                    c.success_border
                                } else {
                                    c.danger_muted_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if tool.revoked { c.ok } else { c.danger_muted })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let reason = if next_revoke {
                                        Some("Revogada manualmente no desktop".to_string())
                                    } else {
                                        None
                                    };
                                    this.revoke_or_restore_permission(
                                        tname_click.clone(),
                                        next_revoke,
                                        reason,
                                        cx,
                                    );
                                }))
                                .child(if is_busy {
                                    "Atualizando…"
                                } else if tool.revoked {
                                    "Restaurar"
                                } else {
                                    "Revogar"
                                }),
                        ),
                );

                group_card = group_card.child(item);
            }

            section = section.child(group_card);
        }

        section
    }

    fn render_agent_confirmations_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let armed = self.agent_confirm_armed;
        let busy_key = self.agent_confirm_busy.clone();
        let perm_busy = self.agent_permission_busy.clone();

        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        if let Some(err) = self.agent_permissions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        // Passo 1 de segurança (espelha o padrão de confirmação dupla da aba Memória).
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if armed { c.accent } else { c.line_strong })
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.muted)
                        .child("Aprovações usam token efêmero de uso único (CONFIRMAR) no core. Nenhum token é exibido ou armazenado na interface."),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed { c.warn } else { c.faint })
                                .child(if armed {
                                    "Armado para 1 confirmação"
                                } else {
                                    "Protegido — arme antes de aprovar"
                                }),
                        )
                        .child(
                            div()
                                .id("agent-confirm-arm-btn")
                                .cursor_pointer()
                                .px(px(11.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(if armed { c.surface_soft } else { c.accent_tint })
                                .border_1()
                                .border_color(if armed { c.warn } else { c.accent })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_confirm_armed = !this.agent_confirm_armed;
                                    this.poke(cx);
                                    cx.notify();
                                }))
                                .child(if armed {
                                    "Cancelar armamento"
                                } else {
                                    "Armar confirmação"
                                }),
                        ),
                ),
        );

        // Junta skills em estado pending + proposals pending sem duplicar ids.
        struct PendingSkillEntry {
            skill_id: String,
            title: String,
            version: Option<String>,
            benefit: Option<String>,
            risks: Option<String>,
            dependencies: Vec<String>,
            max_level: u8,
        }
        let mut pending_entries: Vec<PendingSkillEntry> = Vec::new();
        for skill in &self.agent_skills {
            if skill.state.as_deref() == Some("pending") {
                let proposal = self
                    .agent_proposals
                    .iter()
                    .find(|p| p.skill_id == skill.id && p.status == "pending");
                pending_entries.push(PendingSkillEntry {
                    skill_id: skill.id.clone(),
                    title: skill
                        .name
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| skill.id.clone()),
                    version: skill.version.clone(),
                    benefit: proposal
                        .map(|p| p.benefit.clone())
                        .or_else(|| skill.description.clone()),
                    risks: proposal.map(|p| p.risks.clone()),
                    dependencies: proposal
                        .map(|p| p.dependencies.clone())
                        .unwrap_or_default(),
                    max_level: skill.max_permission_level.unwrap_or(1),
                });
            }
        }
        for prop in &self.agent_proposals {
            if prop.status == "pending"
                && !pending_entries.iter().any(|e| e.skill_id == prop.skill_id)
            {
                let max_level = prop.permissions.iter().copied().max().unwrap_or(1);
                pending_entries.push(PendingSkillEntry {
                    skill_id: prop.skill_id.clone(),
                    title: prop.skill_id.clone(),
                    version: prop.version.clone(),
                    benefit: Some(prop.benefit.clone()),
                    risks: Some(prop.risks.clone()),
                    dependencies: prop.dependencies.clone(),
                    max_level,
                });
            }
        }

        let pending_plans: Vec<api::AgentPlan> = self
            .agent_plans
            .iter()
            .filter(|plan| plan.status == "awaiting_approval")
            .cloned()
            .collect();
        let level4_tools: Vec<api::SelfToolSummary> = self
            .agent_tools
            .iter()
            .filter(|tool| tool.permission_level == 4)
            .cloned()
            .collect();

        if pending_entries.is_empty() && pending_plans.is_empty() && level4_tools.is_empty() {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma confirmação pendente no momento."),
            );
        }

        if !pending_entries.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "SKILLS PENDENTES DE APROVAÇÃO ({})",
                        pending_entries.len()
                    )),
            );

            for entry in pending_entries {
                let sid = entry.skill_id.clone();
                let is_busy = busy_key.as_deref() == Some(sid.as_str())
                    || perm_busy.as_deref() == Some(format!("skill.{sid}").as_str());
                let sid_approve = sid.clone();
                let sid_deny = sid.clone();
                let ver = entry.version.unwrap_or_default();
                let mut card = div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.accent_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.md))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(if ver.is_empty() {
                                        entry.title
                                    } else {
                                        format!("{} v{ver}", entry.title)
                                    }),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_full()
                                    .bg(c.deep)
                                    .border_1()
                                    .border_color(c.accent_border)
                                    .text_size(px(ty.xs))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.warn)
                                    .child(permission_level_label(entry.max_level)),
                            ),
                    );

                if let Some(benefit) = entry.benefit.filter(|s| !s.trim().is_empty()) {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(format!("Benefício: {benefit}")),
                    );
                }
                if let Some(risks) = entry.risks.filter(|s| !s.trim().is_empty()) {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.warn)
                            .child(format!("Riscos: {risks}")),
                    );
                }
                if !entry.dependencies.is_empty() {
                    card = card.child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.muted)
                            .child(format!("Dependências: {}", entry.dependencies.join(", "))),
                    );
                }

                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-skill-deny-{sid}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.danger_muted_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.danger_muted)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.deny_pending_skill(sid_deny.clone(), cx);
                                }))
                                .child("Negar"),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-skill-approve-{sid}")))
                                .when(armed && !is_busy, |el| el.cursor_pointer())
                                .px(px(11.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(if armed { c.accent_tint } else { c.deep })
                                .border_1()
                                .border_color(if armed { c.accent } else { c.line })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed { c.pearl } else { c.faint })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.approve_pending_skill(sid_approve.clone(), cx);
                                }))
                                .child(if is_busy {
                                    "Confirmando…"
                                } else {
                                    "Aprovar e instalar"
                                }),
                        ),
                );

                section = section.child(card);
            }
        }

        if !pending_plans.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "PLANOS AGUARDANDO APROVAÇÃO ({})",
                        pending_plans.len()
                    )),
            );

            for plan in pending_plans {
                let mut steps_summary = Vec::new();
                for (idx, step) in plan.steps.iter().enumerate() {
                    let label = step
                        .tool
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(step.title.as_str());
                    let line = if let Some(lvl) = step.permission_level {
                        format!("Passo {}: {label} (nível {lvl})", idx + 1)
                    } else {
                        format!("Passo {}: {label}", idx + 1)
                    };
                    steps_summary.push(line);
                }
                let when = format_execution_time(&plan.created_at);
                section = section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(sp.s1_5))
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .text_size(px(ty.sm))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .child(plan.goal),
                        )
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .text_color(c.muted)
                                .child(format!(
                                    "ID {} · {} · {}",
                                    plan.id,
                                    plan.origin.unwrap_or_else(|| "core".into()),
                                    when
                                )),
                        )
                        .when(!steps_summary.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.pearl)
                                    .child(steps_summary.join(" · ")),
                            )
                        }),
                );
            }
        }

        if !level4_tools.is_empty() {
            section = section.child(
                div()
                    .text_size(px(ty.xs))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c.lavender)
                    .child(format!(
                        "FERRAMENTAS SENSÍVEIS — NÍVEL 4 ({})",
                        level4_tools.len()
                    )),
            );

            for tool in level4_tools {
                let tname = tool.name.clone();
                let is_busy = busy_key.as_deref() == Some(tname.as_str());
                let tname_click = tname.clone();
                section = section.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .p(px(sp.s3))
                        .rounded(px(r.md))
                        .bg(c.card_bg)
                        .border_1()
                        .border_color(c.line)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(ty.sm))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(tname.clone()),
                                )
                                .when(!tool.description.trim().is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .text_color(c.muted)
                                            .child(tool.description),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-tool-confirm-{tname}")))
                                .when(armed && !is_busy && !tool.revoked, |el| el.cursor_pointer())
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(if armed && !tool.revoked {
                                    c.accent_tint
                                } else {
                                    c.deep
                                })
                                .border_1()
                                .border_color(if armed && !tool.revoked {
                                    c.accent
                                } else {
                                    c.line
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if armed && !tool.revoked {
                                    c.pearl
                                } else {
                                    c.faint
                                })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if !tool.revoked {
                                        this.request_level4_tool_confirmation(
                                            tname_click.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child(if tool.revoked {
                                    "Revogada"
                                } else if is_busy {
                                    "Emitindo…"
                                } else {
                                    "Preparar uso único"
                                }),
                        ),
                );
            }
        }

        section
    }

    fn render_agent_suggestions_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let quiet_now = is_local_quiet_hours_now();
        let unanswered = self.agent_suggestions_unanswered;
        let mut section = self.push_agent_feedback_banners(div().flex().flex_col().gap(px(sp.s3)));

        // Barra DND / Throttle + Ações explícitas.
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(sp.s2))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(c.line)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(sp.s1_5))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.5))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(if quiet_now {
                                    c.accent_border
                                } else {
                                    c.success_border
                                })
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(if quiet_now { c.warn } else { c.ok })
                                .child(if quiet_now {
                                    "Modo silencioso (23:30–08:30)"
                                } else {
                                    "Janela ativa"
                                }),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.5))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .text_color(if unanswered >= 2 { c.warn } else { c.muted })
                                .child(format!("Sem resposta: {unanswered}/2")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .id("agent-eval-suggestions-btn")
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(c.surface_soft)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.primary)
                                .hover(|s| s.border_color(c.primary))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.evaluate_suggestions(cx);
                                }))
                                .child(if self.agent_suggestions_loading {
                                    "Verificando…"
                                } else {
                                    "Verificar sugestões"
                                }),
                        )
                        .child(
                            div()
                                .id("agent-briefing-btn")
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(r.sm))
                                .bg(c.accent_tint)
                                .border_1()
                                .border_color(c.accent)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.fetch_daily_briefing(cx);
                                }))
                                .child(if self.agent_briefing_loading {
                                    "Gerando briefing…"
                                } else {
                                    "Gerar Briefing de Hoje"
                                }),
                        ),
                ),
        );

        if let Some(err) = self.agent_briefing_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if let Some(briefing) = self.agent_briefing.clone() {
            let counts = briefing.counts;
            let source = briefing
                .narrative_source
                .unwrap_or_else(|| "template".to_string());
            let body = if briefing.empty && briefing.text.trim().is_empty() {
                "Nenhum item real encontrado no vault para o briefing de hoje.".to_string()
            } else {
                briefing.text
            };
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(sp.s2))
                    .p(px(sp.s3))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.accent_border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.sm))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.pearl)
                                    .child(format!("Briefing · {}", briefing.date)),
                            )
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(source),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.muted)
                            .child(format!(
                                "{} compromissos · {} tarefas · {} projetos · {} pendentes · {} planos",
                                counts.commitments,
                                counts.priority_tasks,
                                counts.projects,
                                counts.pending,
                                counts.pending_plans
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(body),
                    ),
            );
        }

        if let Some(err) = self.agent_suggestions_error.clone() {
            section = section.child(
                div()
                    .p(px(sp.s2_5))
                    .rounded(px(r.sm))
                    .bg(c.danger_muted_bg)
                    .border_1()
                    .border_color(c.danger_muted_border)
                    .text_size(px(ty.xs))
                    .text_color(c.danger_muted)
                    .child(err),
            );
        }

        if self.agent_suggestions_loading && !self.agent_suggestions_loaded {
            return section.child(
                div()
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Carregando sugestões contextuais…"),
            );
        }

        if self.agent_suggestions.is_empty() && self.agent_suggestions_loaded {
            return section.child(
                div()
                    .p(px(sp.s4))
                    .rounded(px(r.md))
                    .bg(c.card_bg)
                    .border_1()
                    .border_color(c.line)
                    .text_size(px(ty.sm))
                    .text_color(c.muted)
                    .child("Nenhuma sugestão contextual disponível no momento."),
            );
        }

        let mut ordered = self.agent_suggestions.clone();
        ordered.sort_by_key(|s| if s.status == "pending" { 0 } else { 1 });

        for item in ordered {
            let sid = item.id.clone();
            let is_pending = item.status == "pending";
            let is_busy = self.agent_suggestion_busy.as_deref() == Some(sid.as_str());
            let kind_lbl = suggestion_kind_label(&item.kind);
            let status_lbl = suggestion_status_label(&item.status);
            let rel_lbl = format_relevance_pct(item.relevance);
            let when = format_execution_time(&item.created_at);
            let status_color = match item.status.as_str() {
                "accepted" => c.ok,
                "dismissed" | "expired" => c.muted,
                _ => c.warn,
            };

            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(sp.s1_5))
                .p(px(sp.s3))
                .rounded(px(r.md))
                .bg(c.card_bg)
                .border_1()
                .border_color(if is_pending { c.accent_border } else { c.line })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(sp.s2))
                        .child(
                            div()
                                .text_size(px(ty.xs))
                                .text_color(c.muted)
                                .child(format!("{kind_lbl} · Relevância {rel_lbl} · {when}")),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded_full()
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(status_color)
                                .child(status_lbl),
                        ),
                )
                .child(
                    div()
                        .text_size(px(ty.sm))
                        .font_weight(FontWeight::BOLD)
                        .text_color(c.pearl)
                        .child(item.title),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.pearl)
                        .child(item.summary),
                )
                .child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.muted)
                        .child(format!("Objetivo sugerido: {}", item.recommended_goal)),
                );

            if let Some(plan_id) = item.plan_id.filter(|s| !s.trim().is_empty()) {
                card = card.child(
                    div()
                        .text_size(px(ty.xs))
                        .text_color(c.ok)
                        .child(format!("Plano associado: {plan_id}")),
                );
            }

            if is_pending {
                let sid_accept = sid.clone();
                let sid_dismiss = sid.clone();
                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(sp.s2))
                        .pt(px(sp.s1))
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-sugg-dismiss-{sid}")))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line_strong)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c.muted)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.respond_suggestion(sid_dismiss.clone(), false, cx);
                                }))
                                .child(if is_busy { "…" } else { "Descartar" }),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("agent-sugg-accept-{sid}")))
                                .cursor_pointer()
                                .px(px(11.0))
                                .py(px(4.5))
                                .rounded(px(r.sm))
                                .bg(c.accent_tint)
                                .border_1()
                                .border_color(c.accent)
                                .text_size(px(ty.xs))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c.pearl)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.respond_suggestion(sid_accept.clone(), true, cx);
                                }))
                                .child(if is_busy { "…" } else { "Aceitar" }),
                        ),
                );
            }

            section = section.child(card);
        }

        section
    }

    pub(crate) fn render_agent_center_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let r = t.radius;

        let active_tab = self.agent_tab;
        let skill_filter = self.agent_skill_filter;
        let exec_filter = self.agent_exec_filter;
        let loading = self.agent_skills_loading
            || self.agent_executions_loading
            || self.agent_permissions_loading
            || self.agent_suggestions_loading;
        let busy_id = self.agent_skill_busy_id.clone();

        let skills_count = self.agent_skills.len();
        let execs_count = self.agent_executions.len();
        let tools_count = self.agent_tools.len();
        let pending_confirm_count = pending_confirmations_count(
            &self.agent_skills,
            &self.agent_proposals,
            &self.agent_plans,
        );
        let pending_sugg_count = pending_suggestions_count(&self.agent_suggestions);
        let (buckets, invalid_skills) = group_skills_by_state(&self.agent_skills);
        let (exec_total, exec_verified, exec_failed) = summarize_executions(&self.agent_executions);

        let pill = |id: &'static str, label: String, active: bool| {
            div()
                .id(id)
                .cursor_pointer()
                .px(px(10.0))
                .py(px(4.5))
                .rounded_full()
                .border_1()
                .bg(if active { c.accent_tint } else { c.surface_soft })
                .border_color(if active { c.accent } else { c.line_strong })
                .text_size(px(ty.xs))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if active { c.pearl } else { c.muted })
                .hover(|s| s.border_color(c.primary).text_color(c.pearl))
                .child(label)
        };

        let body_el: AnyElement = match active_tab {
            AgentCenterTab::Skills => {
                let installed_len = buckets[0].skills.len();
                let available_len = buckets[1].skills.len();
                let pending_len = buckets[2].skills.len();
                let disabled_len = buckets[3].skills.len();

                let mut section = div().flex().flex_col().gap(px(sp.s3));

                section = section.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(sp.s1_5))
                        .child(
                            pill(
                                "agent-skill-filter-all",
                                format!("Todas ({skills_count})"),
                                skill_filter == SkillFilter::All,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::All;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-installed",
                                format!("Instaladas ({installed_len})"),
                                skill_filter == SkillFilter::Installed,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Installed;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-available",
                                format!("Disponíveis ({available_len})"),
                                skill_filter == SkillFilter::Available,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Available;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-pending",
                                format!("Pendentes ({pending_len})"),
                                skill_filter == SkillFilter::Pending,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Pending;
                                this.poke(cx);
                                cx.notify();
                            })),
                        )
                        .child(
                            pill(
                                "agent-skill-filter-disabled",
                                format!("Desativadas ({disabled_len})"),
                                skill_filter == SkillFilter::Disabled,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_skill_filter = SkillFilter::Disabled;
                                this.poke(cx);
                                cx.notify();
                            })),
                        ),
                );

                if let Some(err) = self.agent_action_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }
                if let Some(notice) = self.agent_action_notice.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.success_border)
                            .text_size(px(ty.xs))
                            .text_color(c.pearl)
                            .child(notice),
                    );
                }
                if let Some(err) = self.agent_skills_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }

                if !self.agent_skills_loaded {
                    section = section.child(settings_hint("Lendo catálogo de skills…"));
                } else {
                    for bucket in buckets
                        .into_iter()
                        .filter(|b| skill_filter.matches_state(b.key))
                    {
                        let count = bucket.skills.len();
                        let mut group_el = div()
                            .flex()
                            .flex_col()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .mt(px(sp.s1))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_family(theme::FONT_DISPLAY)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(bucket.label.to_uppercase()),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(1.5))
                                            .rounded_full()
                                            .bg(c.card_bg)
                                            .border_1()
                                            .border_color(c.line)
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c.primary)
                                            .child(format!("{count}")),
                                    ),
                            );

                        if bucket.skills.is_empty() {
                            group_el = group_el.child(
                                div()
                                    .p(px(sp.s2_5))
                                    .rounded(px(r.sm))
                                    .bg(c.deep)
                                    .border_1()
                                    .border_color(c.line)
                                    .text_size(px(ty.xs))
                                    .text_color(c.muted)
                                    .child(bucket.empty_message),
                            );
                        } else {
                            for skill in bucket.skills {
                                let sid = skill.id.clone();
                                let title = skill.name.clone().unwrap_or_else(|| sid.clone());
                                let st = skill.state.as_deref().unwrap_or("unknown");
                                let version = skill.version.as_deref().unwrap_or("0.0.0");
                                let origin = origin_label(skill.origin.as_deref());
                                let perm = skill.max_permission_level.unwrap_or(1);
                                let reasons = collect_skill_reasons(&skill);
                                let badge_color = match st {
                                    "installed" => c.ok,
                                    "available" => c.primary,
                                    "pending" => c.warn,
                                    _ => c.muted,
                                };
                                let badge_border = match st {
                                    "installed" => c.success_border,
                                    "available" => c.accent_border,
                                    _ => c.line_strong,
                                };
                                let is_busy = busy_id.as_deref() == Some(sid.as_str());
                                let can_disable = st == "installed";
                                let can_enable = st == "disabled";

                                let mut card = div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(sp.s2))
                                    .p(px(sp.s3))
                                    .rounded(px(r.md))
                                    .bg(c.card_bg)
                                    .border_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .flex()
                                            .items_start()
                                            .justify_between()
                                            .gap(px(sp.s2))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.md))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(c.pearl)
                                                            .child(title),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(ty.xs))
                                                            .text_color(c.muted)
                                                            .child(format!(
                                                                "{sid} · v{version} · {origin} · nível {perm}"
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px(px(8.0))
                                                    .py(px(2.5))
                                                    .rounded_full()
                                                    .bg(c.deep)
                                                    .border_1()
                                                    .border_color(badge_border)
                                                    .text_size(px(ty.xs))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(badge_color)
                                                    .child(state_label(st)),
                                            ),
                                    );

                                if let Some(desc) = skill.description.filter(|d| !d.trim().is_empty()) {
                                    card = card.child(
                                        div()
                                            .text_size(px(ty.sm))
                                            .text_color(c.pearl)
                                            .child(desc),
                                    );
                                }

                                let mut foot = div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(sp.s2))
                                    .child(
                                        div()
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if skill.activation.can_activate {
                                                c.ok
                                            } else {
                                                c.muted
                                            })
                                            .child(if skill.activation.can_activate {
                                                "Pronta para ativar"
                                            } else {
                                                "Não ativável agora"
                                            }),
                                    );

                                if can_disable || can_enable {
                                    let btn_id = format!("skill-toggle-{sid}");
                                    let skill_for_click = sid.clone();
                                    let next_enable = can_enable;
                                    let label = if is_busy {
                                        "Atualizando…"
                                    } else if can_enable {
                                        "Ativar"
                                    } else {
                                        "Desativar"
                                    };
                                    foot = foot.child(
                                        div()
                                            .id(SharedString::from(btn_id))
                                            .cursor_pointer()
                                            .px(px(10.0))
                                            .py(px(4.5))
                                            .rounded(px(r.sm))
                                            .bg(if can_enable {
                                                c.accent_tint
                                            } else {
                                                c.deep
                                            })
                                            .border_1()
                                            .border_color(if can_enable {
                                                c.accent
                                            } else {
                                                c.line_strong
                                            })
                                            .text_size(px(ty.xs))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c.pearl)
                                            .when(busy_id.is_some(), |el| el.opacity(0.55))
                                            .hover(|s| s.border_color(c.primary))
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.toggle_agent_skill(
                                                    skill_for_click.clone(),
                                                    next_enable,
                                                    cx,
                                                );
                                            }))
                                            .child(label),
                                    );
                                }

                                card = card.child(foot);

                                if !reasons.is_empty() {
                                    let mut reasons_el = div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .pt(px(2.0));
                                    for reason in reasons {
                                        reasons_el = reasons_el.child(
                                            div()
                                                .text_size(px(ty.xs))
                                                .text_color(c.primary)
                                                .child(format!("• {reason}")),
                                        );
                                    }
                                    card = card.child(reasons_el);
                                }

                                group_el = group_el.child(card);
                            }
                        }

                        section = section.child(group_el);
                    }

                    if !invalid_skills.is_empty() {
                        let mut inv_el = div()
                            .flex()
                            .flex_col()
                            .gap(px(sp.s2))
                            .child(
                                div()
                                    .text_size(px(ty.xs))
                                    .font_family(theme::FONT_DISPLAY)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.err)
                                    .child("MANIFESTOS INVÁLIDOS"),
                            );
                        for skill in invalid_skills {
                            let reasons = collect_skill_reasons(&skill);
                            let mut card = div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1_5))
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.danger_muted_border)
                                .child(
                                    div()
                                        .text_size(px(ty.md))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c.pearl)
                                        .child(skill.id),
                                );
                            for reason in reasons {
                                card = card.child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.err)
                                        .child(format!("• {reason}")),
                                );
                            }
                            inv_el = inv_el.child(card);
                        }
                        section = section.child(inv_el);
                    }
                }

                section.into_any_element()
            }
            AgentCenterTab::Executions => {
                let mut section = div().flex().flex_col().gap(px(sp.s3));

                section = section
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(sp.s3))
                            .p(px(sp.s2_5))
                            .rounded(px(r.md))
                            .bg(c.card_bg)
                            .border_1()
                            .border_color(c.line)
                            .text_size(px(ty.xs))
                            .child(
                                div()
                                    .text_color(c.pearl)
                                    .child(format!("Total: {exec_total}")),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.ok)
                                    .child(format!("Verificadas: {exec_verified}")),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c.err)
                                    .child(format!("Falhas: {exec_failed}")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(sp.s1_5))
                            .child(
                                pill(
                                    "agent-exec-filter-all",
                                    format!("Todas ({exec_total})"),
                                    exec_filter == ExecFilter::All,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::All;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            )
                            .child(
                                pill(
                                    "agent-exec-filter-verified",
                                    format!("Verificadas ({exec_verified})"),
                                    exec_filter == ExecFilter::Verified,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::Verified;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            )
                            .child(
                                pill(
                                    "agent-exec-filter-failed",
                                    format!("Falhas ({exec_failed})"),
                                    exec_filter == ExecFilter::Failed,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.agent_exec_filter = ExecFilter::Failed;
                                    this.poke(cx);
                                    cx.notify();
                                })),
                            ),
                    );

                if let Some(err) = self.agent_executions_error.clone() {
                    section = section.child(
                        div()
                            .p(px(sp.s2_5))
                            .rounded(px(r.sm))
                            .bg(c.danger_muted_bg)
                            .border_1()
                            .border_color(c.danger_muted_border)
                            .text_size(px(ty.xs))
                            .text_color(c.danger_muted)
                            .child(err),
                    );
                }

                if !self.agent_executions_loaded {
                    section = section.child(settings_hint("Lendo histórico de execuções…"));
                } else {
                    let visible: Vec<_> = self
                        .agent_executions
                        .iter()
                        .filter(|rec| match exec_filter {
                            ExecFilter::All => true,
                            ExecFilter::Verified => classify_execution_status(rec).verified,
                            ExecFilter::Failed => !classify_execution_status(rec).verified,
                        })
                        .cloned()
                        .collect();

                    if visible.is_empty() {
                        let empty_msg = if self.agent_executions.is_empty() {
                            "Nenhuma execução registrada ainda."
                        } else {
                            "Nenhuma execução corresponde ao filtro selecionado."
                        };
                        section = section.child(
                            div()
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.deep)
                                .border_1()
                                .border_color(c.line)
                                .text_size(px(ty.sm))
                                .text_color(c.muted)
                                .child(empty_msg),
                        );
                    } else {
                        for record in visible {
                            let status = classify_execution_status(&record);
                            let status_color = match status.tone {
                                ExecutionTone::Ok => c.ok,
                                ExecutionTone::Warn => c.warn,
                                ExecutionTone::Err => c.err,
                            };
                            let status_border = match status.tone {
                                ExecutionTone::Ok => c.success_border,
                                ExecutionTone::Warn => c.accent_border,
                                ExecutionTone::Err => c.danger_muted_border,
                            };
                            let when = format_execution_time(&record.started_at);
                            let dur = format_duration_ms(record.duration_ms);
                            let mut meta_parts = Vec::new();
                            if !when.is_empty() {
                                meta_parts.push(when);
                            }
                            if !dur.is_empty() {
                                meta_parts.push(dur);
                            }
                            if let Some(orig) = record.origin.as_deref() {
                                meta_parts.push(orig.to_string());
                            }
                            if let Some(perm) = record.permission_level {
                                meta_parts.push(format!("nível {perm}"));
                            }

                            let mut card = div()
                                .flex()
                                .flex_col()
                                .gap(px(sp.s1_5))
                                .p(px(sp.s3))
                                .rounded(px(r.md))
                                .bg(c.card_bg)
                                .border_1()
                                .border_color(c.line)
                                .child(
                                    div()
                                        .flex()
                                        .items_start()
                                        .justify_between()
                                        .gap(px(sp.s2))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_size(px(ty.md))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(c.pearl)
                                                        .child(record.tool),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(ty.xs))
                                                        .text_color(c.muted)
                                                        .child(meta_parts.join(" · ")),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .px(px(8.0))
                                                .py(px(2.5))
                                                .rounded_full()
                                                .bg(c.deep)
                                                .border_1()
                                                .border_color(status_border)
                                                .text_size(px(ty.xs))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(status_color)
                                                .child(status.label),
                                        ),
                                );

                            if let Some(summary) = record
                                .input_summary
                                .filter(|s| !s.trim().is_empty())
                            {
                                card = card.child(
                                    div()
                                        .text_size(px(ty.xs))
                                        .text_color(c.muted)
                                        .child(summary),
                                );
                            }

                            if let Some(reason) = record.reason.filter(|s| !s.trim().is_empty()) {
                                card = card.child(
                                    div()
                                        .p(px(sp.s2))
                                        .rounded(px(r.sm))
                                        .bg(if status.tone == ExecutionTone::Ok {
                                            c.surface_soft
                                        } else {
                                            c.danger_muted_bg
                                        })
                                        .border_1()
                                        .border_color(status_border)
                                        .text_size(px(ty.xs))
                                        .text_color(c.pearl)
                                        .child(format!("Motivo: {reason}")),
                                );
                            }

                            section = section.child(card);
                        }
                    }
                }

                section.into_any_element()
            }
            AgentCenterTab::Permissions => self.render_agent_permissions_tab(cx).into_any_element(),
            AgentCenterTab::Confirmations => {
                self.render_agent_confirmations_tab(cx).into_any_element()
            }
            AgentCenterTab::Suggestions => self.render_agent_suggestions_tab(cx).into_any_element(),
        };

        let agent_tab_btn = |id: &'static str, label: String, tab: AgentCenterTab| {
            let is_active = active_tab == tab;
            div()
                .id(id)
                .cursor_pointer()
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(r.sm))
                .border_1()
                .bg(if is_active {
                    c.accent_tint
                } else {
                    c.surface_soft
                })
                .border_color(if is_active { c.accent } else { c.line })
                .text_size(px(ty.xs))
                .font_weight(FontWeight::BOLD)
                .text_color(c.pearl)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.agent_tab = tab;
                    this.poke(cx);
                    cx.notify();
                }))
                .child(label)
        };

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(sp.s3))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(sp.s2))
                    .pb(px(sp.s2))
                    .border_b_1()
                    .border_color(c.line)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(sp.s1_5))
                            .child(agent_tab_btn(
                                "agent-tab-skills",
                                format!("Skills ({skills_count})"),
                                AgentCenterTab::Skills,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-executions",
                                format!("Execuções ({execs_count})"),
                                AgentCenterTab::Executions,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-permissions",
                                format!("Permissões ({tools_count})"),
                                AgentCenterTab::Permissions,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-confirmations",
                                format!("Confirmações ({pending_confirm_count})"),
                                AgentCenterTab::Confirmations,
                            ))
                            .child(agent_tab_btn(
                                "agent-tab-suggestions",
                                format!("Sugestões ({pending_sugg_count})"),
                                AgentCenterTab::Suggestions,
                            )),
                    )
                    .child(
                        div()
                            .id("agent-center-refresh")
                            .cursor_pointer()
                            .px(px(10.0))
                            .py(px(6.0))
                            .rounded(px(r.sm))
                            .bg(c.surface_soft)
                            .border_1()
                            .border_color(c.line_strong)
                            .text_size(px(ty.xs))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c.primary)
                            .hover(|s| s.border_color(c.primary))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.fetch_agent_center(cx);
                                this.poke(cx);
                            }))
                            .child(if loading { "Atualizando…" } else { "Atualizar" }),
                    ),
            )
            .child(
                div()
                    .id("agent-center-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .pr(px(4.0))
                    .child(body_el),
            )
    }
}
