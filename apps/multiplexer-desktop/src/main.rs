//! Multiplexer desktop: glass chrome, live grok -p, working inspector and terminal.

mod controls;
mod inspector;
mod rows;
mod theme;
mod widgets;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, ClipboardItem, Context, CursorStyle,
    KeyDownEvent, MouseButton, MouseMoveEvent, SharedString, Window,
};
use inspector::tab_buttons;
use multiplexer_checkpoint::{HiddenGitStore, ProcessGitExec};
use multiplexer_client::{
    list_project_tree, spawn_command, spawn_grok_tui, spawn_grok_turn, windows_cmd, CommandResult,
    ListOptions, TuiLaunch, TurnRequest, TurnResult,
};
use multiplexer_mcp::{
    list_dir_entry_names, load_user_mcp_inventory, merge_skill_rows, parse_hooks_tomlish,
    parse_skill_names, skill_dir_candidates,
};
use multiplexer_resman::sample_cores;
use multiplexer_server::Server;
use multiplexer_shell::{
    about_info, activity_items, apply_deep_link, apply_layout_action, auto_dismisses,
    bottom_height_from_mouse, cap_text, default_browser_candidates, default_crash_path,
    default_first_run_path, default_layout_path, default_settings_path, delete_forward,
    detect_browsers, detect_remotes, first_run_completed, first_run_keychain_notice, format_line,
    git_diff_line, help_text, hit_action, insert_at, inspector_rows, is_tui_hatch,
    join_project_path, journal_from_workspace, leaf_name, menu_for, merge_cores, merge_mcp,
    merge_models, move_end, move_home, move_left, move_right, move_word_left, move_word_right,
    open_external_program, palette_hits, parse_builtin, parse_deep_link, parse_model_keys,
    parse_slash, plan_send, read_crash_journal, read_layout, read_settings, remotes_pill_label,
    remotes_serve_note, row_detail, search_workspace, slash_arg, status_from, status_line,
    thread_leaf_title, title_overflow, visible_notices, visible_tail, working_copy,
    write_crash_journal, write_first_run_done, write_layout, write_settings, BindingTable,
    BuiltinCmd, CenterMode, CheckpointRow, Chord, ChromeGlyph, ClientAction, CoreRow, FocusRegion,
    InspectorTab, LeftSection, McpRow, MenuKind, NoticeKind, PaletteState, RemoteRow, Role,
    SearchKind, SendPlan, SettingsSection, SkillItem, SlashCommand, TermLineKind, TuiLife,
    Workspace, WorktreeCard, DIFF_TEXT_CAP, NOTICE_AUTO_MS, TERM_PROMPT,
};
use multiplexer_terminal::ProcessCapture;
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use multiplexer_wire::protocol::PROTOCOL_VERSION;
use multiplexer_worktree::{reminder_from_list, Worktree};
use rows::{inspector_row_el, list_row};
use serde_json::{json, Value};
use theme::Theme;
use widgets::{chip, click_pill, empty_center, ghost_btn, glass_bar, glass_pane, icon_btn};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragRail {
    Left,
    Right,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Composer,
    Terminal,
    Palette,
    Search,
    FileFilter,
}

pub(crate) struct ShellView {
    workspace: Workspace,
    server: Server<
        multiplexer_server::ProviderBridge<
            multiplexer_provider::GrokAdapter<multiplexer_provider::CliGrokFactory>,
        >,
    >,
    session_id: Option<String>,
    drag: Option<DragRail>,
    pending_turn: Option<Receiver<TurnResult>>,
    pending_cmd: Option<Receiver<CommandResult>>,
    palette: PaletteState,
    focus: Focus,
    ignore_turn: bool,
    last_core_sample: Instant,
    remotes: Vec<RemoteRow>,
    grok_tui: Option<std::process::Child>,
    pending_turn_diffs: bool,
    win_w: f32,
    bindings: BindingTable,
    notice_born: Vec<(u64, Instant)>,
    capture: Option<ProcessCapture>,
    pending_diff: Option<(String, Receiver<CommandResult>)>,
    turn_started: Option<Instant>,
}

