# 16: Files tree UI

**Scope:** left Projects (`LeftSection::Files`) + right inspector Files
**Against:** plan/36 C / §4.1, plus plan/10 §2.3 and plan/30 Files click
**Read:** `left_rail` Files, inspector Files (`tab_buttons` / `file_rows`), `list_project_tree`, `CycleFile`, plan/36 C
**Date:** 2026-08-12

## Verdict

The Files *tabs* exist. The Files *tree* does not. Both rails paint a shallow, one-shot `Vec<String>` dump. Clicking a row does not select, open, reveal, mention, expand, or refresh. Plan/36 C is the first this-wave gap and is still inventory chrome.

What shipped:

- Left icon-rail label **Projects** (`LeftSection::Files.rail_label()`), list header **FILES**.
- Right inspector tab **Files** (`InspectorTab::Files`).
- Startup `list_project_tree` (depth 2, cap 80, skip `.git` / `node_modules` / `target`), dirs stored with a trailing `/`.
- Headless `selected_file`, `select_file`, `insert_file_mention` on `Workspace`.
- A test *named* `file_tree_select_expand_and_mention`.

What did not ship: `FileNode`, expand/collapse, `files_visible()`, `ClientAction::{SelectFile, ToggleFileExpand, CopyFilePath, InsertFileMention, RefreshFiles}`, Files tab buttons, palette `file:`, `/files`, or any GPUI path that calls `select_file`.

---

## Findings

### F1. Click does nothing useful

- **Severity:** Major
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`left_rail` Files, `inspector_row_el`)
- **Now:** Left file click writes `file {path}` into the terminal strip and returns. Right file click only toggles `right_expanded_id`. File rows have empty `meta`, so expand paints no extra line. `ListRowSpec.selected` stays false. `Workspace.select_file` is never called from GPUI.
- **Expected (plan/36 C):** click / Enter selects the row (mark `*`), copies the relative path to flash. Plan/30: left click focuses the right Files tab and expands `file:{path}`.
- **Evidence:**

```1142:1147:apps/multiplexer-desktop/src/main.rs
                            move |this, cx| {
                                this.term_meta(&format!("file {p}"));
                                cx.notify();
                            },
```

```1883:1887:apps/multiplexer-desktop/src/main.rs
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.workspace.toggle_right_row(id.clone());
                cx.notify();
```

`file_rows` never sets `row.selected` from `ws.selected_file`. Left rows pass `selected: false` always.

---

### F2. No Open, Reveal, or `@` mention

- **Severity:** Major
- **Where:** `apps/multiplexer-desktop/src/inspector.rs` `tab_buttons`; `crates/multiplexer-shell/src/actions.rs`; `crates/multiplexer-shell/src/workspace.rs`
- **Now:** `tab_buttons(InspectorTab::Files)` is `Vec::new()`. `InspectorAction` has no Reveal / mention / copy-path arm. `ClientAction` has `CycleFile` only. `insert_file_mention` exists on `Workspace` and is unused by the desktop. There is no editor open (honest), but there is also no reveal (copy absolute path) and no composer `` `@path` `` insert.
- **Expected (plan/36 C):** buttons **Reveal**, **@ mention**, **Reload**. Double-click / Enter copies the relative path to flash. It does not open an editor.
- **Evidence:**

```77:79:apps/multiplexer-desktop/src/inspector.rs
        InspectorTab::Terminal | InspectorTab::Skills => Vec::new(),
        InspectorTab::Files => Vec::new(),
        InspectorTab::Activity => Vec::new(),
```

```850:866:crates/multiplexer-shell/src/workspace.rs
    pub fn select_file(&mut self, path: impl Into<String>) -> bool {
        // ...
    }

    pub fn insert_file_mention(&mut self) -> bool {
        let Some(path) = self.selected_file.clone() else {
            return false;
        };
        let mention = format!(" `@{path}` ");
```

Dead headless: no `ClientAction::InsertFileMention`, no Files button, no palette `file:` row. Search can *find* a path (`search.rs`) but Enter is not wired to `SelectFile`. Slash has no `/files`. Controls catalog has no `tab_files` / reveal / mention ids.

