//! Multiplexer desktop: glass chrome, live grok -p, working inspector and terminal.

mod controls;
mod inspector;
mod theme;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use gpui::{
    div, hsla, prelude::*, px, size, App, Application, Bounds, ClipboardItem, Context, CursorStyle,
    KeyDownEvent, MouseButton, MouseMoveEvent, SharedString, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use inspector::{tab_buttons, InspectorAction};
use multiplexer_checkpoint::CheckpointStore;
use multiplexer_client::{
    list_project_tree, spawn_command, spawn_grok_tui, spawn_grok_turn, windows_cmd, CommandResult,
    ListOptions, TuiLaunch, TurnRequest, TurnResult,
};
use multiplexer_mcp::{
    list_dir_entry_names, load_user_mcp_inventory, merge_skill_rows, parse_skill_names,
    skill_dir_candidates,
};
use multiplexer_resman::sample_cores;
use multiplexer_server::Server;
use multiplexer_shell::{
    apply_layout_action, default_items, delete_forward, detect_remotes, empty_state_tiles,
    format_line, help_text, insert_at, inspector_rows, move_end, move_home, move_left, move_right,
    move_word_left, move_word_right, palette_hits, parse_builtin, parse_slash, row_detail,
    status_from, status_line, visible_tail, BuiltinCmd, CenterMode, CheckpointRow, ChromeGlyph,
    ClientAction, CoreRow, EmptyStateSpec, InspectorTab, LeftSection, ListRowSpec, McpRow,
    NoticeKind, PaletteState, RemoteRow, Role, SearchKind, SlashCommand, TermLineKind, TuiLife,
    Workspace, TERM_PROMPT,
};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};
use theme::Theme;

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
}

struct ShellView {
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
    flash: Option<String>,
    remotes: Vec<RemoteRow>,
    grok_tui: Option<std::process::Child>,
    pending_turn_diffs: bool,
}

