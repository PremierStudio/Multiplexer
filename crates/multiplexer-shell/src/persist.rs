//! AppData persist: leaf names, deep links, About, first-run, crash, layout.

use std::path::{Path, PathBuf};

use crate::Workspace;

/// Last path component. Never empty. Windows and POSIX separators.
pub fn leaf_name(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return path.trim().to_owned();
    }
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

/// Card title for a thread. Never paint raw `thr-N`.
pub fn thread_leaf_title(title: &str, id: &str) -> String {
    let t = title.trim();
    if t.is_empty() || t == id || t.starts_with("thr-") {
        "New chat".to_owned()
    } else {
        t.to_owned()
    }
}

/// `multiplexer://pair|session|open` plus optional query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLink {
    Pair { hint: Option<String> },
    Session { id: Option<String> },
    Open { path: Option<String> },
}

impl DeepLink {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Pair { .. } => "pair",
            Self::Session { .. } => "session",
            Self::Open { .. } => "open",
        }
    }
}

/// Parse `multiplexer://pair`, `multiplexer://session?id=x`, `multiplexer://open?path=y`.
pub fn parse_deep_link(raw: &str) -> Option<DeepLink> {
    let rest = raw.trim().strip_prefix("multiplexer://")?;
    if rest.is_empty() {
        return None;
    }
    let (kind, query) = rest.split_once('?').unwrap_or((rest, ""));
    let kind = kind.trim().trim_end_matches('/').to_ascii_lowercase();
    let value = query_value(query);
    match kind.as_str() {
        "pair" => Some(DeepLink::Pair { hint: value }),
        "session" => Some(DeepLink::Session { id: value }),
        "open" => Some(DeepLink::Open { path: value }),
        _ => None,
    }
}

fn query_value(query: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if matches!(k, "id" | "path" | "hint" | "code" | "q") && !v.is_empty() {
                return Some(v.to_owned());
            }
        } else if !part.is_empty() {
            return Some(part.to_owned());
        }
    }
    None
}

/// Apply a deep link. Pair is an honest stub until relay exists.
pub fn apply_deep_link(ws: &mut Workspace, link: &DeepLink) -> String {
    match link {
        DeepLink::Pair { .. } => {
            ws.push_notice(
                crate::notices::NoticeKind::Info,
                "Device exchange later. Pairing is a stub.",
            );
            "Device exchange later".into()
        }
        DeepLink::Session { id } => {
            if let Some(id) = id {
                if ws.select_thread_id(id) {
                    format!("opened thread {id}")
                } else {
                    ws.connect(vec![id.clone()]);
                    format!("session id noted {id}")
                }
            } else {
                "session link missing id".into()
            }
        }
        DeepLink::Open { path } => {
            if let Some(path) = path {
                let _ = ws.select_file(path.clone());
                let _ = ws.select_left_section(crate::workspace::LeftSection::Files);
                format!("open {path}")
            } else {
                "open link missing path".into()
            }
        }
    }
}

/// About box. Updates stay `not shipped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AboutInfo {
    pub version: String,
    pub sha: String,
    pub license: String,
    pub grok_path: String,
    pub updates: String,
}

pub fn about_info(grok_path: Option<&str>) -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        sha: option_env!("GIT_SHA").unwrap_or("dev").to_owned(),
        license: "Apache-2.0".into(),
        grok_path: grok_path
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("grok (not detected)")
            .to_owned(),
        updates: "not shipped".into(),
    }
}

impl AboutInfo {
    pub fn lines(&self) -> String {
        format!(
            "Multiplexer {}\nSHA {}\n{}\ngrok {}\nUpdates: {}",
            self.version, self.sha, self.license, self.grok_path, self.updates
        )
    }
}

pub fn first_run_keychain_notice() -> &'static str {
    "Secrets stay in the OS keychain. Multiplexer never asks you to type a key here."
}

pub fn crash_restore_notice() -> &'static str {
    "Restored chats and drafts. Files and checkpoints were not replayed."
}

pub fn default_appdata_dir() -> PathBuf {
    let root = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("Multiplexer")
}

pub fn default_first_run_path() -> PathBuf {
    default_appdata_dir().join("first-run.json")
}

pub fn default_crash_path() -> PathBuf {
    default_appdata_dir().join("crash-journal.json")
}