impl ShellView {
    fn new() -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
        let mut workspace = Workspace::new(cwd.clone(), "grok");
        let loaded = read_settings(&default_settings_path());
        workspace.settings = loaded;
        if !workspace.settings.default_model.trim().is_empty() {
            workspace.model = workspace.settings.default_model.clone();
        }
        workspace.set_models(vec!["grok".into(), "grok-4.6".into(), "fake".into()]);
        workspace.cores = sample_cores(&[])
            .into_iter()
            .map(|c| CoreRow {
                index: c.index,
                usage: c.usage,
                reserved: c.reserved,
            })
            .collect();
        workspace.mcp = load_user_mcp_inventory()
            .into_iter()
            .map(|row| McpRow {
                name: row.name,
                command: row.command,
                transport: row.transport,
                state: multiplexer_shell::McpLife::Stopped,
            })
            .collect();
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
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        let cands = skill_dir_candidates(&home, &cwd);
        let user = list_dir_entry_names(&cands[0].0);
        let project = list_dir_entry_names(&cands[1].0);
        let user_names: Vec<&str> = user.iter().map(String::as_str).collect();
        let project_names: Vec<&str> = project.iter().map(String::as_str).collect();
        let rows = merge_skill_rows(
            &parse_skill_names(&user_names),
            &parse_skill_names(&project_names),
        );
        workspace.set_skills(
            rows.into_iter()
                .map(|r| format!("{} [{}]", r.name, r.source))
                .collect(),
        );
        let mut store = HiddenGitStore::new(ProcessGitExec::new(), cwd.clone());
        match store.create("local", "start") {
            Ok(start) => {
                workspace.git_checkpoints = true;
                workspace.checkpoints.push(CheckpointRow {
                    id: start.id.to_string(),
                    label: start.label,
                });
            }
            Err(err) => {
                workspace.push_notice(
                    NoticeKind::Warn,
                    format!("hidden-git start snapshot skipped: {err}"),
                );
            }
        }
        let server = Server::with_local();
        server.install_checkpoints(store);
        let bindings = workspace.settings.binding_table();
        let mut view = Self {
            workspace,
            server,
            session_id: None,
            drag: None,
            pending_turn: None,
            pending_cmd: None,
            palette: PaletteState::new(),
            focus: Focus::Composer,
            ignore_turn: false,
            last_core_sample: Instant::now(),
            remotes: detect_remotes(tailscale_which().as_deref()),
            grok_tui: None,
            pending_turn_diffs: false,
            win_w: 1360.0,
            bindings,
            notice_born: Vec::new(),
            capture: None,
            pending_diff: None,
            turn_started: None,
        };
        view.apply_theme();
        view.restore_persist();
        view.handshake();
        view.bootstrap_catalogs();
        view.refresh_skills();
        view.refresh_worktrees();
        view.refresh_reminder();
        view.run_shell("git status --short --branch");
        view.term_meta("ready  grok -p off the UI thread  Ctrl+K palette  F1 help  F2 settings");
        assert!(controls::no_dead_labels());
        assert_eq!(controls::REQUIRED_IDS.len(), controls::all_controls().len());
        assert!(controls::control_by_id("send").is_some());
        assert!(!controls::controls_on(controls::Surface::TitleBar).is_empty());
        assert!(controls::shortcut_map()
            .iter()
            .any(|(key, _)| *key == "ctrl-k"));
        assert_eq!(controls::Surface::all().len(), 12);
        view
    }

    fn action_ctx(&self) -> multiplexer_shell::ActionContext {
        multiplexer_shell::ActionContext {
            session_id: self.session_id.clone(),
            project: self.workspace.project.clone(),
            checkpoint_id: self.workspace.selected_checkpoint.clone(),
            approval_request_id: self
                .workspace
                .pending_approval()
                .map(|a| a.request_id.clone()),
            model: self.workspace.model.clone(),
            wt_path: self.workspace.wt_path.clone(),
            wt_branch: self.workspace.wt_branch.clone(),
            wt_create_branch: self.workspace.wt_create_branch,
        }
    }

    fn apply_theme(&self) {
        Theme::set_mode(self.workspace.settings.mode);
        Theme::set_density(self.workspace.settings.density);
        Theme::set_high_contrast(self.workspace.settings.high_contrast);
        Theme::set_ui_scale(self.workspace.settings.ui_scale);
    }

    fn restore_persist(&mut self) {
        let first = default_first_run_path();
        if !first_run_completed(&first) {
            self.workspace.first_run_open = true;
            self.workspace
                .push_notice(NoticeKind::Info, first_run_keychain_notice());
        }
        let crash = read_crash_journal(&default_crash_path());
        if crash.marker {
            let _ = self.workspace.restore_crash(&crash);
            let mut cleared = crash;
            cleared.marker = false;
            let _ = write_crash_journal(&default_crash_path(), &cleared);
        }
        if let Some(layout) = read_layout(&default_layout_path(&self.workspace.project)) {
            self.workspace.apply_layout_persist(&layout);
        }
        if let Some(raw) = std::env::args().nth(1) {
            if let Some(link) = parse_deep_link(&raw) {
                let note = apply_deep_link(&mut self.workspace, &link);
                self.term_meta(&note);
            }
        }
    }

    fn persist_layout(&self) {
        let snap = multiplexer_shell::LayoutPersist::from_workspace(&self.workspace);
        let _ = write_layout(&default_layout_path(&self.workspace.project), &snap);
    }

    fn persist_crash(&self) {
        let j = journal_from_workspace(&self.workspace);
        let _ = write_crash_journal(&default_crash_path(), &j);
    }

    pub(crate) fn open_row_menu(&mut self, id: &str) {
        let mut menu = if id.starts_with("thr-") || id.starts_with("agent-") {
            Some(menu_for(MenuKind::Thread, id))
        } else if id.starts_with("file:") || id.starts_with("file-") {
            Some(menu_for(MenuKind::File, id))
        } else if id.starts_with("mcp:") {
            Some(menu_for(MenuKind::Mcp, id))
        } else if id.starts_with("diff:") {
            Some(menu_for(MenuKind::Diff, id))
        } else {
            None
        };
        if let Some(menu) = menu.as_mut() {
            if menu.kind == MenuKind::Thread {
                let idx = id
                    .strip_prefix("thr-")
                    .and_then(|s| s.parse().ok())
                    .or_else(|| {
                        id.strip_prefix("agent-")
                            .and_then(|tid| self.workspace.threads.iter().position(|t| t.id == tid))
                    })
                    .unwrap_or(self.workspace.selected);
                if let Some(item) = menu.items.first_mut() {
                    item.action = ClientAction::SelectThread(idx);
                }
                let _ = self.workspace.select(idx);
            }
        }
        self.workspace.context_menu = menu;
    }

    fn bootstrap_catalogs(&mut self) {
        let models = self
            .server
            .handle_frame(&rpc("ml", methods::MODEL_LIST, json!({})));
        let rpc_models = models_from(&models).unwrap_or_default();
        let cfg_models = load_config_models();
        let merged = merge_models(&cfg_models, &rpc_models);
        if !merged.is_empty() {
            self.workspace.set_models(merged);
        }
        let usage = self
            .server
            .handle_frame(&rpc("tu", methods::TELEMETRY_USAGE, json!({})));
        if let Some((turns, tokens, note)) = usage_from(&usage) {
            self.workspace.usage_turns = turns.max(self.workspace.usage_turns);
            self.workspace.usage_tokens = tokens.max(self.workspace.usage_tokens);
            if !note.is_empty() {
                self.term_meta(&format!("usage {note}"));
            }
        }
        let remote_frames = self
            .server
            .handle_frame(&rpc("rl", methods::REMOTE_LIST, json!({})));
        if let Some(rows) = remotes_from(&remote_frames) {
            if !rows.is_empty() {
                let mut merged = rows;
                for extra in detect_remotes(tailscale_which().as_deref()) {
                    if !merged.iter().any(|r| r.id == extra.id) {
                        merged.push(extra);
                    }
                }
                self.remotes = merged;
            }
        }
    }

    fn wt_available() -> bool {
        std::process::Command::new("where.exe")
            .arg("wt.exe")
            .output()
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false)
    }

    fn launch_grok_tui(&mut self) {
        if self.workspace.grok_tui.life == TuiLife::Running && self.grok_tui.is_some() {
            self.workspace
                .push_notice(NoticeKind::Info, "Grok TUI already running");
            return;
        }
        let _ = self.workspace.set_center_mode(CenterMode::GrokTui);
        let launch = TuiLaunch::prefer_wt(&self.workspace.project, "grok", Self::wt_available());
        match spawn_grok_tui(&launch) {
            Ok(child) => {
                let wt = launch
                    .program
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case("wt.exe"));
                if wt {
                    drop(child);
                    self.grok_tui = None;
                    self.workspace
                        .grok_tui
                        .mark_running(None, "grok", "Windows Terminal");
                    self.workspace.push_notice(
                        NoticeKind::Good,
                        "Grok TUI launched in Windows Terminal. Multiplexer does not own the grok pid.",
                    );
                } else {
                    let pid = child.id();
                    self.workspace
                        .grok_tui
                        .mark_running(Some(pid), "grok", "new console");
                    self.grok_tui = Some(child);
                    self.workspace
                        .push_notice(NoticeKind::Good, format!("Grok TUI launched (pid {pid})"));
                    self.term_meta(&format!("grok tui pid {pid}"));
                }
            }
            Err(err) => {
                self.workspace
                    .grok_tui
                    .mark_failed(format!("spawn failed: {err}"));
                self.workspace
                    .push_notice(NoticeKind::Danger, format!("Grok TUI: {err}"));
            }
        }
    }

    fn stop_grok_tui(&mut self) {
        if let Some(mut child) = self.grok_tui.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.workspace.grok_tui.mark_exited();
        self.workspace
            .push_notice(NoticeKind::Info, "Grok TUI stopped");
    }

    fn reload_diffs(&mut self) {
        self.run_shell("git status --porcelain");
    }

    fn open_browser(&mut self) {
        let url = if self.workspace.browser_url.trim().is_empty() {
            "https://grok.com".to_owned()
        } else {
            self.workspace.browser_url.clone()
        };
        self.workspace.browser_url = url.clone();
        let found = detect_browsers(&default_browser_candidates());
        if let Some((name, exe)) = found.first() {
            self.run_shell(&format!("start \"\" \"{exe}\" \"{url}\""));
            self.workspace.push_notice(
                NoticeKind::Info,
                format!("opened {url} with {name} (no CDP)"),
            );
        } else {
            self.run_shell(&format!("start \"\" \"{url}\""));
            self.workspace.push_notice(
                NoticeKind::Info,
                format!("opened {url} (system browser, no CDP)"),
            );
        }
    }

    fn create_worktree(&mut self) {
        let path = self.workspace.wt_path.clone();
        let branch = self.workspace.wt_branch.clone();
        if path.trim().is_empty() || branch.trim().is_empty() {
            self.workspace
                .push_notice(NoticeKind::Warn, "set path and branch first");
            return;
        }
        let frames = self.server.handle_frame(&rpc(
            "wtc",
            methods::GIT_WORKTREE_CREATE,
            json!({
                "cwd": self.workspace.project,
                "path": path,
                "branch": branch,
                "create_branch": self.workspace.wt_create_branch,
            }),
        ));
        if let Some(err) = first_error(&frames) {
            self.workspace.push_notice(NoticeKind::Danger, err);
        } else {
            self.workspace
                .push_notice(NoticeKind::Good, "worktree create accepted");
            self.refresh_worktrees();
        }
    }

    pub(crate) fn activate_inspector_row(&mut self, id: &str) {
        self.workspace.toggle_right_row(id.to_owned());
        if let Some(path) = id.strip_prefix("file:") {
            if path.ends_with('/') {
                let _ = self.workspace.toggle_file_expand(path);
            } else {
                let _ = self.workspace.select_file(path);
            }
        } else if let Some(cid) = id.strip_prefix("point:") {
            self.workspace.select_checkpoint(Some(cid.to_owned()));
            self.load_checkpoint_diff(cid);
        } else if let Some(aid) = id.strip_prefix("agent:") {
            if let Some(i) = self.workspace.threads.iter().position(|t| t.id == aid) {
                let _ = self.workspace.select(i);
            }
        } else if let Some(rest) = id.strip_prefix("git:wt:") {
            if let Ok(i) = rest.parse::<usize>() {
                self.workspace.selected_worktree = Some(i);
            }
        } else if let Some(path) = id.strip_prefix("diff:") {
            if path != "empty" && self.workspace.select_diff(path) {
                self.load_diff_preview(path);
            }
        } else if let Some(idx) = id.strip_prefix("core:") {
            if let Ok(i) = idx.parse::<usize>() {
                let _ = self.workspace.toggle_core_reserved(i);
            }
        } else if let Some(name) = id.strip_prefix("mcp:") {
            if name != "empty" {
                self.workspace.remember_mcp(name);
            }
        }
    }

    fn load_diff_preview(&mut self, path: &str) {
        let line = git_diff_line(path);
        let rx = spawn_command(windows_cmd(&line, PathBuf::from(&self.workspace.project)));
        self.pending_diff = Some((path.to_owned(), rx));
        self.workspace.diff_text = format!("loading {path}…");
    }

    fn activate_palette(&mut self, cx: &mut Context<Self>) {
        let query = self.palette.query.clone();
        let selected = self.palette.selected;
        let hits = palette_hits(&self.workspace, &query);
        self.palette.close();
        self.workspace
            .close_overlay(multiplexer_shell::OverlayKind::Palette);
        self.focus = Focus::Composer;
        let Some(hit) = hits.get(selected).cloned() else {
            return;
        };
        if matches!(
            hit.kind,
            SearchKind::Command | SearchKind::Recent | SearchKind::Pane
        ) {
            self.workspace.remember_command(&hit.id);
        }
        if hit.kind == SearchKind::File {
            let _ = self.workspace.select_file(&hit.id);
        }
        if let Some(action) = hit_action(&self.workspace, &hit) {
            self.dispatch(action, cx);
        }
    }

    fn activate_search(&mut self, cx: &mut Context<Self>) {
        let query = self.workspace.search_query.clone();
        let selected = self.workspace.search_selected;
        let hits = search_workspace(&self.workspace, &query);
        self.workspace
            .close_overlay(multiplexer_shell::OverlayKind::Search);
        self.focus = Focus::Composer;
        let Some(hit) = hits.get(selected).cloned() else {
            return;
        };
        if hit.kind == SearchKind::File {
            let _ = self.workspace.select_file(&hit.id);
        }
        if let Some(action) = hit_action(&self.workspace, &hit) {
            self.dispatch(action, cx);
        }
    }

    fn search_key(
        &mut self,
        key: &str,
        control: bool,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let n = search_workspace(&self.workspace, &self.workspace.search_query).len();
        if key == "up" {
            if n > 0 {
                self.workspace.search_selected = if self.workspace.search_selected == 0 {
                    n - 1
                } else {
                    self.workspace.search_selected - 1
                };
            }
        } else if key == "down" {
            if n > 0 {
                self.workspace.search_selected = (self.workspace.search_selected + 1) % n;
            }
        } else if key == "backspace" {
            self.workspace.search_query.pop();
            self.workspace.search_selected = 0;
        } else if key == "space" {
            self.workspace.search_query.push(' ');
            self.workspace.search_selected = 0;
        } else if let Some(ch) = event.keystroke.key_char.as_deref() {
            if !control {
                for c in ch.chars() {
                    if c == '\n' || c == '\r' {
                        continue;
                    }
                    self.workspace.search_query.push(c);
                }
                self.workspace.search_selected = 0;
            }
        } else if !control && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                self.workspace.search_query.push(c);
                self.workspace.search_selected = 0;
            }
        }
        cx.notify();
    }

    fn dispatch(&mut self, action: ClientAction, cx: &mut Context<Self>) {
        match multiplexer_shell::host_call(action, &self.action_ctx()) {
            multiplexer_shell::HostCall::Local => {
                let changed = apply_layout_action(&mut self.workspace, action);
                match action {
                    ClientAction::NewThread => self.session_id = None,
                    ClientAction::TogglePalette
                    | ClientAction::ClosePalette
                    | ClientAction::CloseOverlay
                    | ClientAction::ToggleSearch
                    | ClientAction::CloseSearch
                    | ClientAction::ToggleSettings
                    | ClientAction::ToggleHelp
                    | ClientAction::OpenSettingsRemotes => {
                        self.sync_overlays();
                        if matches!(
                            action,
                            ClientAction::ToggleSettings | ClientAction::OpenSettingsRemotes
                        ) {
                            self.apply_theme();
                        }
                    }
                    ClientAction::CycleModel | ClientAction::SelectModel => {
                        self.term_meta(&format!("model {}", self.workspace.model));
                    }
                    ClientAction::DeleteThread => {
                        if !changed {
                            self.workspace
                                .push_notice(NoticeKind::Warn, "keep at least one chat");
                        }
                    }
                    ClientAction::InsertFileMention => {
                        if !changed {
                            self.workspace
                                .push_notice(NoticeKind::Warn, "select a file first");
                        }
                    }
                    ClientAction::MentionMcp => {
                        if changed {
                            self.workspace
                                .push_notice(NoticeKind::Info, "mentioned @mcp (text only)");
                        } else {
                            self.workspace
                                .push_notice(NoticeKind::Warn, "select an MCP row first");
                        }
                    }
                    ClientAction::ToggleSkill => {
                        if !changed {
                            self.workspace
                                .push_notice(NoticeKind::Warn, "select a skill first");
                        }
                    }
                    ClientAction::SelectTab(InspectorTab::Checkpoints) => {
                        self.refresh_checkpoints();
                    }
                    ClientAction::PopOutInspector
                    | ClientAction::DockInspector
                    | ClientAction::ClosePopOut
                    | ClientAction::ResetOutlook
                    | ClientAction::HideRight
                    | ClientAction::ToggleRight => {
                        self.persist_layout();
                    }
                    ClientAction::NextRegion => {
                        self.focus = match self.workspace.focus_region {
                            FocusRegion::Left | FocusRegion::Center => Focus::Composer,
                            FocusRegion::Right => Focus::FileFilter,
                            FocusRegion::Bottom => Focus::Terminal,
                        };
                    }
                    ClientAction::OpenAbout => {
                        self.sync_overlays();
                    }
                    _ => {}
                }
            }
            multiplexer_shell::HostCall::NeedsHost | multiplexer_shell::HostCall::Rpc { .. } => {
                self.host_action(action, cx);
            }
        }
        cx.notify();
    }

    fn host_action(&mut self, action: ClientAction, cx: &mut Context<Self>) {
        match action {
            ClientAction::Send => self.send(cx),
            ClientAction::Interrupt => self.interrupt(),
            ClientAction::RefreshCores => self.refresh_cores(),
            ClientAction::RefreshMcp => self.refresh_mcp(),
            ClientAction::CreateCheckpoint => self.create_checkpoint(),
            ClientAction::RestoreCheckpoint => self.revert_checkpoint(),
            ClientAction::RefreshGit => self.refresh_worktrees(),
            ClientAction::RunTerminal => self.run_terminal_draft(),
            ClientAction::CycleFile => self.cycle_file(),
            ClientAction::CopyLastMessage => self.copy_last_message(cx),
            ClientAction::Approve => self.respond_approval("allow"),
            ClientAction::Deny => self.respond_approval("deny"),
            ClientAction::CreateWorktree => self.create_worktree(),
            ClientAction::LaunchGrokTui => self.launch_grok_tui(),
            ClientAction::StopGrokTui => self.stop_grok_tui(),
            ClientAction::OpenBrowser => self.open_browser(),
            ClientAction::CopySession => self.copy_session(cx),
            ClientAction::ReloadDiffs => self.reload_diffs(),
            ClientAction::RunGitStatus => self.run_shell("git status"),
            ClientAction::RefreshFiles => self.reload_files(),
            ClientAction::RevealFile => self.reveal_file(cx),
            ClientAction::OpenExternal => self.open_external(),
            ClientAction::SwitchWorktree => self.switch_worktree(),
            ClientAction::RemoveWorktree => self.remove_worktree(),
            ClientAction::KillTerm => self.kill_capture(),
            ClientAction::RefreshSkills => self.refresh_skills(),
            ClientAction::DetectBrowsers => self.detect_browser_notice(),
            ClientAction::CreateSkill => self.create_skill(),
            ClientAction::ApproveOnce => self.respond_approval("once"),
            ClientAction::Later => self.respond_approval("later"),
            ClientAction::CopyThreadId => self.copy_thread_id(cx),
            ClientAction::StartMcp | ClientAction::StopMcp => {
                if !apply_layout_action(&mut self.workspace, action) {
                    self.workspace
                        .push_notice(NoticeKind::Warn, "select an MCP row first");
                }
            }
            other => {
                let _ = apply_layout_action(&mut self.workspace, other);
            }
        }
    }

    fn jump_activity(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == "act:reminder" {
            self.dispatch(ClientAction::OpenGitTab, cx);
        } else if id == "act:busy" {
            self.focus = Focus::Composer;
            cx.notify();
        } else if id.starts_with("act:notice:") {
            let _ = self.workspace.dismiss_newest_notice();
            cx.notify();
        } else if id.starts_with("act:log:") || id == "act:empty" {
            self.dispatch(ClientAction::SelectTab(InspectorTab::Activity), cx);
            self.focus = Focus::Terminal;
        } else if id.starts_with("act:approval:") {
            cx.notify();
        } else {
            self.dispatch(ClientAction::SelectTab(InspectorTab::Activity), cx);
        }
    }

    fn handshake(&mut self) {
        let hello = self.server.handle_frame(&rpc(
            "hello",
            methods::SYSTEM_HELLO,
            json!({ "protocol_version": PROTOCOL_VERSION }),
        ));
        let ping = self
            .server
            .handle_frame(&rpc("ping", methods::SYSTEM_PING, json!({})));
        let hello_ok = first_error(&hello).is_none() && !hello.is_empty();
        let ping_ok = first_error(&ping).is_none() && !ping.is_empty();
        self.workspace.apply_handshake(hello_ok, ping_ok);
    }

    fn persist_settings(&mut self) {
        self.workspace.settings.bindings = self.bindings.pairs();
        let _ = write_settings(&default_settings_path(), &self.workspace.settings);
    }

    fn age_notices(&mut self) {
        let now = Instant::now();
        let live: Vec<u64> = self.workspace.notices.iter().map(|n| n.id).collect();
        self.notice_born.retain(|(id, _)| live.contains(id));
        for n in &self.workspace.notices {
            if auto_dismisses(n.kind) && !self.notice_born.iter().any(|(id, _)| *id == n.id) {
                self.notice_born.push((n.id, now));
            }
        }
        let expired: Vec<u64> = self
            .notice_born
            .iter()
            .filter_map(|(id, born)| {
                if now.duration_since(*born).as_millis() as u64 >= NOTICE_AUTO_MS {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in expired {
            let sticky = self
                .workspace
                .notices
                .iter()
                .any(|n| n.id == id && !auto_dismisses(n.kind));
            if !sticky {
                let _ = multiplexer_shell::dismiss_notice(&mut self.workspace.notices, id);
            }
        }
    }

    fn sync_overlays(&mut self) {
        if self.workspace.palette_open {
            self.palette.open = true;
            self.focus = Focus::Palette;
        } else if self.palette.open {
            self.palette.close();
        }
        if self.workspace.search_open {
            self.focus = Focus::Search;
        } else if self.focus == Focus::Search {
            self.focus = Focus::Composer;
        }
        if !self.workspace.palette_open && self.focus == Focus::Palette {
            self.focus = Focus::Composer;
        }
    }

    fn interrupt(&mut self) {
        if self.capture.is_some() {
            self.kill_capture();
            return;
        }
        if let Some(sid) = &self.session_id {
            let _ = self.server.handle_frame(&rpc(
                "int",
                methods::SESSION_INTERRUPT,
                json!({ "session_id": sid }),
            ));
        }
        self.ignore_turn = true;
        self.workspace.mark_interrupted();
        self.term_meta("interrupted");
    }

    fn refresh_cores(&mut self) {
        let reserved: Vec<usize> = self
            .workspace
            .cores
            .iter()
            .filter(|c| c.reserved)
            .map(|c| c.index)
            .collect();
        let incoming: Vec<CoreRow> = sample_cores(&reserved)
            .into_iter()
            .map(|c| CoreRow {
                index: c.index,
                usage: c.usage,
                reserved: c.reserved,
            })
            .collect();
        self.workspace.cores = merge_cores(&self.workspace.cores, incoming);
        self.last_core_sample = Instant::now();
        self.term_meta("cores resampled");
    }

    fn refresh_mcp(&mut self) {
        let incoming: Vec<McpRow> = load_user_mcp_inventory()
            .into_iter()
            .map(|row| McpRow {
                name: row.name,
                command: row.command,
                transport: row.transport,
                state: multiplexer_shell::McpLife::Stopped,
            })
            .collect();
        self.workspace.mcp = merge_mcp(&self.workspace.mcp, incoming);
        self.term_meta(&format!("mcp inventory {}", self.workspace.mcp.len()));
    }

    fn reload_files(&mut self) {
        let cwd = PathBuf::from(&self.workspace.project);
        self.workspace.set_files(
            list_project_tree(&cwd, ListOptions::default())
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
        self.workspace.push_notice(
            NoticeKind::Info,
            format!("files {}", self.workspace.files.len()),
        );
    }

    fn selected_or_expanded_file(&self) -> Option<String> {
        if let Some(p) = self.workspace.selected_file.clone() {
            return Some(p);
        }
        self.workspace
            .right_expanded_id
            .as_deref()
            .and_then(|id| id.strip_prefix("file:"))
            .map(str::to_owned)
    }

    fn reveal_file(&mut self, cx: &mut Context<Self>) {
        let Some(rel) = self.selected_or_expanded_file() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a file first");
            return;
        };
        let abs = join_project_path(&self.workspace.project, &rel);
        let text = abs.display().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        self.workspace
            .push_notice(NoticeKind::Info, format!("copied {text}"));
    }

    fn open_external(&mut self) {
        let Some(rel) = self.selected_or_expanded_file() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a file first");
            return;
        };
        let abs = join_project_path(&self.workspace.project, &rel);
        let visual = std::env::var("VISUAL").ok();
        let editor = std::env::var("EDITOR").ok();
        let prog = open_external_program(visual.as_deref(), editor.as_deref());
        if prog == "start" {
            self.run_shell(&format!("start \"\" \"{}\"", abs.display()));
        } else {
            self.run_shell(&format!("{prog} \"{}\"", abs.display()));
        }
        self.workspace.push_notice(
            NoticeKind::Info,
            format!("opened {} ({prog})", abs.display()),
        );
    }

    fn refresh_reminder(&mut self) {
        let frames = self.server.handle_frame(&rpc(
            "wt",
            methods::GIT_WORKTREES,
            json!({ "cwd": self.workspace.project }),
        ));
        let trees = worktree_records(&frames);
        let branch = self.workspace.branch_label();
        if let Some(r) = reminder_from_list(&trees, &branch) {
            self.workspace.set_reminder(r.branch, r.path);
        }
    }

    fn refresh_worktrees(&mut self) {
        let frames = self.server.handle_frame(&rpc(
            "wt",
            methods::GIT_WORKTREES,
            json!({ "cwd": self.workspace.project }),
        ));
        let cards = worktree_cards(&frames);
        self.workspace.worktrees = cards.iter().map(|c| c.path.clone()).collect();
        self.workspace.worktree_cards = cards;
        self.term_meta(&format!("worktrees {}", self.workspace.worktrees.len()));
        self.refresh_reminder();
    }

    fn refresh_skills(&mut self) {
        let cwd = self.workspace.project.clone();
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        let cands = skill_dir_candidates(&home, &cwd);
        let user = list_dir_entry_names(&cands[0].0);
        let project = list_dir_entry_names(&cands[1].0);
        let user_names: Vec<&str> = user.iter().map(String::as_str).collect();
        let project_names: Vec<&str> = project.iter().map(String::as_str).collect();
        let rows = merge_skill_rows(
            &parse_skill_names(&user_names),
            &parse_skill_names(&project_names),
        );
        let items: Vec<SkillItem> = rows
            .into_iter()
            .map(|r| {
                let preview = skill_preview(&cands, &r.name, &r.source);
                SkillItem {
                    name: r.name,
                    source: r.source,
                    enabled: true,
                    preview,
                }
            })
            .collect();
        self.workspace.set_skill_items(items);
        let hooks_path = PathBuf::from(&cwd).join(".grok").join("hooks.toml");
        if let Ok(text) = std::fs::read_to_string(hooks_path) {
            self.workspace.hooks = parse_hooks_tomlish(&text)
                .into_iter()
                .map(|h| (h.name, h.when))
                .collect();
        } else {
            self.workspace.hooks.clear();
        }
        self.workspace.push_notice(
            NoticeKind::Info,
            format!(
                "skills {}  hooks {}",
                self.workspace.skill_items.len(),
                self.workspace.hooks.len()
            ),
        );
    }

    fn create_skill(&mut self) {
        let name = self
            .workspace
            .draft
            .split_whitespace()
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("new-skill");
        let dir = PathBuf::from(&self.workspace.project)
            .join(".grok")
            .join("skills")
            .join(name);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.workspace
                .push_notice(NoticeKind::Warn, format!("create skill: {err}"));
            return;
        }
        let path = dir.join("SKILL.md");
        let body = format!("# {name}\n\nLocal skill stub. Not loaded into grok.\n");
        if let Err(err) = std::fs::write(&path, body) {
            self.workspace
                .push_notice(NoticeKind::Warn, format!("write skill: {err}"));
            return;
        }
        self.refresh_skills();
        self.workspace
            .push_notice(NoticeKind::Good, format!("wrote {}", path.display()));
    }

    fn detect_browser_notice(&mut self) {
        let found = detect_browsers(&default_browser_candidates());
        if found.is_empty() {
            self.workspace.push_notice(
                NoticeKind::Info,
                "no browsers at well-known paths (CDP later)",
            );
        } else {
            let names: Vec<_> = found.iter().map(|(n, _)| n.as_str()).collect();
            self.workspace.push_notice(
                NoticeKind::Info,
                format!("detected {} (CDP later)", names.join(", ")),
            );
        }
    }

    fn refresh_checkpoints(&mut self) {
        let sid = self
            .session_id
            .clone()
            .unwrap_or_else(|| "local".to_owned());
        let frames = self.server.handle_frame(&rpc(
            "cpl",
            methods::CHECKPOINT_LIST,
            json!({ "session_id": sid }),
        ));
        if let Some(rows) = checkpoints_from(&frames) {
            self.workspace.checkpoints = rows;
        }
    }

    fn switch_worktree(&mut self) {
        let Some(i) = self.workspace.selected_worktree else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a worktree first");
            return;
        };
        let Some(path) = self.workspace.worktrees.get(i).cloned() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a worktree first");
            return;
        };
        self.workspace.project = path.clone();
        self.reload_files();
        self.reload_diffs();
        self.workspace
            .push_notice(NoticeKind::Info, format!("cwd {path}"));
    }

    fn remove_worktree(&mut self) {
        let Some(i) = self.workspace.selected_worktree else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a worktree first");
            return;
        };
        let Some(path) = self.workspace.worktrees.get(i).cloned() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a worktree first");
            return;
        };
        if i == 0 {
            self.workspace
                .push_notice(NoticeKind::Warn, "refuse removing the primary worktree");
            return;
        }
        self.run_shell(&format!("git worktree remove \"{path}\""));
        self.workspace.selected_worktree = None;
        self.workspace
            .push_notice(NoticeKind::Info, format!("remove requested for {path}"));
    }

    fn kill_capture(&mut self) {
        if let Some(mut cap) = self.capture.take() {
            let _ = cap.kill();
            self.workspace
                .push_notice(NoticeKind::Info, "command killed");
        } else {
            self.workspace
                .push_notice(NoticeKind::Info, "no running command");
        }
    }

    fn create_checkpoint(&mut self) {
        let sid = self
            .session_id
            .clone()
            .unwrap_or_else(|| "local".to_owned());
        let frames = self.server.handle_frame(&rpc(
            "cp",
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": sid, "label": "manual" }),
        ));
        if let Some(err) = first_error(&frames) {
            self.workspace.push_notice(
                NoticeKind::Danger,
                format!("checkpoint create failed: {err}"),
            );
            return;
        }
        if let Some((id, label, sha)) = checkpoint_from(&frames) {
            self.workspace.create_local_checkpoint(id.clone(), label);
            self.workspace.select_checkpoint(Some(id.clone()));
            self.workspace.git_checkpoints = true;
            if sha.is_empty() {
                self.term_meta("checkpoint created (no git sha)");
            } else {
                self.term_meta(&format!("checkpoint {id} sha {sha}"));
            }
        }
        self.refresh_checkpoints();
    }

    fn revert_checkpoint(&mut self) {
        let Some(id) = self.workspace.selected_checkpoint.clone() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "select a checkpoint");
            return;
        };
        let frames = self.server.handle_frame(&rpc(
            "cpr",
            methods::CHECKPOINT_REVERT,
            json!({ "checkpoint_id": id }),
        ));
        if let Some(err) = first_error(&frames) {
            self.term_meta(&format!("revert failed: {err}"));
            return;
        }
        self.workspace.select_checkpoint(Some(id.clone()));
        let restored = revert_restored(&frames);
        if restored {
            self.workspace.git_checkpoints = true;
            self.workspace
                .push_notice(NoticeKind::Good, format!("restored working tree to {id}"));
            self.reload_files();
            self.reload_diffs();
        } else {
            self.workspace.push_notice(
                NoticeKind::Info,
                format!("pointer set to {id}; files unchanged"),
            );
        }
        self.refresh_checkpoints();
    }

    fn load_checkpoint_diff(&mut self, id: &str) {
        let frames = self.server.handle_frame(&rpc(
            "cpd",
            methods::CHECKPOINT_DIFF,
            json!({ "checkpoint_id": id }),
        ));
        if let Some(err) = first_error(&frames) {
            self.workspace
                .push_notice(NoticeKind::Warn, format!("checkpoint.diff: {err}"));
            return;
        }
        if let Some(text) = checkpoint_diff_text(&frames) {
            self.workspace.diff_text = cap_text(&text, DIFF_TEXT_CAP);
            if !text.is_empty() {
                self.workspace
                    .push_notice(NoticeKind::Info, format!("diff vs {id}"));
            }
        }
    }

    fn respond_approval(&mut self, decision: &str) {
        let Some(pending) = self.workspace.pending_approval().cloned() else {
            self.term_meta("no pending approval");
            return;
        };
        let _ = self.server.handle_frame(&rpc(
            "ap",
            methods::APPROVAL_RESPOND,
            json!({
                "session_id": pending.session_id,
                "request_id": pending.request_id,
                "decision": decision,
            }),
        ));
        self.workspace.clear_pending_approval();
        self.term_meta(&format!("approval {decision}"));
    }

    fn copy_last_message(&mut self, cx: &mut Context<Self>) {
        let text = self
            .workspace
            .selected_thread()
            .and_then(|t| t.messages.last())
            .map(|m| m.text.clone())
            .unwrap_or_else(|| "no message".into());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.workspace
            .push_notice(NoticeKind::Info, "copied last message");
        self.term_meta("copied last message");
    }

    fn copy_thread_id(&mut self, cx: &mut Context<Self>) {
        let id = self
            .workspace
            .selected_thread()
            .map(|t| t.id.clone())
            .unwrap_or_else(|| "no thread".into());
        cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
        self.workspace
            .push_notice(NoticeKind::Info, format!("copied {id}"));
    }

    fn copy_session(&mut self, cx: &mut Context<Self>) {
        let text = self
            .session_id
            .clone()
            .unwrap_or_else(|| "(none yet)".into());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.workspace
            .push_notice(NoticeKind::Info, "copied session id");
        self.term_meta("copied session id");
    }

    fn cycle_file(&mut self) {
        self.workspace
            .push_notice(NoticeKind::Info, "use Files filter. Rotate is gone.");
    }

    fn ensure_session(&mut self) -> bool {
        if self.session_id.is_some() {
            return true;
        }
        let frames = self.server.handle_frame(&rpc(
            "start",
            methods::SESSION_START,
            json!({
                "provider": "grok",
                "model": self.workspace.model,
                "workspace": self.workspace.project,
            }),
        ));
        if let Some(err) = first_error(&frames) {
            self.workspace.mark_error(err);
            return false;
        }
        if let Some(id) = session_id_from(&frames) {
            self.session_id = Some(id.clone());
            self.workspace.connect(vec![id]);
            true
        } else {
            self.workspace.mark_error("session.start returned no id");
            false
        }
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let busy = self.pending_turn.is_some() || self.workspace.busy;
        match plan_send(&self.workspace.draft, busy) {
            SendPlan::IgnoreEmpty => {
                self.workspace.push_notice(NoticeKind::Warn, "draft empty");
                return;
            }
            SendPlan::IgnoreBusy => return,
            SendPlan::Slash(SlashCommand::Unknown(name)) => {
                self.workspace
                    .push_notice(NoticeKind::Warn, format!("unknown /{name}"));
                return;
            }
            SendPlan::Slash(cmd) => {
                let arg = slash_arg(&self.workspace.draft);
                self.workspace.draft.clear();
                self.workspace.cursor = 0;
                self.handle_slash(cmd, arg, cx);
                return;
            }
            SendPlan::StartTurn(_) => {}
        }
        if busy {
            return;
        }
        let Some(text) = self.workspace.send_draft() else {
            return;
        };
        self.persist_crash();
        if !self.ensure_session() {
            return;
        }
        let cwd = PathBuf::from(&self.workspace.project);
        let rx = spawn_grok_turn(TurnRequest {
            cwd,
            prompt: text,
            program: PathBuf::from("grok"),
        });
        self.pending_turn = Some(rx);
        self.ignore_turn = false;
        self.turn_started = Some(Instant::now());
        self.workspace.busy = true;
        self.term_meta("grok -p running in background");
    }

    fn handle_slash(&mut self, cmd: SlashCommand, arg: Option<String>, cx: &mut Context<Self>) {
        match cmd {
            SlashCommand::New => {
                self.workspace.new_thread();
                self.session_id = None;
            }
            SlashCommand::Stop => self.interrupt(),
            SlashCommand::Help => self.workspace.toggle_help(),
            SlashCommand::Checkpoint => self.create_checkpoint(),
            SlashCommand::Cores => self.workspace.inspector = InspectorTab::Resources,
            SlashCommand::Mcp => self.workspace.inspector = InspectorTab::Mcp,
            SlashCommand::Points => self.workspace.inspector = InspectorTab::Checkpoints,
            SlashCommand::Git => self.workspace.inspector = InspectorTab::Git,
            SlashCommand::Terminal => {
                self.workspace.inspector = InspectorTab::Terminal;
                self.focus = Focus::Terminal;
            }
            SlashCommand::Skills => self.workspace.inspector = InspectorTab::Skills,
            SlashCommand::Palette => {
                self.dispatch(ClientAction::TogglePalette, cx);
            }
            SlashCommand::Model => {
                if let Some(id) = arg {
                    if self.workspace.select_model(&id) {
                        self.term_meta(&format!("model {id}"));
                    } else {
                        self.workspace
                            .push_notice(NoticeKind::Warn, format!("unknown model {id}"));
                    }
                } else {
                    self.workspace.cycle_model();
                    self.term_meta(&format!("model {}", self.workspace.model));
                }
            }
            SlashCommand::Search => {
                self.dispatch(ClientAction::ToggleSearch, cx);
            }
            SlashCommand::Settings => {
                self.dispatch(ClientAction::ToggleSettings, cx);
            }
            SlashCommand::Files => {
                if let Some(q) = arg {
                    self.workspace.set_file_filter(q);
                }
                self.dispatch(ClientAction::OpenProjectFiles, cx);
            }
            SlashCommand::Agents => {
                self.dispatch(ClientAction::SelectLeftSection(LeftSection::Agents), cx);
            }
            SlashCommand::Diff => {
                self.dispatch(ClientAction::SelectTab(InspectorTab::Diff), cx);
            }
            SlashCommand::Browser => {
                self.dispatch(ClientAction::SelectTab(InspectorTab::Browser), cx);
            }
            SlashCommand::Tui => {
                self.dispatch(ClientAction::SetCenterTui, cx);
            }
            SlashCommand::About => {
                self.dispatch(ClientAction::OpenAbout, cx);
            }
            SlashCommand::Unknown(name) => {
                self.workspace
                    .push_notice(NoticeKind::Warn, format!("unknown /{name}"));
            }
        }
    }

    fn run_terminal_draft(&mut self) {
        let Some(line) = self.workspace.take_term_draft() else {
            self.workspace
                .push_notice(NoticeKind::Warn, "command empty");
            return;
        };
        if self.capture.is_some() {
            self.workspace
                .push_notice(NoticeKind::Warn, "a command is already running");
            return;
        }
        self.term_line(TermLineKind::Input, &line);
        let lower = line.trim().to_ascii_lowercase();
        if lower == "pwd" {
            let cwd = self.workspace.term_cwd.clone();
            self.term_meta(&cwd);
            return;
        }
        if lower == "cd" || lower.starts_with("cd ") {
            let rest = line.trim()[2..].trim();
            if rest.is_empty() || rest == "~" {
                self.workspace.term_cwd = self.workspace.project.clone();
            } else {
                let next = PathBuf::from(&self.workspace.term_cwd).join(rest);
                self.workspace.term_cwd = next.display().to_string();
            }
            let cwd = self.workspace.term_cwd.clone();
            self.term_meta(&cwd);
            return;
        }
        if let Some(builtin) = parse_builtin(&line) {
            self.run_builtin(builtin);
            return;
        }
        match ProcessCapture::spawn(
            "cmd.exe",
            &["/C", &line],
            PathBuf::from(&self.workspace.term_cwd).as_path(),
        ) {
            Ok(cap) => {
                self.capture = Some(cap);
                self.term_meta("command running (killable)");
            }
            Err(err) => {
                self.term_line(TermLineKind::Error, &err.to_string());
            }
        }
    }

    fn run_builtin(&mut self, builtin: BuiltinCmd) {
        match builtin {
            BuiltinCmd::Clear => self.workspace.terminal_log.clear(),
            BuiltinCmd::Help => {
                self.term_meta(help_text());
            }
            BuiltinCmd::Cores => {
                let _ = self.workspace.select_inspector(InspectorTab::Resources);
            }
            BuiltinCmd::Mcp => {
                let _ = self.workspace.select_inspector(InspectorTab::Mcp);
            }
            BuiltinCmd::Git => {
                let _ = self.workspace.select_inspector(InspectorTab::Git);
            }
            BuiltinCmd::Checkpoint => {
                let _ = self.workspace.select_inspector(InspectorTab::Checkpoints);
            }
            BuiltinCmd::Skills => {
                let _ = self.workspace.select_inspector(InspectorTab::Skills);
            }
            BuiltinCmd::Unknown => self.term_meta("unknown builtin"),
        }
    }

    fn run_shell(&mut self, line: &str) {
        self.term_line(TermLineKind::Input, line);
        if let Some(builtin) = parse_builtin(line) {
            match builtin {
                BuiltinCmd::Clear => self.workspace.terminal_log.clear(),
                BuiltinCmd::Help => {
                    self.term_meta(help_text());
                    self.workspace.toggle_help();
                }
                BuiltinCmd::Cores => {
                    let _ = self.workspace.select_inspector(InspectorTab::Resources);
                }
                BuiltinCmd::Mcp => {
                    let _ = self.workspace.select_inspector(InspectorTab::Mcp);
                }
                BuiltinCmd::Git => {
                    let _ = self.workspace.select_inspector(InspectorTab::Git);
                }
                BuiltinCmd::Checkpoint => {
                    let _ = self.workspace.select_inspector(InspectorTab::Checkpoints);
                }
                BuiltinCmd::Skills => {
                    let _ = self.workspace.select_inspector(InspectorTab::Skills);
                }
                BuiltinCmd::Unknown => self.term_meta("unknown builtin"),
            }
            return;
        }
        if self.pending_cmd.is_some() {
            self.term_meta("a shell command is already running");
            return;
        }
        let rx = spawn_command(windows_cmd(line, PathBuf::from(&self.workspace.project)));
        self.pending_cmd = Some(rx);
        self.term_meta("shell running in background");
    }

    fn term_line(&mut self, kind: TermLineKind, text: &str) {
        multiplexer_shell::push_capped(&mut self.workspace.terminal_log, format_line(kind, text));
    }

    fn term_meta(&mut self, text: &str) {
        self.term_line(TermLineKind::Meta, text);
    }

    fn pump(&mut self, window: &mut Window) {
        if self.workspace.inspector == InspectorTab::Resources
            && self.last_core_sample.elapsed() > Duration::from_millis(1500)
        {
            self.refresh_cores();
        }
        if let Some(cap) = self.capture.as_mut() {
            for line in cap.try_read() {
                multiplexer_shell::push_capped(
                    &mut self.workspace.terminal_log,
                    format_line(TermLineKind::Output, &line),
                );
            }
        }
        if let Some((path, rx)) = self.pending_diff.take() {
            match rx.try_recv() {
                Ok(out) => {
                    let body = if out.ok { out.stdout } else { out.stderr };
                    self.workspace.diff_text = cap_text(&body, DIFF_TEXT_CAP);
                    if self.workspace.diff_text.trim().is_empty() {
                        self.workspace.diff_text =
                            format!("(no unstaged diff for {path}; untracked or cached only)");
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.pending_diff = Some((path, rx));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.workspace.diff_text = format!("diff worker dropped for {path}");
                }
            }
        }
        if let Some(rx) = &self.pending_turn {
            match rx.try_recv() {
                Ok(out) => {
                    self.pending_turn = None;
                    self.workspace.busy = false;
                    self.turn_started = None;
                    if self.ignore_turn {
                        self.ignore_turn = false;
                    } else if out.ok {
                        let text = out.stdout.trim();
                        if text.is_empty() {
                            self.workspace.push_assistant("(no text from grok -p)");
                        } else {
                            self.workspace.push_assistant(text.to_owned());
                        }
                        let estimate = ((text.chars().count() / 4) as u64).max(1);
                        self.workspace.usage_tokens =
                            self.workspace.usage_tokens.saturating_add(estimate);
                        self.workspace.usage_turns = self.workspace.usage_turns.saturating_add(1);
                        self.workspace.push_notice(
                            multiplexer_shell::NoticeKind::Good,
                            format!("turn complete · ~{estimate} tok"),
                        );
                        self.pending_turn_diffs = true;
                        self.reload_diffs();
                    } else {
                        let err = out.stderr.trim();
                        self.workspace.mark_error(if err.is_empty() {
                            "grok -p failed".into()
                        } else {
                            err.to_owned()
                        });
                    }
                    self.refresh_worktrees();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_turn = None;
                    self.workspace.busy = false;
                    self.turn_started = None;
                    if !self.ignore_turn {
                        self.workspace.mark_error("grok worker dropped");
                    }
                    self.ignore_turn = false;
                }
            }
        }
        if let Some(rx) = &self.pending_cmd {
            match rx.try_recv() {
                Ok(out) => {
                    self.pending_cmd = None;
                    let body = if !out.ok && !out.stderr.is_empty() {
                        out.stderr
                    } else {
                        out.stdout
                    };
                    if body.trim().is_empty() {
                        self.term_line(
                            if out.ok {
                                TermLineKind::Meta
                            } else {
                                TermLineKind::Error
                            },
                            if out.ok { "ok" } else { "command failed" },
                        );
                    } else {
                        for line in body.lines().take(40) {
                            self.term_line(
                                if out.ok {
                                    TermLineKind::Output
                                } else {
                                    TermLineKind::Error
                                },
                                line,
                            );
                        }
                    }
                    if body.to_ascii_lowercase().contains("git")
                        || self.workspace.inspector == InspectorTab::Git
                    {
                        self.workspace
                            .set_git_status(body.chars().take(800).collect::<String>());
                    }
                    if self.workspace.inspector == InspectorTab::Diff
                        || self.pending_turn_diffs
                        || body.lines().any(|l| {
                            l.len() >= 4
                                && (l.starts_with(" M")
                                    || l.starts_with("M ")
                                    || l.starts_with("??")
                                    || l.starts_with("A ")
                                    || l.starts_with("D ")
                                    || l.starts_with("R "))
                        })
                    {
                        self.workspace.apply_porcelain(&body);
                        if self.pending_turn_diffs {
                            let paths: Vec<String> = self
                                .workspace
                                .diff_rows
                                .iter()
                                .map(|r| r.path.clone())
                                .collect();
                            self.workspace.remember_turn_paths(paths);
                            self.pending_turn_diffs = false;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_cmd = None;
                    self.term_line(TermLineKind::Error, "shell worker dropped");
                }
            }
        }
        if let Some(child) = self.grok_tui.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.grok_tui = None;
                    self.workspace.grok_tui.mark_exited();
                    self.workspace
                        .push_notice(NoticeKind::Info, "Grok TUI exited. Diffs reloaded.");
                    self.reload_diffs();
                }
                Ok(None) => {}
                Err(_) => {
                    self.grok_tui = None;
                    self.workspace.grok_tui.mark_exited();
                }
            }
        }
        self.age_notices();
        if self.pending_turn.is_some()
            || self.pending_cmd.is_some()
            || self.capture.is_some()
            || self.pending_diff.is_some()
            || self.workspace.grok_tui.life == TuiLife::Running
            || self
                .workspace
                .notices
                .iter()
                .any(|n| auto_dismisses(n.kind))
        {
            window.request_animation_frame();
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        let chord = Chord::new(key, mods.control, mods.shift, mods.alt);
        let bound = self.bindings.lookup(&chord);

        if key == "v" && mods.control && !mods.shift && !mods.alt {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    self.insert_text(&text);
                }
            }
            cx.notify();
            return;
        }

        let overlay_edit = self.palette.open
            || self.workspace.search_open
            || self.focus == Focus::Palette
            || self.focus == Focus::Search;
        if overlay_edit {
            if matches!(bound, Some(ClientAction::Send)) || key == "enter" {
                if self.workspace.search_open {
                    self.activate_search(cx);
                } else {
                    self.activate_palette(cx);
                }
                return;
            }
            if let Some(action) = bound {
                if !matches!(action, ClientAction::Send) {
                    self.dispatch_bound(action, cx);
                    return;
                }
            }
            if self.workspace.search_open || self.focus == Focus::Search {
                self.search_key(key, mods.control, event, cx);
            } else {
                self.palette_key(key, mods.control, cx);
            }
            return;
        }

        if let Some(action) = bound {
            let tui_running = self.workspace.center_mode == CenterMode::GrokTui
                && self.workspace.grok_tui.life == TuiLife::Running;
            if tui_running && !is_tui_hatch(action) {
                return;
            }
            if matches!(action, ClientAction::Send) && mods.shift {
                self.insert_text("\n");
                cx.notify();
                return;
            }
            self.dispatch_bound(action, cx);
            return;
        }

        if self.workspace.pending.is_some()
            && !self.workspace.overlay_flags().any()
            && !mods.control
        {
            if key == "a" {
                self.dispatch(ClientAction::Approve, cx);
                return;
            }
            if key == "d" {
                self.dispatch(ClientAction::Deny, cx);
                return;
            }
            if key == "o" {
                self.dispatch(ClientAction::ApproveOnce, cx);
                return;
            }
            if key == "l" {
                self.dispatch(ClientAction::Later, cx);
                return;
            }
        }

        if self.workspace.help_open || self.workspace.settings_open {
            cx.notify();
            return;
        }

        if self.focus == Focus::FileFilter {
            if key == "backspace" {
                self.workspace.file_filter.pop();
            } else if key == "space" {
                self.workspace.file_filter.push(' ');
            } else if let Some(ch) = event.keystroke.key_char.as_deref() {
                if !mods.control && !mods.alt {
                    for c in ch.chars() {
                        if c == '\n' || c == '\r' {
                            continue;
                        }
                        self.workspace.file_filter.push(c);
                    }
                }
            }
            cx.notify();
            return;
        }

        if self.focus == Focus::Terminal {
            self.terminal_key(key, mods.control, cx);
            return;
        }

        if key == "backspace" {
            if mods.control {
                self.workspace.cursor = {
                    let start = move_word_left(&self.workspace.draft, self.workspace.cursor);
                    let mut draft = self.workspace.draft.clone();
                    draft.replace_range(
                        char_byte(&draft, start)..char_byte(&draft, self.workspace.cursor),
                        "",
                    );
                    self.workspace.draft = draft;
                    start
                };
            } else {
                self.workspace.backspace();
            }
        } else if key == "delete" {
            self.workspace.cursor =
                delete_forward(&mut self.workspace.draft, self.workspace.cursor);
        } else if key == "left" {
            self.workspace.cursor = if mods.control {
                move_word_left(&self.workspace.draft, self.workspace.cursor)
            } else {
                move_left(&self.workspace.draft, self.workspace.cursor)
            };
        } else if key == "right" {
            self.workspace.cursor = if mods.control {
                move_word_right(&self.workspace.draft, self.workspace.cursor)
            } else {
                move_right(&self.workspace.draft, self.workspace.cursor)
            };
        } else if key == "home" {
            self.workspace.cursor = move_home(&self.workspace.draft, self.workspace.cursor);
        } else if key == "end" {
            self.workspace.cursor = move_end(&self.workspace.draft, self.workspace.cursor);
        } else if key == "tab" {
            self.focus = Focus::Terminal;
        } else if let Some(ch) = event.keystroke.key_char.as_deref() {
            if !mods.control && !mods.alt {
                for c in ch.chars() {
                    if c == '\n' || c == '\r' {
                        continue;
                    }
                    self.workspace.type_char(c);
                }
            }
        } else if key == "space" {
            self.workspace.type_char(' ');
        } else if key.len() == 1 {
            if let Some(c) = key.chars().next() {
                if !mods.control {
                    self.workspace.type_char(c);
                }
            }
        }
        cx.notify();
    }

    fn dispatch_bound(&mut self, action: ClientAction, cx: &mut Context<Self>) {
        if matches!(action, ClientAction::CloseOverlay) && !self.workspace.overlay_flags().any() {
            if self.workspace.context_menu.take().is_some() {
                cx.notify();
                return;
            }
            if self.workspace.first_run_open {
                self.workspace.first_run_open = false;
                let _ = write_first_run_done(&default_first_run_path());
                cx.notify();
                return;
            }
            if self.workspace.dismiss_newest_notice() {
                cx.notify();
                return;
            }
            if self.workspace.reminder.is_some() {
                self.workspace.dismiss_reminder();
                cx.notify();
                return;
            }
            self.focus = Focus::Composer;
            cx.notify();
            return;
        }
        self.dispatch(action, cx);
    }

    fn insert_text(&mut self, text: &str) {
        if self.focus == Focus::Terminal {
            self.workspace.term_draft.push_str(text);
            return;
        }
        if self.palette.open {
            let mut q = self.palette.query.clone();
            q.push_str(text);
            self.palette.set_query(&q);
            return;
        }
        if self.workspace.search_open {
            self.workspace.search_query.push_str(text);
            self.workspace.search_selected = 0;
            return;
        }
        if self.focus == Focus::FileFilter {
            self.workspace.file_filter.push_str(text);
            return;
        }
        self.workspace.cursor = insert_at(&mut self.workspace.draft, self.workspace.cursor, text);
    }

    fn palette_key(&mut self, key: &str, control: bool, cx: &mut Context<Self>) {
        if key == "enter" {
            self.activate_palette(cx);
            return;
        }
        if key == "up" {
            let n = palette_hits(&self.workspace, &self.palette.query).len();
            if n > 0 {
                self.palette.selected = if self.palette.selected == 0 {
                    n - 1
                } else {
                    self.palette.selected - 1
                };
            }
        } else if key == "down" {
            let n = palette_hits(&self.workspace, &self.palette.query).len();
            if n > 0 {
                self.palette.selected = (self.palette.selected + 1) % n;
            }
        } else if key == "backspace" {
            let mut q = self.palette.query.clone();
            q.pop();
            self.palette.set_query(&q);
        } else if key == "space" {
            let mut q = self.palette.query.clone();
            q.push(' ');
            self.palette.set_query(&q);
        } else if !control && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                let mut q = self.palette.query.clone();
                q.push(c);
                self.palette.set_query(&q);
            }
        }
        cx.notify();
    }

    fn terminal_key(&mut self, key: &str, control: bool, cx: &mut Context<Self>) {
        if key == "enter" {
            self.run_terminal_draft();
        } else if key == "backspace" {
            self.workspace.backspace_term();
        } else if key == "tab" {
            self.focus = Focus::Composer;
        } else if key == "space" {
            self.workspace.type_term_char(' ');
        } else if !control && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                self.workspace.type_term_char(c);
            }
        }
        cx.notify();
    }
}

