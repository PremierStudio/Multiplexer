//! Skills and hooks inventory parse (plan/26).
//!
//! Library functions are pure: the caller passes directory listings. Use
//! [`list_dir_entry_names`] off the UI thread when the desktop needs a listing.

use std::collections::HashSet;
use std::path::PathBuf;

/// One discovered skill and whether it came from the user or project dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRow {
    pub name: String,
    /// `"user"` or `"project"`.
    pub source: String,
}

/// User then project skill directories: `{home}/.grok/skills`, `{project}/.grok/skills`.
pub fn skill_dir_candidates(home: &str, project: &str) -> Vec<(String, &'static str)> {
    vec![
        (join_skill_dir(home), "user"),
        (join_skill_dir(project), "project"),
    ]
}

fn join_skill_dir(base: &str) -> String {
    PathBuf::from(base)
        .join(".grok")
        .join("skills")
        .display()
        .to_string()
}

/// Names from a directory listing.
///
/// Directories keep their name. Files ending in `.md` drop that suffix.
/// Skips empty names, `.`, `..`, and any name that starts with `.`.
/// The result is sorted case-insensitively and unique.
pub fn parse_skill_names(dir_listing: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = dir_listing
        .iter()
        .filter_map(|name| skill_name(name))
        .collect();
    names.sort_by_key(|a| a.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

fn skill_name(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.starts_with('.') {
        return None;
    }
    let name = raw.strip_suffix(".md").unwrap_or(raw);
    if name.is_empty() {
        return None;
    }
    Some(name.to_owned())
}

/// Project skills first. User names already present in project are dropped.
pub fn merge_skill_rows(user: &[String], project: &[String]) -> Vec<SkillRow> {
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for name in project {
        if seen.insert(name.as_str()) {
            rows.push(SkillRow {
                name: name.clone(),
                source: String::from("project"),
            });
        }
    }
    for name in user {
        if seen.insert(name.as_str()) {
            rows.push(SkillRow {
                name: name.clone(),
                source: String::from("user"),
            });
        }
    }
    rows
}

/// One hook from a `name:when` inventory line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRow {
    pub name: String,
    pub when: String,
}

/// Parse `name:when` lines. Empty lines, `#` comments, and lines without `:` are skipped.
pub fn parse_hooks_tomlish(text: &str) -> Vec<HookRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, when)) = line.split_once(':') else {
            continue;
        };
        rows.push(HookRow {
            name: name.trim().to_owned(),
            when: when.trim().to_owned(),
        });
    }
    rows
}

/// Immediate entry names under `path`. Missing or unreadable dirs yield `[]`.
///
/// Call this off the UI thread, then pass the names to [`parse_skill_names`].
pub fn list_dir_entry_names(path: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn candidates_two_paths() {
        let home = "C:\\Users\\example";
        let project = "C:\\src\\repo";
        let got = skill_dir_candidates(home, project);
        let expected_user = PathBuf::from(home)
            .join(".grok")
            .join("skills")
            .display()
            .to_string();
        let expected_project = PathBuf::from(project)
            .join(".grok")
            .join("skills")
            .display()
            .to_string();
        assert_eq!(
            got,
            vec![
                (expected_user.clone(), "user"),
                (expected_project.clone(), "project"),
            ]
        );
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, "user");
        assert_eq!(got[1].1, "project");
        assert_ne!(got[0].0, got[1].0);
        assert!(got[0].0.contains(".grok"));
        assert!(got[1].0.contains("skills"));
    }

    #[test]
    fn parse_skips_hidden() {
        let names = parse_skill_names(&[".", "..", ".hidden", ".secret.md", "", "visible", "also"]);
        assert_eq!(names, vec!["also".to_string(), "visible".to_string()]);
        assert!(!names.iter().any(|n| n.starts_with('.')));
        assert!(!names.iter().any(|n| n.is_empty()));
        assert_eq!(parse_skill_names(&[]), Vec::<String>::new());
    }

    #[test]
    fn md_strip() {
        let names = parse_skill_names(&["review.md", "fmt", "deep.md.md", "Zebra.md", "apple"]);
        assert_eq!(
            names,
            vec![
                "apple".to_string(),
                "deep.md".to_string(),
                "fmt".to_string(),
                "review".to_string(),
                "Zebra".to_string(),
            ]
        );
        assert!(!names.iter().any(|n| n.ends_with(".md") && *n != "deep.md"));
        assert!(!names.contains(&"review.md".to_string()));
        assert!(!names.contains(&"Zebra.md".to_string()));

        let dup = parse_skill_names(&["dup.md", "dup", "DUP"]);
        assert_eq!(dup.len(), 1);
        assert!(dup[0].eq_ignore_ascii_case("dup"));
    }

    #[test]
    fn merge_project_wins() {
        let user = vec!["fmt".to_string(), "review".to_string()];
        let project = vec!["fmt".to_string(), "commit".to_string()];
        let rows = merge_skill_rows(&user, &project);
        assert_eq!(
            rows,
            vec![
                SkillRow {
                    name: "fmt".to_string(),
                    source: "project".to_string(),
                },
                SkillRow {
                    name: "commit".to_string(),
                    source: "project".to_string(),
                },
                SkillRow {
                    name: "review".to_string(),
                    source: "user".to_string(),
                },
            ]
        );
        assert_eq!(rows.iter().filter(|r| r.name == "fmt").count(), 1);
        assert_eq!(rows[0].source, "project");
        assert_eq!(rows[2].source, "user");

        let only_user = merge_skill_rows(&["solo".to_string()], &[]);
        assert_eq!(only_user.len(), 1);
        assert_eq!(only_user[0].source, "user");
        let only_project = merge_skill_rows(&[], &["solo".to_string()]);
        assert_eq!(only_project.len(), 1);
        assert_eq!(only_project[0].source, "project");
    }

    #[test]
    fn hooks_skip_comments() {
        let text = "\
# header
fmt:SessionStart

  # indented comment
junk without colon
lint:PreToolUse
fmt = \"nope\"
  wrap : UserPromptSubmit  
";
        let rows = parse_hooks_tomlish(text);
        assert_eq!(
            rows,
            vec![
                HookRow {
                    name: "fmt".to_string(),
                    when: "SessionStart".to_string(),
                },
                HookRow {
                    name: "lint".to_string(),
                    when: "PreToolUse".to_string(),
                },
                HookRow {
                    name: "wrap".to_string(),
                    when: "UserPromptSubmit".to_string(),
                },
            ]
        );
        assert!(!rows.iter().any(|r| r.name.contains('#')));
        assert!(!rows.iter().any(|r| r.name.contains("junk")));
        assert!(!rows.iter().any(|r| r.when.contains("nope")));
    }

    #[test]
    fn list_missing_path_is_empty() {
        let missing = PathBuf::from("C:\\Users\\example")
            .join("definitely-not-a-skills-dir")
            .join("missing");
        assert!(!missing.exists());
        assert!(list_dir_entry_names(&missing.display().to_string()).is_empty());
    }
}