pub fn default_layout_path(project: &str) -> PathBuf {
    let key = leaf_name(project);
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = if safe.is_empty() {
        "workspace".to_owned()
    } else {
        safe
    };
    default_appdata_dir()
        .join("layouts")
        .join(format!("{safe}.json"))
}

pub fn first_run_completed(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(raw) => raw.contains("\"done\":true") || raw.contains("\"done\": true"),
        Err(_) => false,
    }
}

pub fn write_first_run_done(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, r#"{"done":true}"#).map_err(|e| e.to_string())
}

/// Threads plus drafts. Marker means a crash restore is pending.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrashJournal {
    pub threads: Vec<CrashThread>,
    pub drafts: Vec<(String, String, usize)>,
    pub marker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashThread {
    pub id: String,
    pub title: String,
    pub status: String,
    pub model: String,
    pub messages: Vec<(String, String)>,
}

pub fn crash_journal_to_json(j: &CrashJournal) -> String {
    let threads: Vec<serde_json::Value> = j
        .threads
        .iter()
        .map(|t| {
            let msgs: Vec<serde_json::Value> = t
                .messages
                .iter()
                .map(|(role, text)| serde_json::json!([role, text]))
                .collect();
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "status": t.status,
                "model": t.model,
                "messages": msgs,
            })
        })
        .collect();
    let drafts: Vec<serde_json::Value> = j
        .drafts
        .iter()
        .map(|(id, text, cur)| serde_json::json!([id, text, cur]))
        .collect();
    serde_json::json!({
        "threads": threads,
        "drafts": drafts,
        "marker": j.marker,
    })
    .to_string()
}

pub fn crash_journal_from_json(raw: &str) -> CrashJournal {
    let mut out = CrashJournal::default();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return out;
    };
    if let Some(arr) = v.get("threads").and_then(|x| x.as_array()) {
        for item in arr {
            let id = item
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_owned();
            if id.is_empty() {
                continue;
            }
            let mut messages = Vec::new();
            if let Some(msgs) = item.get("messages").and_then(|x| x.as_array()) {
                for m in msgs {
                    if let Some(pair) = m.as_array() {
                        if pair.len() == 2 {
                            if let (Some(r), Some(t)) = (pair[0].as_str(), pair[1].as_str()) {
                                messages.push((r.to_owned(), t.to_owned()));
                            }
                        }
                    }
                }
            }
            out.threads.push(CrashThread {
                id,
                title: item
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("New chat")
                    .to_owned(),
                status: item
                    .get("status")
                    .and_then(|x| x.as_str())
                    .unwrap_or("idle")
                    .to_owned(),
                model: item
                    .get("model")
                    .and_then(|x| x.as_str())
                    .unwrap_or("grok")
                    .to_owned(),
                messages,
            });
        }
    }
    if let Some(arr) = v.get("drafts").and_then(|x| x.as_array()) {
        for item in arr {
            if let Some(pair) = item.as_array() {
                if pair.len() >= 2 {
                    if let (Some(id), Some(text)) = (pair[0].as_str(), pair[1].as_str()) {
                        let cur = pair.get(2).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        out.drafts.push((id.to_owned(), text.to_owned(), cur));
                    }
                }
            }
        }
    }
    out.marker = v.get("marker").and_then(|x| x.as_bool()).unwrap_or(false);
    out
}

pub fn write_crash_journal(path: &Path, j: &CrashJournal) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, crash_journal_to_json(j)).map_err(|e| e.to_string())
}

pub fn read_crash_journal(path: &Path) -> CrashJournal {
    match std::fs::read_to_string(path) {
        Ok(raw) => crash_journal_from_json(&raw),
        Err(_) => CrashJournal::default(),
    }
}

pub fn journal_from_workspace(ws: &Workspace) -> CrashJournal {
    CrashJournal {
        threads: ws
            .threads
            .iter()
            .map(|t| CrashThread {
                id: t.id.clone(),
                title: t.title.clone(),
                status: t.status.clone(),
                model: t.model.clone(),
                messages: t
                    .messages
                    .iter()
                    .map(|m| {
                        let role = match m.role {
                            crate::workspace::Role::User => "user",
                            crate::workspace::Role::Assistant => "assistant",
                        };
                        (role.to_owned(), m.text.clone())
                    })
                    .collect(),
            })
            .collect(),
        drafts: ws.thread_drafts.clone(),
        marker: true,
    }
}