impl Render for ShellView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.pump(window);
        self.apply_theme();
        let win_w = f32::from(window.viewport_size().width);
        let win_h = f32::from(window.viewport_size().height);
        self.win_w = win_w;
        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(Theme::ink())
            .text_color(Theme::text())
            .text_size(Theme::text_ui())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key(event, cx);
            }))
            .on_mouse_move(cx.listener(
                move |this, event: &MouseMoveEvent, _, cx| match this.drag {
                    Some(DragRail::Left) => {
                        this.workspace
                            .chrome
                            .set_left_width(f32::from(event.position.x));
                        cx.notify();
                    }
                    Some(DragRail::Right) => {
                        this.workspace
                            .chrome
                            .set_right_width(win_w - f32::from(event.position.x));
                        cx.notify();
                    }
                    Some(DragRail::Bottom) => {
                        this.workspace.set_bottom_height(bottom_height_from_mouse(
                            win_h,
                            f32::from(event.position.y),
                            28.0,
                            8.0,
                        ));
                        cx.notify();
                    }
                    None => {}
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    cx.notify();
                }),
            )
            .child(self.title_bar(cx))
            .child(self.reminder_bar(cx))
            .child(self.approval_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.left_rail(cx))
                    .child(self.resize_handle(DragRail::Left, cx))
                    .child(self.center(cx))
                    .child(self.resize_handle(DragRail::Right, cx))
                    .child(self.right_rail(cx)),
            )
            .child(self.bottom_resize_handle(cx))
            .child(self.terminal_strip(cx))
            .child(self.status_bar());
        if self.palette.open {
            root = root.child(self.palette_overlay(cx));
        }
        if self.workspace.help_open {
            root = root.child(self.help_overlay(cx));
        }
        if self.workspace.search_open {
            root = root.child(self.search_overlay(cx));
        }
        if self.workspace.settings_open {
            root = root.child(self.settings_overlay(cx));
        }
        if self.workspace.inspector_popped {
            root = root.child(self.popout_inspector(cx));
        }
        if self.workspace.context_menu.is_some() {
            root = root.child(self.context_menu_overlay(cx));
        }
        if self.workspace.first_run_open {
            root = root.child(self.first_run_overlay(cx));
        }
        root = root.child(self.notice_bar(cx));
        root
    }
}

