//! Multiplexer desktop: glass chrome, hideable/resizable rails, live grok -p.

use gpui::{
    div, hsla, point, prelude::*, px, rgb, size, App, Application, Bounds, BoxShadow, Context,
    CursorStyle, Hsla, KeyDownEvent, MouseButton, MouseMoveEvent, SharedString, TitlebarOptions,
    Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use multiplexer_server::Server;
use multiplexer_shell::{InspectorTab, Role, Workspace};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};

#[derive(Clone, Copy)]
enum DragRail {
    Left,
    Right,
}

struct Theme;

impl Theme {
    fn glass() -> Hsla {
        hsla(0.64, 0.16, 0.10, 0.52)
    }
    fn glass_strong() -> Hsla {
        hsla(0.64, 0.18, 0.12, 0.68)
    }
    fn ink() -> Hsla {
        hsla(0.64, 0.22, 0.06, 0.35)
    }
    fn hairline() -> Hsla {
        hsla(0.0, 0.0, 1.0, 0.10)
    }
    fn hairline_bright() -> Hsla {
        hsla(0.0, 0.0, 1.0, 0.18)
    }
    fn text() -> Hsla {
        hsla(0.62, 0.08, 0.92, 0.94)
    }
    fn muted() -> Hsla {
        hsla(0.62, 0.08, 0.72, 0.72)
    }
    fn accent() -> Hsla {
        hsla(0.58, 0.72, 0.62, 0.95)
    }
    fn good() -> Hsla {
        hsla(0.38, 0.55, 0.58, 0.95)
    }
    fn shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: hsla(0.64, 0.30, 0.04, 0.45),
                offset: point(px(0.), px(10.)),
                blur_radius: px(28.),
                spread_radius: px(-4.),
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 1.0, 0.04),
                offset: point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
            },
        ]
    }
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
            drag: None,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let win_w = f32::from(window.viewport_size().width);
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::ink())
            .text_color(Theme::text())
            .text_sm()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" {
                    this.send();
                } else if key == "backspace" {
                    this.workspace.backspace();
                } else if key == "[" && event.keystroke.modifiers.control {
                    this.workspace.chrome.toggle_left();
                } else if key == "]" && event.keystroke.modifiers.control {
                    this.workspace.chrome.toggle_right();
                } else if key.len() == 1 {
                    if let Some(c) = key.chars().next() {
                        this.workspace.type_char(c);
                    }
                }
                cx.notify();
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
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .gap_1()
                    .child(self.left_rail(cx))
                    .child(self.resize_handle(DragRail::Left, cx))
                    .child(self.center(cx))
                    .child(self.resize_handle(DragRail::Right, cx))
                    .child(self.right_rail(cx)),
            )
    }
}

