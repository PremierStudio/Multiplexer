//! Multiplexer desktop: chats, transcript, composer, inspector.
//!
//! Product state lives in `multiplexer-shell::Workspace` (tested, headless).
//! This binary paints that state and talks to a local `grok -p` session
//! through `Server::handle_frame`. CI does not launch this window.

use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, KeyDownEvent, MouseButton,
    SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use multiplexer_server::Server;
use multiplexer_shell::{InspectorTab, Role, Workspace};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};

struct ShellView {
    workspace: Workspace,
    server: Server<
        multiplexer_server::ProviderBridge<
            multiplexer_provider::GrokAdapter<multiplexer_provider::CliGrokFactory>,
        >,
    >,
    session_id: Option<String>,
}

impl ShellView {
    fn new() -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
        let mut workspace = Workspace::new(cwd, "grok");
        workspace.connect(Vec::new());
        let server = Server::with_local();
        let mut view = Self {
            workspace,
            server,
            session_id: None,
        };
        view.refresh_worktrees();
        view
    }

    fn refresh_worktrees(&mut self) {
        let frames = self.server.handle_frame(&rpc(
            "wt",
            methods::GIT_WORKTREES,
            json!({ "cwd": self.workspace.project }),
        ));
        self.workspace.worktrees = worktree_paths(&frames);
    }

    fn send(&mut self) {
        let Some(text) = self.workspace.send_draft() else {
            return;
        };
        let frames = if let Some(sid) = &self.session_id {
            self.server.handle_frame(&rpc(
                "turn",
                methods::TURN_SEND,
                json!({ "session_id": sid, "text": text }),
            ))
        } else {
            let frames = self.server.handle_frame(&rpc(
                "start",
                methods::SESSION_START,
                json!({
                    "provider": "grok",
                    "model": self.workspace.model,
                    "workspace": self.workspace.project,
                    "initial_prompt": text,
                }),
            ));
            if let Some(id) = session_id_from(&frames) {
                self.session_id = Some(id.clone());
                self.workspace.connect(vec![id]);
            }
            frames
        };
        if let Some(err) = first_error(&frames) {
            self.workspace.mark_error(err);
        } else {
            let reply = assistant_text(&frames);
            if reply.is_empty() {
                self.workspace.push_assistant("(no text from grok -p)");
            } else {
                self.workspace.push_assistant(reply);
            }
        }
        self.refresh_worktrees();
    }
}

impl Render for ShellView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x12141a))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" {
                    this.send();
                } else if key == "backspace" {
                    this.workspace.backspace();
                } else if key.len() == 1 {
                    if let Some(c) = key.chars().next() {
                        this.workspace.type_char(c);
                    }
                }
                cx.notify();
            }))
            .text_color(rgb(0xe8e8e8))
            .text_sm()
            .child(title_bar(&self.workspace))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.left_rail(cx))
                    .child(self.center(cx))
                    .child(self.right_rail(cx)),
            )
    }
}