impl ShellView {
    fn title_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let left_on = self.workspace.chrome.left_open();
        let right_on = self.workspace.chrome.right_open();
        let hide_pills = title_overflow(self.win_w);
        let hide = |id: &str| hide_pills.contains(&id);
        glass_bar()
            .h(Theme::title_height())
            .px_3()
            .rounded_none()
            .border_b_1()
            .overflow_hidden()
            .child(icon_btn(
                ChromeGlyph::Layout.mark(),
                if left_on { "Hide chats" } else { "Show chats" },
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleLeft, cx),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .text_color(Theme::text())
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(
                                self.workspace
                                    .selected_thread()
                                    .map(|t| thread_leaf_title(&t.title, &t.id))
                                    .unwrap_or_else(|| "New session".into()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(Theme::text_caption())
                            .text_color(Theme::faint())
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(format!(
                                "Multiplexer   {}   {}",
                                short_path(&self.workspace.project),
                                self.workspace.branch_label()
                            )),
                    ),
            )
            .child(div().flex_1())
            .child(if hide("turns_pill") {
                div().into_any()
            } else {
                click_pill(
                    "turns-pill",
                    format!("{} turns", self.workspace.usage_turns),
                    "",
                    cx,
                    |this, cx| this.dispatch(ClientAction::OpenSessionTab, cx),
                )
            })
            .child(if hide("remotes_pill") {
                div().into_any()
            } else {
                click_pill(
                    "remotes-pill",
                    remotes_pill_label(self.remotes.iter().any(|r| r.kind == "tailscale")),
                    "",
                    cx,
                    |this, cx| this.dispatch(ClientAction::OpenSettingsRemotes, cx),
                )
            })
            .child(if self.workspace.busy {
                icon_btn(ChromeGlyph::Stop.mark(), "Stop", cx, |this, cx| {
                    this.interrupt();
                    cx.notify();
                })
            } else {
                icon_btn(ChromeGlyph::Play.mark(), "Run", cx, |this, cx| {
                    this.dispatch(ClientAction::Send, cx);
                    cx.notify();
                })
            })
            .child(icon_btn(
                ChromeGlyph::Palette.mark(),
                "Palette",
                cx,
                |this, cx| this.dispatch(ClientAction::TogglePalette, cx),
            ))
            .child(icon_btn(
                ChromeGlyph::Settings.mark(),
                "Settings",
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleSettings, cx),
            ))
            .child(icon_btn(
                ChromeGlyph::Terminal.mark(),
                "Terminal",
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleBottom, cx),
            ))
            .child(icon_btn(
                if right_on { "▣" } else { "▢" },
                if right_on {
                    "Hide inspector"
                } else {
                    "Show inspector"
                },
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleRight, cx),
            ))
    }

    fn left_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.workspace.chrome.left_open();
        let w = self.workspace.chrome.occupied_left();
        if w < 0.5 {
            return glass_pane()
                .w(px(0.0))
                .h_full()
                .id("left-hidden")
                .into_any();
        }
        let section = self.workspace.left_section;
        let selected = self.workspace.selected;
        let threads = self.workspace.threads.clone();
        let files = self.workspace.files_visible();
        let activity = activity_items(&self.workspace);
        let icons = div()
            .w(Theme::rail_width())
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .bg(Theme::ink())
            .children(LeftSection::all().into_iter().map(|s| {
                let on = section == s;
                div()
                    .id(SharedString::from(s.rail_label()))
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(Theme::transparent())
                    .text_color(if on { Theme::text() } else { Theme::muted() })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.dispatch(ClientAction::SelectLeftSection(s), cx);
                        }),
                    )
                    .child(s.glyph())
            }));
        let rail = glass_pane()
            .w(px(w))
            .h_full()
            .flex()
            .overflow_hidden()
            .bg(Theme::ink())
            .border_r_1();
        if !open {
            return rail.child(icons).into_any();
        }
        let list =
            div()
                .flex_1()
                .flex()
                .flex_col()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .bg(Theme::ink())
                .child(
                    div()
                        .px_3()
                        .pt_3()
                        .pb_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("new_thread")
                                .flex_1()
                                .h(px(28.0))
                                .px_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_color(Theme::text())
                                .cursor_pointer()
                                .hover(|s| s.text_color(Theme::muted()))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.dispatch(ClientAction::NewThread, cx);
                                    }),
                                )
                                .child(ChromeGlyph::Plus.mark())
                                .child("New session"),
                        )
                        .child(icon_btn("⌫", "Delete", cx, |this, cx| {
                            this.dispatch(ClientAction::DeleteThread, cx);
                        }))
                        .child(icon_btn(
                            ChromeGlyph::Close.mark(),
                            "Hide left",
                            cx,
                            |this, cx| {
                                this.dispatch(ClientAction::HideLeft, cx);
                            },
                        )),
                )
                .child(div().px_3().pb_2().flex().gap_1().children(
                    LeftSection::all().into_iter().map(|s| {
                        let on = section == s;
                        div()
                            .id(SharedString::from(format!("sec-{}", s.rail_label())))
                            .h(px(24.0))
                            .px_1()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(Theme::transparent())
                            .text_color(if on { Theme::text() } else { Theme::faint() })
                            .text_size(Theme::text_caption())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.dispatch(ClientAction::SelectLeftSection(s), cx);
                                }),
                            )
                            .child(s.rail_label())
                    }),
                ))
                .child(
                    div()
                        .px_3()
                        .pb_1()
                        .text_size(Theme::text_caption())
                        .text_color(Theme::faint())
                        .child(if section == LeftSection::Threads {
                            "recent"
                        } else {
                            section.rail_label()
                        }),
                );
        let items: Vec<gpui::AnyElement> = match section {
            LeftSection::Threads => threads
                .into_iter()
                .enumerate()
                .map(|(i, t)| {
                    let title = thread_leaf_title(&t.title, &t.id);
                    let on = i == selected;
                    div()
                        .id(SharedString::from(format!("thr-{i}")))
                        .h(px(32.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .bg(if on {
                            Theme::selection()
                        } else {
                            Theme::transparent()
                        })
                        .text_color(Theme::text())
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.dispatch(ClientAction::SelectThread(i), cx);
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(title),
                        )
                        .into_any()
                })
                .collect(),
            LeftSection::Agents => {
                let rows = self.workspace.agent_rows();
                if rows.is_empty() {
                    vec![list_row(
                        "agent-none",
                        ChromeGlyph::Agent.mark(),
                        "No sessions",
                        "Local threads only",
                        "",
                        false,
                        false,
                        cx,
                        |_this, _cx| {},
                    )]
                } else {
                    rows.into_iter()
                        .map(|row| {
                            let tid = row.id.clone();
                            let idx = row.index;
                            list_row(
                                format!("agent-{}", row.id),
                                ChromeGlyph::Agent.mark(),
                                row.title,
                                format!("{} · {} msgs", row.status.as_str(), row.messages),
                                row.model,
                                row.selected,
                                row.status == multiplexer_shell::ThreadStatus::Running,
                                cx,
                                move |this, cx| {
                                    this.dispatch(
                                        ClientAction::SelectThread(
                                            this.workspace
                                                .threads
                                                .iter()
                                                .position(|t| t.id == tid)
                                                .unwrap_or(idx),
                                        ),
                                        cx,
                                    );
                                },
                            )
                        })
                        .collect()
                }
            }
            LeftSection::Files => {
                if files.is_empty() {
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
                } else {
                    files
                        .into_iter()
                        .map(|p| {
                            let title = leaf_name(&p);
                            list_row(
                                format!("file-{p}"),
                                ChromeGlyph::Folder.mark(),
                                title,
                                p.clone(),
                                "",
                                false,
                                false,
                                cx,
                                move |this, cx| {
                                    if p.ends_with('/') {
                                        this.workspace.toggle_file_expand(&p);
                                    } else {
                                        let _ = this.workspace.select_file(&p);
                                        this.workspace.inspector = InspectorTab::Files;
                                    }
                                    cx.notify();
                                },
                            )
                        })
                        .collect()
                }
            }
            LeftSection::Activity => activity
                .into_iter()
                .map(|item| {
                    let jump = item.id.clone();
                    list_row(
                        item.id.clone(),
                        ChromeGlyph::Activity.mark(),
                        item.title,
                        item.hint,
                        "",
                        false,
                        false,
                        cx,
                        move |this, cx| {
                            this.jump_activity(&jump, cx);
                        },
                    )
                })
                .collect(),
        };
        rail.child(
            list.child(
                div()
                    .id("left-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(items),
            ),
        )
        .into_any()
    }

    fn right_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.workspace.chrome.right_open();
        let w = self.workspace.chrome.occupied_right();
        if w < 0.5 {
            return glass_pane()
                .w(px(0.0))
                .h_full()
                .id("right-hidden")
                .into_any();
        }
        let tab = self.workspace.inspector;
        let buttons = tab_buttons(tab);
        let rows = inspector_rows(&self.workspace);
        let icons = div()
            .id("right-icons")
            .w(Theme::rail_width())
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .overflow_y_scroll()
            .children(InspectorTab::all().into_iter().map(|t| {
                let on = tab == t;
                div()
                    .id(SharedString::from(format!("rtab-{}", t.label())))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(Theme::transparent())
                    .text_color(if on { Theme::text() } else { Theme::muted() })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if this.workspace.chrome.right_open() && this.workspace.inspector == t {
                                this.workspace.chrome.toggle_right();
                            } else {
                                this.dispatch(ClientAction::SelectTab(t), cx);
                            }
                            cx.notify();
                        }),
                    )
                    .child(t.glyph())
            }));
        let rail = glass_pane()
            .w(px(w))
            .h_full()
            .flex()
            .overflow_hidden()
            .bg(Theme::ink())
            .border_l_1();
        if !open {
            return rail.child(icons).into_any();
        }
        let body = div()
            .flex_1()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_color(Theme::faint())
                            .child(tab.label().to_ascii_uppercase()),
                    )
                    .child(icon_btn(
                        ChromeGlyph::Close.mark(),
                        "Hide right",
                        cx,
                        |this, cx| {
                            this.dispatch(ClientAction::HideRight, cx);
                        },
                    )),
            )
            .child(if tab == InspectorTab::Files {
                let q = self.workspace.file_filter.clone();
                div()
                    .id("files_filter")
                    .px_3()
                    .py_1()
                    .text_color(if self.focus == Focus::FileFilter {
                        Theme::text()
                    } else {
                        Theme::muted()
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.focus = Focus::FileFilter;
                            cx.notify();
                        }),
                    )
                    .child(if q.is_empty() {
                        SharedString::from("Filter files…")
                    } else {
                        SharedString::from(q)
                    })
                    .into_any()
            } else {
                div().into_any()
            })
            .child(if buttons.is_empty() {
                div().into_any()
            } else {
                div()
                    .px_2()
                    .flex()
                    .gap_1()
                    .flex_wrap()
                    .children(buttons.into_iter().map(|b| {
                        let action = b.action;
                        ghost_btn(b.label, b.hint, cx, move |this, cx| {
                            this.dispatch(action, cx);
                        })
                    }))
                    .into_any()
            })
            .child(
                div()
                    .id("right-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(rows.into_iter().map(|row| {
                        let detail = if row.expanded {
                            row_detail(&self.workspace, &row.id)
                        } else {
                            String::new()
                        };
                        inspector_row_el(row, detail, cx)
                    })),
            );
        rail.child(icons).child(body).into_any()
    }

    fn center(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.workspace.selected_thread().cloned();
        let draft = draft_display(
            &self.workspace.draft,
            self.workspace.cursor,
            self.focus == Focus::Composer,
        );
        let slash = parse_slash(&self.workspace.draft);
        let tui = self.workspace.center_mode == CenterMode::GrokTui;
        glass_pane()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(Theme::ink())
            .child(self.center_mode_bar(cx))
            .child(if tui {
                self.grok_tui_host(cx)
            } else {
                div()
                    .flex_1()
                    .min_h_0()
                    .px_6()
                    .py_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(match thread.as_ref() {
                        Some(t) if t.messages.is_empty() => vec![empty_center().into_any()],
                        Some(t) => t
                            .messages
                            .iter()
                            .cloned()
                            .map(|m| {
                                let user = m.role == Role::User;
                                div()
                                    .flex()
                                    .justify_center()
                                    .child(
                                        div()
                                            .w_full()
                                            .max_w(px(720.0))
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(Theme::text_caption())
                                                    .text_color(Theme::faint())
                                                    .child(if user { "You" } else { "Grok" }),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Theme::text())
                                                    .text_size(Theme::text_body())
                                                    .child(m.text),
                                            ),
                                    )
                                    .into_any()
                            })
                            .collect(),
                        None => vec![empty_center().into_any()],
                    })
                    .child(if self.workspace.busy {
                        let secs = self
                            .turn_started
                            .map(|t| t.elapsed().as_secs())
                            .unwrap_or(0);
                        div()
                            .text_color(Theme::muted())
                            .child(working_copy(secs))
                            .into_any()
                    } else {
                        div().into_any()
                    })
                    .into_any()
            })
            .child(if tui {
                div().id("no-composer").into_any()
            } else {
                div()
                    .px_5()
                    .pb_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(Theme::hairline())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(if thread.as_ref().is_some_and(|t| !t.messages.is_empty()) {
                        div().into_any()
                    } else {
                        div()
                            .flex()
                            .gap_3()
                            .flex_wrap()
                            .child(chip("What can you do?", cx, |this, cx| {
                                this.workspace.set_draft("What can you do?");
                                this.dispatch(ClientAction::Send, cx);
                                cx.notify();
                            }))
                            .child(chip("Summarize this repo", cx, |this, cx| {
                                this.workspace.set_draft("Summarize this repo");
                                this.dispatch(ClientAction::Send, cx);
                                cx.notify();
                            }))
                            .child(chip("git status", cx, |this, cx| {
                                this.run_shell("git status");
                                cx.notify();
                            }))
                            .child(chip("Run the tests", cx, |this, cx| {
                                this.workspace.set_draft("Run the tests");
                                this.dispatch(ClientAction::Send, cx);
                                cx.notify();
                            }))
                            .into_any()
                    })
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_3()
                            .child(
                                div()
                                    .id("composer")
                                    .flex_1()
                                    .min_h(px(44.0))
                                    .cursor_pointer()
                                    .text_color(if self.workspace.draft.is_empty() {
                                        Theme::faint()
                                    } else {
                                        Theme::text()
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus = Focus::Composer;
                                            cx.notify();
                                        }),
                                    )
                                    .child(if self.workspace.draft.is_empty() {
                                        SharedString::from("Message Grok…  @ files   / commands")
                                    } else {
                                        SharedString::from(draft)
                                    }),
                            )
                            .child(
                                div()
                                    .id("send-circle")
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .text_color(
                                        if self.workspace.draft.trim().is_empty()
                                            || self.workspace.busy
                                        {
                                            Theme::faint()
                                        } else {
                                            Theme::text()
                                        },
                                    )
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.dispatch(ClientAction::Send, cx);
                                            cx.notify();
                                        }),
                                    )
                                    .child("Send"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(Theme::text_caption())
                                    .text_color(Theme::faint())
                                    .child(self.workspace.model.clone()),
                            )
                            .child(div().flex_1())
                            .child(if let Some(cmd) = slash {
                                div()
                                    .text_size(Theme::text_caption())
                                    .text_color(Theme::muted())
                                    .child(multiplexer_shell::slash_hint(&cmd))
                            } else {
                                div()
                            }),
                    )
                    .into_any()
            })
    }

    fn center_mode_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.workspace.center_mode;
        div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(Theme::hairline())
            .flex()
            .items_center()
            .gap_2()
            .child(
                self.mode_chip("Chat log", mode == CenterMode::Gui, cx, |this, cx| {
                    this.dispatch(ClientAction::SetCenterGui, cx);
                }),
            )
            .child(
                self.mode_chip("Grok TUI", mode == CenterMode::GrokTui, cx, |this, cx| {
                    this.dispatch(ClientAction::SetCenterTui, cx);
                }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(Theme::text_caption())
                    .text_color(Theme::faint())
                    .child(if mode == CenterMode::GrokTui {
                        "Grok owns the agent. This is the host, not a rewrite."
                    } else {
                        "Headless log (grok -p). Switch to Grok TUI for the real pager."
                    }),
            )
    }

    fn mode_chip(
        &mut self,
        label: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("center-{label}")))
            .h(px(28.0))
            .px_1()
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(Theme::transparent())
            .border_b_1()
            .border_color(if on {
                Theme::text()
            } else {
                Theme::transparent()
            })
            .text_color(if on { Theme::text() } else { Theme::faint() })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| on_click(this, cx)),
            )
            .child(label)
    }

    fn grok_tui_host(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let host = self.workspace.grok_tui.clone();
        div()
            .id("grok-tui-host")
            .flex_1()
            .min_h_0()
            .p_6()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_size(Theme::text_body())
                    .text_color(Theme::text())
                    .child("Grok TUI host"),
            )
            .child(
                div()
                    .max_w(px(560.0))
                    .text_color(Theme::muted())
                    .child(host.summary()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(ghost_btn("Launch", "console", cx, |this, cx| {
                        this.launch_grok_tui();
                        cx.notify();
                    }))
                    .child(ghost_btn("Stop", "kill", cx, |this, cx| {
                        this.stop_grok_tui();
                        cx.notify();
                    }))
                    .child(ghost_btn("Diffs", "g d", cx, |this, cx| {
                        this.dispatch(ClientAction::SelectTab(InspectorTab::Diff), cx);
                    }))
                    .child(ghost_btn("Browser", "g b", cx, |this, cx| {
                        this.dispatch(ClientAction::SelectTab(InspectorTab::Browser), cx);
                    })),
            )
            .child(
                div()
                    .text_size(Theme::text_caption())
                    .text_color(Theme::faint())
                    .child("Launches interactive grok (no -p) in Windows Terminal or a new console. In-pane ConPTY is later."),
            )
            .into_any()
    }

    fn reminder_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((branch, path)) = self.workspace.reminder.clone() else {
            return div().id("no-reminder").into_any();
        };
        div()
            .id("worktree-reminder")
            .px_3()
            .py_1()
            .bg(Theme::reminder_fill())
            .border_b_1()
            .border_color(Theme::hairline())
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .child(format!("Existing worktree on {branch}: {path}")),
            )
            .child(ghost_btn("Dismiss", "Esc", cx, |this, cx| {
                this.dispatch(ClientAction::DismissReminder, cx);
            }))
            .into_any()
    }

    fn approval_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(pending) = self.workspace.pending_approval().cloned() else {
            return div().id("no-approval").into_any();
        };
        div()
            .id("approval-card")
            .px_3()
            .py_2()
            .bg(Theme::approval_fill())
            .border_b_1()
            .border_color(Theme::hairline())
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(format!(
                "{}  {}",
                pending.card_title(),
                pending.card_body()
            )))
            .child(ghost_btn(pending.allow_label(), "A", cx, |this, cx| {
                this.dispatch(ClientAction::Approve, cx);
            }))
            .child(ghost_btn(pending.deny_label(), "D", cx, |this, cx| {
                this.dispatch(ClientAction::Deny, cx);
            }))
            .child(ghost_btn("Once", "O", cx, |this, cx| {
                this.dispatch(ClientAction::ApproveOnce, cx);
            }))
            .child(ghost_btn("Later", "L", cx, |this, cx| {
                this.dispatch(ClientAction::Later, cx);
            }))
            .into_any()
    }

    fn notice_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.workspace.notices.is_empty() {
            return div().id("no-notices").into_any();
        }
        let notices = visible_notices(&self.workspace.notices).to_vec();
        div()
            .id("notice-stack")
            .absolute()
            .top(px(40.0))
            .right(px(12.0))
            .w(px(320.0))
            .flex()
            .flex_col()
            .gap_1()
            .children(notices.into_iter().rev().map(|n| {
                let id = n.id;
                div()
                    .id(SharedString::from(format!("notice-{id}")))
                    .px_3()
                    .py_2()
                    .bg(match n.kind {
                        NoticeKind::Good => Theme::toast_fill(NoticeKind::Good),
                        NoticeKind::Warn => Theme::toast_fill(NoticeKind::Warn),
                        NoticeKind::Danger => Theme::toast_fill(NoticeKind::Danger),
                        NoticeKind::Info => Theme::selection(),
                    })
                    .border_1()
                    .border_color(Theme::hairline())
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(n.text))
                    .child(icon_btn(
                        ChromeGlyph::Close.mark(),
                        "dismiss",
                        cx,
                        move |this, cx| {
                            multiplexer_shell::dismiss_notice(&mut this.workspace.notices, id);
                            cx.notify();
                        },
                    ))
            }))
            .into_any()
    }

    fn settings_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = format!("{:?}", self.workspace.settings.mode);
        let density = format!("{:?}", self.workspace.settings.density);
        let model = self.workspace.settings.default_model.clone();
        let models = self.workspace.models.clone();
        let remotes = self.remotes.clone();
        let turns = self.workspace.usage_turns;
        let tokens = self.workspace.usage_tokens;
        let section = self.workspace.settings_section;
        let pairs = self.bindings.pairs();
        let project = self.workspace.project.clone();
        let body = match section {
            SettingsSection::Appearance => format!(
                "Theme {mode}   Density {density}\nMotion {}   Scale {}%   Contrast {}\nSaved to %APPDATA%\\Multiplexer\\settings.json. No secrets.",
                if self.workspace.settings.reduce_motion { "reduced" } else { "full" },
                self.workspace.settings.ui_scale,
                if self.workspace.settings.high_contrast { "high" } else { "standard" },
            ),
            SettingsSection::Models => format!("Default {model}. Click a row to apply."),
            SettingsSection::Bindings => "Chord table. Ctrl+P is search. Ctrl+Shift+P is palette.".into(),
            SettingsSection::Inspector => "Inspector customize later. Not shipped.".into(),
            SettingsSection::Session => format!("{turns} turns  {tokens} tok (local snapshot only)\nProject {project}"),
            SettingsSection::Remotes => remotes_serve_note().into(),
            SettingsSection::About => about_info(which_grok().as_deref()).lines(),
        };
        div()
            .id("settings")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(48.0))
            .bg(Theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dispatch(ClientAction::ToggleSettings, cx);
                }),
            )
            .child(
                div()
                    .w(px(640.0))
                    .max_w(px(640.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .p_4()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(Theme::text_body())
                                    .text_color(Theme::text())
                                    .child("Settings"),
                            )
                            .child(ghost_btn("Close", "Esc", cx, |this, cx| {
                                this.dispatch(ClientAction::CloseOverlay, cx);
                            })),
                    )
                    .child(div().mt_2().flex().gap_1().flex_wrap().children(
                        SettingsSection::all().into_iter().map(|sec| {
                            let on = section == sec;
                            ghost_btn(
                                sec.label(),
                                if on { "on" } else { "" },
                                cx,
                                move |this, cx| {
                                    this.workspace.settings_section = sec;
                                    cx.notify();
                                },
                            )
                        }),
                    ))
                    .child(div().mt_3().text_color(Theme::muted()).child(body))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .gap_2()
                            .flex_wrap()
                            .child(ghost_btn("Theme", "cycle", cx, |this, cx| {
                                this.workspace.settings.cycle_mode();
                                this.apply_theme();
                                this.persist_settings();
                                cx.notify();
                            }))
                            .child(ghost_btn("Density", "cycle", cx, |this, cx| {
                                this.workspace.settings.cycle_density();
                                this.apply_theme();
                                this.persist_settings();
                                cx.notify();
                            }))
                            .child(ghost_btn("Use model", "apply", cx, |this, cx| {
                                this.dispatch(ClientAction::SelectModel, cx);
                                this.persist_settings();
                            }))
                            .child(ghost_btn("Motion", "a11y", cx, |this, cx| {
                                this.workspace.settings.toggle_reduce_motion();
                                this.persist_settings();
                                cx.notify();
                            }))
                            .child(ghost_btn("Scale", "100-200", cx, |this, cx| {
                                this.workspace.settings.bump_ui_scale();
                                this.apply_theme();
                                this.persist_settings();
                                cx.notify();
                            }))
                            .child(ghost_btn("Contrast", "a11y", cx, |this, cx| {
                                this.workspace.settings.toggle_high_contrast();
                                this.apply_theme();
                                this.persist_settings();
                                cx.notify();
                            })),
                    )
                    .children(if section == SettingsSection::Models {
                        models
                            .into_iter()
                            .map(|m| {
                                let shown = m.clone();
                                let pick = m.clone();
                                div()
                                    .id(SharedString::from(format!("set-model-{shown}")))
                                    .mt_1()
                                    .px_2()
                                    .py_1()
                                    .bg(if shown == self.workspace.model {
                                        Theme::selection()
                                    } else {
                                        Theme::transparent()
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.workspace.settings.set_default_model(pick.clone());
                                            let _ = this.workspace.select_model(pick.clone());
                                            this.persist_settings();
                                            let frames = this.server.handle_frame(&rpc(
                                                "ms",
                                                methods::MODEL_SELECT,
                                                json!({ "model": pick }),
                                            ));
                                            if let Some(err) = first_error(&frames) {
                                                this.workspace.push_notice(NoticeKind::Warn, err);
                                            } else {
                                                this.workspace.push_notice(
                                                    NoticeKind::Good,
                                                    format!("model {pick}"),
                                                );
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(shown)
                                    .into_any()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    })
                    .children(if section == SettingsSection::Bindings {
                        pairs
                            .into_iter()
                            .take(16)
                            .map(|(chord, action)| {
                                div()
                                    .id(SharedString::from(format!("bind-{chord}")))
                                    .mt_1()
                                    .px_2()
                                    .py_1()
                                    .child(format!("{chord}  →  {action}"))
                                    .into_any()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    })
                    .children(if section == SettingsSection::Remotes {
                        remotes
                            .into_iter()
                            .map(|r| {
                                div()
                                    .id(SharedString::from(format!("remote-{}", r.id)))
                                    .mt_1()
                                    .px_2()
                                    .py_1()
                                    .child(format!("{}  ·  {}  ·  {}", r.label, r.kind, r.id))
                                    .into_any()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    }),
            )
    }

    fn terminal_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.workspace.bottom_hidden {
            return glass_bar().h(px(0.0)).id("term-hidden").into_any();
        }
        let open = self.workspace.bottom_open;
        let tail_n = if open { 14 } else { 2 };
        let lines = if self.workspace.terminal_log.is_empty() {
            "Terminal: type a command and Enter. Builtins: clear, help, cores, mcp, git, points, skills."
                .to_owned()
        } else {
            visible_tail(&self.workspace.terminal_log, tail_n)
        };
        let draft = if self.workspace.term_draft.is_empty() {
            format!("{TERM_PROMPT}  git status, dir, clear")
        } else {
            format!("{TERM_PROMPT} {}", self.workspace.term_draft)
        };
        let bar = glass_bar()
            .h(px(self.workspace.occupied_bottom()))
            .px_3()
            .rounded_none()
            .border_t_1()
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .child(
                div()
                    .w_full()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_color(Theme::faint())
                            .child("TERMINAL  drag the handle above to resize"),
                    )
                    .child(ghost_btn("Hide", "hide", cx, |this, cx| {
                        this.dispatch(ClientAction::HideBottom, cx);
                    }))
                    .child(if open {
                        div()
                    } else {
                        div().child(ghost_btn("Show", "ctrl-`", cx, |this, cx| {
                            this.dispatch(ClientAction::ToggleBottom, cx);
                        }))
                    }),
            );
        if !open {
            return bar.into_any();
        }
        bar.child(
            div()
                .id("term-scroll")
                .flex_1()
                .w_full()
                .min_h_0()
                .overflow_y_scroll()
                .text_color(Theme::muted())
                .child(lines),
        )
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id("term-input")
                        .flex_1()
                        .px_1()
                        .py_1()
                        .text_color(if self.focus == Focus::Terminal {
                            Theme::text()
                        } else {
                            Theme::muted()
                        })
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.focus = Focus::Terminal;
                                cx.notify();
                            }),
                        )
                        .child(draft),
                )
                .child(ghost_btn("Run", "↵", cx, |this, cx| {
                    this.run_terminal_draft();
                    cx.notify();
                }))
                .child(ghost_btn("Clear", "cls", cx, |this, cx| {
                    this.workspace.terminal_log.clear();
                    cx.notify();
                }))
                .child(ghost_btn("Kill", "Ctrl+.", cx, |this, cx| {
                    this.kill_capture();
                    cx.notify();
                })),
        )
        .into_any()
    }

    fn status_bar(&self) -> impl IntoElement {
        let s = status_from(&self.workspace, self.session_id.clone());
        let cores = self.workspace.cores.len();
        let remotes = remotes_pill_label(self.remotes.iter().any(|r| r.kind == "tailscale"));
        let live = format!(
            "{}   ·   {} turns  {} tok  mcp listed {}  cpu {cores}  remotes {remotes}",
            status_line(&s),
            self.workspace.usage_turns,
            self.workspace.usage_tokens,
            self.workspace.mcp.len(),
        );
        glass_bar()
            .h(px(22.0))
            .px_3()
            .rounded_none()
            .border_t_1()
            .child(
                div()
                    .flex_1()
                    .text_size(Theme::text_caption())
                    .text_color(Theme::faint())
                    .child(live),
            )
    }

    fn search_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let hits = search_workspace(&self.workspace, &self.workspace.search_query);
        let selected = self.workspace.search_selected;
        let q = self.workspace.search_query.clone();
        div()
            .id("search")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(80.0))
            .bg(Theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dispatch(ClientAction::CloseSearch, cx);
                }),
            )
            .child(
                div()
                    .w(px(560.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .p_3()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .px_2()
                            .py_2()
                            .mb_2()
                            .border_b_1()
                            .border_color(Theme::hairline())
                            .child(if q.is_empty() {
                                SharedString::from("Type a file, thread, or command name")
                            } else {
                                SharedString::from(q)
                            }),
                    )
                    .children(if hits.is_empty() {
                        vec![div()
                            .px_2()
                            .py_2()
                            .text_color(Theme::faint())
                            .child("No name hits. Content search is not shipped.")
                            .into_any()]
                    } else {
                        hits.into_iter()
                            .enumerate()
                            .take(14)
                            .map(|(i, hit)| {
                                let kind = match hit.kind {
                                    SearchKind::Thread => "thread",
                                    SearchKind::File => "file",
                                    SearchKind::Command => "cmd",
                                    SearchKind::Pane => "pane",
                                    SearchKind::Recent => "recent",
                                };
                                div()
                                    .id(SharedString::from(format!("srch-{i}")))
                                    .px_2()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(if i == selected {
                                        Theme::selection()
                                    } else {
                                        Theme::transparent()
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.workspace.search_selected = i;
                                            this.activate_search(cx);
                                        }),
                                    )
                                    .child(format!("{}  {}   {}", kind, hit.title, hit.hint))
                                    .into_any()
                            })
                            .collect()
                    }),
            )
    }

    fn palette_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let hits = palette_hits(&self.workspace, &self.palette.query);
        let selected = self.palette.selected;
        div()
            .id("palette")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(80.0))
            .bg(Theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dispatch(ClientAction::ClosePalette, cx);
                }),
            )
            .child(
                div()
                    .w(px(560.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .p_3()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .px_2()
                            .py_2()
                            .mb_2()
                            .border_b_1()
                            .border_color(Theme::hairline())
                            .child(if self.palette.query.is_empty() {
                                SharedString::from("Search threads, files, commands…")
                            } else {
                                SharedString::from(self.palette.query.clone())
                            }),
                    )
                    .children(hits.into_iter().enumerate().take(14).map(|(i, hit)| {
                        let kind = match hit.kind {
                            SearchKind::Thread => "thread",
                            SearchKind::File => "file",
                            SearchKind::Command => "cmd",
                            SearchKind::Pane => "pane",
                            SearchKind::Recent => "recent",
                        };
                        div()
                            .id(SharedString::from(format!("pal-{i}")))
                            .px_2()
                            .py_2()
                            .rounded_lg()
                            .bg(if i == selected {
                                Theme::selection()
                            } else {
                                Theme::transparent()
                            })
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.palette.selected = i;
                                    this.activate_palette(cx);
                                }),
                            )
                            .child(format!("{}  {}   {}", kind, hit.title, hit.hint))
                    })),
            )
    }

    fn help_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("help")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(72.0))
            .bg(Theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.workspace.toggle_help();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(560.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .p_4()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child("Keyboard")
                            .child(icon_btn("×", "Close", cx, |this, cx| {
                                this.dispatch(ClientAction::ToggleHelp, cx);
                            })),
                    )
                    .child(div().text_color(Theme::muted()).mt_2().child(
                        "Enter send   Shift+Enter newline   Ctrl+K / Ctrl+Shift+P palette\nCtrl+P / Ctrl+Shift+F name search   F1 help   Ctrl+, / F2 settings\nCtrl+Shift+G Grok TUI / chat log   Ctrl+N new chat   Ctrl+[ / ] rails   Ctrl+` terminal\nCtrl+. stop   Ctrl+S checkpoint pointer   Ctrl+Shift+L reset   Ctrl+Shift+H focus\nCtrl+Shift+D pop-out inspector   Ctrl+Shift+E dock   Ctrl+W close pop-out   Ctrl+Tab region\nCtrl+Alt+Up/Down bottom   A/D/O/L approval when pending   Ctrl+1..4 left sections\nEsc pops overlay then toast\nSlash: /new /stop /help /search /settings /files /agents /diff /browser /tui /about\nGrok TUI is the real pager in a console. Chat log is grok -p only.",
                    )),
            )
    }

    fn popout_inspector(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.workspace.inspector;
        let rows = inspector_rows(&self.workspace);
        div()
            .id("popout-inspector")
            .absolute()
            .top(px(64.0))
            .right(px(16.0))
            .w(px(360.0))
            .max_h(px(640.0))
            .rounded_xl()
            .bg(Theme::glass_strong())
            .border_1()
            .border_color(Theme::hairline_bright())
            .p_2()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_color(Theme::text())
                            .child(format!("Inspector · {} · same HWND", tab.label())),
                    )
                    .child(ghost_btn("Dock", "Ctrl+Shift+E", cx, |this, cx| {
                        this.dispatch(ClientAction::DockInspector, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .children(rows.into_iter().map(|row| {
                        let detail = row_detail(&self.workspace, &row.id);
                        inspector_row_el(row, detail, cx)
                    })),
            )
    }

    fn context_menu_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(menu) = self.workspace.context_menu.clone() else {
            return div().id("no-menu").into_any();
        };
        div()
            .id("context-menu")
            .absolute()
            .top(px(80.0))
            .left(px(80.0))
            .w(px(220.0))
            .rounded_xl()
            .bg(Theme::glass_strong())
            .border_1()
            .border_color(Theme::hairline_bright())
            .p_2()
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .child(
                div()
                    .text_color(Theme::faint())
                    .child(format!("{:?}  {}", menu.kind, menu.target)),
            )
            .children(menu.items.into_iter().map(|item| {
                let action = item.action;
                ghost_btn(item.label, item.id, cx, move |this, cx| {
                    this.workspace.context_menu = None;
                    this.dispatch(action, cx);
                })
            }))
            .child(ghost_btn("Close", "Esc", cx, |this, cx| {
                this.workspace.context_menu = None;
                cx.notify();
            }))
            .into_any()
    }

    fn first_run_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("first-run")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(80.0))
            .bg(Theme::overlay_scrim())
            .child(
                div()
                    .w(px(480.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .p_4()
                    .child(
                        div()
                            .text_color(Theme::text())
                            .child("Welcome to Multiplexer"),
                    )
                    .child(div().mt_2().text_color(Theme::muted()).child(
                        format!(
                            "Project {}\nTheme {:?}\n{}\nGrok TUI hosts the real pager. Chat log is grok -p.",
                            leaf_name(&self.workspace.project),
                            self.workspace.settings.mode,
                            first_run_keychain_notice()
                        ),
                    ))
                    .child(
                        div()
                            .mt_3()
                            .flex()
                            .gap_2()
                            .child(ghost_btn("Continue", "enter", cx, |this, cx| {
                                this.workspace.first_run_open = false;
                                let _ = write_first_run_done(&default_first_run_path());
                                cx.notify();
                            }))
                            .child(ghost_btn("Skip", "esc", cx, |this, cx| {
                                this.workspace.first_run_open = false;
                                let _ = write_first_run_done(&default_first_run_path());
                                cx.notify();
                            })),
                    ),
            )
    }

    fn resize_handle(&mut self, rail: DragRail, cx: &mut Context<Self>) -> impl IntoElement {
        let open = match rail {
            DragRail::Left => self.workspace.chrome.left_open(),
            DragRail::Right => self.workspace.chrome.right_open(),
            DragRail::Bottom => !self.workspace.bottom_hidden && self.workspace.bottom_open,
        };
        if !open {
            return div().id("resize-hidden").w(px(0.0)).into_any();
        }
        div()
            .id(SharedString::from(match rail {
                DragRail::Left => "resize-left",
                DragRail::Right => "resize-right",
                DragRail::Bottom => "resize-bottom",
            }))
            .w(px(7.0))
            .h_full()
            .cursor(CursorStyle::ResizeLeftRight)
            .hover(|s| s.bg(Theme::hover_fill()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.drag = Some(rail);
                    cx.notify();
                }),
            )
            .into_any()
    }

    fn bottom_resize_handle(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.workspace.bottom_hidden {
            return div().id("resize-bottom-hidden").h(px(0.0)).into_any();
        }
        div()
            .id("resize-bottom")
            .w_full()
            .h(px(8.0))
            .cursor(CursorStyle::ResizeUpDown)
            .hover(|s| s.bg(Theme::hover_strong()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = Some(DragRail::Bottom);
                    cx.notify();
                }),
            )
            .into_any()
    }
}