---

### F3. No refresh

- **Severity:** Major
- **Where:** `apps/multiplexer-desktop/src/main.rs` `new` / `cycle_file`; `list_project_tree` call sites
- **Now:** `list_project_tree` runs once at window construct. There is no `RefreshFiles`, no Files **Reload** button, no host re-walk. Palette **Cycle file** rotates the in-memory vec. Center chip **List project files** only switches both rails to Files. Cores has Reload; Files does not.
- **Expected (plan/36 C):** **Reload** re-runs `list_project_tree`. `ClientAction::RefreshFiles`. Host copies use the existing clipboard path.
- **Evidence:** repo-wide, `list_project_tree` is called only in `ShellView::new`:

```96:107:apps/multiplexer-desktop/src/main.rs
        workspace.set_files(
            list_project_tree(std::path::Path::new(&cwd), ListOptions::default())
                .into_iter()
                .map(|e| {
                    if e.is_dir {
                        format!("{}/", e.path)
                    } else {
                        e.path
                    }
                })
                .collect(),
        );
```

```1357:1362:apps/multiplexer-desktop/src/main.rs
                            .child(chip("List project files", cx, |this, cx| {
                                this.workspace.inspector = InspectorTab::Files;
                                this.workspace.left_section = LeftSection::Files;
                                cx.notify();
                            }))
```

After the first paint, new files on disk never appear.

---

### F4. Flat dump, not a tree

- **Severity:** Major
- **Where:** `crates/multiplexer-client/src/files.rs` `list_project_tree`; `Workspace.files: Vec<String>`; `file_rows`; left Files map
- **Now:** The walker is a depth-2 flatten: all root entries first (dirs first inside that slice), then each dir's children *appended after every sibling*, including root files. UI prints those strings with no indent, no nest, no expand. `FileNode { path, name, is_dir, expanded }` does not exist. `toggle_file_expand` / `files_visible()` do not exist. Cap is 80 with no "truncated" hint. Left rail uses the folder glyph for every row, file or dir.
- **Expected (plan/36 C):** tree rows, directories first, expand/collapse, selected marked `*`. Collapse hides children from `files_visible()`.
- **Evidence:**

```51:68:crates/multiplexer-client/src/files.rs
pub fn list_project_tree(root: &Path, opts: ListOptions) -> Vec<FileEntry> {
    // ...
    let mut out = read_children(root, "", &opts, true);
    out.truncate(opts.max_entries);
    let dirs: Vec<FileEntry> = out.iter().filter(|e| e.is_dir).cloned().collect();
    for dir in dirs {
        // kids appended after the full root slice
        out.extend(kids);
    }
    out
}
```

```208:226:crates/multiplexer-shell/src/inspector_model.rs
fn file_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    // ...
    ws.files.iter().map(|p| {
        let icon = if p.ends_with('/') { Folder } else { Diff };
        ListRowSpec::new(format!("file:{p}"), p.clone())
    })
}
```

On this repo that means `crates/` then `Cargo.toml` then `crates/multiplexer-shell/`, never `crates/` containing children. Depth 2 also hides `crates/*/src/*.rs`, so the "tree" is crate folders plus root files, not a workspace tree. Cores `resource_detail` still dumps the same flat `Files` block (`workspace.rs` ~673).

---

### F5. Empty state is a dead end

- **Severity:** Major
- **Where:** left `file-none` row; right `file:empty` row; `files_detail`
- **Now:** Left empty title **No files listed**, subtitle **Reload from the Files tab**. Click only sets `inspector = Files`. The Files tab has no Reload (F3). Right empty is a single **No files** row with no hint and no action. `files_detail` is `"No project files listed."` Headless `Workspace::new` starts with `files: []`; only the desktop `new` fills it. Missing `cwd` from `list_project_tree` is a silent empty vec.
- **Expected (plan/30):** one muted row **No files yet**, not a blank pane, and not a pointer at a control that does not exist. Plan/36 C: Reload is how you recover.
- **Evidence:**

