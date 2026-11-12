//! Lógica pura de apresentação da Central de Skills e Histórico de Execuções (S14).
//!
//! Sem dependências de GPUI ou rede: agrupa apenas os dados reais vindos do core
//! (`/api/agent/skills` e `/api/agent/executions`) sem inventar placeholders.

use crate::api::{AgentExecution, AgentSkill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCenterTab {
    Skills,
    Executions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillFilter {
    All,
    Installed,
    Available,
    Pending,
    Disabled,
}

impl SkillFilter {
    pub fn matches_state(self, state: &str) -> bool {
        match self {
            Self::All => true,
            Self::Installed => state == "installed",
            Self::Available => state == "available",
            Self::Pending => state == "pending",
            Self::Disabled => state == "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecFilter {
    All,
    Verified,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStateBucket {
    pub key: &'static str,
    pub label: &'static str,
    pub empty_message: &'static str,
    pub skills: Vec<AgentSkill>,
}

const STATE_SPECS: [(&str, &str, &str); 4] = [
    (
        "installed",
        "Instaladas",
        "Nenhuma skill instalada no momento.",
    ),
    (
        "available",
        "Disponíveis",
        "Nenhuma skill disponível aguardando instalação.",
    ),
    (
        "pending",
        "Pendentes",
        "Nenhuma skill pendente de confirmação.",
    ),
    ("disabled", "Desativadas", "Nenhuma skill desativada."),
];

pub fn state_label(state: &str) -> &'static str {
    match state {
        "installed" => "Instaladas",
        "available" => "Disponíveis",
        "pending" => "Pendentes",
        "disabled" => "Desativadas",
        _ => "Inválida",
    }
}

pub fn origin_label(origin: Option<&str>) -> &'static str {
    match origin {
        Some("dev") => "dev",
        Some("imported") => "importada",
        Some("model") => "proposta",
        Some(_) => "desconhecida",
        None => "dev",
    }
}

/// Agrupa as skills reais pelos 4 estados canônicos e separa manifestos inválidos
/// para exibição explícita sem descarte nem placeholders.
pub fn group_skills_by_state(skills: &[AgentSkill]) -> (Vec<SkillStateBucket>, Vec<AgentSkill>) {
    let mut buckets: Vec<SkillStateBucket> = STATE_SPECS
        .iter()
        .map(|&(key, label, empty_message)| SkillStateBucket {
            key,
            label,
            empty_message,
            skills: Vec::new(),
        })
        .collect();
    let mut invalid = Vec::new();

    for skill in skills {
        let state = skill.state.as_deref().unwrap_or("");
        if !skill.valid {
            invalid.push(skill.clone());
            continue;
        }
        if let Some(bucket) = buckets.iter_mut().find(|b| b.key == state) {
            bucket.skills.push(skill.clone());
        } else {
            invalid.push(skill.clone());
        }
    }

    for bucket in &mut buckets {
        bucket.skills.sort_by(|a, b| {
            let na = a.name.as_deref().unwrap_or(&a.id).to_lowercase();
            let nb = b.name.as_deref().unwrap_or(&b.id).to_lowercase();
            na.cmp(&nb).then_with(|| a.id.cmp(&b.id))
        });
    }
    invalid.sort_by(|a, b| a.id.cmp(&b.id));

    (buckets, invalid)
}

/// Extrai motivos reais de ativação/dependência ou erros de manifesto sem duplicatas.
pub fn collect_skill_reasons(skill: &AgentSkill) -> Vec<String> {
    let mut out = Vec::new();
    for item in skill
        .activation
        .reasons
        .iter()
        .chain(skill.errors.iter())
    {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.iter().any(|existing: &String| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionTone {
    Ok,
    Warn,
    Err,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionStatusDisplay {
    pub key: &'static str,
    pub label: &'static str,
    pub tone: ExecutionTone,
    pub verified: bool,
}

/// Só considera "Verificada" quando `status == "ok"` e `verified == true`.
pub fn classify_execution_status(record: &AgentExecution) -> ExecutionStatusDisplay {
    if record.status == "ok" && record.verified {
        return ExecutionStatusDisplay {
            key: "ok",
            label: "Verificada",
            tone: ExecutionTone::Ok,
            verified: true,
        };
    }
    match record.status.as_str() {
        "denied" => ExecutionStatusDisplay {
            key: "denied",
            label: "Negada",
            tone: ExecutionTone::Warn,
            verified: false,
        },
        "timeout" => ExecutionStatusDisplay {
            key: "timeout",
            label: "Tempo esgotado",
            tone: ExecutionTone::Err,
            verified: false,
        },
        _ => ExecutionStatusDisplay {
            key: "failed",
            label: "Falhou",
            tone: ExecutionTone::Err,
            verified: false,
        },
    }
}

pub fn summarize_executions(executions: &[AgentExecution]) -> (usize, usize, usize) {
    let mut verified = 0;
    let mut failed = 0;
    for exec in executions {
        if classify_execution_status(exec).verified {
            verified += 1;
        } else {
            failed += 1;
        }
    }
    (verified + failed, verified, failed)
}

/// Formata timestamp ISO-8601 (`YYYY-MM-DDTHH:MM:SS...`) em `DD/MM HH:MM:SS`
/// sem dependências extras; preserva a string original se estiver fora do formato.
pub fn format_execution_time(iso: &str) -> String {
    let trimmed = iso.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some((date_part, rest)) = trimmed.split_once('T') {
        let mut date_segs = date_part.split('-');
        let _year = date_segs.next();
        let month = date_segs.next();
        let day = date_segs.next();
        let time_core = rest
            .split(['.', 'Z', '+', '-'])
            .next()
            .unwrap_or(rest);
        if let (Some(m), Some(d)) = (month, day) {
            if time_core.len() >= 5 {
                let hhmmss = if time_core.len() >= 8 {
                    &time_core[..8]
                } else {
                    time_core
                };
                return format!("{d}/{m} {hhmmss}");
            }
        }
    }
    trimmed.to_string()
}

pub fn format_duration_ms(duration_ms: Option<u64>) -> String {
    match duration_ms {
        None => String::new(),
        Some(ms) if ms < 1000 => format!("{ms} ms"),
        Some(ms) => format!("{:.1} s", ms as f64 / 1000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SkillActivation, SkillMissing};

    fn make_skill(id: &str, name: &str, state: &str) -> AgentSkill {
        AgentSkill {
            id: id.into(),
            valid: true,
            name: Some(name.into()),
            version: Some("1.0.0".into()),
            description: Some("Descrição real".into()),
            state: Some(state.into()),
            origin: Some("dev".into()),
            max_permission_level: Some(1),
            errors: Vec::new(),
            activation: SkillActivation {
                can_activate: state == "installed",
                reasons: Vec::new(),
            },
            missing: SkillMissing::default(),
        }
    }

    fn make_exec(status: &str, verified: bool, reason: Option<&str>) -> AgentExecution {
        AgentExecution {
            id: "ex-0123456789ab".into(),
            tool: "briefing.build".into(),
            status: status.into(),
            permission_level: Some(1),
            origin: Some("agent".into()),
            started_at: "2026-11-11T22:10:05.123Z".into(),
            duration_ms: Some(42),
            verified,
            reason: reason.map(ToString::to_string),
            input_summary: Some("args: date".into()),
        }
    }

    #[test]
    fn groups_skills_by_all_four_states_without_placeholders() {
        let (empty_buckets, empty_invalid) = group_skills_by_state(&[]);
        assert_eq!(empty_buckets.len(), 4);
        assert!(empty_buckets.iter().all(|b| b.skills.is_empty()));
        assert!(empty_invalid.is_empty());

        let skills = vec![
            make_skill("personal-planner", "Personal Planner", "installed"),
            make_skill("daily-briefing", "Daily Briefing", "installed"),
            make_skill("z-avail", "Z Avail", "available"),
            make_skill("a-pending", "A Pending", "pending"),
            make_skill("muted", "Muted Skill", "disabled"),
            AgentSkill {
                id: "broken".into(),
                valid: false,
                name: None,
                version: None,
                description: None,
                state: None,
                origin: None,
                max_permission_level: None,
                errors: vec!["manifesto inválido".into()],
                activation: SkillActivation::default(),
                missing: SkillMissing::default(),
            },
        ];

        let (buckets, invalid) = group_skills_by_state(&skills);
        assert_eq!(buckets[0].key, "installed");
        assert_eq!(
            buckets[0]
                .skills
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec!["daily-briefing", "personal-planner"]
        );
        assert_eq!(buckets[1].skills[0].id, "z-avail");
        assert_eq!(buckets[2].skills[0].id, "a-pending");
        assert_eq!(buckets[3].skills[0].id, "muted");
        assert_eq!(invalid.len(), 1);
        assert_eq!(invalid[0].id, "broken");
    }

    #[test]
    fn deduplicates_reasons_and_errors() {
        let mut skill = make_skill("s1", "Skill 1", "disabled");
        skill.activation.reasons = vec![
            "skill desativada".into(),
            "skill desativada".into(),
            "ferramenta ausente: planner.sync".into(),
        ];
        skill.errors = vec!["ferramenta ausente: planner.sync".into(), "erro extra".into()];
        assert_eq!(
            collect_skill_reasons(&skill),
            vec![
                "skill desativada",
                "ferramenta ausente: planner.sync",
                "erro extra"
            ]
        );
    }

    #[test]
    fn classifies_verified_vs_failed_denied_timeout() {
        assert_eq!(
            classify_execution_status(&make_exec("ok", true, None)),
            ExecutionStatusDisplay {
                key: "ok",
                label: "Verificada",
                tone: ExecutionTone::Ok,
                verified: true,
            }
        );
        let unverified_ok = classify_execution_status(&make_exec("ok", false, None));
        assert!(!unverified_ok.verified);
        assert_eq!(unverified_ok.label, "Falhou");

        assert_eq!(
            classify_execution_status(&make_exec("denied", false, Some("negada")))
                .tone,
            ExecutionTone::Warn
        );
        assert_eq!(
            classify_execution_status(&make_exec("timeout", false, Some("tempo esgotado")))
                .label,
            "Tempo esgotado"
        );
    }

    #[test]
    fn summarizes_and_formats_timestamps_and_durations() {
        assert_eq!(summarize_executions(&[]), (0, 0, 0));
        assert_eq!(
            summarize_executions(&[
                make_exec("ok", true, None),
                make_exec("failed", false, Some("erro")),
                make_exec("denied", false, Some("negada")),
                make_exec("timeout", false, Some("timeout")),
            ]),
            (4, 1, 3)
        );

        assert_eq!(
            format_execution_time("2026-11-11T22:10:05.123Z"),
            "11/11 22:10:05"
        );
        assert_eq!(format_execution_time(""), "");
        assert_eq!(format_duration_ms(None), "");
        assert_eq!(format_duration_ms(Some(0)), "0 ms");
        assert_eq!(format_duration_ms(Some(42)), "42 ms");
        assert_eq!(format_duration_ms(Some(1460)), "1.5 s");
    }
}