fn draft_display(draft: &str, cursor: usize, show_caret: bool) -> String {
    if !show_caret {
        return draft.to_owned();
    }
    let cursor = cursor.min(draft.chars().count());
    let mut out = String::new();
    for (i, ch) in draft.chars().enumerate() {
        if i == cursor {
            out.push('|');
        }
        out.push(ch);
    }
    if cursor == draft.chars().count() {
        out.push('|');
    }
    out
}

fn char_byte(s: &str, cursor: usize) -> usize {
    s.char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn short_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn rpc(id: &str, method: &str, params: Value) -> String {
    encode_frame(&Message::Request(Request::new(
        Id::String(id.to_owned()),
        method,
        params,
    )))
    .expect("encode")
}

fn first_error(frames: &[String]) -> Option<String> {
    for f in frames {
        if let Ok(Message::Error(e)) = decode_frame(f) {
            return Some(e.error.message);
        }
    }
    None
}

fn worktree_cards(frames: &[String]) -> Vec<WorktreeCard> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            if let Some(arr) = r.result.get("worktrees").and_then(Value::as_array) {
                return arr
                    .iter()
                    .filter_map(|row| {
                        let path = row.get("path").and_then(Value::as_str)?;
                        Some(WorktreeCard {
                            path: path.to_owned(),
                            branch: row.get("branch").and_then(Value::as_str).map(str::to_owned),
                        })
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

fn worktree_records(frames: &[String]) -> Vec<Worktree> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            if let Some(arr) = r.result.get("worktrees").and_then(Value::as_array) {
                return arr
                    .iter()
                    .filter_map(|row| {
                        let path = row.get("path").and_then(Value::as_str)?;
                        Some(Worktree {
                            path: path.to_owned(),
                            head: row.get("head").and_then(Value::as_str).map(str::to_owned),
                            branch: row.get("branch").and_then(Value::as_str).map(str::to_owned),
                            detached: row
                                .get("detached")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            locked: row.get("locked").and_then(Value::as_bool).unwrap_or(false),
                            prunable: row
                                .get("prunable")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        })
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

fn session_id_from(frames: &[String]) -> Option<String> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            if let Some(id) = r.result.get("session_id").and_then(Value::as_str) {
                return Some(id.to_owned());
            }
        }
    }
    None
}

fn which_grok() -> Option<String> {
    let out = std::process::Command::new("where.exe")
        .arg("grok")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_owned())
    }
}

fn tailscale_which() -> Option<String> {
    let out = std::process::Command::new("where.exe")
        .arg("tailscale")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_owned())
    }
}

