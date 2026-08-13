//! Keystrokes for an in-app PTY. No GPUI types.

/// Bytes to write for a named key. `char` is the OS text if any.
pub fn pty_key_bytes(key: &str, ch: Option<&str>) -> Option<Vec<u8>> {
    pty_input(key, ch, false)
}

/// Same as [`pty_key_bytes`], plus Ctrl+letter (ETX, EOT, …).
pub fn pty_input(key: &str, ch: Option<&str>, ctrl: bool) -> Option<Vec<u8>> {
    if ctrl {
        return pty_ctrl_bytes(key, ch);
    }
    match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" => Some(b"\t".to_vec()),
        "space" => Some(b" ".to_vec()),
        "escape" => Some(b"\x1b".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        _ => {
            let text = ch.filter(|s| !s.is_empty())?;
            if text == "\n" || text == "\r" {
                return Some(b"\r".to_vec());
            }
            Some(text.as_bytes().to_vec())
        }
    }
}

fn pty_ctrl_bytes(key: &str, ch: Option<&str>) -> Option<Vec<u8>> {
    let letter = if key.len() == 1 {
        key.chars().next()?
    } else {
        let text = ch.filter(|s| s.chars().count() == 1)?;
        text.chars().next()?
    };
    let lower = letter.to_ascii_lowercase();
    if !lower.is_ascii_lowercase() {
        return None;
    }
    Some(vec![lower as u8 - b'a' + 1])
}