/// Outlook chrome snapshot keyed by project.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutPersist {
    pub project: String,
    pub left: String,
    pub right: String,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_open: bool,
    pub bottom_hidden: bool,
    pub bottom_height: f32,
    pub inspector_popped: bool,
}

impl LayoutPersist {
    pub fn from_workspace(ws: &Workspace) -> Self {
        Self {
            project: ws.project.clone(),
            left: rail_name(ws.chrome.left),
            right: rail_name(ws.chrome.right),
            left_width: ws.chrome.left_width,
            right_width: ws.chrome.right_width,
            bottom_open: ws.bottom_open,
            bottom_hidden: ws.bottom_hidden,
            bottom_height: ws.bottom_height,
            inspector_popped: ws.inspector_popped,
        }
    }
}

fn rail_name(v: crate::workspace::RailVis) -> String {
    match v {
        crate::workspace::RailVis::Open => "open".into(),
        crate::workspace::RailVis::IconRail => "icon".into(),
        crate::workspace::RailVis::Hidden => "hidden".into(),
    }
}

pub fn parse_rail(name: &str) -> crate::workspace::RailVis {
    match name {
        "icon" => crate::workspace::RailVis::IconRail,
        "hidden" => crate::workspace::RailVis::Hidden,
        _ => crate::workspace::RailVis::Open,
    }
}

pub fn layout_to_json(l: &LayoutPersist) -> String {
    serde_json::json!({
        "project": l.project,
        "left": l.left,
        "right": l.right,
        "left_width": l.left_width,
        "right_width": l.right_width,
        "bottom_open": l.bottom_open,
        "bottom_hidden": l.bottom_hidden,
        "bottom_height": l.bottom_height,
        "inspector_popped": l.inspector_popped,
    })
    .to_string()
}

pub fn layout_from_json(raw: &str) -> Option<LayoutPersist> {
    let v = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    Some(LayoutPersist {
        project: v
            .get("project")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_owned(),
        left: v
            .get("left")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_owned(),
        right: v
            .get("right")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_owned(),
        left_width: v
            .get("left_width")
            .and_then(|x| x.as_f64())
            .unwrap_or(248.0) as f32,
        right_width: v
            .get("right_width")
            .and_then(|x| x.as_f64())
            .unwrap_or(300.0) as f32,
        bottom_open: v
            .get("bottom_open")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        bottom_hidden: v
            .get("bottom_hidden")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        bottom_height: v
            .get("bottom_height")
            .and_then(|x| x.as_f64())
            .unwrap_or(36.0) as f32,
        inspector_popped: v
            .get("inspector_popped")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
    })
}

pub fn write_layout(path: &Path, l: &LayoutPersist) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, layout_to_json(l)).map_err(|e| e.to_string())
}

