//! Unified search over threads, files, and palette commands.

use crate::palette::{default_items, PaletteItem};
use crate::Workspace;

/// Where a hit lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Thread,
    File,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub title: String,
    pub hint: String,
}

/// Case-insensitive substring search. Empty query returns nothing (palette owns empty).
pub fn search_workspace(ws: &Workspace, query: &str) -> Vec<SearchHit> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for t in &ws.threads {
        if t.title.to_ascii_lowercase().contains(&q) || t.id.to_ascii_lowercase().contains(&q) {
            hits.push(SearchHit {
                kind: SearchKind::Thread,
                id: t.id.clone(),
                title: t.title.clone(),
                hint: "thread".into(),
            });
        }
    }
    for f in &ws.files {
        if f.to_ascii_lowercase().contains(&q) {
            hits.push(SearchHit {
                kind: SearchKind::File,
                id: f.clone(),
                title: f.clone(),
                hint: "file".into(),
            });
        }
    }
    for item in default_items() {
        if matches_item(&item, &q) {
            hits.push(SearchHit {
                kind: SearchKind::Command,
                id: item.id.to_owned(),
                title: item.label.to_owned(),
                hint: item.hint.to_owned(),
            });
        }
    }
    hits
}

fn matches_item(item: &PaletteItem, q: &str) -> bool {
    item.id.to_ascii_lowercase().contains(q)
        || item.label.to_ascii_lowercase().contains(q)
        || item.hint.to_ascii_lowercase().contains(q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_thread_file_and_command() {
        let mut ws = Workspace::new("p", "m");
        ws.threads[0].title = "Fix palette".into();
        ws.set_files(vec!["src/lib.rs".into(), "Cargo.toml".into()]);
        assert!(search_workspace(&ws, "").is_empty());
        let hits = search_workspace(&ws, "pal");
        assert!(hits.iter().any(|h| h.kind == SearchKind::Thread));
        assert!(hits.iter().any(|h| h.kind == SearchKind::Command));
        let files = search_workspace(&ws, "lib.rs");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].kind, SearchKind::File);
        assert!(search_workspace(&ws, "zzz-none").is_empty());
    }

    #[test]
    fn search_hits_rank_files_threads_commands() {
        let mut ws = Workspace::new("p", "m");
        ws.threads[0].title = "Alpha thread".into();
        ws.set_files(vec!["src/main.rs".into(), "Cargo.toml".into()]);
        let hits = search_workspace(&ws, "a");
        assert!(hits.iter().any(|h| h.kind == SearchKind::File));
        assert!(hits.iter().any(|h| h.kind == SearchKind::Thread));
        assert!(hits.iter().any(|h| h.kind == SearchKind::Command));
    }
}