impl ShellView {
    fn title_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let left_on = self.workspace.chrome.left_open;
        let right_on = self.workspace.chrome.right_open;
        glass_bar()
            .h(px(48.0))
            .px_4()
            .rounded_none()
            .border_b_1()
            .child(ghost_btn(
                if left_on { "Hide chats" } else { "Show chats" },
                cx,
                |this, cx| {
                    this.workspace.chrome.toggle_left();
                    cx.notify();
                },
            ))
            .child(
                div().flex_1().flex().justify_center().child(
                    div()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(self.workspace.title_bar()),
                ),
            )
            .child(ghost_btn(
                if right_on {
                    "Hide inspector"
                } else {
                    "Show inspector"
                },
                cx,
                |this, cx| {
                    this.workspace.chrome.toggle_right();
                    cx.notify();
                },
            ))
    }

    fn left_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.workspace.chrome.left_open;
        let w = self.workspace.chrome.occupied_left();
        let selected = self.workspace.selected;
        let threads = self.workspace.threads.clone();
        let rail = glass_pane().w(px(w)).h_full().flex().flex_col();
        if !open {
            return rail.child(collapsed_strip("Chats", cx, |this, cx| {
                this.workspace.chrome.toggle_left();
                cx.notify();
            }));
        }
        rail.child(
            div()
                .px_3()
                .py_2()
                .flex()
                .justify_between()
                .items_center()
                .child(div().text_color(Theme::muted()).child("CHATS"))
                .child(pill("New", Theme::accent(), cx, |this, cx| {
                    this.workspace.new_thread();
                    this.session_id = None;
                    cx.notify();
                })),
        )
        .children(threads.into_iter().enumerate().map(|(i, t)| {
            let on = i == selected;
            div()
                .id(SharedString::from(format!("thr-{i}")))
                .mx_2()
                .mb_1()
                .px_3()
                .py_2()
                .rounded_xl()
                .bg(if on {
                    hsla(0.58, 0.35, 0.22, 0.45)
                } else {
                    hsla(0.0, 0.0, 1.0, 0.03)
                })
                .border_1()
                .border_color(if on {
                    Theme::hairline_bright()
                } else {
                    hsla(0.0, 0.0, 1.0, 0.04)
                })
                .cursor_pointer()
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
                        .text_color(Theme::muted())
                        .child(format!("{} · {}", t.status, t.id)),
                )
        }))
    }

    fn right_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.workspace.chrome.right_open;
        let w = self.workspace.chrome.occupied_right();
        let tab = self.workspace.inspector;
        let session = self
            .session_id
            .clone()
            .unwrap_or_else(|| "(none yet)".into());
        let rail = glass_pane().w(px(w)).h_full().flex().flex_col();
        if !open {
            return rail.child(collapsed_strip("Info", cx, |this, cx| {
                this.workspace.chrome.toggle_right();
                cx.notify();
            }));
        }
        rail.child(
            div().flex().px_2().pt_2().gap_1().children(
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
                        .px_2()
                        .py_1()
                        .rounded_lg()
                        .cursor_pointer()
                        .bg(if on {
                            hsla(0.58, 0.40, 0.28, 0.50)
                        } else {
                            hsla(0.0, 0.0, 1.0, 0.03)
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.workspace.inspector = t;
                                cx.notify();
                            }),
                        )
                        .child(t.label())
                }),
            ),
        )
        .child(div().p_3().text_color(Theme::muted()).child(match tab {
            InspectorTab::Session => format!(
                "Project\n{}\n\nModel  {}\nSession  {}\nThreads  {}",
                self.workspace.project,
                self.workspace.model,
                session,
                self.workspace.threads.len()
            ),
            InspectorTab::Resources => {
                if self.workspace.worktrees.is_empty() {
                    "git worktrees\n(none listed)".into()
                } else {
                    format!("git worktrees\n{}", self.workspace.worktrees.join("\n"))
                }
            }
            InspectorTab::Mcp => {
                "MCP supervisor reuses servers by config hash.\nTeardown at zero refs.".into()
            }
        }))
    }

    fn center(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.workspace.selected_thread().cloned();
        let draft = self.workspace.draft.clone();
        glass_pane()
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
                    }),
            )
            .child(
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
                            .child(chip("What can you do?", cx))
                            .child(chip("Summarize this repo", cx)),
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
                                    .bg(hsla(0.0, 0.0, 1.0, 0.06))
                                    .border_1()
                                    .border_color(Theme::hairline_bright())
                                    .child(if draft.is_empty() {
                                        SharedString::from("Message Grok…  Enter to send")
                                    } else {
                                        SharedString::from(draft)
                                    }),
                            )
                            .child(pill("Send", Theme::good(), cx, |this, cx| {
                                this.send();
                                cx.notify();
                            })),
                    ),
            )
    }

    fn resize_handle(&mut self, rail: DragRail, cx: &mut Context<Self>) -> impl IntoElement {
        let open = match rail {
            DragRail::Left => self.workspace.chrome.left_open,
            DragRail::Right => self.workspace.chrome.right_open,
        };
        if !open {
            return div().id("resize-hidden").w(px(0.0)).into_any();
        }
        div()
            .id(SharedString::from(match rail {
                DragRail::Left => "resize-left",
                DragRail::Right => "resize-right",
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
}

fn glass_pane() -> gpui::Div {
    div()
        .rounded_xl()
        .bg(Theme::glass())
        .border_1()
        .border_color(Theme::hairline())
        .shadow(Theme::shadow())
}

fn glass_bar() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .bg(Theme::glass_strong())
        .border_color(Theme::hairline())
}

fn collapsed_strip(
    label: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(div().text_color(Theme::muted()).child(label))
}

fn empty_center() -> gpui::Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_color(Theme::muted())
        .child("Start a chat. Ctrl+[ and Ctrl+] toggle the rails.")
}

fn chip(label: &'static str, cx: &mut Context<ShellView>) -> impl IntoElement {
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
            cx.listener(move |this, _, _, cx| {
                this.workspace.set_draft(label);
                this.send();
                cx.notify();
            }),
        )
        .child(label)
}

fn pill(
    label: &'static str,
    color: Hsla,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .px_3()
        .py_1()
        .rounded_lg()
        .bg(color)
        .text_color(rgb(0x0c1018))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(label)
}

fn ghost_btn(
    label: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(label))
        .px_2()
        .py_1()
        .rounded_lg()
        .border_1()
        .border_color(Theme::hairline())
        .bg(hsla(0.0, 0.0, 1.0, 0.04))
        .text_color(Theme::muted())
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
        let bounds = Bounds::centered(None, size(px(1360.0), px(860.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Blurred,
                titlebar: Some(TitlebarOptions {
                    title: Some("Multiplexer".into()),
                    appears_transparent: true,
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
