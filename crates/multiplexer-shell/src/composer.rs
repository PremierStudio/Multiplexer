//! Cursor-aware composer draft helpers.
//!
//! Cursor values are Unicode scalar (char) indices, not bytes.

/// Byte offset of `cursor` (a char index) in `draft`.
fn byte_at(draft: &str, cursor: usize) -> usize {
    draft
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(draft.len())
}

/// Clamp `cursor` to `[0, draft.chars().count()]`.
pub fn clamp_cursor(draft: &str, cursor: usize) -> usize {
    cursor.min(draft.chars().count())
}

/// Insert `text` at the char `cursor`. Returns the cursor after the insert.
pub fn insert_at(draft: &mut String, cursor: usize, text: &str) -> usize {
    let cursor = clamp_cursor(draft, cursor);
    let byte = byte_at(draft, cursor);
    draft.insert_str(byte, text);
    cursor + text.chars().count()
}

/// Delete the char immediately before `cursor`. Returns the new cursor.
pub fn delete_back(draft: &mut String, cursor: usize) -> usize {
    let cursor = clamp_cursor(draft, cursor);
    if cursor == 0 {
        return 0;
    }
    let new_cursor = cursor - 1;
    let start = byte_at(draft, new_cursor);
    let end = byte_at(draft, cursor);
    draft.replace_range(start..end, "");
    new_cursor
}

/// Move one char left. Stays at 0.
pub fn move_left(draft: &str, cursor: usize) -> usize {
    clamp_cursor(draft, cursor).saturating_sub(1)
}

/// Move one char right. Stays at the end.
pub fn move_right(draft: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor(draft, cursor);
    clamp_cursor(draft, cursor.saturating_add(1))
}

/// Jump to the start of the draft.
pub fn move_home(_draft: &str, _cursor: usize) -> usize {
    0
}

/// Jump to the end of the draft (char count).
pub fn move_end(draft: &str, _cursor: usize) -> usize {
    draft.chars().count()
}

/// Delete the char at `cursor`. Cursor stays; no-op at the end.
pub fn delete_forward(draft: &mut String, cursor: usize) -> usize {
    let cursor = clamp_cursor(draft, cursor);
    if cursor == draft.chars().count() {
        return cursor;
    }
    let start = byte_at(draft, cursor);
    let end = byte_at(draft, cursor + 1);
    draft.replace_range(start..end, "");
    cursor
}

/// Skip whitespace left, then skip non-whitespace. Lands at the word start.
pub fn move_word_left(draft: &str, cursor: usize) -> usize {
    let mut cursor = clamp_cursor(draft, cursor);
    while cursor > 0 {
        let prev = draft.chars().nth(cursor - 1).expect("cursor > 0");
        if !prev.is_whitespace() {
            break;
        }
        cursor -= 1;
    }
    while cursor > 0 {
        let prev = draft.chars().nth(cursor - 1).expect("cursor > 0");
        if prev.is_whitespace() {
            break;
        }
        cursor -= 1;
    }
    cursor
}

