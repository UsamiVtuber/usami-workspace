//! Edição de texto e manipulação de cursor/seleção no input do chat.

use crate::UsamiApp;
use crate::media::AttachedMedia;
use crate::model::ActiveDrawer;
use crate::text_input::resolve_key_char_with_dead_key;
use gpui::*;

impl UsamiApp {
    pub(crate) fn cursor_idx(&self) -> usize {
        self.cursor_char_idx.min(self.input_text.chars().count())
    }

    pub(crate) fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let cursor = self.cursor_idx();
        if anchor == cursor {
            None
        } else {
            Some((anchor.min(cursor), anchor.max(cursor)))
        }
    }

    pub(crate) fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            let chars: Vec<char> = self.input_text.chars().collect();
            let mut new_chars = Vec::with_capacity(chars.len());
            new_chars.extend_from_slice(&chars[..start]);
            new_chars.extend_from_slice(&chars[end..]);
            self.input_text = new_chars.into_iter().collect();
            self.cursor_char_idx = start;
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn insert_str_at_cursor(&mut self, text: &str) {
        self.delete_selection();
        let chars: Vec<char> = self.input_text.chars().collect();
        let idx = self.cursor_idx();
        let mut new_chars = Vec::with_capacity(chars.len() + text.chars().count());
        new_chars.extend_from_slice(&chars[..idx]);
        new_chars.extend(text.chars());
        let count = text.chars().count();
        new_chars.extend_from_slice(&chars[idx..]);
        self.input_text = new_chars.into_iter().collect();
        self.cursor_char_idx = idx + count;
        self.selection_anchor = None;
    }

    pub(crate) fn backspace_at_cursor(&mut self) {
        if !self.delete_selection() {
            let cursor = self.cursor_idx();
            if cursor > 0 {
                let chars: Vec<char> = self.input_text.chars().collect();
                let mut new_chars = Vec::with_capacity(chars.len().saturating_sub(1));
                new_chars.extend_from_slice(&chars[..cursor - 1]);
                new_chars.extend_from_slice(&chars[cursor..]);
                self.input_text = new_chars.into_iter().collect();
                self.cursor_char_idx = cursor - 1;
            }
        }
    }

    pub(crate) fn delete_forward_at_cursor(&mut self) {
        if !self.delete_selection() {
            let cursor = self.cursor_idx();
            let chars: Vec<char> = self.input_text.chars().collect();
            if cursor < chars.len() {
                let mut new_chars = Vec::with_capacity(chars.len().saturating_sub(1));
                new_chars.extend_from_slice(&chars[..cursor]);
                new_chars.extend_from_slice(&chars[cursor + 1..]);
                self.input_text = new_chars.into_iter().collect();
            }
        }
    }

    pub(crate) fn move_cursor_left(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        }
        let chars: Vec<char> = self.input_text.chars().collect();
        let current = self.cursor_idx();
        if ctrl {
            let mut target = current;
            while target > 0 && chars.get(target - 1).map_or(false, |c| c.is_whitespace()) {
                target -= 1;
            }
            while target > 0 && chars.get(target - 1).map_or(false, |c| !c.is_whitespace()) {
                target -= 1;
            }
            self.cursor_char_idx = target;
        } else if !shift {
            if let Some((start, _)) = self.selection_range() {
                self.cursor_char_idx = start;
            } else if current > 0 {
                self.cursor_char_idx = current - 1;
            }
        } else if current > 0 {
            self.cursor_char_idx = current - 1;
        }
        if !shift {
            self.selection_anchor = None;
        }
    }

    pub(crate) fn move_cursor_right(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        }
        let chars: Vec<char> = self.input_text.chars().collect();
        let current = self.cursor_idx();
        if ctrl {
            let mut target = current;
            while target < chars.len() && chars.get(target).map_or(false, |c| !c.is_whitespace()) {
                target += 1;
            }
            while target < chars.len() && chars.get(target).map_or(false, |c| c.is_whitespace()) {
                target += 1;
            }
            self.cursor_char_idx = target;
        } else if !shift {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_char_idx = end;
            } else if current < chars.len() {
                self.cursor_char_idx = current + 1;
            }
        } else if current < chars.len() {
            self.cursor_char_idx = current + 1;
        }
        if !shift {
            self.selection_anchor = None;
        }
    }

    pub(crate) fn move_cursor_home(&mut self, shift: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        } else if !shift {
            self.selection_anchor = None;
        }
        self.cursor_char_idx = 0;
    }

    pub(crate) fn move_cursor_end(&mut self, shift: bool) {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_idx());
        } else if !shift {
            self.selection_anchor = None;
        }
        self.cursor_char_idx = self.input_text.chars().count();
    }

    pub(crate) fn select_word_at(&mut self, char_idx: usize) {
        let chars: Vec<char> = self.input_text.chars().collect();
        if chars.is_empty() {
            return;
        }
        let idx = char_idx.min(chars.len().saturating_sub(1));
        let mut start = idx;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let mut end = idx;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        self.selection_anchor = Some(start);
        self.cursor_char_idx = end;
    }

    pub(crate) fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        self.touch_ui(cx);
        let ks = &event.keystroke;
        let key = ks.key.as_str();

        // Atalhos com Control (ex: Ctrl+V para colar, Ctrl+Backspace / Ctrl+W para apagar palavra)
        if ks.modifiers.control && !ks.modifiers.alt {
            if key.eq_ignore_ascii_case("v") {
                if let Some(item) = cx.read_from_clipboard() {
                    let mut attached_image = false;
                    for entry in item.entries() {
                        if let gpui::ClipboardEntry::Image(image) = entry {
                            let bytes = &image.bytes;
                            if !bytes.is_empty() {
                                let mime = if bytes.starts_with(b"\x89PNG") {
                                    "image/png"
                                } else if bytes.starts_with(b"\xFF\xD8\xFF") {
                                    "image/jpeg"
                                } else if bytes.starts_with(b"GIF") {
                                    "image/gif"
                                } else if bytes.starts_with(b"RIFF")
                                    && bytes.len() > 12
                                    && &bytes[8..12] == b"WEBP"
                                {
                                    "image/webp"
                                } else {
                                    "image/png"
                                };
                                self.attached_media = Some(AttachedMedia::from_bytes(
                                    "screenshot.png".into(),
                                    mime,
                                    bytes.clone(),
                                ));
                                self.input_active = true;
                                attached_image = true;
                                cx.notify();
                                break;
                            }
                        }
                    }

                    if !attached_image {
                        if let Some(text) = item.text() {
                            let trimmed = text.trim().trim_matches('"');
                            let path = std::path::Path::new(trimmed);
                            let is_image_ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|ext| {
                                    matches!(
                                        ext.to_ascii_lowercase().as_str(),
                                        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
                                    )
                                })
                                .unwrap_or(false);

                            if is_image_ext && path.is_file() {
                                if let Ok(bytes) = std::fs::read(path) {
                                    let filename = path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("imagem.png")
                                        .to_string();
                                    let mime = match path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .map(|s| s.to_ascii_lowercase())
                                        .as_deref()
                                    {
                                        Some("jpg") | Some("jpeg") => "image/jpeg",
                                        Some("png") => "image/png",
                                        Some("webp") => "image/webp",
                                        Some("gif") => "image/gif",
                                        Some("bmp") => "image/bmp",
                                        _ => "image/png",
                                    };
                                    self.attached_media =
                                        Some(AttachedMedia::from_bytes(filename, mime, bytes));
                                    self.input_active = true;
                                    cx.notify();
                                    return;
                                }
                            }

                            self.input_active = true;
                            self.pending_dead_key = None;
                            self.insert_str_at_cursor(&text);
                            self.caret_on = true;
                            cx.notify();
                        }
                    }
                }
            } else if key.eq_ignore_ascii_case("a") {
                self.input_active = true;
                self.selection_anchor = Some(0);
                self.cursor_char_idx = self.input_text.chars().count();
                self.caret_on = true;
                cx.notify();
                return;
            } else if key.eq_ignore_ascii_case("c") {
                if let Some((start, end)) = self.selection_range() {
                    let chars: Vec<char> = self.input_text.chars().collect();
                    let text: String = chars[start..end].iter().collect();
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                }
                return;
            } else if key.eq_ignore_ascii_case("x") {
                if let Some((start, end)) = self.selection_range() {
                    let chars: Vec<char> = self.input_text.chars().collect();
                    let text: String = chars[start..end].iter().collect();
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    self.delete_selection();
                    self.caret_on = true;
                    cx.notify();
                }
                return;
            } else if key.eq_ignore_ascii_case("w") || key == "backspace" {
                self.input_active = true;
                self.pending_dead_key = None;
                if !self.delete_selection() {
                    let chars: Vec<char> = self.input_text.chars().collect();
                    let current = self.cursor_idx();
                    if current > 0 {
                        let mut target = current;
                        while target > 0
                            && chars.get(target - 1).is_some_and(|c| c.is_whitespace())
                        {
                            target -= 1;
                        }
                        while target > 0
                            && chars.get(target - 1).is_some_and(|c| !c.is_whitespace())
                        {
                            target -= 1;
                        }
                        let mut new_chars = Vec::with_capacity(chars.len());
                        new_chars.extend_from_slice(&chars[..target]);
                        new_chars.extend_from_slice(&chars[current..]);
                        self.input_text = new_chars.into_iter().collect();
                        self.cursor_char_idx = target;
                    }
                }
                self.caret_on = true;
                cx.notify();
                return;
            } else if key.eq_ignore_ascii_case("u") {
                self.pending_dead_key = None;
                self.input_text.clear();
                self.cursor_char_idx = 0;
                self.selection_anchor = None;
                self.caret_on = true;
                cx.notify();
                return;
            }
            return;
        }

        // Ignora teclas com Alt isolado
        if ks.modifiers.alt && !ks.modifiers.control {
            return;
        }

        if key == "enter" {
            if ks.modifiers.shift {
                self.input_active = true;
                self.pending_dead_key = None;
                self.insert_str_at_cursor("\n");
                self.caret_on = true;
                cx.notify();
            } else {
                self.submit(cx);
            }
        } else if key == "left" || key == "arrowleft" {
            self.input_active = true;
            self.pending_dead_key = None;
            self.move_cursor_left(ks.modifiers.shift, ks.modifiers.control);
            self.caret_on = true;
            cx.notify();
        } else if key == "right" || key == "arrowright" {
            self.input_active = true;
            self.pending_dead_key = None;
            self.move_cursor_right(ks.modifiers.shift, ks.modifiers.control);
            self.caret_on = true;
            cx.notify();
        } else if key == "home" {
            self.input_active = true;
            self.pending_dead_key = None;
            self.move_cursor_home(ks.modifiers.shift);
            self.caret_on = true;
            cx.notify();
        } else if key == "end" {
            self.input_active = true;
            self.pending_dead_key = None;
            self.move_cursor_end(ks.modifiers.shift);
            self.caret_on = true;
            cx.notify();
        } else if key == "delete" {
            self.input_active = true;
            self.pending_dead_key = None;
            self.delete_forward_at_cursor();
            self.caret_on = true;
            cx.notify();
        } else if key == "backspace" {
            self.input_active = true;
            if self.pending_dead_key.take().is_none() {
                self.backspace_at_cursor();
            }
            self.caret_on = true;
            cx.notify();
        } else if key == "escape" {
            self.pending_dead_key = None;
            self.input_active = false;
            self.active_drawer = ActiveDrawer::None;
            cx.notify();
        } else if key == "space" || key == " " {
            self.input_active = true;
            if let Some(dead) = self.pending_dead_key.take() {
                let mut s = String::new();
                s.push(dead);
                self.insert_str_at_cursor(&s);
            } else {
                self.insert_str_at_cursor(" ");
            }
            self.caret_on = true;
            cx.notify();
        } else if let Some(ref ch) = ks.key_char {
            if !ch.chars().any(|c| c.is_control()) {
                self.input_active = true;
                if let Some(resolved) =
                    resolve_key_char_with_dead_key(&mut self.pending_dead_key, ch)
                {
                    self.insert_str_at_cursor(&resolved);
                }
                self.caret_on = true;
                cx.notify();
            }
        } else if key.chars().count() == 1 {
            let c = key.chars().next().unwrap();
            if !c.is_control() {
                self.input_active = true;
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                if let Some(resolved) =
                    resolve_key_char_with_dead_key(&mut self.pending_dead_key, s)
                {
                    self.insert_str_at_cursor(&resolved);
                }
                self.caret_on = true;
                cx.notify();
            }
        }
    }
}
