//! In-memory [`ProviderAdapter`] used until grok-build is embedded.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use multiplexer_wire::approval::ApprovalDecision;

use crate::adapter::{ProviderAdapter, SessionSnapshot, SessionStartParams, TurnInput};
use crate::error::ProviderError;
use crate::event::ProviderEvent;
use crate::ids::{ModelId, ProviderKind, SessionId};

struct SessionState {
    id: SessionId,
    model: ModelId,
    workspace: PathBuf,
    running: bool,
    events: VecDeque<ProviderEvent>,
    pending: HashSet<String>,
    last_approval: Option<(String, ApprovalDecision)>,
}

struct Inner {
    next: u64,
    block_next: bool,
    order: Vec<String>,
    sessions: HashMap<String, SessionState>,
}

/// Test double: deterministic ids, FIFO events, optional blocked turns.
#[derive(Clone)]
pub struct FakeProvider {
    inner: Arc<Mutex<Inner>>,
}

impl FakeProvider {
    /// Empty provider. The first session id is `sess-1`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next: 1,
                block_next: false,
                order: Vec::new(),
                sessions: HashMap::new(),
            })),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("fake provider mutex")
    }

    /// Next [`ProviderAdapter::send_turn`] (or initial prompt) stays running
    /// until [`Self::complete_turn`] or [`ProviderAdapter::interrupt_turn`].
    pub fn block_next_turn(&self) {
        self.lock().block_next = true;
    }

    /// Finish a blocked turn and enqueue [`ProviderEvent::TurnFinished`].
    pub fn complete_turn(&self, session: &SessionId) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        if !state.running {
            return Err(ProviderError::InvalidState("no running turn".into()));
        }
        state.running = false;
        let id = state.id.clone();
        state
            .events
            .push_back(ProviderEvent::TurnFinished { session: id });
        Ok(())
    }

    /// Record a pending approval and enqueue [`ProviderEvent::ApprovalRequested`].
    pub fn request_approval(
        &self,
        session: &SessionId,
        request_id: &str,
        tool: &str,
    ) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        state.pending.insert(request_id.to_owned());
        let id = state.id.clone();
        state.events.push_back(ProviderEvent::ApprovalRequested {
            session: id,
            request_id: request_id.to_owned(),
            tool: tool.to_owned(),
        });
        Ok(())
    }

    /// Last successful [`ProviderAdapter::approval_respond`] for `session`.
    pub fn last_approval(&self, session: &SessionId) -> Option<(String, ApprovalDecision)> {
        self.lock()
            .sessions
            .get(&session.0)
            .and_then(|state| state.last_approval.clone())
    }
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self::new()
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

fn begin_turn(inner: &mut Inner, key: &str, text: String) -> Result<(), ProviderError> {
    let running = inner
        .sessions
        .get(key)
        .ok_or_else(|| ProviderError::NotFound(format!("session {key}")))?
        .running;
    if running {
        return Err(ProviderError::Conflict("turn already running".into()));
    }
    let block = inner.block_next;
    if block {
        inner.block_next = false;
    }
    let state = inner
        .sessions
        .get_mut(key)
        .expect("session exists: running flag was just read");
    let id = state.id.clone();
    state.events.push_back(ProviderEvent::TextDelta {
        session: id.clone(),
        text,
    });
    if block {
        state.running = true;
    } else {
        state.running = false;
        state
            .events
            .push_back(ProviderEvent::TurnFinished { session: id });
    }
    Ok(())
}

impl ProviderAdapter for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Fake
    }

    fn start_session(&self, params: SessionStartParams) -> Result<SessionId, ProviderError> {
        let SessionStartParams {
            provider: _,
            model,
            workspace,
            initial_prompt,
            resume: _,
        } = params;
        let mut inner = self.lock();
        let n = inner.next;
        inner.next += 1;
        let id = SessionId(format!("sess-{n}"));
        let key = id.0.clone();
        inner.sessions.insert(
            key.clone(),
            SessionState {
                id: id.clone(),
                model,
                workspace,
                running: false,
                events: VecDeque::new(),
                pending: HashSet::new(),
                last_approval: None,
            },
        );
        inner.order.push(key.clone());
        let ready_id = id.clone();
        inner
            .sessions
            .get_mut(&key)
            .expect("session exists: just inserted")
            .events
            .push_back(ProviderEvent::SessionReady { session: ready_id });
        if let Some(text) = initial_prompt {
            begin_turn(&mut inner, &key, text)
                .expect("session exists: just inserted with running=false");
        }
        Ok(id)
    }

    fn send_turn(&self, session: &SessionId, turn: TurnInput) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        begin_turn(&mut inner, &session.0, turn.text)
    }

    fn interrupt_turn(&self, session: &SessionId) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        if !state.running {
            return Err(ProviderError::InvalidState("no running turn".into()));
        }
        state.running = false;
        let id = state.id.clone();
        state.events.push_back(ProviderEvent::TurnFailed {
            session: id,
            message: "interrupted".into(),
        });
        Ok(())
    }

    fn approval_respond(
        &self,
        session: &SessionId,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        let state = session_mut(&mut inner, session)?;
        if !state.pending.remove(request_id) {
            return Err(ProviderError::NotFound(format!(
                "approval request {request_id}"
            )));
        }
        state.last_approval = Some((request_id.to_owned(), decision));
        Ok(())
    }

    fn session_stop(&self, session: &SessionId) -> Result<(), ProviderError> {
        let mut inner = self.lock();
        if inner.sessions.remove(&session.0).is_none() {
            return Err(not_found(session));
        }
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
                running: state.running,
            })
    }
}