pub fn read_layout(path: &Path) -> Option<LayoutPersist> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| layout_from_json(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    #[test]
    fn leaf_name_takes_last_segment() {
        assert_eq!(leaf_name("C:\\repo\\src\\main.rs"), "main.rs");
        assert_eq!(leaf_name("/home/u/proj/lib.rs"), "lib.rs");
        assert_eq!(leaf_name("src/"), "src");
        assert_eq!(leaf_name("plain"), "plain");
        assert_ne!(leaf_name("a/b/c"), "a/b/c");
        assert_eq!(leaf_name(""), "");
        assert_eq!(leaf_name("  "), "");
    }

    #[test]
    fn thread_leaf_never_shows_raw_id() {
        assert_eq!(thread_leaf_title("Hello", "thr-1"), "Hello");
        assert_eq!(thread_leaf_title("thr-2", "thr-2"), "New chat");
        assert_eq!(thread_leaf_title("thr-9", "other"), "New chat");
        assert_eq!(thread_leaf_title("", "thr-9"), "New chat");
        assert_eq!(thread_leaf_title("  ", "thr-9"), "New chat");
        assert_ne!(thread_leaf_title("Build it", "thr-1"), "thr-1");
    }

    #[test]
    fn deep_links_parse_three_kinds() {
        assert_eq!(
            parse_deep_link("multiplexer://pair"),
            Some(DeepLink::Pair { hint: None })
        );
        assert_eq!(
            parse_deep_link("multiplexer://pair?code=abc"),
            Some(DeepLink::Pair {
                hint: Some("abc".into())
            })
        );
        assert_eq!(
            parse_deep_link("multiplexer://session?id=thr-1"),
            Some(DeepLink::Session {
                id: Some("thr-1".into())
            })
        );
        assert_eq!(
            parse_deep_link("multiplexer://open?path=src/lib.rs"),
            Some(DeepLink::Open {
                path: Some("src/lib.rs".into())
            })
        );
        assert_eq!(
            parse_deep_link("multiplexer://pair?code="),
            Some(DeepLink::Pair { hint: None })
        );
        assert_eq!(
            parse_deep_link("multiplexer://pair?foo=bar"),
            Some(DeepLink::Pair { hint: None })
        );
        assert_eq!(
            parse_deep_link("multiplexer://pair?&x"),
            Some(DeepLink::Pair {
                hint: Some("x".into())
            })
        );
        assert_eq!(parse_deep_link("https://example.com"), None);
        assert_eq!(parse_deep_link("multiplexer://"), None);
        assert_eq!(parse_deep_link("multiplexer://nope"), None);
        assert_eq!(
            parse_deep_link("  multiplexer://PAIR/  ")
                .unwrap()
                .kind_label(),
            "pair"
        );
    }

    #[test]
    fn pair_link_is_honest_stub() {
        let mut ws = Workspace::new("p", "m");
        let note = apply_deep_link(&mut ws, &DeepLink::Pair { hint: None });
        assert!(note.contains("later"));
        assert!(ws.notices.iter().any(|n| n.text.contains("later")));
        assert!(!note.to_lowercase().contains("paired"));
    }

    #[test]
    fn session_and_open_links_mutate() {
        let mut ws = Workspace::new("p", "m");
        ws.new_thread();
        let id = ws.threads[0].id.clone();
        let note = apply_deep_link(
            &mut ws,
            &DeepLink::Session {
                id: Some(id.clone()),
            },
        );
        assert!(note.contains("opened"));
        assert_eq!(ws.selected, 0);
        ws.set_files(vec!["src/lib.rs".into()]);
        let open = apply_deep_link(
            &mut ws,
            &DeepLink::Open {
                path: Some("src/lib.rs".into()),
            },
        );
        assert!(open.contains("src/lib.rs"));
        assert_eq!(ws.selected_file.as_deref(), Some("src/lib.rs"));
        assert_eq!(
            apply_deep_link(&mut ws, &DeepLink::Session { id: None }),
            "session link missing id"
        );
        assert_eq!(
            apply_deep_link(&mut ws, &DeepLink::Open { path: None }),
            "open link missing path"
        );
    }

    #[test]
    fn about_never_claims_updates() {
        let a = about_info(None);
        assert_eq!(a.updates, "not shipped");
        assert_eq!(a.license, "Apache-2.0");
        assert!(a.grok_path.contains("not detected"));
        assert!(!a.lines().to_lowercase().contains("up to date"));
        let with = about_info(Some("C:\\bin\\grok.exe"));
        assert!(with.grok_path.contains("grok.exe"));
        assert_ne!(with.grok_path, a.grok_path);
        assert!(!a.version.is_empty());
        assert_eq!(a.sha, "dev");
        let lines = a.lines();
        assert!(lines.contains(&a.version));
        assert!(lines.contains("Apache-2.0"));
        assert!(lines.contains("not shipped"));
        assert!(lines.contains("SHA"));
        assert!(!lines.is_empty());
        assert_ne!(lines, "xyzzy");
    }

    #[test]
    fn first_run_and_crash_notices_are_honest() {
        assert!(first_run_keychain_notice().contains("keychain"));
        assert!(first_run_keychain_notice().contains("never"));
        assert!(!first_run_keychain_notice().to_lowercase().contains("paste"));
        assert!(crash_restore_notice().contains("drafts"));
        assert!(crash_restore_notice().contains("not replayed"));
        assert!(!crash_restore_notice().contains("resumed"));
    }

    #[test]
    fn first_run_flag_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "mux-fr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(1)
        ));
        let path = dir.join("first-run.json");
        assert!(!first_run_completed(&path));
        write_first_run_done(&path).expect("write");
        assert!(first_run_completed(&path));
        assert_eq!(
            default_first_run_path()
                .file_name()
                .and_then(|s| s.to_str()),
            Some("first-run.json")
        );
        assert_eq!(
            default_crash_path().file_name().and_then(|s| s.to_str()),
            Some("crash-journal.json")
        );
        assert_eq!(
            default_appdata_dir().file_name().and_then(|s| s.to_str()),
            Some("Multiplexer")
        );
        assert!(!default_appdata_dir().as_os_str().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crash_journal_roundtrip_and_invalid() {
        let j = CrashJournal {
            threads: vec![CrashThread {
                id: "thr-1".into(),
                title: "Hello".into(),
                status: "idle".into(),
                model: "grok".into(),
                messages: vec![("user".into(), "hi".into())],
            }],
            drafts: vec![("thr-1".into(), "draft".into(), 2)],
            marker: true,
        };
        let raw = crash_journal_to_json(&j);
        assert!(raw.contains("thr-1"));
        assert!(raw.contains("draft"));
        assert!(!raw.contains("op://"));
        let back = crash_journal_from_json(&raw);
        assert_eq!(back.threads.len(), 1);
        assert_eq!(back.threads[0].title, "Hello");
        assert_eq!(back.drafts[0].1, "draft");
        assert!(back.marker);
        assert_eq!(crash_journal_from_json("nope"), CrashJournal::default());
        let empty_id = crash_journal_from_json(r#"{"threads":[{"id":""}],"marker":false}"#);
        assert!(empty_id.threads.is_empty());
        assert!(!empty_id.marker);
        let short_msg = crash_journal_from_json(
            r#"{"threads":[{"id":"t","messages":[["only"]]}],"drafts":[["id"]],"marker":true}"#,
        );
        assert!(short_msg.threads[0].messages.is_empty());
        assert!(short_msg.drafts.is_empty());
    }

    #[test]
    fn crash_journal_write_read_temp() {
        let dir = std::env::temp_dir().join(format!(
            "mux-cj-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(2)
        ));
        let path = dir.join("crash-journal.json");
        let mut ws = Workspace::new("p", "m");
        ws.set_draft("keep");
        ws.send_draft();
        let mut j = journal_from_workspace(&ws);
        assert!(j.marker);
        assert!(!j.threads.is_empty());
        write_crash_journal(&path, &j).expect("write");
        let loaded = read_crash_journal(&path);
        assert_eq!(loaded.threads[0].title, "keep");
        assert_eq!(
            read_crash_journal(Path::new("C:/no/such/mux-crash.json")),
            CrashJournal::default()
        );
        j.marker = false;
        assert!(!j.marker);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_json_roundtrip() {
        let mut ws = Workspace::new("C:/work/proj", "m");
        ws.chrome.hide_left();
        ws.inspector_popped = true;
        let snap = LayoutPersist::from_workspace(&ws);
        let raw = layout_to_json(&snap);
        assert!(raw.contains("hidden"));
        assert!(raw.contains("inspector_popped"));
        let back = layout_from_json(&raw).unwrap();
        assert_eq!(back.left, "hidden");
        assert!(back.inspector_popped);
        assert_eq!(parse_rail("icon"), crate::workspace::RailVis::IconRail);
        assert_eq!(parse_rail("hidden"), crate::workspace::RailVis::Hidden);
        assert_eq!(parse_rail("nope"), crate::workspace::RailVis::Open);
        assert!(layout_from_json("nope").is_none());
        let path = default_layout_path("C:/work/proj");
        assert!(path.to_string_lossy().contains("layouts"));
        assert!(path.file_name().unwrap().to_string_lossy().contains("proj"));
        let spaced = default_layout_path("My Project!");
        assert!(spaced
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("My_Project_"));
        let empty = default_layout_path("");
        assert_eq!(
            empty.file_name().and_then(|s| s.to_str()),
            Some("workspace.json")
        );
        let dashed = default_layout_path("feat-name_1");
        assert_eq!(
            dashed.file_name().and_then(|s| s.to_str()),
            Some("feat-name_1.json")
        );
        let dir = std::env::temp_dir().join(format!(
            "mux-lay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(3)
        ));
        let disk = dir.join("layout.json");
        write_layout(&disk, &snap).expect("write layout");
        let loaded = read_layout(&disk).expect("read layout");
        assert_eq!(loaded.left, "hidden");
        assert!(loaded.inspector_popped);
        assert!(read_layout(Path::new("C:/no/such/mux-layout.json")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
