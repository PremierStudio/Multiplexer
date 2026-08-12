//! In-process Grok adapter: [`GrokAdapter`] over an injected [`GrokShellFactory`].
//!
//! Tests drive [`RecordingGrokFactory`]. The real [`VendoredGrokFactory`] is
//! gated on the `embed-grok` crate feature (off by default) so CI does not
//! compile `third_party/grok-build`.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use multiplexer_wire::approval::ApprovalDecision;

use crate::adapter::{ProviderAdapter, SessionSnapshot, SessionStartParams, TurnInput};
use crate::error::ProviderError;
use crate::event::ProviderEvent;
use crate::ids::{ModelId, ProviderKind, SessionId};

/// One live in-process grok-build session.
pub trait GrokHandle: Send + std::fmt::Debug {
    /// Push a user turn into the session.
    fn send_turn(&mut self, turn: &TurnInput) -> Result<(), ProviderError>;

    /// Abort the in-flight turn, if any.
    fn interrupt(&mut self) -> Result<(), ProviderError>;

    /// Tear down the session.
    fn stop(&mut self) -> Result<(), ProviderError>;
}

/// Constructs a [`GrokHandle`] for one [`SessionStartParams`].
pub trait GrokShellFactory: Send + Sync {
    /// Start a shell session. Failures are [`ProviderError::Provider`].
    fn start(&self, params: &SessionStartParams) -> Result<Box<dyn GrokHandle>, ProviderError>;
}

/// Recorded [`GrokShellFactory`] / [`GrokHandle`] invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrokCall {
    /// [`GrokShellFactory::start`].
    Start {
        provider: ProviderKind,
        model: ModelId,
        workspace: PathBuf,
        initial_prompt: Option<String>,
        resume: Option<String>,
    },
    /// [`GrokHandle::send_turn`].
    SendTurn { text: String },
    /// [`GrokHandle::interrupt`].
    Interrupt,
    /// [`GrokHandle::stop`].
    Stop,
}

/// Test double: records factory and handle calls, never talks to a model.
#[derive(Clone)]
pub struct RecordingGrokFactory {
    calls: Arc<Mutex<Vec<GrokCall>>>,
}

impl RecordingGrokFactory {
    /// Empty call log.
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Snapshot of recorded calls, in order.
    pub fn calls(&self) -> Vec<GrokCall> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<GrokCall>> {
        self.calls.lock().expect("recording grok mutex")
    }
}

impl Default for RecordingGrokFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct RecordingGrokHandle {
    calls: Arc<Mutex<Vec<GrokCall>>>,
}

impl RecordingGrokHandle {
    fn push(&mut self, call: GrokCall) {
        self.calls.lock().expect("recording grok mutex").push(call);
    }
}

impl GrokHandle for RecordingGrokHandle {
    fn send_turn(&mut self, turn: &TurnInput) -> Result<(), ProviderError> {
        self.push(GrokCall::SendTurn {
            text: turn.text.clone(),
        });
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), ProviderError> {
        self.push(GrokCall::Interrupt);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), ProviderError> {
        self.push(GrokCall::Stop);
        Ok(())
    }
}

impl GrokShellFactory for RecordingGrokFactory {
    fn start(&self, params: &SessionStartParams) -> Result<Box<dyn GrokHandle>, ProviderError> {
        self.lock().push(GrokCall::Start {
            provider: params.provider,
            model: params.model.clone(),
            workspace: params.workspace.clone(),
            initial_prompt: params.initial_prompt.clone(),
            resume: params.resume.clone(),
        });
        Ok(Box::new(RecordingGrokHandle {
            calls: Arc::clone(&self.calls),
        }))
    }
}

/// Builds an in-process `xai-grok-shell` session when `embed-grok` is on.
///
/// The vendored crate is not a default dependency. Without the feature,
/// [`Self::start`] returns [`ProviderError::Provider`].
pub struct VendoredGrokFactory;