fn models_from(frames: &[String]) -> Option<Vec<String>> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let arr = r.result.get("models")?.as_array()?;
            return Some(
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            );
        }
    }
    None
}

fn usage_from(frames: &[String]) -> Option<(u64, u64, String)> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let turns = r.result.get("turns")?.as_u64()?;
            let tokens = r.result.get("tokens")?.as_u64()?;
            let note = r
                .result
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Some((turns, tokens, note));
        }
    }
    None
}

fn remotes_from(frames: &[String]) -> Option<Vec<RemoteRow>> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let arr = r.result.get("remotes")?.as_array()?;
            return Some(
                arr.iter()
                    .filter_map(|row| {
                        Some(RemoteRow {
                            id: row.get("id")?.as_str()?.to_owned(),
                            kind: row.get("kind")?.as_str()?.to_owned(),
                            label: row.get("label")?.as_str()?.to_owned(),
                        })
                    })
                    .collect(),
            );
        }
    }
    None
}

fn load_config_models() -> Vec<String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"));
    let Ok(home) = home else {
        return Vec::new();
    };
    let path = PathBuf::from(home).join(".grok").join("config.toml");
    match std::fs::read_to_string(path) {
        Ok(text) => parse_model_keys(&text),
        Err(_) => Vec::new(),
    }
}

