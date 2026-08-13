//! Local and Tailscale remote status rows (detect only, no Serve).

/// One remote target shown in Settings / Session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRow {
    pub id: String,
    pub kind: String,
    pub label: String,
}

/// Always list this machine. Add Tailscale only when `which` found a non-empty path.
pub fn detect_remotes(tailscale_which: Option<&str>) -> Vec<RemoteRow> {
    let mut rows = vec![RemoteRow {
        id: "local".into(),
        kind: "local".into(),
        label: "this machine".into(),
    }];
    if let Some(path) = tailscale_which {
        if !path.is_empty() {
            rows.push(RemoteRow {
                id: "tailscale".into(),
                kind: "tailscale".into(),
                label: "Tailscale detected".into(),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_row() -> RemoteRow {
        RemoteRow {
            id: "local".into(),
            kind: "local".into(),
            label: "this machine".into(),
        }
    }

    fn tailscale_row() -> RemoteRow {
        RemoteRow {
            id: "tailscale".into(),
            kind: "tailscale".into(),
            label: "Tailscale detected".into(),
        }
    }

    #[test]
    fn remote_status_lists_local_and_tailscale_detect() {
        let none = detect_remotes(None);
        assert_eq!(none, vec![local_row()]);
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].id, "local");
        assert_eq!(none[0].kind, "local");
        assert_eq!(none[0].label, "this machine");
        assert!(none.iter().all(|r| r.id != "tailscale"));

        let empty = detect_remotes(Some(""));
        assert_eq!(empty, vec![local_row()]);
        assert_eq!(empty.len(), 1);
        assert!(empty.iter().all(|r| r.kind != "tailscale"));

        let found = detect_remotes(Some("/usr/bin/tailscale"));
        assert_eq!(found, vec![local_row(), tailscale_row()]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "local");
        assert_eq!(found[1].id, "tailscale");
        assert_eq!(found[1].kind, "tailscale");
        assert_eq!(found[1].label, "Tailscale detected");
        assert_ne!(found[1].label, "not found");
    }
}