impl VendoredGrokFactory {
    /// Stateless factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VendoredGrokFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokShellFactory for VendoredGrokFactory {
    fn start(&self, _params: &SessionStartParams) -> Result<Box<dyn GrokHandle>, ProviderError> {
        #[cfg(feature = "embed-grok")]
        {
            // TODO: construct xai-grok-shell (Session / run_headless) from
            // `_params`. Embedding is a heavy compile and needs a live
            // composition root; keep the optional path dep off this revision.
            Err(ProviderError::Provider(
                "embed-grok vendored shell not wired".into(),
            ))
        }
        #[cfg(not(feature = "embed-grok"))]
        {
            Err(ProviderError::Provider("embed-grok feature off".into()))
        }
    }
}

struct SessionState {
    id: SessionId,
    model: ModelId,
    workspace: PathBuf,
    events: VecDeque<ProviderEvent>,
    handle: Box<dyn GrokHandle>,
}

struct Inner {
    next: u64,
    order: Vec<String>,
    sessions: HashMap<String, SessionState>,
}

/// [`ProviderAdapter`] that owns sessions via [`GrokShellFactory`].
pub struct GrokAdapter<F> {
    factory: F,
    inner: Arc<Mutex<Inner>>,
}

impl<F: Clone> Clone for GrokAdapter<F> {
    fn clone(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<F: GrokShellFactory> GrokAdapter<F> {
    /// Empty adapter. The first session id is `sess-1`.
    pub fn new(factory: F) -> Self {
        Self {
            factory,
            inner: Arc::new(Mutex::new(Inner {
                next: 1,
                order: Vec::new(),
                sessions: HashMap::new(),
            })),
        }
    }

    /// The injected factory (tests inspect [`RecordingGrokFactory::calls`]).
    pub fn factory(&self) -> &F {
        &self.factory
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("grok adapter mutex")
    }
}

fn not_found(session: &SessionId) -> ProviderError {
    ProviderError::NotFound(format!("session {session}"))
}

fn session_mut<'a>(
    inner: &'a mut Inner,
    session: &SessionId,
) -> Result<&'a mut SessionState, ProviderError> {
    inner
        .sessions
        .get_mut(&session.0)
        .ok_or_else(|| not_found(session))
}

impl<F: GrokShellFactory> ProviderAdapter for GrokAdapter<F> {
    fn kind(&self) -> ProviderKind {
        ProviderKind::GrokInProcess
    }

    fn start_session(&self, params: SessionStartParams) -> Result<SessionId, ProviderError> {
        let handle = self.factory.start(&params)?;
        let SessionStartParams {
            provider: _,
            model,
            workspace,
            initial_prompt: _,
            resume: _,
        } = params;
        let mut inner = self.lock();
        let n = inner.next;
        inner.next += 1;
        let id = SessionId(format!("sess-{n}"));
        let key = id.0.clone();
        let mut events = VecDeque::new();
        events.push_back(ProviderEvent::SessionReady {
            session: id.clone(),
        });
        inner.sessions.insert(
            key.clone(),
            SessionState {
                id: id.clone(),
                model,
                workspace,
                events,
                handle,
            },
        );
        inner.order.push(key);
        Ok(id)
    }

