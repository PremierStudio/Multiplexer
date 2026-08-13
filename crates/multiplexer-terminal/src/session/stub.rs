//! Non-Windows stub: no ConPTY, spawn always fails.

use crate::{TerminalError, TerminalSpec};

/// Stub session. Cannot be constructed: [`Self::spawn`] always errors.
pub struct EmbeddedSession {
    _private: (),
}

impl EmbeddedSession {
    /// Always [`TerminalError::Unsupported`] on this platform.
    pub fn spawn(program: &str, args: &[&str], spec: &TerminalSpec) -> Result<Self, TerminalError> {
        let _ = (program, args, spec);
        Err(unsupported())
    }

    /// Never reached: spawn fails first.
    pub fn try_read(&mut self) -> Vec<u8> {
        Vec::new()
    }

    /// Never reached: spawn fails first.
    pub fn try_read_str(&mut self) -> String {
        String::new()
    }

    /// Always [`TerminalError::Unsupported`].
    pub fn write(&mut self, _data: &[u8]) -> Result<(), TerminalError> {
        Err(unsupported())
    }

    /// Always [`TerminalError::Unsupported`].
    pub fn write_str(&mut self, _text: &str) -> Result<(), TerminalError> {
        Err(unsupported())
    }

    /// Always [`TerminalError::Unsupported`].
    pub fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), TerminalError> {
        Err(unsupported())
    }

    /// Already gone: success.
    pub fn kill(&mut self) -> Result<(), TerminalError> {
        Ok(())
    }

    /// Stub has no child.
    pub fn pid(&self) -> Option<u32> {
        None
    }

    /// Stub is never live.
    pub fn is_alive(&mut self) -> bool {
        false
    }

    /// Last requested size, unused.
    pub fn size(&self) -> (u16, u16) {
        (0, 0)
    }
}

fn unsupported() -> TerminalError {
    TerminalError::Unsupported("ConPTY embedded sessions are Windows-only".into())
}
