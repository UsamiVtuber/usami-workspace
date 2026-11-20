//! Utilitários de fala: deduplicação de turnos de voz e limpeza do texto falado.

use std::time::{Duration, Instant};

pub(crate) fn normalize_utterance_key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub(crate) fn is_duplicate_utterance(
    last_commit: &mut Option<(String, Instant)>,
    text: &str,
    now: Instant,
) -> bool {
    let key = normalize_utterance_key(text);
    if key.is_empty() {
        return true;
    }
    if let Some((prev_key, prev_at)) = last_commit
        && *prev_key == key && now.saturating_duration_since(*prev_at) < Duration::from_secs(5)
    {
        return true;
    }
    *last_commit = Some((key, now));
    false
}

/// Sanitiza o texto de fala da Usami para a interface de diálogo,
/// garantindo que tags técnicas (`[intimate_shy]`, `[blush]`, etc.)
/// e sobras de animações não vazem no balão de fala da Visual Novel.
pub(crate) fn sanitize_speech_text(text: &str) -> String {
    let mut cleaned = String::new();
    let mut in_bracket = false;
    for ch in text.chars() {
        if ch == '[' {
            in_bracket = true;
            continue;
        }
        if ch == ']' {
            in_bracket = false;
            continue;
        }
        if !in_bracket {
            cleaned.push(ch);
        }
    }
    if let Some(pos) = cleaned.find("-prefab_")
        && let Some(start) =
            cleaned[..pos].rfind(|c: char| c.is_ascii_digit() || c == ' ' || c == ',')
    {
        cleaned.truncate(start);
    }
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_utterance_within_five_seconds_is_dropped() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(!is_duplicate_utterance(&mut last, "Oi, tudo bem?", t0));
        assert_eq!(normalize_utterance_key("Oi, tudo bem?"), "oitudobem");
        // Mesmas palavras sem pontuação 3s depois -> descartado
        assert!(is_duplicate_utterance(
            &mut last,
            "oi tudo bem",
            t0 + Duration::from_secs(3)
        ));
        // Palavras diferentes -> aceito
        assert!(!is_duplicate_utterance(
            &mut last,
            "Oi, tudo bem sim!",
            t0 + Duration::from_secs(3)
        ));
        // Mesmas palavras 6s depois -> aceito novamente
        assert!(!is_duplicate_utterance(
            &mut last,
            "oi tudo bem sim",
            t0 + Duration::from_secs(9)
        ));
    }
}