/// Skip whitespace right, then skip non-whitespace. Lands after the word.
pub fn move_word_right(draft: &str, cursor: usize) -> usize {
    let mut cursor = clamp_cursor(draft, cursor);
    let len = draft.chars().count();
    while cursor < len {
        let ch = draft.chars().nth(cursor).expect("cursor < len");
        if !ch.is_whitespace() {
            break;
        }
        cursor += 1;
    }
    while cursor < len {
        let ch = draft.chars().nth(cursor).expect("cursor < len");
        if ch.is_whitespace() {
            break;
        }
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_middle() {
        let mut draft = String::from("héllo");
        let cursor = insert_at(&mut draft, 2, "XX");
        assert_eq!(draft, "héXXllo");
        assert_eq!(cursor, 4);
    }

    #[test]
    fn delete_back_at_zero() {
        let mut draft = String::from("ab");
        let cursor = delete_back(&mut draft, 0);
        assert_eq!(draft, "ab");
        assert_eq!(cursor, 0);
    }

    #[test]
    fn clamp_past_end() {
        assert_eq!(clamp_cursor("café", 99), 4);
        assert_eq!(clamp_cursor("", 3), 0);
        assert_eq!(clamp_cursor("hi", 2), 2);
    }

    #[test]
    fn delete_back_middle_drops_one_char() {
        let mut draft = String::from("héllo");
        let cursor = delete_back(&mut draft, 2);
        assert_eq!(draft, "hllo");
        assert_eq!(cursor, 1);
    }

    #[test]
    fn move_edges() {
        let draft = "café";
        assert_eq!(move_left(draft, 0), 0);
        assert_eq!(move_left(draft, 1), 0);
        assert_eq!(move_left(draft, 4), 3);
        assert_eq!(move_left(draft, 99), 3);
        assert_eq!(move_right(draft, 0), 1);
        assert_eq!(move_right(draft, 3), 4);
        assert_eq!(move_right(draft, 4), 4);
        assert_eq!(move_right(draft, 99), 4);
        assert_eq!(move_home(draft, 3), 0);
        assert_eq!(move_home(draft, 99), 0);
        assert_eq!(move_home("", 5), 0);
        assert_eq!(move_end(draft, 0), 4);
        assert_eq!(move_end(draft, 99), 4);
        assert_eq!(move_end("", 5), 0);
    }

    #[test]
    fn delete_forward_middle() {
        let mut draft = String::from("héllo");
        let cursor = delete_forward(&mut draft, 1);
        assert_eq!(draft, "hllo");
        assert_eq!(cursor, 1);

        let mut draft = String::from("ab");
        let cursor = delete_forward(&mut draft, 2);
        assert_eq!(draft, "ab");
        assert_eq!(cursor, 2);

        let mut draft = String::from("é");
        let cursor = delete_forward(&mut draft, 0);
        assert_eq!(draft, "");
        assert_eq!(cursor, 0);
    }

    #[test]
    fn word_jumps_ascii() {
        let draft = "hello world  next";
        assert_eq!(move_word_left(draft, 17), 13);
        assert_eq!(move_word_left(draft, 13), 6);
        assert_eq!(move_word_left(draft, 6), 0);
        assert_eq!(move_word_left(draft, 0), 0);
        assert_eq!(move_word_left(draft, 8), 6);
        assert_eq!(move_word_right(draft, 0), 5);
        assert_eq!(move_word_right(draft, 5), 11);
        assert_eq!(move_word_right(draft, 11), 17);
        assert_eq!(move_word_right(draft, 17), 17);
        assert_eq!(move_word_right(draft, 3), 5);
    }

    #[test]
    fn word_jumps_unicode() {
        let draft = "café au  日本語";
        assert_eq!(draft.chars().count(), 12);
        assert_eq!(move_word_left(draft, 12), 9);
        assert_eq!(move_word_left(draft, 9), 5);
        assert_eq!(move_word_left(draft, 5), 0);
        assert_eq!(move_word_left(draft, 0), 0);
        assert_eq!(move_word_right(draft, 0), 4);
        assert_eq!(move_word_right(draft, 4), 7);
        assert_eq!(move_word_right(draft, 7), 12);
        assert_eq!(move_word_right(draft, 12), 12);
        assert_eq!(move_word_right("  café", 0), 6);
        assert_eq!(move_word_left("café  ", 6), 0);
    }

    #[test]
    fn insert_then_move_roundtrip() {
        let mut draft = String::new();
        let mut cursor = insert_at(&mut draft, 0, "hello");
        assert_eq!(draft, "hello");
        assert_eq!(cursor, 5);
        cursor = move_home(&draft, cursor);
        cursor = insert_at(&mut draft, cursor, "say ");
        assert_eq!(draft, "say hello");
        assert_eq!(cursor, 4);
        cursor = move_end(&draft, cursor);
        cursor = insert_at(&mut draft, cursor, "!");
        assert_eq!(draft, "say hello!");
        assert_eq!(cursor, 10);
        cursor = move_left(&draft, cursor);
        cursor = delete_forward(&mut draft, cursor);
        assert_eq!(draft, "say hello");
        assert_eq!(cursor, 9);
        cursor = move_word_left(&draft, cursor);
        assert_eq!(cursor, 4);
        cursor = move_word_right(&draft, cursor);
        assert_eq!(cursor, 9);
        cursor = move_left(&draft, cursor);
        cursor = move_right(&draft, cursor);
        assert_eq!(cursor, 9);
    }
}