impl ShellView {
    fn left_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.workspace.selected;
        let threads = self.workspace.threads.clone();
        div()
            .w(px(240.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0x16181f))
            .border_r_1()
            .border_color(rgb(0x2a2d36))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("Chats"))
                    .child(button("New chat", rgb(0x3b82f6), cx, |this, cx| {
                        this.workspace.new_thread();
                        this.session_id = None;
                        cx.notify();
                    })),
            )
            .children(threads.into_iter().enumerate().map(|(i, t)| {
                let bg = if i == selected {
                    rgb(0x252a36)
                } else {
                    rgb(0x16181f)
                };
                div()
                    .id(SharedString::from(format!("thr-{i}")))
                    .px_3()
                    .py_2()
                    .bg(bg)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1e2230)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.workspace.select(i);
                            cx.notify();
                        }),
                    )
                    .child(div().child(t.title.clone()))
                    .child(
                        div()
                            .text_color(rgb(0x8b90a0))
                            .child(format!("{} · {}", t.status, t.id)),
                    )
            }))
    }

    fn center(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.workspace.selected_thread().cloned();
        let draft = self.workspace.draft.clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
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
                                let (who, color) = match m.role {
                                    Role::User => ("You", rgb(0x93c5fd)),
                                    Role::Assistant => ("Agent", rgb(0x86efac)),
                                };
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_color(color).child(who))
                                    .child(div().child(m.text))
                            })
                            .collect(),
                        None => vec![empty_center()],
                    }),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(0x2a2d36))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(chip("What can you do?", cx))
                            .child(chip("List my sessions", cx)),
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
                                    .bg(rgb(0x1c1f28))
                                    .border_1()
                                    .border_color(rgb(0x3a3f4d))
                                    .child(if draft.is_empty() {
                                        SharedString::from("Message the agent…  (type, then Send)")
                                    } else {
                                        SharedString::from(draft)
                                    }),
                            )
                            .child(button("Send", rgb(0x22c55e), cx, |this, cx| {
                                this.send();
                                cx.notify();
                            })),
                    ),
            )
    }

    fn right_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.workspace.inspector;
        let session = self
            .session_id
            .clone()
            .unwrap_or_else(|| "(none yet)".into());
        div()
            .w(px(280.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0x16181f))
            .border_l_1()
            .border_color(rgb(0x2a2d36))
            .child(
                div().flex().children(
                    [
                        InspectorTab::Session,
                        InspectorTab::Resources,
                        InspectorTab::Mcp,
                    ]
                    .into_iter()
                    .map(|t| {
                        let on = tab == t;
                        div()
                            .id(SharedString::from(t.label()))
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .bg(if on { rgb(0x252a36) } else { rgb(0x16181f) })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.workspace.inspector = t;
                                cx.notify();
                            }))
                            .child(t.label())
                    }),
                ),
            )
            .child(
                div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(match tab {
                        InspectorTab::Session => format!(
                            "Project: {}\nModel: {}\nSession: {}\nThreads: {}",
                            self.workspace.project,
                            self.workspace.model,
                            session,
                            self.workspace.threads.len()
                        ),
                        InspectorTab::Resources => {
                            if self.workspace.worktrees.is_empty() {
                                "git worktrees: (none listed)".into()
                            } else {
                                format!(
                                    "git worktrees:\n{}",
                                    self.workspace.worktrees.join("\n")
                                )
                            }
                        }
                        InspectorTab::Mcp => {
                            "MCP supervisor reuses servers by config hash.\nTeardown at zero refs.\nRegistry UI is Phase 2.".into()
                        }
                    }),
            )
    }
}

fn title_bar(ws: &Workspace) -> impl IntoElement {
    div()
        .h(px(40.0))
        .px_4()
        .flex()
        .items_center()
        .bg(rgb(0x0e1016))
        .border_b_1()
        .border_color(rgb(0x2a2d36))
        .child(ws.title_bar())
}

fn empty_center() -> gpui::Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(0x8b90a0))
        .child("Start a chat. This is the control surface, not an empty pane.")
}

fn chip(label: &'static str, cx: &mut Context<ShellView>) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .px_2()
        .py_1()
        .bg(rgb(0x1c1f28))
        .border_1()
        .border_color(rgb(0x3a3f4d))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.workspace.set_draft(label);
                this.send();
                cx.notify();
            }),
        )
        .child(label)
}

fn button(
    label: &'static str,
    color: gpui::Rgba,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .px_2()
        .py_1()
        .bg(color)
        .text_color(rgb(0x0e1016))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(label)
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

fn assistant_text(frames: &[String]) -> String {
    let mut out = String::new();
    for f in frames {
        if let Ok(Message::Notification(n)) = decode_frame(f) {
            if n.method == methods::EVENT {
                if let Some(text) = n
                    .params
                    .get("data")
                    .and_then(|d| d.get("text"))
                    .and_then(Value::as_str)
                {
                    out.push_str(text);
                }
            }
        }
    }
    out
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
