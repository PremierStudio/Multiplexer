//! Immediate and shallow project file listings for the inspector.
//!
//! Pure `std::fs` so tests can use temp dirs. Call off the UI thread.

use std::cmp::Ordering;
use std::fs;
use std::path::Path;

/// Always omitted from [`list_project_tree`] (not listed, not descended).
const SKIP_TREE_DIR_NAMES: &[&str] = &[".git", "node_modules", "target"];

/// One listed file or directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Path relative to the list root. Tree listings use `/` separators.
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

/// Caps and hidden-name behavior for [`list_project_files`] / [`list_project_tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListOptions {
    /// Maximum entries returned. Default 80.
    pub max_entries: usize,
    /// Skip names that start with `.`. Default true.
    pub skip_hidden: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            max_entries: 80,
            skip_hidden: true,
        }
    }
}

/// Immediate children of `root` (not recursive).
///
/// Sorted directories first, then name case-insensitive. Missing `root` is empty.
pub fn list_project_files(root: &Path, opts: ListOptions) -> Vec<FileEntry> {
    let mut entries = read_children(root, "", &opts, false);
    entries.truncate(opts.max_entries);
    entries
}

/// Shallow walk of `root` (max depth 2). Skips `target/`, `node_modules/`, `.git/`.
///
/// Paths are relative to `root` with `/` separators. Missing `root` is empty.
pub fn list_project_tree(root: &Path, opts: ListOptions) -> Vec<FileEntry> {
    if opts.max_entries == 0 {
        return Vec::new();
    }
    let mut out = read_children(root, "", &opts, true);
    out.truncate(opts.max_entries);

    let dirs: Vec<FileEntry> = out.iter().filter(|e| e.is_dir).cloned().collect();
    for dir in dirs {
        if out.len() >= opts.max_entries {
            break;
        }
        let mut kids = read_children(&root.join(&dir.name), &dir.path, &opts, true);
        let room = opts.max_entries - out.len();
        kids.truncate(room);
        out.extend(kids);
    }
    out
}

fn read_children(
    dir: &Path,
    rel_parent: &str,
    opts: &ListOptions,
    skip_tree_dirs: bool,
) -> Vec<FileEntry> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };
    let mut entries = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        if opts.skip_hidden && is_hidden_name(&name) {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if skip_tree_dirs && is_dir && is_skipped_tree_dir(&name) {
            continue;
        }
        let path = rel_path(rel_parent, &name);
        entries.push(FileEntry { path, name, is_dir });
    }
    sort_entries(&mut entries);
    entries
}

fn rel_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn is_skipped_tree_dir(name: &str) -> bool {
    SKIP_TREE_DIR_NAMES.contains(&name)
}

fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| match b.is_dir.cmp(&a.is_dir) {
        Ordering::Equal => a
            .name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name)),
        other => other,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "mux-client-files-{}-{}-{}-{}",
                label,
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed),
                nanos
            ));
            fs::create_dir_all(&path).expect("create temp root");
            Self { path }
        }

        fn mkdir(&self, name: &str) -> PathBuf {
            let p = self.path.join(name);
            fs::create_dir_all(&p).expect("mkdir");
            p
        }

        fn write(&self, name: &str) {
            fs::write(self.path.join(name), b"x").expect("write file");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn names(entries: &[FileEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    fn paths(entries: &[FileEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn lists_dirs_before_files() {
        let root = TempRoot::new("dirs-first");
        root.write("zebra.txt");
        root.write("B.txt");
        root.write("a.txt");
        root.mkdir("src");
        root.mkdir("Lib");

        let got = list_project_files(&root.path, ListOptions::default());
        assert_eq!(names(&got), ["Lib", "src", "a.txt", "B.txt", "zebra.txt"]);
        assert!(got[0].is_dir && got[1].is_dir);
        assert!(!got[2].is_dir && !got[3].is_dir && !got[4].is_dir);
        assert_eq!(got[0].path, "Lib");
        assert_eq!(got[2].name, "a.txt");
        assert_eq!(got[3].name, "B.txt");
    }

    #[test]
    fn skip_hidden_default() {
        let d = ListOptions::default();
        assert_eq!(d.max_entries, 80);
        assert!(d.skip_hidden);

        let root = TempRoot::new("hidden");
        root.write("visible.txt");
        root.write(".env");
        root.mkdir(".hidden-dir");
        root.mkdir("src");

        let skipped = list_project_files(&root.path, ListOptions::default());
        assert_eq!(names(&skipped), ["src", "visible.txt"]);
        assert!(skipped.iter().all(|e| !e.name.starts_with('.')));

        let shown = list_project_files(
            &root.path,
            ListOptions {
                max_entries: 80,
                skip_hidden: false,
            },
        );
        assert_eq!(names(&shown), [".hidden-dir", "src", ".env", "visible.txt"]);
        assert!(shown.iter().any(|e| e.name == ".env"));
    }

    #[test]
    fn respects_max_entries() {
        let root = TempRoot::new("max");
        root.write("a.txt");
        root.write("b.txt");
        root.write("c.txt");
        root.write("d.txt");
        let d0 = root.mkdir("d0");
        fs::write(d0.join("nested.txt"), b"x").expect("nested");
        fs::write(d0.join("other.txt"), b"x").expect("other");

        let files = list_project_files(
            &root.path,
            ListOptions {
                max_entries: 3,
                skip_hidden: true,
            },
        );
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].name, "d0");
        assert!(files[0].is_dir);
        assert_eq!(names(&files), ["d0", "a.txt", "b.txt"]);

        let none = list_project_files(
            &root.path,
            ListOptions {
                max_entries: 0,
                skip_hidden: true,
            },
        );
        assert!(none.is_empty());
        assert!(list_project_tree(
            &root.path,
            ListOptions {
                max_entries: 0,
                skip_hidden: true,
            },
        )
        .is_empty());

        // 5 immediate children; cap 6 leaves room for one nested path only.
        let tree = list_project_tree(
            &root.path,
            ListOptions {
                max_entries: 6,
                skip_hidden: true,
            },
        );
        assert_eq!(tree.len(), 6);
        assert!(tree.iter().any(|e| e.path == "d0/nested.txt"));
        assert!(tree.iter().all(|e| e.path != "d0/other.txt"));
    }

    #[test]
    fn tree_skips_target_and_git() {
        let root = TempRoot::new("tree-skip");
        root.write("README.md");
        let src = root.mkdir("src");
        fs::write(src.join("lib.rs"), b"x").expect("src/lib.rs");
        let nested = src.join("inner");
        fs::create_dir_all(&nested).expect("src/inner");
        fs::write(nested.join("too-deep.rs"), b"x").expect("depth 3");

        let target = root.mkdir("target");
        fs::create_dir_all(target.join("debug")).expect("target/debug");
        fs::write(target.join("debug").join("foo"), b"x").expect("target file");

        let git = root.mkdir(".git");
        fs::write(git.join("HEAD"), b"ref").expect(".git/HEAD");

        let nm = root.mkdir("node_modules");
        fs::create_dir_all(nm.join("pkg")).expect("node_modules/pkg");
        fs::write(nm.join("pkg").join("index.js"), b"x").expect("nm file");

        let got = list_project_tree(
            &root.path,
            ListOptions {
                max_entries: 80,
                skip_hidden: false,
            },
        );
        let got_paths = paths(&got);
        assert!(got_paths.contains(&"README.md"));
        assert!(got_paths.contains(&"src"));
        assert!(got_paths.contains(&"src/lib.rs"));
        assert!(got_paths.contains(&"src/inner"));
        assert!(
            !got_paths.iter().any(|p| p.contains("too-deep")),
            "max depth 2 must not include grandchildren of src: {got_paths:?}"
        );
        assert!(
            !got_paths
                .iter()
                .any(|p| p.split('/').any(|s| s == "target")),
            "target must be skipped: {got_paths:?}"
        );
        assert!(
            !got_paths.iter().any(|p| p.split('/').any(|s| s == ".git")),
            ".git must be skipped: {got_paths:?}"
        );
        assert!(
            !got_paths
                .iter()
                .any(|p| p.split('/').any(|s| s == "node_modules")),
            "node_modules must be skipped: {got_paths:?}"
        );
        assert!(got.iter().all(|e| !e.path.contains('\\')));
        assert_eq!(
            got.iter()
                .find(|e| e.name == "lib.rs")
                .map(|e| e.path.as_str()),
            Some("src/lib.rs")
        );
    }

    #[test]
    fn missing_root_is_empty_vec() {
        let missing = std::env::temp_dir().join(format!(
            "mux-client-files-missing-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!missing.exists());
        let files = list_project_files(&missing, ListOptions::default());
        let tree = list_project_tree(&missing, ListOptions::default());
        assert!(files.is_empty());
        assert!(tree.is_empty());
        assert_eq!(files, tree);
    }
}
