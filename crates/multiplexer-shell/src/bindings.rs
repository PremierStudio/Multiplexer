//! Map [`crate::ClientAction`] plus session context to a host call.
//!
//! The desktop must not inline JSON-RPC method strings for inspector or host
//! actions. This module is the single map.

use crate::ClientAction;

/// Session-scoped values the desktop supplies when mapping an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionContext {
    pub session_id: Option<String>,
    pub project: String,
    pub checkpoint_id: Option<String>,
    pub approval_request_id: Option<String>,
    pub model: String,
    pub wt_path: String,
    pub wt_branch: String,
    pub wt_create_branch: bool,
}

/// Where a [`ClientAction`] should be executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCall {
    Rpc {
        method: &'static str,
        params_json: String,
    },
    /// Chrome-only, already handled by [`crate::apply_layout_action`].
    Local,
    /// Desktop does I/O itself (cores sample, grok turn, shell cmd).
    NeedsHost,
}

/// Map `action` to a host RPC, local chrome mutation, or desktop I/O.
pub fn host_call(action: crate::ClientAction, ctx: &ActionContext) -> HostCall {
    match action {
        ClientAction::NewThread
        | ClientAction::SelectThread(_)
        | ClientAction::ToggleLeft
        | ClientAction::ToggleRight
        | ClientAction::SelectTab(_)
        | ClientAction::DismissReminder
        | ClientAction::TogglePalette
        | ClientAction::ClosePalette
        | ClientAction::ToggleHelp
        | ClientAction::DeleteThread
        | ClientAction::CycleModel
        | ClientAction::SelectLeftSection(_)
        | ClientAction::ToggleBottom
        | ClientAction::InsertFileMention
        | ClientAction::ToggleSettings
        | ClientAction::SelectModel
        | ClientAction::ToggleCenterMode
        | ClientAction::SetCenterGui
        | ClientAction::SetCenterTui
        | ClientAction::SortDiffLastTurn
        | ClientAction::SortDiffFileName
        | ClientAction::HideLeft
        | ClientAction::HideRight
        | ClientAction::HideBottom
        | ClientAction::FocusLayout
        | ClientAction::OpenProjectFiles
        | ClientAction::OpenGitTab
        | ClientAction::OpenSessionTab
        | ClientAction::OpenSettingsRemotes => HostCall::Local,
        ClientAction::CreateWorktree => worktree_create_call(ctx),
        ClientAction::Send
        | ClientAction::RefreshCores
        | ClientAction::RunTerminal
        | ClientAction::CycleFile
        | ClientAction::CopyLastMessage
        | ClientAction::RefreshMcp
        | ClientAction::StartMcp
        | ClientAction::StopMcp
        | ClientAction::LaunchGrokTui
        | ClientAction::StopGrokTui
        | ClientAction::OpenBrowser => HostCall::NeedsHost,
        ClientAction::Interrupt => match ctx.session_id.as_deref() {
            Some(session_id) => HostCall::Rpc {
                method: "session.interrupt",
                params_json: format!(r#"{{"session_id":"{session_id}"}}"#),
            },
            None => HostCall::NeedsHost,
        },
        ClientAction::CreateCheckpoint => match ctx.session_id.as_deref() {
            Some(session_id) => HostCall::Rpc {
                method: "checkpoint.create",
                params_json: format!(r#"{{"session_id":"{session_id}","label":"manual"}}"#),
            },
            None => HostCall::NeedsHost,
        },
        ClientAction::RestoreCheckpoint => match ctx.checkpoint_id.as_deref() {
            Some(checkpoint_id) => HostCall::Rpc {
                method: "checkpoint.revert",
                params_json: format!(r#"{{"checkpoint_id":"{checkpoint_id}"}}"#),
            },
            None => HostCall::NeedsHost,
        },
        ClientAction::RefreshGit => HostCall::Rpc {
            method: "git.worktrees",
            params_json: format!(r#"{{"cwd":"{}"}}"#, ctx.project),
        },
        ClientAction::Approve => approval_call(ctx, "allow"),
        ClientAction::Deny => approval_call(ctx, "deny"),
    }
}

/// Draft path/branch to `git.worktree.create`. Empty path or branch is NeedsHost.
pub fn worktree_create_call(ctx: &ActionContext) -> HostCall {
    if ctx.wt_path.trim().is_empty() || ctx.wt_branch.trim().is_empty() {
        return HostCall::NeedsHost;
    }
    HostCall::Rpc {
        method: "git.worktree.create",
        params_json: format!(
            r#"{{"cwd":"{}","path":"{}","branch":"{}","create_branch":{}}}"#,
            ctx.project, ctx.wt_path, ctx.wt_branch, ctx.wt_create_branch
        ),
    }
}

fn approval_call(ctx: &ActionContext, decision: &'static str) -> HostCall {
    match (
        ctx.session_id.as_deref(),
        ctx.approval_request_id.as_deref(),
    ) {
        (Some(session_id), Some(request_id)) => HostCall::Rpc {
            method: "approval.respond",
            params_json: format!(
                r#"{{"session_id":"{session_id}","request_id":"{request_id}","decision":"{decision}"}}"#
            ),
        },
        _ => HostCall::NeedsHost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InspectorTab;

    fn bare() -> ActionContext {
        ActionContext {
            session_id: None,
            project: "C:/repo".to_owned(),
            checkpoint_id: None,
            approval_request_id: None,
            model: "grok".to_owned(),
            wt_path: String::new(),
            wt_branch: String::new(),
            wt_create_branch: false,
        }
    }

    fn with_session(session_id: &str) -> ActionContext {
        ActionContext {
            session_id: Some(session_id.to_owned()),
            ..bare()
        }
    }

    fn assert_rpc(call: HostCall, method: &str, params_json: &str) {
        match call {
            HostCall::Rpc {
                method: got_method,
                params_json: got_params,
            } => {
                assert_eq!(got_method, method);
                assert_eq!(got_params, params_json);
                assert_ne!(got_method, "");
                assert_ne!(got_params, "{}");
            }
            other => panic!(
                "expected Rpc {{ method: {method}, params_json: {params_json} }}, got {other:?}"
            ),
        }
    }

    #[test]
    fn interrupt_without_session_is_needs_host() {
        assert_eq!(
            host_call(ClientAction::Interrupt, &bare()),
            HostCall::NeedsHost
        );
        assert_ne!(host_call(ClientAction::Interrupt, &bare()), HostCall::Local);
    }

    #[test]
    fn interrupt_json() {
        assert_rpc(
            host_call(ClientAction::Interrupt, &with_session("sess-9")),
            "session.interrupt",
            r#"{"session_id":"sess-9"}"#,
        );
        assert_ne!(
            host_call(ClientAction::Interrupt, &with_session("sess-9")),
            host_call(ClientAction::CreateCheckpoint, &with_session("sess-9"))
        );
    }

    #[test]
    fn create_checkpoint_json() {
        assert_rpc(
            host_call(ClientAction::CreateCheckpoint, &with_session("sess-1")),
            "checkpoint.create",
            r#"{"session_id":"sess-1","label":"manual"}"#,
        );
        assert_ne!(
            host_call(ClientAction::CreateCheckpoint, &with_session("sess-1")),
            HostCall::Rpc {
                method: "checkpoint.revert",
                params_json: r#"{"checkpoint_id":"sess-1"}"#.to_owned(),
            }
        );
        assert_eq!(
            host_call(ClientAction::CreateCheckpoint, &bare()),
            HostCall::NeedsHost
        );
    }

    #[test]
    fn revert_json() {
        let mut ctx = with_session("sess-1");
        ctx.checkpoint_id = Some("cp-1".to_owned());
        assert_rpc(
            host_call(ClientAction::RestoreCheckpoint, &ctx),
            "checkpoint.revert",
            r#"{"checkpoint_id":"cp-1"}"#,
        );
        assert_ne!(
            host_call(ClientAction::RestoreCheckpoint, &ctx),
            host_call(ClientAction::CreateCheckpoint, &ctx)
        );
        ctx.checkpoint_id = None;
        assert_eq!(
            host_call(ClientAction::RestoreCheckpoint, &ctx),
            HostCall::NeedsHost
        );
    }

    #[test]
    fn approve_json() {
        let mut ctx = with_session("sess-1");
        ctx.approval_request_id = Some("req-1".to_owned());
        assert_rpc(
            host_call(ClientAction::Approve, &ctx),
            "approval.respond",
            r#"{"session_id":"sess-1","request_id":"req-1","decision":"allow"}"#,
        );
        assert_rpc(
            host_call(ClientAction::Deny, &ctx),
            "approval.respond",
            r#"{"session_id":"sess-1","request_id":"req-1","decision":"deny"}"#,
        );
        assert_ne!(
            host_call(ClientAction::Approve, &ctx),
            host_call(ClientAction::Deny, &ctx)
        );
        ctx.approval_request_id = None;
        assert_eq!(host_call(ClientAction::Approve, &ctx), HostCall::NeedsHost);
        assert_eq!(host_call(ClientAction::Deny, &ctx), HostCall::NeedsHost);
        ctx.approval_request_id = Some("req-1".to_owned());
        ctx.session_id = None;
        assert_eq!(host_call(ClientAction::Approve, &ctx), HostCall::NeedsHost);
        assert_eq!(host_call(ClientAction::Deny, &ctx), HostCall::NeedsHost);
    }

    #[test]
    fn refresh_git_json() {
        assert_rpc(
            host_call(ClientAction::RefreshGit, &bare()),
            "git.worktrees",
            r#"{"cwd":"C:/repo"}"#,
        );
        assert_ne!(
            host_call(ClientAction::RefreshGit, &bare()),
            HostCall::Rpc {
                method: "git.worktree.create",
                params_json: r#"{"cwd":"C:/repo"}"#.to_owned(),
            }
        );
    }

    #[test]
    fn worktree_create_draft_dispatches_rpc() {
        let mut ctx = bare();
        ctx.wt_path = "../mux-feat".to_owned();
        ctx.wt_branch = "feat".to_owned();
        ctx.wt_create_branch = true;
        assert_rpc(
            worktree_create_call(&ctx),
            "git.worktree.create",
            r#"{"cwd":"C:/repo","path":"../mux-feat","branch":"feat","create_branch":true}"#,
        );
        ctx.wt_create_branch = false;
        assert_rpc(
            worktree_create_call(&ctx),
            "git.worktree.create",
            r#"{"cwd":"C:/repo","path":"../mux-feat","branch":"feat","create_branch":false}"#,
        );
        assert_ne!(worktree_create_call(&ctx), HostCall::Local);
        assert_ne!(
            worktree_create_call(&ctx),
            host_call(ClientAction::RefreshGit, &ctx)
        );

        ctx.wt_path.clear();
        ctx.wt_create_branch = true;
        assert_eq!(worktree_create_call(&ctx), HostCall::NeedsHost);
        ctx.wt_path = "   ".to_owned();
        assert_eq!(worktree_create_call(&ctx), HostCall::NeedsHost);
        ctx.wt_path = "../mux-feat".to_owned();
        ctx.wt_branch.clear();
        assert_eq!(worktree_create_call(&ctx), HostCall::NeedsHost);
        ctx.wt_branch = "   ".to_owned();
        assert_eq!(worktree_create_call(&ctx), HostCall::NeedsHost);
        assert_ne!(
            worktree_create_call(&ctx),
            HostCall::Rpc {
                method: "git.worktree.create",
                params_json: r#"{"cwd":"C:/repo"}"#.to_owned(),
            }
        );

        ctx.wt_path = "../mux-feat".to_owned();
        ctx.wt_branch = "feat".to_owned();
        ctx.wt_create_branch = true;
        assert_eq!(
            host_call(ClientAction::CreateWorktree, &ctx),
            worktree_create_call(&ctx)
        );
        ctx.wt_path.clear();
        assert_eq!(
            host_call(ClientAction::CreateWorktree, &ctx),
            HostCall::NeedsHost
        );
        assert_eq!(host_call(ClientAction::SelectModel, &ctx), HostCall::Local);
    }

    #[test]
    fn layout_is_local() {
        let ctx = with_session("sess-1");
        for action in [
            ClientAction::NewThread,
            ClientAction::SelectThread(3),
            ClientAction::ToggleLeft,
            ClientAction::ToggleRight,
            ClientAction::SelectTab(InspectorTab::Mcp),
            ClientAction::DismissReminder,
            ClientAction::TogglePalette,
            ClientAction::ClosePalette,
            ClientAction::ToggleHelp,
            ClientAction::DeleteThread,
            ClientAction::CycleModel,
            ClientAction::SelectLeftSection(crate::workspace::LeftSection::Files),
            ClientAction::ToggleBottom,
            ClientAction::InsertFileMention,
            ClientAction::ToggleSettings,
            ClientAction::SelectModel,
            ClientAction::HideLeft,
            ClientAction::FocusLayout,
            ClientAction::OpenGitTab,
        ] {
            assert_eq!(host_call(action, &ctx), HostCall::Local, "{action:?}");
            assert_ne!(host_call(action, &ctx), HostCall::NeedsHost, "{action:?}");
        }
    }

    #[test]
    fn send_is_needs_host() {
        let ctx = with_session("sess-1");
        for action in [
            ClientAction::Send,
            ClientAction::RefreshCores,
            ClientAction::RunTerminal,
            ClientAction::CycleFile,
            ClientAction::CopyLastMessage,
            ClientAction::RefreshMcp,
        ] {
            assert_eq!(host_call(action, &ctx), HostCall::NeedsHost, "{action:?}");
            assert_ne!(host_call(action, &ctx), HostCall::Local, "{action:?}");
        }
    }
}