```1115:1128:apps/multiplexer-desktop/src/main.rs
            LeftSection::Files => list.children(if files.is_empty() {
                vec![list_row(
                    "file-none",
                    ChromeGlyph::Folder.mark(),
                    "No files listed",
                    "Reload from the Files tab",
                    "",
                    false,
                    false,
                    cx,
                    |this, cx| {
                        this.workspace.inspector = InspectorTab::Files;
                        cx.notify();
                    },
                )]
```

```208:211:crates/multiplexer-shell/src/inspector_model.rs
        return vec![
            ListRowSpec::new("file:empty", "No files").with_icon(ChromeGlyph::Folder.mark())
        ];
```

The empty row teaches a false recovery path.

---

### F6. `CycleFile` rotates the vec instead of selecting

- **Severity:** Major
- **Where:** `apps/multiplexer-desktop/src/main.rs` `cycle_file`; palette `cycle-file`; `ClientAction::CycleFile`
- **Now:** The only file action pops index 0 and appends it, then `term_meta` the new head. It does not call `select_file`. It does not jump to the Files tab. Rotating the vec changes row ids (`file-{path}` stays, but visual order and "first file" identity shuffle). Plan/32 already called this hostile to row identity.
- **Expected (plan/32, plan/36 C):** cycle = select the next path after `selected_file` (wrap). Do not rotate `files`. Prefer `SelectFile` + Files tab.
- **Evidence:**

```402:410:apps/multiplexer-desktop/src/main.rs
    fn cycle_file(&mut self) {
        if self.workspace.files.is_empty() {
            self.term_meta("no project files listed");
            return;
        }
        let first = self.workspace.files.remove(0);
        self.workspace.files.push(first);
        self.term_meta(&format!("file {}", self.workspace.files[0]));
    }
```

Palette item id `cycle-file`, label **Cycle file**, no `file:` namespace (`palette.rs`).

---

### F7. Named test does not cover the tree contract

- **Severity:** Major
- **Where:** `crates/multiplexer-shell/src/workspace.rs` `file_tree_select_expand_and_mention`
- **Now:** The plan/36 C test name exists. It only asserts `select_file` + `insert_file_mention` on a two-string vec. It does not expand a dir, does not collapse, does not call `files_visible()` (that API is missing), and does not prove any GPUI button or click. A green test here cannot catch F1 to F6.
- **Expected (plan/36 C):** expand a dir, select a file, `InsertFileMention` puts `` `@src/lib.rs` `` at `cursor`, collapse hides children from `files_visible()`.
- **Evidence:**

```1159:1169:crates/multiplexer-shell/src/workspace.rs
    fn file_tree_select_expand_and_mention() {
        let mut ws = Workspace::new("p", "m");
        ws.set_files(vec!["src/lib.rs".into(), "Cargo.toml".into()]);
        assert!(!ws.select_file("missing.rs"));
        assert!(ws.select_file("src/lib.rs"));
        assert_eq!(ws.selected_file.as_deref(), Some("src/lib.rs"));
        ws.set_draft("see");
        assert!(ws.insert_file_mention());
        assert!(ws.draft.contains("`@src/lib.rs`"));
    }
```

---

## Plan/36 C checklist

| Promised | Now |
|---|---|
| Files inspector tab | Yes |
| Left Projects section | Yes (label only) |
| Tree, dirs first, expand/collapse, `*` | Flat `Vec<String>`, no expand, no `*` |
| Reveal / @ mention / Reload | None |
| Double-click / Enter copies relative path | Left: term line. Right: accordion |
| Palette `file:` | Only **Cycle file** |
| `FileNode`, `toggle_file_expand`, `files_visible` | Missing |
| `selected_file` / `select_file` / `insert_file_mention` | Headless only, unwired |
| `ClientAction::{SelectFile, ToggleFileExpand, CopyFilePath, InsertFileMention, RefreshFiles}` | Missing (`CycleFile` only) |
| `tab_buttons(Files)` nonempty | Empty |
| Host `RefreshFiles` re-walks | `list_project_tree` once at startup |
| Test covers expand + mention + collapse | Name only |

---

FINDINGS: 7