impl ShellView {
    fn new() -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
        let mut workspace = Workspace::new(cwd.clone(), "grok");
        workspace.set_models(vec!["grok".into(), "grok-4.6".into(), "fake".into()]);
        workspace.connect(Vec::new());
        workspace.cores = sample_cores(&[0, 1])
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
        let mut store = CheckpointStore::new();
        let start = store.create("local", "start");
        workspace.checkpoints.push(CheckpointRow {
            id: start.id.to_string(),
            label: start.label,
        });
        let server = Server::with_local();
        server.install_checkpoints(store);
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
            flash: None,
            remotes: detect_remotes(tailscale_which().as_deref()),
            grok_tui: None,
            pending_turn_diffs: false,
        };
        view.apply_theme();
        view.bootstrap_catalogs();
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
        assert_eq!(controls::Surface::all().len(), 10);
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
    }

    fn bootstrap_catalogs(&mut self) {
        let models = self
            .server
            .handle_frame(&rpc("ml", methods::MODEL_LIST, json!({})));
        if let Some(list) = models_from(&models) {
            if !list.is_empty() {
                self.workspace.set_models(list);
            }
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
                let pid = child.id();
                self.workspace
                    .grok_tui
                    .mark_running(pid, launch.program.display().to_string());
                self.grok_tui = Some(child);
                self.workspace
                    .push_notice(NoticeKind::Good, format!("Grok TUI launched (pid {pid})"));
                self.term_meta(&format!("grok tui pid {pid}"));
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
        self.run_shell(&format!("start \"\" \"{url}\""));
        self.workspace.push_notice(
            NoticeKind::Info,
            format!("opened {url} (system browser, no CDP)"),
        );
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

    fn activate_inspector_row(&mut self, id: &str) {
        self.workspace.toggle_right_row(id.to_owned());
        if let Some(path) = id.strip_prefix("file:") {
            if path.ends_with('/') {
                let _ = self.workspace.toggle_file_expand(path);
            } else {
                let _ = self.workspace.select_file(path);
            }
        } else if let Some(cid) = id.strip_prefix("point:") {
            self.workspace.select_checkpoint(Some(cid.to_owned()));
        } else if let Some(aid) = id.strip_prefix("agent:") {
            if let Some(i) = self.workspace.threads.iter().position(|t| t.id == aid) {
                let _ = self.workspace.select(i);
            }
        } else if let Some(rest) = id.strip_prefix("git:wt:") {
            if let Ok(i) = rest.parse::<usize>() {
                self.workspace.selected_worktree = Some(i);
            }
        }
    }

    fn activate_palette(&mut self, cx: &mut Context<Self>) {
        let query = self.palette.query.clone();
        let selected = self.palette.selected;
        self.palette.close();
        self.workspace.close_palette();
        self.focus = Focus::Composer;
        if query.is_empty() {
            if let Some(item) = default_items().get(selected).copied() {
                self.dispatch(item.action, cx);
            }
            return;
        }
        let hits = palette_hits(&self.workspace, &query);
        let Some(hit) = hits.get(selected).cloned() else {
            return;
        };
        match hit.kind {
            SearchKind::Thread => {
                if let Some(i) = self.workspace.threads.iter().position(|t| t.id == hit.id) {
                    self.dispatch(ClientAction::SelectThread(i), cx);
                }
            }
            SearchKind::File => {
                let _ = self.workspace.select_file(&hit.id);
                self.dispatch(ClientAction::SelectTab(InspectorTab::Files), cx);
            }
            SearchKind::Command => {
                if let Some(item) = default_items().into_iter().find(|i| i.id == hit.id) {
                    self.dispatch(item.action, cx);
                }
            }
        }
    }

    fn dispatch(&mut self, action: ClientAction, cx: &mut Context<Self>) {
        match multiplexer_shell::host_call(action, &self.action_ctx()) {
            multiplexer_shell::HostCall::Local => {
                let _ = apply_layout_action(&mut self.workspace, action);
                match action {
                    ClientAction::NewThread => self.session_id = None,
                    ClientAction::TogglePalette => {
                        self.palette.toggle();
                        self.workspace.palette_open = self.palette.open;
                        if self.palette.open {
                            self.focus = Focus::Palette;
                        } else if self.focus == Focus::Palette {
                            self.focus = Focus::Composer;
                        }
                    }
                    ClientAction::ClosePalette => {
                        self.palette.close();
                        self.workspace.close_palette();
                        if self.focus == Focus::Palette {
                            self.focus = Focus::Composer;
                        }
                    }
                    ClientAction::ToggleHelp => {}
                    ClientAction::CycleModel | ClientAction::SelectModel => {
                        self.term_meta(&format!("model {}", self.workspace.model));
                    }
                    ClientAction::ToggleSettings => {
                        self.apply_theme();
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
            ClientAction::Send => self.send(),
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
            ClientAction::StartMcp | ClientAction::StopMcp => {
                let _ = apply_layout_action(&mut self.workspace, action);
            }
            other => {
                let _ = apply_layout_action(&mut self.workspace, other);
            }
        }
    }

    fn inspector_click(&mut self, action: InspectorAction, cx: &mut Context<Self>) {
        match action {
            InspectorAction::RefreshCores => self.refresh_cores(),
            InspectorAction::RefreshMcp => self.refresh_mcp(),
            InspectorAction::RefreshGit => self.refresh_worktrees(),
            InspectorAction::CreateCheckpoint => self.create_checkpoint(),
            InspectorAction::RevertCheckpoint => self.revert_checkpoint(),
            InspectorAction::CycleModel => {
                self.workspace.cycle_model();
                self.term_meta(&format!("model {}", self.workspace.model));
            }
            InspectorAction::CopySession => self.copy_session(cx),
            InspectorAction::RunGitStatus => self.run_shell("git status"),
            InspectorAction::NewWorktreeHint => self.create_worktree(),
            InspectorAction::StartMcp => {
                if let Some(id) = self.workspace.right_expanded_id.clone() {
                    if let Some(name) = id.strip_prefix("mcp:") {
                        if self.workspace.start_mcp(name) {
                            self.workspace.push_notice(
                                multiplexer_shell::NoticeKind::Good,
                                format!("{name} ready (supervised table, no child spawn)"),
                            );
                        }
                    }
                }
            }
            InspectorAction::StopMcp => {
                if let Some(id) = self.workspace.right_expanded_id.clone() {
                    if let Some(name) = id.strip_prefix("mcp:") {
                        if self.workspace.stop_mcp(name) {
                            self.workspace.push_notice(
                                multiplexer_shell::NoticeKind::Info,
                                format!("{name} stopped"),
                            );
                        }
                    }
                }
            }
            InspectorAction::ReloadDiffs => self.reload_diffs(),
            InspectorAction::SortDiffLastTurn => {
                let _ = self
                    .workspace
                    .set_diff_sort(multiplexer_shell::DiffSort::LastTurn);
            }
            InspectorAction::SortDiffFileName => {
                let _ = self
                    .workspace
                    .set_diff_sort(multiplexer_shell::DiffSort::FileName);
            }
            InspectorAction::OpenBrowser => self.open_browser(),
            InspectorAction::MentionFile => {
                if let Some(id) = self.workspace.right_expanded_id.clone() {
                    if let Some(path) = id.strip_prefix("file:") {
                        let _ = self.workspace.select_file(path);
                    }
                }
                if self.workspace.insert_file_mention() {
                    self.focus = Focus::Composer;
                    self.workspace.push_notice(
                        multiplexer_shell::NoticeKind::Info,
                        "mentioned file in composer",
                    );
                }
            }
        }
        cx.notify();
    }

    fn interrupt(&mut self) {
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
        self.workspace.cores = sample_cores(&(0..8).collect::<Vec<_>>())
            .into_iter()
            .map(|c| CoreRow {
                index: c.index,
                usage: c.usage,
                reserved: c.reserved || c.index < 2,
            })
            .collect();
        self.last_core_sample = Instant::now();
        self.term_meta("cores resampled");
    }

    fn refresh_mcp(&mut self) {
        self.workspace.mcp = load_user_mcp_inventory()
            .into_iter()
            .map(|row| McpRow {
                name: row.name,
                command: row.command,
                transport: row.transport,
                state: multiplexer_shell::McpLife::Stopped,
            })
            .collect();
        self.term_meta(&format!("mcp inventory {}", self.workspace.mcp.len()));
    }

    fn refresh_reminder(&mut self) {
        let frames = self.server.handle_frame(&rpc(
            "wt",
            methods::GIT_WORKTREES,
            json!({ "cwd": self.workspace.project }),
        ));
        let paths = worktree_paths(&frames);
        if let Some(path) = paths.into_iter().nth(1) {
            self.workspace.set_reminder("existing", path);
        }
    }

    fn refresh_worktrees(&mut self) {
        let frames = self.server.handle_frame(&rpc(
            "wt",
            methods::GIT_WORKTREES,
            json!({ "cwd": self.workspace.project }),
        ));
        self.workspace.worktrees = worktree_paths(&frames);
        self.term_meta(&format!("worktrees {}", self.workspace.worktrees.len()));
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
            self.workspace.create_local_checkpoint(
                format!("local-{}", self.workspace.checkpoints.len() + 1),
                "manual",
            );
            self.term_meta(&format!("checkpoint local ({err})"));
            return;
        }
        if let Some((id, label)) = checkpoint_from(&frames) {
            self.workspace.create_local_checkpoint(id.clone(), label);
            self.workspace.select_checkpoint(Some(id));
            self.term_meta("checkpoint created");
        }
    }

    fn revert_checkpoint(&mut self) {
        let Some(id) = self
            .workspace
            .selected_checkpoint
            .clone()
            .or_else(|| self.workspace.checkpoints.last().map(|c| c.id.clone()))
        else {
            self.term_meta("no checkpoint to revert");
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
        self.term_meta(&format!("reverted to {id}"));
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
        self.flash = Some("copied last message".into());
        self.term_meta("copied last message");
    }

    fn copy_session(&mut self, cx: &mut Context<Self>) {
        let text = self
            .session_id
            .clone()
            .unwrap_or_else(|| "(none yet)".into());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.flash = Some("copied session id".into());
        self.term_meta("copied session id");
    }

    fn cycle_file(&mut self) {
        if self.workspace.files.is_empty() {
            self.term_meta("no project files listed");
            return;
        }
        let first = self.workspace.files.remove(0);
        self.workspace.files.push(first);
        self.term_meta(&format!("file {}", self.workspace.files[0]));
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

    fn send(&mut self) {
        if self.pending_turn.is_some() || self.workspace.busy {
            return;
        }
        let raw = self.workspace.draft.trim().to_owned();
        if raw.is_empty() {
            return;
        }
        if let Some(cmd) = parse_slash(&raw) {
            self.workspace.draft.clear();
            self.workspace.cursor = 0;
            self.handle_slash(cmd);
            return;
        }
        let Some(text) = self.workspace.send_draft() else {
            return;
        };
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
        self.term_meta("grok -p running in background");
    }

    fn handle_slash(&mut self, cmd: SlashCommand) {
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
                self.palette.toggle();
                self.workspace.palette_open = self.palette.open;
                self.focus = if self.palette.open {
                    Focus::Palette
                } else {
                    Focus::Composer
                };
            }
            SlashCommand::Model => {
                self.workspace.cycle_model();
                self.term_meta(&format!("model {}", self.workspace.model));
            }
            SlashCommand::Unknown(name) => {
                self.term_meta(&format!(
                    "unknown /{name}  try /help /new /stop /cp /cores /mcp /git /term /skills"
                ));
            }
        }
    }

    fn run_terminal_draft(&mut self) {
        let Some(line) = self.workspace.take_term_draft() else {
            return;
        };
        self.run_shell(&line);
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
            self.workspace.cores = sample_cores(&[0, 1])
                .into_iter()
                .map(|c| CoreRow {
                    index: c.index,
                    usage: c.usage,
                    reserved: c.reserved,
                })
                .collect();
            self.last_core_sample = Instant::now();
        }
        if let Some(rx) = &self.pending_turn {
            match rx.try_recv() {
                Ok(out) => {
                    self.pending_turn = None;
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
                        .push_notice(NoticeKind::Info, "Grok TUI exited");
                }
                Ok(None) => {}
                Err(_) => {
                    self.grok_tui = None;
                    self.workspace.grok_tui.mark_exited();
                }
            }
        }
        if self.pending_turn.is_some()
            || self.pending_cmd.is_some()
            || self.workspace.grok_tui.life == TuiLife::Running
        {
            window.request_animation_frame();
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        if key == "escape" {
            if self.palette.open {
                self.dispatch(ClientAction::ClosePalette, cx);
                return;
            }
            if self.workspace.help_open {
                self.workspace.toggle_help();
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
        if (key == "k" || key == "p") && mods.control {
            self.dispatch(ClientAction::TogglePalette, cx);
            return;
        }
        if key == "n" && mods.control {
            self.dispatch(ClientAction::NewThread, cx);
            return;
        }
        if key == "f1" {
            self.dispatch(ClientAction::ToggleHelp, cx);
            return;
        }
        if key == "f2" {
            self.dispatch(ClientAction::ToggleSettings, cx);
            return;
        }
        if key == "g" && mods.control && mods.shift {
            self.dispatch(ClientAction::ToggleCenterMode, cx);
            return;
        }
        if key == "l" && mods.control && mods.shift {
            self.workspace.reset_outlook_chrome();
            cx.notify();
            return;
        }
        if key == "[" && mods.control {
            self.dispatch(ClientAction::ToggleLeft, cx);
            return;
        }
        if key == "]" && mods.control {
            self.dispatch(ClientAction::ToggleRight, cx);
            return;
        }
        if key == "." && mods.control {
            self.interrupt();
            cx.notify();
            return;
        }
        if key == "s" && mods.control {
            self.create_checkpoint();
            cx.notify();
            return;
        }
        if (key == "`" || key == "oem_3") && mods.control {
            self.dispatch(ClientAction::ToggleBottom, cx);
            return;
        }
        if key == "v" && mods.control {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    self.insert_text(&text);
                }
            }
            cx.notify();
            return;
        }
        if self.palette.open || self.focus == Focus::Palette {
            self.palette_key(key, mods.control, cx);
            return;
        }
        if self.focus == Focus::Terminal {
            self.terminal_key(key, mods.control, cx);
            return;
        }
        if key == "enter" {
            if mods.shift {
                self.insert_text("\n");
            } else {
                self.send();
            }
        } else if key == "backspace" {
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
                        this.workspace
                            .set_bottom_height(win_h - f32::from(event.position.y));
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
            .child(self.notice_bar(cx))
            .child(self.reminder_bar(cx))
            .child(self.approval_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .p_2()
                    .gap_1()
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
        if self.workspace.settings_open {
            root = root.child(self.settings_overlay(cx));
        }
        root
    }
}

impl ShellView {
    fn title_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let left_on = self.workspace.chrome.left_open;
        let right_on = self.workspace.chrome.right_open;
        glass_bar()
            .h(px(44.0))
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
            .child(pill(
                short_path(&self.workspace.project),
                ChromeGlyph::Folder.mark(),
            ))
            .child(pill(self.workspace.branch_label(), ChromeGlyph::Git.mark()))
            .child(
                div()
                    .id("model-pill")
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.dispatch(ClientAction::CycleModel, cx);
                        }),
                    )
                    .child(pill(
                        self.workspace.model.clone(),
                        ChromeGlyph::Sparkle.mark(),
                    )),
            )
            .child(div().flex_1())
            .child(pill(
                format!("{} turns", self.workspace.usage_turns),
                ChromeGlyph::Activity.mark(),
            ))
            .child(pill(
                if self.remotes.iter().any(|r| r.kind == "tailscale") {
                    "local+ts"
                } else {
                    "local"
                },
                ChromeGlyph::Plug.mark(),
            ))
            .child(if self.workspace.busy {
                div().child(icon_btn(
                    ChromeGlyph::Stop.mark(),
                    "Stop",
                    cx,
                    |this, cx| {
                        this.interrupt();
                        cx.notify();
                    },
                ))
            } else {
                div().child(icon_btn(ChromeGlyph::Play.mark(), "Run", cx, |this, cx| {
                    this.send();
                    cx.notify();
                }))
            })
            .child(icon_btn(
                ChromeGlyph::Palette.mark(),
                "Palette",
                cx,
                |this, cx| {
                    this.dispatch(ClientAction::TogglePalette, cx);
                },
            ))
            .child(icon_btn("?", "Help", cx, |this, cx| {
                this.dispatch(ClientAction::ToggleHelp, cx);
            }))
            .child(icon_btn(
                ChromeGlyph::Terminal.mark(),
                "Terminal",
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleBottom, cx),
            ))
            .child(icon_btn(
                ChromeGlyph::Settings.mark(),
                "Settings",
                cx,
                |this, cx| this.dispatch(ClientAction::ToggleSettings, cx),
            ))
            .child(icon_btn("↺", "Reset layout", cx, |this, cx| {
                this.workspace.reset_outlook_chrome();
                cx.notify();
            }))
            .child(icon_btn(
                "▣",
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
        let open = self.workspace.chrome.left_open;
        let w = self.workspace.chrome.occupied_left();
        let section = self.workspace.left_section;
        let selected = self.workspace.selected;
        let threads = self.workspace.threads.clone();
        let files = self.workspace.files_visible();
        let activity = self.workspace.terminal_log.clone();
        let sessions = match &self.workspace.connection {
            multiplexer_shell::ConnectionState::Connected { session_ids } => session_ids.clone(),
            _ => Vec::new(),
        };
        let icons = div()
            .w(px(44.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .children(LeftSection::all().into_iter().map(|s| {
                let on = section == s;
                div()
                    .id(SharedString::from(s.rail_label()))
                    .h(px(36.0))
                    .mx_1()
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if on {
                        Theme::selection()
                    } else {
                        hsla(0.0, 0.0, 1.0, 0.0)
                    })
                    .text_color(if on { Theme::accent() } else { Theme::muted() })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.dispatch(ClientAction::SelectLeftSection(s), cx);
                        }),
                    )
                    .child(s.glyph())
            }));
        let rail = glass_pane().w(px(w)).h_full().flex().overflow_hidden();
        if !open {
            return rail.child(icons);
        }
        let list = div()
            .flex_1()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(Theme::faint())
                            .child(section.label().to_ascii_uppercase()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(if section == LeftSection::Threads {
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(icon_btn(
                                        ChromeGlyph::Plus.mark(),
                                        "New",
                                        cx,
                                        |this, cx| {
                                            this.dispatch(ClientAction::NewThread, cx);
                                        },
                                    ))
                                    .child(icon_btn("⌫", "Delete", cx, |this, cx| {
                                        this.dispatch(ClientAction::DeleteThread, cx);
                                    }))
                            } else {
                                div()
                            })
                            .child(icon_btn(
                                ChromeGlyph::Close.mark(),
                                "Hide left",
                                cx,
                                |this, cx| {
                                    this.workspace.chrome.hide_left();
                                    cx.notify();
                                },
                            )),
                    ),
            );
        let items: Vec<gpui::AnyElement> = match section {
            LeftSection::Threads => threads
                .into_iter()
                .enumerate()
                .map(|(i, t)| {
                    list_row(
                        format!("thr-{i}"),
                        ChromeGlyph::Chat.mark(),
                        t.title.clone(),
                        Workspace::thread_preview(&t),
                        format!("{} · {}", t.status, t.id),
                        i == selected,
                        false,
                        cx,
                        move |this, cx| this.dispatch(ClientAction::SelectThread(i), cx),
                    )
                })
                .collect(),
            LeftSection::Agents => {
                if sessions.is_empty() {
                    vec![list_row(
                        "agent-none",
                        ChromeGlyph::Agent.mark(),
                        "No live session",
                        "Send a turn to start",
                        "",
                        false,
                        false,
                        cx,
                        |this, cx| {
                            this.term_meta("start a session from the composer");
                            cx.notify();
                        },
                    )]
                } else {
                    sessions
                        .into_iter()
                        .map(|id| {
                            let shown = id.clone();
                            list_row(
                                format!("agent-{id}"),
                                ChromeGlyph::Agent.mark(),
                                shown,
                                "connected",
                                "",
                                true,
                                self.workspace.busy,
                                cx,
                                |this, cx| {
                                    this.term_meta("session selected");
                                    cx.notify();
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
                            let title = p.clone();
                            list_row(
                                format!("file-{p}"),
                                ChromeGlyph::Folder.mark(),
                                title,
                                "",
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
                .rev()
                .take(20)
                .enumerate()
                .map(|(i, line)| {
                    list_row(
                        format!("act-{i}"),
                        ChromeGlyph::Activity.mark(),
                        line,
                        "",
                        "",
                        false,
                        false,
                        cx,
                        |this, cx| {
                            this.focus = Focus::Terminal;
                            cx.notify();
                        },
                    )
                })
                .collect(),
        };
        rail.child(icons).child(
            list.child(
                div()
                    .id("left-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(items),
            ),
        )
    }

    fn right_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.workspace.chrome.right_open;
        let w = self.workspace.chrome.occupied_right();
        let tab = self.workspace.inspector;
        let buttons = tab_buttons(tab);
        let rows = inspector_rows(&self.workspace);
        let icons = div()
            .id("right-icons")
            .w(px(44.0))
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
                    .mx_1()
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if on {
                        Theme::selection()
                    } else {
                        hsla(0., 0., 1., 0.)
                    })
                    .text_color(if on { Theme::accent() } else { Theme::muted() })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if this.workspace.chrome.right_open && this.workspace.inspector == t {
                                this.workspace.chrome.hide_right();
                            } else {
                                this.dispatch(ClientAction::SelectTab(t), cx);
                                this.workspace.chrome.right_open = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(t.glyph())
            }));
        let rail = glass_pane().w(px(w)).h_full().flex().overflow_hidden();
        if !open {
            return rail.child(icons);
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
                            this.workspace.chrome.hide_right();
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .px_2()
                    .flex()
                    .gap_1()
                    .flex_wrap()
                    .children(buttons.into_iter().map(|b| {
                        let action = b.action;
                        ghost_btn(b.label, b.hint, cx, move |this, cx| {
                            this.inspector_click(action, cx);
                        })
                    })),
            )
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
        rail.child(icons).child(body)
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
            .child(self.center_mode_bar(cx))
            .child(if tui {
                self.grok_tui_host(cx)
            } else {
                div()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(match thread {
                        Some(t) if t.messages.is_empty() => vec![empty_center()],
                        Some(t) => t
                            .messages
                            .into_iter()
                            .map(|m| {
                                let user = m.role == Role::User;
                                let row = if user {
                                    div().flex().justify_end()
                                } else {
                                    div().flex().justify_start()
                                };
                                row.child(
                                    div()
                                        .max_w(px(640.0))
                                        .px_3()
                                        .py_2()
                                        .rounded_xl()
                                        .bg(if user {
                                            hsla(0.58, 0.45, 0.28, 0.55)
                                        } else {
                                            hsla(0.0, 0.0, 1.0, 0.06)
                                        })
                                        .border_1()
                                        .border_color(Theme::hairline())
                                        .shadow(Theme::shadow())
                                        .child(
                                            div()
                                                .text_color(if user {
                                                    Theme::accent()
                                                } else {
                                                    Theme::good()
                                                })
                                                .child(if user { "You" } else { "Agent" }),
                                        )
                                        .child(div().child(m.text)),
                                )
                            })
                            .collect(),
                        None => vec![empty_center()],
                    })
                    .into_any()
            })
            .child(if tui {
                div().id("no-composer").into_any()
            } else {
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(Theme::hairline())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .flex_wrap()
                            .child(chip("What can you do?", cx, |this, cx| {
                                this.workspace.set_draft("What can you do?");
                                this.send();
                                cx.notify();
                            }))
                            .child(chip("Summarize this repo", cx, |this, cx| {
                                this.workspace.set_draft("Summarize this repo");
                                this.send();
                                cx.notify();
                            }))
                            .child(chip("git status", cx, |this, cx| {
                                this.run_shell("git status");
                                cx.notify();
                            }))
                            .child(chip("Run the tests", cx, |this, cx| {
                                this.run_shell("cargo test --workspace --offline");
                                cx.notify();
                            }))
                            .child(chip("Copy last", cx, |this, cx| {
                                this.copy_last_message(cx);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("composer")
                                    .flex_1()
                                    .px_3()
                                    .py_2()
                                    .rounded_xl()
                                    .bg(Theme::surface())
                                    .border_1()
                                    .border_color(if self.focus == Focus::Composer {
                                        Theme::accent()
                                    } else {
                                        Theme::hairline_bright()
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.focus = Focus::Composer;
                                            cx.notify();
                                        }),
                                    )
                                    .child(if self.workspace.draft.is_empty() {
                                        SharedString::from(
                                            "Message Grok…  Enter send  Shift+Enter newline  /help",
                                        )
                                    } else {
                                        SharedString::from(draft)
                                    }),
                            )
                            .child(
                                div()
                                    .id("send-circle")
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(
                                        if self.workspace.draft.trim().is_empty()
                                            || self.workspace.busy
                                        {
                                            Theme::send_bg()
                                        } else {
                                            Theme::accent()
                                        },
                                    )
                                    .text_color(Theme::text())
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.send();
                                            cx.notify();
                                        }),
                                    )
                                    .child(ChromeGlyph::Play.mark()),
                            ),
                    )
                    .child(if let Some(cmd) = slash {
                        div()
                            .text_color(Theme::accent())
                            .child(format!("/  {}", multiplexer_shell::slash_hint(&cmd)))
                    } else {
                        div().child("")
                    })
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
            .px_3()
            .rounded_lg()
            .flex()
            .items_center()
            .cursor_pointer()
            .bg(if on {
                Theme::selection()
            } else {
                Theme::glass_ultra()
            })
            .border_1()
            .border_color(if on {
                Theme::hairline_bright()
            } else {
                Theme::hairline()
            })
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
                    .text_color(Theme::accent())
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
            .bg(hsla(0.12, 0.45, 0.22, 0.55))
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
            .bg(hsla(0.08, 0.55, 0.22, 0.70))
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
            .into_any()
    }

    fn notice_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.workspace.notices.is_empty() {
            return div().id("no-notices").into_any();
        }
        let notices = self.workspace.notices.clone();
        div()
            .id("notice-stack")
            .flex()
            .flex_col()
            .children(notices.into_iter().map(|n| {
                let id = n.id;
                div()
                    .id(SharedString::from(format!("notice-{id}")))
                    .px_3()
                    .py_1()
                    .bg(match n.kind {
                        NoticeKind::Good => hsla(0.38, 0.45, 0.22, 0.70),
                        NoticeKind::Warn => hsla(0.12, 0.55, 0.24, 0.70),
                        NoticeKind::Danger => hsla(0.02, 0.60, 0.24, 0.75),
                        NoticeKind::Info => Theme::accent_muted(),
                    })
                    .border_b_1()
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
        let wt = format!(
            "{}  {}  create={}",
            self.workspace.wt_path, self.workspace.wt_branch, self.workspace.wt_create_branch
        );
        div()
            .id("settings")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(56.0))
            .bg(hsla(0.64, 0.20, 0.04, 0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.workspace.settings_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(520.0))
                    .rounded_xl()
                    .bg(Theme::glass_strong())
                    .border_1()
                    .border_color(Theme::hairline_bright())
                    .shadow(Theme::shadow())
                    .p_4()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .text_size(Theme::text_body())
                            .text_color(Theme::accent())
                            .child("Settings"),
                    )
                    .child(div().text_color(Theme::muted()).mt_2().child(format!(
                        "Theme {mode}   Density {density}   Default {model}"
                    )))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .gap_2()
                            .flex_wrap()
                            .child(ghost_btn("Theme", "cycle", cx, |this, cx| {
                                this.workspace.settings.cycle_mode();
                                this.apply_theme();
                                cx.notify();
                            }))
                            .child(ghost_btn("Density", "cycle", cx, |this, cx| {
                                this.workspace.settings.cycle_density();
                                this.apply_theme();
                                cx.notify();
                            }))
                            .child(ghost_btn("Use model", "apply", cx, |this, cx| {
                                this.dispatch(ClientAction::SelectModel, cx);
                            }))
                            .child(ghost_btn("New WT", "git", cx, |this, cx| {
                                this.create_worktree();
                                cx.notify();
                            }))
                            .child(ghost_btn("Create branch", "toggle", cx, |this, cx| {
                                this.workspace.wt_create_branch = !this.workspace.wt_create_branch;
                                cx.notify();
                            }))
                            .child(ghost_btn("Close", "F2", cx, |this, cx| {
                                this.workspace.settings_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_color(Theme::faint())
                            .child("Models (click to select)"),
                    )
                    .children(models.into_iter().map(|m| {
                        let shown = m.clone();
                        let pick = m.clone();
                        div()
                            .id(SharedString::from(format!("set-model-{shown}")))
                            .mt_1()
                            .px_2()
                            .py_1()
                            .rounded_lg()
                            .bg(if shown == self.workspace.model {
                                Theme::selection()
                            } else {
                                Theme::glass_ultra()
                            })
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.workspace.settings.set_default_model(pick.clone());
                                    let _ = this.workspace.select_model(pick.clone());
                                    let frames = this.server.handle_frame(&rpc(
                                        "ms",
                                        methods::MODEL_SELECT,
                                        json!({ "model": pick }),
                                    ));
                                    if let Some(err) = first_error(&frames) {
                                        this.workspace.push_notice(NoticeKind::Warn, err);
                                    } else {
                                        this.workspace
                                            .push_notice(NoticeKind::Good, format!("model {pick}"));
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(shown)
                    }))
                    .child(div().mt_3().text_color(Theme::faint()).child(format!(
                        "Usage  {turns} turns  {tokens} tok (local snapshot)"
                    )))
                    .child(
                        div()
                            .mt_2()
                            .text_color(Theme::faint())
                            .child(format!("Worktree draft  {wt}")),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_color(Theme::faint())
                            .child("Remotes (detect only, no Tailscale Serve)"),
                    )
                    .children(remotes.into_iter().map(|r| {
                        div()
                            .id(SharedString::from(format!("remote-{}", r.id)))
                            .mt_1()
                            .px_2()
                            .py_1()
                            .rounded_lg()
                            .bg(Theme::glass_ultra())
                            .child(format!("{}  ·  {}  ·  {}", r.label, r.kind, r.id))
                    })),
            )
    }

    fn terminal_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(ghost_btn(
                        if open { "Hide" } else { "Show" },
                        "ctrl-`",
                        cx,
                        |this, cx| {
                            this.dispatch(ClientAction::ToggleBottom, cx);
                        },
                    )),
            );
        if !open {
            return bar;
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
                        .px_2()
                        .py_1()
                        .rounded_lg()
                        .bg(hsla(0.0, 0.0, 1.0, 0.05))
                        .border_1()
                        .border_color(if self.focus == Focus::Terminal {
                            Theme::accent()
                        } else {
                            Theme::hairline()
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
                })),
        )
    }

    fn status_bar(&self) -> impl IntoElement {
        let s = status_from(&self.workspace, self.session_id.clone());
        let extra = self.flash.clone().unwrap_or_default();
        let ready = self
            .workspace
            .mcp
            .iter()
            .filter(|m| m.state == multiplexer_shell::McpLife::Ready)
            .count();
        let cores = self.workspace.cores.len();
        let remotes = self
            .remotes
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let live = format!(
            "{}   ·   {} turns  {} tok  mcp {ready}/{}  cpu {cores}  remotes {remotes}",
            status_line(&s),
            self.workspace.usage_turns,
            self.workspace.usage_tokens,
            self.workspace.mcp.len(),
        );
        glass_bar()
            .h(px(28.0))
            .px_3()
            .rounded_none()
            .border_t_1()
            .child(
                div()
                    .flex_1()
                    .text_color(Theme::muted())
                    .child(if extra.is_empty() {
                        live
                    } else {
                        format!("{live}   ·   {extra}")
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
            .bg(hsla(0.64, 0.20, 0.04, 0.45))
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
                    .shadow(Theme::shadow())
                    .p_3()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(
                        div()
                            .px_2()
                            .py_2()
                            .mb_2()
                            .rounded_lg()
                            .bg(hsla(0.0, 0.0, 1.0, 0.06))
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
                        };
                        div()
                            .id(SharedString::from(format!("pal-{i}")))
                            .px_2()
                            .py_2()
                            .rounded_lg()
                            .bg(if i == selected {
                                hsla(0.58, 0.40, 0.28, 0.50)
                            } else {
                                hsla(0.0, 0.0, 0.0, 0.0)
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
            .bg(hsla(0.64, 0.20, 0.04, 0.45))
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
                    .shadow(Theme::shadow())
                    .p_4()
                    .child("Keyboard")
                    .child(div().text_color(Theme::muted()).mt_2().child(
                        "Enter send   Shift+Enter newline   Ctrl+K palette   F1 help   F2 settings\nCtrl+Shift+G Grok TUI / chat log   Ctrl+N new chat   Ctrl+[ / ] rails   Ctrl+` terminal\nCtrl+. stop   Ctrl+S checkpoint   Ctrl+Shift+L reset layout\nSlash: /new /stop /help /cp /cores /mcp /git /term /skills /model\nRight rail Diffs sorts by last turn or file name. Browser opens the system browser.\nGrok TUI is the real pager in a console. Chat log is grok -p only.",
                    )),
            )
    }

    fn resize_handle(&mut self, rail: DragRail, cx: &mut Context<Self>) -> impl IntoElement {
        let open = match rail {
            DragRail::Left => self.workspace.chrome.left_open,
            DragRail::Right => self.workspace.chrome.right_open,
            DragRail::Bottom => true,
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
            .hover(|s| s.bg(hsla(0.58, 0.50, 0.55, 0.28)))
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
        div()
            .id("resize-bottom")
            .w_full()
            .h(px(8.0))
            .cursor(CursorStyle::ResizeUpDown)
            .hover(|s| s.bg(hsla(0.58, 0.50, 0.55, 0.35)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = Some(DragRail::Bottom);
                    cx.notify();
                }),
            )
    }
}

fn glass_pane() -> gpui::Div {
    div()
        .rounded(Theme::panel_radius())
        .bg(Theme::glass())
        .border_1()
        .border_color(Theme::hairline())
        .shadow(Theme::shadow())
        .overflow_hidden()
        .min_h_0()
}

fn glass_bar() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .bg(Theme::glass_strong())
        .border_color(Theme::hairline())
}

fn empty_center() -> gpui::Div {
    let spec = EmptyStateSpec::chat();
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .text_color(Theme::muted())
        .child(
            div()
                .text_size(Theme::text_body())
                .text_color(Theme::accent())
                .child(format!("{}  {}", ChromeGlyph::Sparkle.mark(), spec.title)),
        )
        .child(div().child(spec.body))
        .child(
            div()
                .text_size(Theme::text_caption())
                .text_color(Theme::faint())
                .child(empty_state_tiles().join("   ·   ")),
        )
        .child(
            div()
                .text_color(Theme::faint())
                .child("Ctrl+Shift+G Grok TUI   Ctrl+K palette   F2 settings"),
        )
}

fn chip(
    label: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .px_2()
        .py_1()
        .rounded_lg()
        .bg(hsla(0.0, 0.0, 1.0, 0.06))
        .border_1()
        .border_color(Theme::hairline())
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(label)
}

fn icon_btn(
    mark: &'static str,
    hint: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("icon-{hint}")))
        .w(Theme::icon_size())
        .h(Theme::icon_size())
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(Theme::hairline())
        .bg(Theme::glass_ultra())
        .text_color(Theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(mark)
}

fn pill(text: impl Into<String>, mark: &'static str) -> impl IntoElement {
    div()
        .h(px(28.0))
        .px_2()
        .rounded_lg()
        .flex()
        .items_center()
        .gap_1()
        .bg(Theme::glass_ultra())
        .border_1()
        .border_color(Theme::hairline())
        .text_color(Theme::muted())
        .child(mark)
        .child(text.into())
}

#[allow(clippy::too_many_arguments)]
fn list_row(
    id: impl Into<String>,
    icon: &'static str,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    meta: impl Into<String>,
    selected: bool,
    busy: bool,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    let title = title.into();
    let _subtitle = subtitle.into();
    let _meta = meta.into();
    let id = id.into();
    div()
        .id(SharedString::from(id))
        .mx_2()
        .mb_1()
        .h(px(36.0))
        .px_2()
        .overflow_hidden()
        .rounded_lg()
        .bg(if selected {
            Theme::selection()
        } else {
            Theme::glass_ultra()
        })
        .border_1()
        .border_color(if selected {
            Theme::hairline_bright()
        } else {
            Theme::hairline()
        })
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .overflow_hidden()
                .child(div().text_color(Theme::accent()).child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .text_color(Theme::faint())
                        .child(if busy { "…" } else { "" }),
                ),
        )
        .into_any()
}

fn inspector_row_el(
    row: ListRowSpec,
    detail: String,
    cx: &mut Context<ShellView>,
) -> impl IntoElement {
    let id = row.id.clone();
    let click_id = id.clone();
    let title = row.title.clone();
    let subtitle = row.subtitle.clone();
    let meta = row.meta.clone();
    let icon = row.icon.clone();
    let selected = row.selected || row.expanded;
    let expanded = row.expanded;
    let badge = row.badge.clone();
    let body = if !detail.is_empty() { detail } else { meta };
    div()
        .id(SharedString::from(id.clone()))
        .mx_2()
        .mb_1()
        .min_h(Theme::row_height())
        .px_2()
        .py_1()
        .overflow_hidden()
        .rounded_lg()
        .bg(if selected {
            Theme::selection()
        } else {
            Theme::glass_ultra()
        })
        .border_1()
        .border_color(if selected {
            Theme::hairline_bright()
        } else {
            Theme::hairline()
        })
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.activate_inspector_row(&click_id);
                cx.notify();
            }),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(div().text_color(Theme::accent()).child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(if let Some(b) = badge {
                    let tone_bg = match b.tone {
                        multiplexer_shell::Tone::Warn => Theme::warn(),
                        multiplexer_shell::Tone::Danger => Theme::danger(),
                        multiplexer_shell::Tone::Good => Theme::good(),
                        _ => Theme::accent_muted(),
                    };
                    div()
                        .px_1()
                        .rounded_md()
                        .bg(tone_bg)
                        .text_color(Theme::text())
                        .child(b.text)
                } else {
                    div()
                }),
        )
        .child(if subtitle.is_empty() {
            div()
        } else {
            div().text_color(Theme::muted()).child(subtitle)
        })
        .child(if expanded && !body.is_empty() {
            div()
                .mt_1()
                .px_2()
                .py_1()
                .rounded_lg()
                .bg(hsla(0.0, 0.0, 1.0, 0.04))
                .text_color(Theme::faint())
                .child(body)
        } else {
            div()
        })
}

fn ghost_btn(
    label: &'static str,
    hint: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("{label}-{hint}")))
        .h(px(32.0))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_lg()
        .border_1()
        .border_color(Theme::hairline_bright())
        .bg(if label == "Stop" {
            Theme::danger()
        } else if label == "Send" {
            Theme::send_bg()
        } else {
            hsla(0.0, 0.0, 1.0, 0.07)
        })
        .text_color(Theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0.58, 0.35, 0.28, 0.40)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .overflow_hidden()
        .child(label)
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

fn worktree_paths(frames: &[String]) -> Vec<String> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            if let Some(arr) = r.result.get("worktrees").and_then(Value::as_array) {
                return arr
                    .iter()
                    .filter_map(|row| row.get("path").and_then(Value::as_str))
                    .map(str::to_owned)
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

fn checkpoint_from(frames: &[String]) -> Option<(String, String)> {
    for f in frames {
        if let Ok(Message::Response(r)) = decode_frame(f) {
            let id = r.result.get("id").and_then(Value::as_str)?;
            let label = r
                .result
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("manual");
            return Some((id.to_owned(), label.to_owned()));
        }
    }
    None
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1360.0), px(860.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Blurred,
                is_movable: true,
                is_resizable: true,
                is_minimizable: true,
                window_min_size: Some(size(px(920.0), px(620.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Multiplexer".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ShellView::new()),
        )
        .expect("open Multiplexer window");
        cx.activate(true);
    });
}