/// Paste text: every line break becomes a single `\r` so ConPTY / posix
/// see Enter, not a raw LF.
pub fn pty_paste_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                out.push(b'\r');
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'\n' => {
                out.push(b'\r');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

/// Convert a host pane size in pixels to PTY cols/rows.
/// Zero, NaN, or non-positive cell size falls back to 8x16 and clamps to 1..=i16::MAX.
pub fn pty_grid_from_px(width_px: f32, height_px: f32, cell_w: f32, cell_h: f32) -> (u16, u16) {
    let cell_w = if cell_w.is_finite() && cell_w > 0.0 {
        cell_w
    } else {
        8.0
    };
    let cell_h = if cell_h.is_finite() && cell_h > 0.0 {
        cell_h
    } else {
        16.0
    };
    let width_px = if width_px.is_finite() {
        width_px.max(0.0)
    } else {
        0.0
    };
    let height_px = if height_px.is_finite() {
        height_px.max(0.0)
    } else {
        0.0
    };
    let cols = (width_px / cell_w).floor() as i32;
    let rows = (height_px / cell_h).floor() as i32;
    (
        cols.clamp(1, i16::MAX as i32) as u16,
        rows.clamp(1, i16::MAX as i32) as u16,
    )
}

/// Reject a zero or overflow PTY size. Shared by ConPTY and posix.
pub fn validate_pty_size(cols: u16, rows: u16) -> Result<(u16, u16), crate::TerminalError> {
    if cols == 0 || rows == 0 {
        return Err(crate::TerminalError::Io(
            "cols and rows must be greater than 0".into(),
        ));
    }
    if cols > i16::MAX as u16 || rows > i16::MAX as u16 {
        return Err(crate::TerminalError::Io(
            "cols or rows exceed COORD (i16)".into(),
        ));
    }
    Ok((cols, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_are_control_bytes() {
        assert_eq!(pty_key_bytes("enter", None).unwrap(), b"\r");
        assert_eq!(pty_key_bytes("backspace", None).unwrap(), vec![0x7f]);
        assert_eq!(pty_key_bytes("tab", None).unwrap(), b"\t");
        assert_eq!(pty_key_bytes("space", Some("x")).unwrap(), b" ");
        assert_eq!(pty_key_bytes("escape", None).unwrap(), b"\x1b");
        assert_eq!(pty_key_bytes("left", None).unwrap(), b"\x1b[D");
        assert_eq!(pty_key_bytes("right", None).unwrap(), b"\x1b[C");
        assert_eq!(pty_key_bytes("up", None).unwrap(), b"\x1b[A");
        assert_eq!(pty_key_bytes("down", None).unwrap(), b"\x1b[B");
        assert_eq!(pty_key_bytes("home", None).unwrap(), b"\x1b[H");
        assert_eq!(pty_key_bytes("end", None).unwrap(), b"\x1b[F");
        assert_eq!(pty_key_bytes("delete", None).unwrap(), b"\x1b[3~");
        assert_eq!(pty_key_bytes("pageup", None).unwrap(), b"\x1b[5~");
        assert_eq!(pty_key_bytes("pagedown", None).unwrap(), b"\x1b[6~");
        assert_ne!(pty_key_bytes("enter", None).unwrap(), b"\n");
        assert_ne!(pty_key_bytes("left", None).unwrap(), b"\x1b[C");
        assert_ne!(pty_key_bytes("up", None).unwrap(), b"\x1b[B");
        assert!(pty_key_bytes("f1", None).is_none());
        assert_eq!(pty_key_bytes("x", Some("a")).unwrap(), b"a");
        assert_eq!(pty_key_bytes("x", Some("\n")).unwrap(), b"\r");
        assert_eq!(pty_key_bytes("x", Some("\r")).unwrap(), b"\r");
        assert!(pty_key_bytes("x", Some("")).is_none());
        assert!(pty_key_bytes("x", None).is_none());
        assert_eq!(pty_input("c", None, true).unwrap(), vec![0x03]);
        assert_eq!(pty_input("d", None, true).unwrap(), vec![0x04]);
        assert_eq!(pty_input("z", None, true).unwrap(), vec![0x1a]);
        assert_eq!(pty_input("unknown", Some("c"), true).unwrap(), vec![0x03]);
        assert_ne!(pty_input("c", None, true).unwrap(), b"c");
        assert!(pty_input("c", None, false).is_none());
        assert!(pty_input("1", None, true).is_none());
        assert!(pty_input("left", None, true).is_none());
        assert_eq!(pty_paste_bytes("ab\ncd\r\nef\r"), b"ab\rcd\ref\r");
        assert_eq!(pty_paste_bytes("plain"), b"plain");
        assert_ne!(pty_paste_bytes("a\nb"), b"a\nb");
        assert_ne!(pty_paste_bytes("a\r\nb"), b"a\r\nb");
    }

    #[test]
    fn pty_size_rejects_zero_and_i16_overflow() {
        assert!(validate_pty_size(0, 24).is_err());
        assert!(validate_pty_size(80, 0).is_err());
        assert_eq!(validate_pty_size(80, 24).unwrap(), (80, 24));
        assert!(validate_pty_size(i16::MAX as u16, 1).is_ok());
        assert!(validate_pty_size(1, i16::MAX as u16).is_ok());
        assert!(validate_pty_size(i16::MAX as u16 + 1, 1).is_err());
        assert!(validate_pty_size(1, i16::MAX as u16 + 1).is_err());
        assert_eq!(
            validate_pty_size(i16::MAX as u16, i16::MAX as u16).unwrap(),
            (i16::MAX as u16, i16::MAX as u16)
        );
        assert_ne!(validate_pty_size(80, 24).unwrap(), (0, 0));
    }

    #[test]
    fn pty_grid_floors_and_clamps() {
        assert_eq!(pty_grid_from_px(800.0, 480.0, 8.0, 16.0), (100, 30));
        assert_eq!(pty_grid_from_px(100.0, 160.0, 10.0, 20.0), (10, 8));
        assert_eq!(pty_grid_from_px(80.0, 160.0, -8.0, -16.0), (10, 10));
        assert_ne!(pty_grid_from_px(100.0, 160.0, 10.0, 20.0), (12, 10));
        assert_eq!(pty_grid_from_px(7.0, 15.0, 8.0, 16.0), (1, 1));
        assert_eq!(pty_grid_from_px(0.0, 0.0, 8.0, 16.0), (1, 1));
        assert_eq!(pty_grid_from_px(-40.0, 160.0, 8.0, 16.0), (1, 10));
        assert_eq!(pty_grid_from_px(16.0, 32.0, 0.0, 0.0), (2, 2));
        assert_eq!(pty_grid_from_px(f32::NAN, 160.0, 8.0, 16.0), (1, 10));
        assert_eq!(
            pty_grid_from_px(1_000_000.0, 16.0, 8.0, 16.0).0,
            i16::MAX as u16
        );
        assert_ne!(pty_grid_from_px(800.0, 480.0, 8.0, 16.0), (0, 0));
        assert_ne!(pty_grid_from_px(800.0, 480.0, 8.0, 16.0), (100, 29));
        assert_ne!(pty_grid_from_px(800.0, 480.0, 8.0, 16.0), (99, 30));
    }
}