fn skill_preview(cands: &[(String, &str)], name: &str, source: &str) -> String {
    let dir = cands
        .iter()
        .find(|(_, src)| *src == source)
        .map(|(p, _)| p.as_str())
        .unwrap_or("");
    let path = PathBuf::from(dir).join(name).join("SKILL.md");
    let alt = PathBuf::from(dir).join(format!("{name}.md"));
    let text = std::fs::read_to_string(&path).or_else(|_| std::fs::read_to_string(&alt));
    match text {
        Ok(body) => cap_text(&body, 4096),
        Err(_) => String::new(),
    }
}

fn checkpoints_from(frames: &[String]) -> Option<Vec<CheckpointRow>> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let arr = r.result.get("checkpoints")?.as_array()?;
            return Some(
                arr.iter()
                    .filter_map(|row| {
                        Some(CheckpointRow {
                            id: row.get("id")?.as_str()?.to_owned(),
                            label: row
                                .get("label")
                                .and_then(Value::as_str)
                                .unwrap_or("manual")
                                .to_owned(),
                        })
                    })
                    .collect(),
            );
        }
    }
    None
}

fn checkpoint_from(frames: &[String]) -> Option<(String, String, String)> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let id = r.result.get("id").and_then(Value::as_str)?;
            let label = r
                .result
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("manual");
            let sha = r
                .result
                .get("sha")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Some((id.to_owned(), label.to_owned(), sha));
        }
    }
    None
}

fn revert_restored(frames: &[String]) -> bool {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            return r
                .result
                .get("restored")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
    }
    false
}

fn checkpoint_diff_text(frames: &[String]) -> Option<String> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            return r
                .result
                .get("diff")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
    }
    None
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1360.0), px(860.0)), cx);
        cx.open_window(Theme::window_options(bounds), |_, cx| {
            cx.new(|_| ShellView::new())
        })
        .expect("open Multiplexer window");
        cx.activate(true);
    });
}
