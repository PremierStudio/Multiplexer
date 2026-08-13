//! Read grok `chat_history.jsonl` into Outlook chat lines.

use std::path::{Path, PathBuf};

use crate::workspace::{ChatMessage, Role};

/// One user or assistant line from grok session history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryLine {
    pub role: Role,
    pub text: String,
}

/// Deterministic UUID so a thread finds the same grok session after restart.
pub fn grok_session_uuid(project: &str, thread_id: &str) -> String {
    let mut hi: u64 = 0xcbf2_9ce4_8422_2325;
    let mut lo: u64 = 0x0000_0100_0000_01b3;
    for b in project.as_bytes().iter().chain(thread_id.as_bytes()) {
        hi ^= u64::from(*b);
        hi = hi.wrapping_mul(0x0100_0000_01b3);
        lo ^= hi.rotate_left(11);
        lo = lo.wrapping_mul(0x0100_0000_01b3);
    }
    let time = (hi >> 32) as u32;
    let mid = (hi >> 16) as u16;
    let ver = ((hi as u16) & 0x0fff) | 0x4000;
    let var = ((lo >> 48) as u16 & 0x3fff) | 0x8000;
    let node = lo & 0x0000_ffff_ffff_ffff;
    format!("{time:08x}-{mid:04x}-{ver:04x}-{var:04x}-{node:012x}")
}

