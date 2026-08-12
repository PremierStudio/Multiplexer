//! Method-name constants (plan/04 §4).
//!
//! The RPC surface is grouped by namespace. These constants are the single
//! source of the wire spellings so the server dispatch table and client
//! builders cannot drift.

/// The single method for all server-pushed events (plan/04 §3.5).
pub const EVENT: &str = "event";

// session.* (plan/04 §4.1)
pub const SESSION_START: &str = "session.start";
pub const SESSION_STOP: &str = "session.stop";
pub const SESSION_INTERRUPT: &str = "session.interrupt";
pub const SESSION_LIST: &str = "session.list";
pub const SESSION_GET: &str = "session.get";

// turn.* (plan/04 §4.2)
pub const TURN_SEND: &str = "turn.send";
pub const TURN_CANCEL: &str = "turn.cancel";
pub const TURN_HISTORY: &str = "turn.history";

// approval.* (plan/04 §4.3)
pub const APPROVAL_RESPOND: &str = "approval.respond";
pub const APPROVAL_LIST: &str = "approval.list";

// userInput.* (plan/04 §4.4)
pub const USER_INPUT_RESPOND: &str = "userInput.respond";
pub const USER_INPUT_CANCEL: &str = "userInput.cancel";

// checkpoint.* (plan/04 §4.5)
pub const CHECKPOINT_LIST: &str = "checkpoint.list";
pub const CHECKPOINT_DIFF: &str = "checkpoint.diff";
pub const CHECKPOINT_REVERT: &str = "checkpoint.revert";
pub const CHECKPOINT_APPLY: &str = "checkpoint.apply";

// terminal.* (plan/04 §4.6)
pub const TERMINAL_CREATE: &str = "terminal.create";
pub const TERMINAL_RESIZE: &str = "terminal.resize";
pub const TERMINAL_INPUT: &str = "terminal.input";
pub const TERMINAL_KILL: &str = "terminal.kill";
pub const TERMINAL_LIST: &str = "terminal.list";
pub const TERMINAL_ATTACH: &str = "terminal.attach";

// fs.* (plan/04 §4.7)
pub const FS_READ: &str = "fs.read";
pub const FS_WRITE: &str = "fs.write";
pub const FS_LIST: &str = "fs.list";
pub const FS_WATCH: &str = "fs.watch";
pub const FS_UNWATCH: &str = "fs.unwatch";
pub const FS_STAT: &str = "fs.stat";

// git.* (plan/04 §4.8)
pub const GIT_STATUS: &str = "git.status";
pub const GIT_DIFF: &str = "git.diff";
pub const GIT_COMMIT: &str = "git.commit";
pub const GIT_BRANCHES: &str = "git.branches";
pub const GIT_CHECKOUT: &str = "git.checkout";
pub const GIT_WORKTREES: &str = "git.worktrees";
pub const GIT_WORKTREE_CREATE: &str = "git.worktree.create";

// browser.* (plan/04 §4.9)
pub const BROWSER_LIST: &str = "browser.list";
pub const BROWSER_LAUNCH: &str = "browser.launch";
pub const BROWSER_NAVIGATE: &str = "browser.navigate";
pub const BROWSER_CDP: &str = "browser.cdp";
pub const BROWSER_CLOSE: &str = "browser.close";
pub const BROWSER_SCREENSHOT: &str = "browser.screenshot";

// har.* (plan/04 §4.10)
pub const HAR_START: &str = "har.start";
pub const HAR_STOP: &str = "har.stop";
pub const HAR_REPLAY: &str = "har.replay";
pub const HAR_LIST: &str = "har.list";

// orchestration.* (plan/04 §4.11)
pub const ORCHESTRATION_SPAWN: &str = "orchestration.spawn";
pub const ORCHESTRATION_SUBSCRIBE: &str = "orchestration.subscribe";
pub const ORCHESTRATION_UNSUBSCRIBE: &str = "orchestration.unsubscribe";
pub const ORCHESTRATION_LIST: &str = "orchestration.list";

// model.* (plan/04 §4.12)
pub const MODEL_LIST: &str = "model.list";
pub const MODEL_SELECT: &str = "model.select";
pub const MODEL_GET: &str = "model.get";

// remote.* (plan/04 §4.13)
pub const REMOTE_LIST: &str = "remote.list";
pub const REMOTE_CONNECT: &str = "remote.connect";
pub const REMOTE_DISCONNECT: &str = "remote.disconnect";

// auth.* (plan/04 §4.14)
pub const AUTH_PROVIDERS: &str = "auth.providers";
pub const AUTH_LOGIN: &str = "auth.login";
pub const AUTH_STATUS: &str = "auth.status";
pub const AUTH_LOGOUT: &str = "auth.logout";

// telemetry.* (plan/04 §4.15)
pub const TELEMETRY_USAGE: &str = "telemetry.usage";
pub const TELEMETRY_RESOURCES: &str = "telemetry.resources";
pub const TELEMETRY_SUBSCRIBE: &str = "telemetry.subscribe";

// system.* (plan/04 §4.16)
pub const SYSTEM_HELLO: &str = "system.hello";
pub const SYSTEM_PING: &str = "system.ping";
pub const SYSTEM_CAPABILITIES: &str = "system.capabilities";

// subscription control (plan/04 §2.3, §8.2)
pub const SUBSCRIBE: &str = "subscribe";
pub const UNSUBSCRIBE: &str = "unsubscribe";
pub const ATTACH_STREAM: &str = "attach_stream";
pub const STREAM_ACK: &str = "stream.ack";
