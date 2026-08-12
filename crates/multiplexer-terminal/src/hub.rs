//! In-memory table of terminals. No PTY is spawned.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{TerminalError, TerminalId, TerminalSpec};

struct Terminal {
    spec: TerminalSpec,
    input: Vec<u8>,
    alive: bool,
}

struct Inner {
    next: u64,
    order: Vec<String>,
    terminals: HashMap<String, Terminal>,
}

/// Point-in-time view of one hub entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub id: TerminalId,
    pub spec: TerminalSpec,
    pub input: Vec<u8>,
    pub alive: bool,
}

/// Observer that outlives [`TerminalHub`] so Drop can be asserted.
#[derive(Clone)]
pub struct TerminalWatch {
    inner: Arc<Mutex<Inner>>,
}

/// Session table: create, list, resize, buffered input, kill.
pub struct TerminalHub {
    inner: Arc<Mutex<Inner>>,
}

impl Default for TerminalHub {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalHub {
    /// Empty hub. The first created id is `term-1`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next: 1,
                order: Vec::new(),
                terminals: HashMap::new(),
            })),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("terminal hub mutex")
    }

    /// Cheap observer. Drop of the observer does not kill sessions.
    pub fn watch(&self) -> TerminalWatch {
        TerminalWatch {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Allocate a new live terminal. Ids increment globally (`term-1`, ...).
    pub fn create(&mut self, spec: TerminalSpec) -> TerminalId {
        let mut inner = self.lock();
        let n = inner.next;
        inner.next += 1;
        let id = TerminalId(format!("term-{n}"));
        let key = id.0.clone();
        inner.terminals.insert(
            key.clone(),
            Terminal {
                spec,
                input: Vec::new(),
                alive: true,
            },
        );
        inner.order.push(key);
        id
    }

    /// Live terminal ids in creation order.
    pub fn list(&self) -> Vec<TerminalId> {
        list_live(&self.lock())
    }

    /// Snapshot a terminal, including killed ones still held for observers.
    pub fn get(&self, id: &TerminalId) -> Option<TerminalSnapshot> {
        snapshot(&self.lock(), id)
    }

    /// True while the terminal exists and has not been killed (or hub-dropped).
    pub fn is_alive(&self, id: &TerminalId) -> bool {
        is_alive(&self.lock(), id)
    }

    /// Change cols/rows of a live terminal.
    pub fn resize(&mut self, id: &TerminalId, cols: u16, rows: u16) -> Result<(), TerminalError> {
        let mut inner = self.lock();
        let term = live_mut(&mut inner, id)?;
        term.spec.cols = cols;
        term.spec.rows = rows;
        Ok(())
    }

    /// Append bytes to the input buffer of a live terminal.
    pub fn input(&mut self, id: &TerminalId, data: &[u8]) -> Result<(), TerminalError> {
        let mut inner = self.lock();
        let term = live_mut(&mut inner, id)?;
        term.input.extend_from_slice(data);
        Ok(())
    }

    /// Borrow the recorded input of a terminal (live or killed).
    pub fn input_buffer(&self, id: &TerminalId) -> Option<Vec<u8>> {
        self.lock()
            .terminals
            .get(&id.0)
            .map(|term| term.input.clone())
    }

    /// Mark a live terminal dead. Further commands return [`TerminalError::NotFound`].
    pub fn kill(&mut self, id: &TerminalId) -> Result<(), TerminalError> {
        let mut inner = self.lock();
        let term = live_mut(&mut inner, id)?;
        term.alive = false;
        Ok(())
    }
}

impl Drop for TerminalHub {
    fn drop(&mut self) {
        let mut inner = self.lock();
        for term in inner.terminals.values_mut() {
            term.alive = false;
        }
    }
}

impl TerminalWatch {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("terminal watch mutex")
    }

    /// Same liveness rule as [`TerminalHub::is_alive`].
    pub fn is_alive(&self, id: &TerminalId) -> bool {
        is_alive(&self.lock(), id)
    }

    /// Live ids after the hub may have been dropped.
    pub fn list(&self) -> Vec<TerminalId> {
        list_live(&self.lock())
    }

    /// Snapshot after the hub may have been dropped.
    pub fn get(&self, id: &TerminalId) -> Option<TerminalSnapshot> {
        snapshot(&self.lock(), id)
    }
}

fn is_alive(inner: &Inner, id: &TerminalId) -> bool {
    inner
        .terminals
        .get(&id.0)
        .map(|term| term.alive)
        .unwrap_or(false)
}

fn list_live(inner: &Inner) -> Vec<TerminalId> {
    inner
        .order
        .iter()
        .filter(|key| inner.terminals.get(*key).is_some_and(|term| term.alive))
        .map(|key| TerminalId(key.clone()))
        .collect()
}

fn snapshot(inner: &Inner, id: &TerminalId) -> Option<TerminalSnapshot> {
    inner.terminals.get(&id.0).map(|term| TerminalSnapshot {
        id: id.clone(),
        spec: term.spec.clone(),
        input: term.input.clone(),
        alive: term.alive,
    })
}

fn live_mut<'a>(inner: &'a mut Inner, id: &TerminalId) -> Result<&'a mut Terminal, TerminalError> {
    let term = inner
        .terminals
        .get_mut(&id.0)
        .ok_or_else(|| TerminalError::NotFound(id.clone()))?;
    if !term.alive {
        return Err(TerminalError::NotFound(id.clone()));
    }
    Ok(term)
}