/// Percent-encode a cwd the way grok names `~/.grok/sessions/<dir>`.
pub fn encode_session_cwd(cwd: &str) -> String {
    let mut out = String::new();
    for b in cwd.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b'.' | b'~') {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn default_grok_home() -> PathBuf {
    if let Ok(home) = std::env::var("GROK_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join(".grok")
}

pub fn grok_history_path(grok_home: &Path, cwd: &str, session_id: &str) -> PathBuf {
    grok_home
        .join("sessions")
        .join(encode_session_cwd(cwd))
        .join(session_id)
        .join("chat_history.jsonl")
}

/// Walk `sessions/*/<id>/chat_history.jsonl` when cwd encoding does not match.
pub fn find_history_file(grok_home: &Path, session_id: &str) -> Option<PathBuf> {
    if session_id.trim().is_empty() {
        return None;
    }
    let sessions = grok_home.join("sessions");
    let entries = std::fs::read_dir(sessions).ok()?;
    for ent in entries.flatten() {
        let path = ent.path().join(session_id).join("chat_history.jsonl");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn parse_chat_history(raw: &str) -> Vec<HistoryLine> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("synthetic_reason").and_then(|s| s.as_str()).is_some() {
            continue;
        }
        let kind = v
            .get("type")
            .and_then(|t| t.as_str())
            .or_else(|| v.get("role").and_then(|t| t.as_str()))
            .unwrap_or("");
        let role = match kind {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => continue,
        };
        let Some(text) = extract_text(&v) else {
            continue;
        };
        let text = unwrap_user_query(&text);
        if text.is_empty() {
            continue;
        }
        out.push(HistoryLine { role, text });
    }
    out
}

pub fn apply_grok_history(messages: &mut Vec<ChatMessage>, incoming: &[HistoryLine]) -> bool {
    if incoming.is_empty() {
        return false;
    }
    let mapped: Vec<ChatMessage> = incoming
        .iter()
        .map(|h| ChatMessage {
            role: h.role,
            text: h.text.clone(),
        })
        .collect();
    if mapped == *messages {
        return false;
    }
    *messages = mapped;
    true
}

fn extract_text(v: &serde_json::Value) -> Option<String> {
    let content = v.get("content")?;
    if let Some(s) = content.as_str() {
        return Some(s.trim().to_owned());
    }
    let arr = content.as_array()?;
    let mut parts = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                if !t.trim().is_empty() {
                    parts.push(t.trim().to_owned());
                }
            }
        } else if let Some(t) = block.as_str() {
            if !t.trim().is_empty() {
                parts.push(t.trim().to_owned());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn unwrap_user_query(text: &str) -> String {
    if let Some(start) = text.find("<user_query>") {
        let after = &text[start + "<user_query>".len()..];
        let end = after.find("</user_query>").unwrap_or(after.len());
        after[..end].trim().to_owned()
    } else {
        text.trim().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_is_stable_and_shaped() {
        let a = grok_session_uuid("C:/repo", "thr-1");
        let b = grok_session_uuid("C:/repo", "thr-1");
        let c = grok_session_uuid("C:/repo", "thr-2");
        assert_eq!(a, "15e1198a-711f-41cf-a080-7cc7bdfd8a5b");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 36);
        assert_eq!(a.chars().filter(|ch| *ch == '-').count(), 4);
        assert_eq!(&a[14..15], "4");
        assert_ne!(grok_session_uuid("D:/x", "thr-1"), a);
        let mixed = grok_session_uuid("C:/repo", "thr-1X");
        assert_ne!(mixed, a);
        assert_ne!(&a[0..8], &c[0..8]);
    }

    #[test]
    fn cwd_encode_matches_grok_windows_dir() {
        let got = encode_session_cwd(r"C:\Users\gollum\Development\PremierStudio\Multiplexer");
        assert!(got.starts_with("C%3A%5CUsers%5C"));
        assert!(got.ends_with("Multiplexer"));
        assert!(!got.contains('\\'));
        assert_ne!(
            got,
            r"C:\Users\gollum\Development\PremierStudio\Multiplexer"
        );
        assert_eq!(encode_session_cwd("abc"), "abc");
    }

    #[test]
    fn grok_home_is_never_empty() {
        let home = default_grok_home();
        assert!(!home.as_os_str().is_empty());
        assert!(home.ends_with(".grok") || std::env::var("GROK_HOME").is_ok());
        assert_ne!(home, PathBuf::new());
    }

    #[test]
    fn history_path_joins_session_id() {
        let p = grok_history_path(Path::new("H"), "C:/w", "sid-1");
        assert!(
            p.ends_with("sid-1/chat_history.jsonl") || p.ends_with("sid-1\\chat_history.jsonl")
        );
        assert!(p.to_string_lossy().contains("sessions"));
    }

    #[test]
    fn find_history_walks_session_id() {
        let root = std::env::temp_dir().join(format!(
            "mux-hist-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(1)
        ));
        let sid = "11111111-2222-4333-8444-555555555555";
        let file = root
            .join("sessions")
            .join("C%3A%5Cwork")
            .join(sid)
            .join("chat_history.jsonl");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{\"type\":\"user\",\"content\":\"z\"}\n").unwrap();
        let found = find_history_file(&root, sid).unwrap();
        assert_eq!(found, file);
        assert!(find_history_file(&root, "").is_none());
        assert!(find_history_file(&root, "no-such").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_user_and_assistant_and_skip_synthetic() {
        let raw = r#"
{"type":"system","content":"ignore"}
{"type":"user","content":[{"type":"text","text":"<user_query>hello there</user_query>"}]}
{"type":"user","content":"nope","synthetic_reason":"auto_continue"}
{"type":"assistant","content":[{"type":"text","text":"hi back"}]}
{"type":"tool_result","content":"no"}
"#;
        let got = parse_chat_history(raw);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].role, Role::User);
        assert_eq!(got[0].text, "hello there");
        assert_eq!(got[1].role, Role::Assistant);
        assert_eq!(got[1].text, "hi back");
        assert_ne!(got[0].text, "nope");
    }

    #[test]
    fn apply_history_replaces_when_changed() {
        let incoming = parse_chat_history(
            r#"{"type":"user","content":"q"}
{"type":"assistant","content":"a"}"#,
        );
        let mut msgs = vec![ChatMessage {
            role: Role::User,
            text: "q".into(),
        }];
        assert!(apply_grok_history(&mut msgs, &incoming));
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1].text, "a");
        assert!(!apply_grok_history(&mut msgs, &incoming));
        assert!(!apply_grok_history(&mut msgs, &[]));
    }
}