    fn send_turn(&self, session: &SessionId, turn: TurnInput) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        state.handle.send_turn(&turn)
    }

    fn interrupt_turn(&self, session: &SessionId) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        state.handle.interrupt()
    }

    fn approval_respond(
        &self,
        session: &SessionId,
        _request_id: &str,
        _decision: ApprovalDecision,
    ) -> Result<(), ProviderError> {
        let inner = self.lock();
        if !inner.sessions.contains_key(&session.0) {
            return Err(not_found(session));
        }
        Err(ProviderError::InvalidState("approvals not wired".into()))
    }

    fn session_stop(&self, session: &SessionId) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        {
            let state = session_mut(&mut inner, session)?;
            state.handle.stop()?;
        }
        inner.sessions.remove(&session.0);
        inner.order.retain(|id| id != &session.0);
        Ok(())
    }

    fn poll_event(&self, session: &SessionId) -> Option<ProviderEvent> {
        let mut inner = self.lock();
        inner
            .sessions
            .get_mut(&session.0)
            .and_then(|state| state.events.pop_front())
    }

    fn list_sessions(&self) -> Vec<SessionId> {
        self.lock()
            .order
            .iter()
            .map(|id| SessionId(id.clone()))
            .collect()
    }

    fn get_session(&self, session: &SessionId) -> Option<SessionSnapshot> {
        self.lock()
            .sessions
            .get(&session.0)
            .map(|state| SessionSnapshot {
                id: state.id.clone(),
                model: state.model.clone(),
                workspace: state.workspace.clone(),
                running: false,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplexer_wire::error::AppErrorKind;

    fn workspace() -> PathBuf {
        PathBuf::from("C:\\work\\demo")
    }

    fn params(prompt: Option<&str>) -> SessionStartParams {
        SessionStartParams {
            provider: ProviderKind::GrokInProcess,
            model: ModelId("grok-test".into()),
            workspace: workspace(),
            initial_prompt: prompt.map(str::to_string),
            resume: None,
        }
    }

    fn turn(text: &str) -> TurnInput {
        TurnInput {
            text: text.to_owned(),
        }
    }

    fn adapter() -> (GrokAdapter<RecordingGrokFactory>, RecordingGrokFactory) {
        let factory = RecordingGrokFactory::new();
        (GrokAdapter::new(factory.clone()), factory)
    }

    fn assert_send_sync<T: Send + Sync>() {}

    struct StartBoomFactory;

    impl GrokShellFactory for StartBoomFactory {
        fn start(&self, _: &SessionStartParams) -> Result<Box<dyn GrokHandle>, ProviderError> {
            Err(ProviderError::Provider("start boom".into()))
        }
    }

    #[derive(Debug)]
    struct BoomHandle;

    impl GrokHandle for BoomHandle {
        fn send_turn(&mut self, _: &TurnInput) -> Result<(), ProviderError> {
            Err(ProviderError::Provider("send boom".into()))
        }

        fn interrupt(&mut self) -> Result<(), ProviderError> {
            Err(ProviderError::Provider("interrupt boom".into()))
        }

        fn stop(&mut self) -> Result<(), ProviderError> {
            Err(ProviderError::Provider("stop boom".into()))
        }
    }

    struct BoomFactory;

    impl GrokShellFactory for BoomFactory {
        fn start(&self, _: &SessionStartParams) -> Result<Box<dyn GrokHandle>, ProviderError> {
            Ok(Box::new(BoomHandle))
        }
    }

    #[test]
    fn adapter_and_factory_are_send_sync() {
        assert_send_sync::<RecordingGrokFactory>();
        assert_send_sync::<VendoredGrokFactory>();
        assert_send_sync::<GrokAdapter<RecordingGrokFactory>>();
        assert_send_sync::<Box<dyn ProviderAdapter>>();
    }

    #[test]
    fn kind_is_grok_in_process() {
        let (p, _) = adapter();
        assert_eq!(p.kind(), ProviderKind::GrokInProcess);
        assert_ne!(p.kind(), ProviderKind::Fake);
        assert_ne!(p.kind(), ProviderKind::Acp);
    }

    #[test]
    fn start_emits_only_session_ready() {
        let (p, factory) = adapter();
        let id = p.start_session(params(None)).expect("start");
        assert_eq!(id.0, "sess-1");
        assert_eq!(
            p.poll_event(&id),
            Some(ProviderEvent::SessionReady {
                session: id.clone()
            })
        );
        assert_eq!(p.poll_event(&id), None);
        assert_eq!(
            factory.calls(),
            vec![GrokCall::Start {
                provider: ProviderKind::GrokInProcess,
                model: ModelId("grok-test".into()),
                workspace: workspace(),
                initial_prompt: None,
                resume: None,
            }]
        );
    }

    #[test]
    fn start_forwards_prompt_and_resume() {
        let (p, factory) = adapter();
        let mut start = params(Some("boot"));
        start.resume = Some("cursor-1".into());
        start.model = ModelId("other".into());
        start.workspace = PathBuf::from("D:\\ws");
        let id = p.start_session(start).expect("start");
        assert_eq!(
            factory.calls(),
            vec![GrokCall::Start {
                provider: ProviderKind::GrokInProcess,
                model: ModelId("other".into()),
                workspace: PathBuf::from("D:\\ws"),
                initial_prompt: Some("boot".into()),
                resume: Some("cursor-1".into()),
            }]
        );
        let snap = p.get_session(&id).expect("snap");
        assert_eq!(snap.model.0, "other");
        assert_eq!(snap.workspace, PathBuf::from("D:\\ws"));
        assert!(!snap.running);
        assert_eq!(snap.id, id);
    }

    #[test]
    fn send_turn_and_stop_are_recorded() {
        let (p, factory) = adapter();
        let id = p.start_session(params(None)).expect("start");
        p.send_turn(&id, turn("hello-grok")).expect("send");
        p.interrupt_turn(&id).expect("interrupt");
        p.session_stop(&id).expect("stop");
        let calls = factory.calls();
        assert_eq!(
            &calls[1..],
            &[
                GrokCall::SendTurn {
                    text: "hello-grok".into(),
                },
                GrokCall::Interrupt,
                GrokCall::Stop,
            ]
        );
        assert!(p.get_session(&id).is_none());
        assert!(p.list_sessions().is_empty());
    }

    #[test]
    fn factory_accessor_matches_injected() {
        let factory = RecordingGrokFactory::new();
        let p = GrokAdapter::new(factory.clone());
        let _ = p.start_session(params(None)).expect("start");
        assert_eq!(p.factory().calls().len(), 1);
        assert_eq!(factory.calls().len(), 1);
    }

    #[test]
    fn start_factory_error_creates_no_session() {
        let p = GrokAdapter::new(StartBoomFactory);
        let err = p.start_session(params(None)).expect_err("boom");
        assert_eq!(err, ProviderError::Provider("start boom".into()));
        assert!(p.list_sessions().is_empty());
        let (q, _) = adapter();
        assert_eq!(q.start_session(params(None)).unwrap().0, "sess-1");
    }

    #[test]
    fn handle_errors_propagate() {
        let p = GrokAdapter::new(BoomFactory);
        let id = p.start_session(params(None)).expect("start");
        assert_eq!(
            p.send_turn(&id, turn("x")).expect_err("send"),
            ProviderError::Provider("send boom".into())
        );
        assert_eq!(
            p.interrupt_turn(&id).expect_err("interrupt"),
            ProviderError::Provider("interrupt boom".into())
        );
        assert_eq!(
            p.session_stop(&id).expect_err("stop"),
            ProviderError::Provider("stop boom".into())
        );
        assert!(p.get_session(&id).is_some());
    }

    #[test]
    fn missing_session_is_not_found() {
        let (p, factory) = adapter();
        let gone = SessionId("sess-99".into());
        assert_eq!(
            p.send_turn(&gone, turn("x")).expect_err("send"),
            ProviderError::NotFound("session sess-99".into())
        );
        assert_eq!(
            p.interrupt_turn(&gone).expect_err("interrupt"),
            ProviderError::NotFound("session sess-99".into())
        );
        assert_eq!(
            p.session_stop(&gone).expect_err("stop"),
            ProviderError::NotFound("session sess-99".into())
        );
        assert_eq!(
            p.approval_respond(&gone, "r", ApprovalDecision::Allow)
                .expect_err("approval"),
            ProviderError::NotFound("session sess-99".into())
        );
        assert!(p.poll_event(&gone).is_none());
        assert!(p.get_session(&gone).is_none());
        assert!(factory.calls().is_empty());
    }

    #[test]
    fn approval_respond_on_live_session_is_invalid_state() {
        let (p, _) = adapter();
        let id = p.start_session(params(None)).expect("start");
        let err = p
            .approval_respond(&id, "req-1", ApprovalDecision::Deny)
            .expect_err("unwired");
        assert_eq!(
            err,
            ProviderError::InvalidState("approvals not wired".into())
        );
        assert_eq!(err.kind(), AppErrorKind::InvalidState);
        assert!(p.get_session(&id).is_some());
    }

    #[test]
    fn list_sessions_is_start_order_without_stopped() {
        let (p, _) = adapter();
        let a = p.start_session(params(None)).expect("a");
        let b = p.start_session(params(None)).expect("b");
        let c = p.start_session(params(None)).expect("c");
        assert_eq!(a.0, "sess-1");
        assert_eq!(b.0, "sess-2");
        assert_eq!(c.0, "sess-3");
        assert_eq!(p.list_sessions(), vec![a.clone(), b.clone(), c.clone()]);
        p.session_stop(&b).expect("stop b");
        assert_eq!(p.list_sessions(), vec![a, c]);
    }

    #[test]
    fn ids_do_not_reuse_after_stop() {
        let (p, _) = adapter();
        let a = p.start_session(params(None)).expect("a");
        p.session_stop(&a).expect("stop");
        let b = p.start_session(params(None)).expect("b");
        assert_eq!(b.0, "sess-2");
    }

    #[test]
    fn clone_shares_session_table() {
        let (p, _) = adapter();
        let q = p.clone();
        let id = p.start_session(params(None)).expect("start");
        assert_eq!(q.list_sessions(), vec![id.clone()]);
        q.session_stop(&id).expect("stop via clone");
        assert!(p.list_sessions().is_empty());
    }

    #[test]
    fn default_recording_matches_new() {
        let factory = RecordingGrokFactory::default();
        assert!(factory.calls().is_empty());
        let p = GrokAdapter::new(factory);
        assert_eq!(p.start_session(params(None)).unwrap().0, "sess-1");
    }

    #[test]
    fn two_adapters_have_independent_counters() {
        let (a, _) = adapter();
        let (b, _) = adapter();
        assert_eq!(a.start_session(params(None)).unwrap().0, "sess-1");
        assert_eq!(b.start_session(params(None)).unwrap().0, "sess-1");
    }

    #[test]
    fn vendored_factory_start_is_provider_error() {
        let factory = VendoredGrokFactory::new();
        let err = factory.start(&params(None)).expect_err("gated");
        assert_eq!(err.kind(), AppErrorKind::ProviderError);
        #[cfg(not(feature = "embed-grok"))]
        {
            assert_eq!(
                err,
                ProviderError::Provider("embed-grok feature off".into())
            );
            assert_eq!(err.to_string(), "provider error: embed-grok feature off");
        }
        #[cfg(feature = "embed-grok")]
        {
            assert_eq!(
                err,
                ProviderError::Provider("embed-grok vendored shell not wired".into())
            );
        }
        let p = GrokAdapter::new(VendoredGrokFactory::new());
        let start_err = p.start_session(params(None)).expect_err("adapter start");
        assert!(matches!(start_err, ProviderError::Provider(_)));
        assert!(p.list_sessions().is_empty());
    }

    #[test]
    fn grok_call_variants_are_distinct() {
        let start = GrokCall::Start {
            provider: ProviderKind::GrokInProcess,
            model: ModelId("m".into()),
            workspace: workspace(),
            initial_prompt: None,
            resume: None,
        };
        assert_ne!(start, GrokCall::SendTurn { text: "x".into() });
        assert_ne!(GrokCall::Interrupt, GrokCall::Stop);
        assert_ne!(
            GrokCall::SendTurn { text: "a".into() },
            GrokCall::SendTurn { text: "b".into() }
        );
    }

    #[test]
    fn trait_object_can_start_and_stop() {
        let factory = RecordingGrokFactory::new();
        let adapter = GrokAdapter::new(factory.clone());
        let p: &dyn ProviderAdapter = &adapter;
        assert_eq!(p.kind(), ProviderKind::GrokInProcess);
        let id = p.start_session(params(Some(""))).expect("start");
        p.send_turn(&id, turn("via-trait")).expect("turn");
        p.interrupt_turn(&id).expect("interrupt");
        assert_eq!(p.list_sessions().len(), 1);
        p.session_stop(&id).expect("stop");
        assert!(p.list_sessions().is_empty());
        assert_eq!(factory.calls().last(), Some(&GrokCall::Stop));
    }
}
