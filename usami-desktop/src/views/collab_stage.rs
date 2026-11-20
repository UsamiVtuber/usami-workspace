//! Palco de Collab ao Vivo & Rede Federada (Fase 4 — RFC-001).
//!
//! Exibe participantes federados (`did:key`), estado do `FloorLease`
//! (`Falando` / `PRE_YIELD` / `Na fila`) e slots agendados em Zero-Knowledge.

use crate::UsamiApp;
use crate::theme;
use gpui::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FloorRole {
    Speaking,
    PreYielding,
    Queued(usize),
    Idle,
}

#[derive(Debug, Clone)]
pub struct CollabPeer {
    #[allow(dead_code)]
    pub did: String,
    pub display_name: String,
    pub role: FloorRole,
}

#[derive(Debug, Clone)]
pub struct CollabStageState {
    pub visible: bool,
    pub peers: Vec<CollabPeer>,
    pub next_slot_label: String,
}

impl Default for CollabStageState {
    fn default() -> Self {
        Self {
            visible: false,
            peers: vec![CollabPeer {
                did: "did:key:zUsamiLocal".into(),
                display_name: "Usami".into(),
                role: FloorRole::Idle,
            }],
            next_slot_label: "Nenhum collab agendado".into(),
        }
    }
}

impl CollabStageState {
    #[allow(dead_code)]
    pub fn grant_floor(&mut self, did: &str) {
        for peer in &mut self.peers {
            peer.role = if peer.did == did {
                FloorRole::Speaking
            } else {
                FloorRole::Idle
            };
        }
    }

    #[allow(dead_code)]
    pub fn mark_pre_yield(&mut self, did: &str) {
        if let Some(peer) = self.peers.iter_mut().find(|p| p.did == did)
            && peer.role == FloorRole::Speaking
        {
            peer.role = FloorRole::PreYielding;
        }
    }
}

impl UsamiApp {
    pub(crate) fn render_collab_stage(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.collab_stage.visible {
            return div().into_any_element();
        }
        let t = theme::theme();
        let c = t.color;
        let sp = t.space;
        let ty = t.text;
        let peers = self.collab_stage.peers.clone();
        let slot_label = self.collab_stage.next_slot_label.clone();

        div()
            .id("collab-stage-card")
            .absolute()
            .right(px(sp.s5))
            .top(px(84.0))
            .w(px(280.0))
            .p(px(sp.s3))
            .rounded(px(14.0))
            .bg(c.panel)
            .border_1()
            .border_color(c.line_strong)
            .shadow(t.shadow.drawer.clone())
            .occlude()
            .flex()
            .flex_col()
            .gap(px(sp.s2))
            .child(
                div()
                    .text_size(px(ty.sm))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c.pearl)
                    .child("Collab Stage (FloorLease)"),
            )
            .children(peers.into_iter().map(|peer| {
                let badge = match peer.role {
                    FloorRole::Speaking => "Falando",
                    FloorRole::PreYielding => "PRE_YIELD",
                    FloorRole::Queued(n) => {
                        return div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(ty.xs))
                            .text_color(c.muted)
                            .child(peer.display_name)
                            .child(format!("Fila #{n}"));
                    }
                    FloorRole::Idle => "Ouvindo",
                };
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(ty.xs))
                    .text_color(c.pearl)
                    .child(peer.display_name)
                    .child(badge)
            }))
            .child(
                div()
                    .pt(px(4.0))
                    .border_t_1()
                    .border_color(c.line)
                    .text_size(px(ty.xs))
                    .text_color(c.muted)
                    .child(slot_label),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{CollabPeer, CollabStageState, FloorRole};

    #[::core::prelude::v1::test]
    fn floor_lease_and_pre_yield_transition_cleanly() {
        let mut stage = CollabStageState::default();
        stage.peers.push(CollabPeer {
            did: "did:key:zPeer".into(),
            display_name: "Kitsune".into(),
            role: FloorRole::Queued(1),
        });
        stage.grant_floor("did:key:zUsamiLocal");
        assert_eq!(stage.peers[0].role, FloorRole::Speaking);
        assert_eq!(stage.peers[1].role, FloorRole::Idle);

        stage.mark_pre_yield("did:key:zUsamiLocal");
        assert_eq!(stage.peers[0].role, FloorRole::PreYielding);
    }
}
