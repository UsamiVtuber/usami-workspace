//! Edição de texto e manipulação de cursor/seleção no input do chat.

use crate::UsamiApp;

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

}
