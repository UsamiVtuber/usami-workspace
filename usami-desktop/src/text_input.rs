//! Lógica pura do campo de texto: acentos de tecla morta e tokenização para seleção.

pub(crate) fn is_dead_accent_char(c: char) -> bool {
    matches!(c, '´' | '`' | '^' | '~' | '¨')
}

pub(crate) fn compose_dead_accent(accent: char, base: char) -> Option<char> {
    match (accent, base) {
        ('´', 'a') => Some('á'),
        ('´', 'e') => Some('é'),
        ('´', 'i') => Some('í'),
        ('´', 'o') => Some('ó'),
        ('´', 'u') => Some('ú'),
        ('´', 'y') => Some('ý'),
        ('´', 'c') => Some('ç'),
        ('´', 'A') => Some('Á'),
        ('´', 'E') => Some('É'),
        ('´', 'I') => Some('Í'),
        ('´', 'O') => Some('Ó'),
        ('´', 'U') => Some('Ú'),
        ('´', 'Y') => Some('Ý'),
        ('´', 'C') => Some('Ç'),
        ('`', 'a') => Some('à'),
        ('`', 'e') => Some('è'),
        ('`', 'i') => Some('ì'),
        ('`', 'o') => Some('ò'),
        ('`', 'u') => Some('ù'),
        ('`', 'A') => Some('À'),
        ('`', 'E') => Some('È'),
        ('`', 'I') => Some('Ì'),
        ('`', 'O') => Some('Ò'),
        ('`', 'U') => Some('Ù'),
        ('^', 'a') => Some('â'),
        ('^', 'e') => Some('ê'),
        ('^', 'i') => Some('î'),
        ('^', 'o') => Some('ô'),
        ('^', 'u') => Some('û'),
        ('^', 'A') => Some('Â'),
        ('^', 'E') => Some('Ê'),
        ('^', 'I') => Some('Î'),
        ('^', 'O') => Some('Ô'),
        ('^', 'U') => Some('Û'),
        ('~', 'a') => Some('ã'),
        ('~', 'o') => Some('õ'),
        ('~', 'n') => Some('ñ'),
        ('~', 'A') => Some('Ã'),
        ('~', 'O') => Some('Õ'),
        ('~', 'N') => Some('Ñ'),
        ('¨', 'a') => Some('ä'),
        ('¨', 'e') => Some('ë'),
        ('¨', 'i') => Some('ï'),
        ('¨', 'o') => Some('ö'),
        ('¨', 'u') => Some('ü'),
        ('¨', 'y') => Some('ÿ'),
        ('¨', 'A') => Some('Ä'),
        ('¨', 'E') => Some('Ë'),
        ('¨', 'I') => Some('Ï'),
        ('¨', 'O') => Some('Ö'),
        ('¨', 'U') => Some('Ü'),
        _ => None,
    }
}

pub(crate) fn resolve_key_char_with_dead_key(
    pending_dead_key: &mut Option<char>,
    ch: &str,
) -> Option<String> {
    let mut chars = ch.chars();
    let first = chars.next()?;

    if chars.next().is_none() {
        if let Some(dead) = pending_dead_key.take() {
            if first == dead {
                return Some(dead.to_string());
            } else if let Some(composed) = compose_dead_accent(dead, first) {
                return Some(composed.to_string());
            } else if !first.is_ascii() {
                return Some(first.to_string());
            } else if first == ' ' {
                return Some(dead.to_string());
            } else {
                let mut s = String::with_capacity(8);
                s.push(dead);
                s.push(first);
                return Some(s);
            }
        }

        if is_dead_accent_char(first) {
            *pending_dead_key = Some(first);
            return None;
        }

        return Some(first.to_string());
    }

    let mut out = String::new();
    if let Some(dead) = pending_dead_key.take() && !ch.starts_with(dead) {
        out.push(dead);
    }
    out.push_str(ch);
    Some(out)
}

#[cfg(test)]
pub(crate) fn push_key_char_with_dead_key(
    input_text: &mut String,
    pending_dead_key: &mut Option<char>,
    ch: &str,
) {
    if let Some(inserted) = resolve_key_char_with_dead_key(pending_dead_key, ch) {
        input_text.push_str(&inserted);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextSpanToken {
    pub(crate) start_char_idx: usize,
    pub(crate) text: String,
    pub(crate) is_whitespace: bool,
}

pub(crate) fn tokenize_text_spans(text: &str) -> Vec<TextSpanToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut start_idx = 0;
    let mut in_ws = false;

    for (idx, ch) in text.chars().enumerate() {
        let is_ws = ch.is_whitespace();
        if current.is_empty() {
            in_ws = is_ws;
            start_idx = idx;
            current.push(ch);
        } else if is_ws == in_ws {
            current.push(ch);
        } else {
            tokens.push(TextSpanToken {
                start_char_idx: start_idx,
                text: current,
                is_whitespace: in_ws,
            });
            current = String::new();
            start_idx = idx;
            in_ws = is_ws;
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(TextSpanToken {
            start_char_idx: start_idx,
            text: current,
            is_whitespace: in_ws,
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_key_accent_does_not_duplicate_when_os_sends_precomposed_vowel() {
        let mut text = String::from("olha voce a");
        let mut pending = None;

        // No GPUI Windows, pressionar '´' envia Some("´") e pressionar 'i' em seguida
        // envia Some("í") já composto pelo kernel (ToUnicode flag 1<<2).
        push_key_char_with_dead_key(&mut text, &mut pending, "´");
        assert_eq!(text, "olha voce a");
        assert_eq!(pending, Some('´'));

        push_key_char_with_dead_key(&mut text, &mut pending, "í");
        assert_eq!(text, "olha voce aí");
        assert_eq!(pending, None);
    }

    #[test]
    fn dead_key_accent_composes_when_os_sends_raw_ascii_vowel() {
        let mut text = String::new();
        let mut pending = None;

        push_key_char_with_dead_key(&mut text, &mut pending, "~");
        push_key_char_with_dead_key(&mut text, &mut pending, "a");
        push_key_char_with_dead_key(&mut text, &mut pending, "o");
        assert_eq!(text, "ão");
        assert_eq!(pending, None);
    }

    #[test]
    fn dead_key_accent_preserves_accent_before_non_composable_consonant() {
        let mut text = String::new();
        let mut pending = None;

        push_key_char_with_dead_key(&mut text, &mut pending, "´");
        push_key_char_with_dead_key(&mut text, &mut pending, "x");
        assert_eq!(text, "´x");
        assert_eq!(pending, None);
    }
}
