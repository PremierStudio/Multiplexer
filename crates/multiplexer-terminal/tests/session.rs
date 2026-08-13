//! Embedded ConPTY session tests. Do not require `grok`.

use std::thread;
use std::time::{Duration, Instant};

use multiplexer_terminal::{EmbeddedSession, TerminalError, TerminalSpec};

fn spec() -> TerminalSpec {
    TerminalSpec::new(80, 24, ".")
}

fn wait_contains(session: &mut EmbeddedSession, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut acc = String::new();
    while Instant::now() < deadline {
        acc.push_str(&session.try_read_str());
        if acc.contains(needle) {
            return acc;
        }
        thread::sleep(Duration::from_millis(20));
    }
    acc
}

#[test]
fn spawn_missing_program_errors() {
    let err = EmbeddedSession::spawn("mux-definitely-not-a-program-xyz", &[], &spec())
        .err()
        .expect("missing program must fail");
    match err {
        TerminalError::Spawn { program, message } => {
            assert_eq!(program, "mux-definitely-not-a-program-xyz");
            assert!(!message.is_empty());
        }
        TerminalError::Unsupported(msg) => {
            assert!(
                msg.to_ascii_lowercase().contains("windows")
                    || msg.to_ascii_lowercase().contains("conpty")
                    || msg.contains("CreatePseudoConsole"),
                "unsupported must name the backend, got {msg}"
            );
        }
        other => panic!("expected Spawn or Unsupported, got {other:?}"),
    }
}

#[cfg(not(windows))]
#[test]
fn non_windows_spawn_is_unsupported() {
    let err = EmbeddedSession::spawn("cmd.exe", &["/C", "echo", "mux-conpty"], &spec())
        .err()
        .expect("stub must fail");
    match err {
        TerminalError::Unsupported(msg) => {
            assert!(msg.contains("Windows"));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[cfg(windows)]
#[test]
fn echo_mux_conpty() {
    let mut session = EmbeddedSession::spawn("cmd.exe", &["/C", "echo", "mux-conpty"], &spec())
        .expect("CreatePseudoConsole + cmd.exe must work on this Windows");
    assert!(session.pid().is_some());
    assert_eq!(session.size(), (80, 24));
    session.resize(100, 30).expect("ResizePseudoConsole");
    assert_eq!(session.size(), (100, 30));

    let out = wait_contains(&mut session, "mux-conpty", Duration::from_secs(15));
    assert!(
        out.contains("mux-conpty"),
        "expected mux-conpty in ConPTY output, got: {out:?}"
    );
    session.kill().expect("kill after echo");
}

#[cfg(windows)]
#[test]
fn write_and_read_second_command() {
    let mut session =
        EmbeddedSession::spawn("cmd.exe", &[], &spec()).expect("interactive cmd.exe on ConPTY");
    let _banner = wait_contains(&mut session, ">", Duration::from_secs(15));
    session
        .write_str("echo mux-second\r")
        .expect("write echo to interactive cmd");
    let out = wait_contains(&mut session, "mux-second", Duration::from_secs(15));
    assert!(
        out.contains("mux-second"),
        "expected mux-second after write, got: {out:?}"
    );
    session.kill().expect("kill interactive cmd");
}

#[cfg(windows)]
#[test]
fn kill_is_idempotent() {
    let mut session = EmbeddedSession::spawn("cmd.exe", &[], &spec()).expect("spawn cmd");
    session.kill().expect("first kill");
    session.kill().expect("second kill");
    session.kill().expect("third kill");
    assert!(!session.is_alive());
    assert!(session.write(b"x").is_err());
}

#[cfg(unix)]
#[test]
fn unix_echo_mux_pty() {
    let mut session = EmbeddedSession::spawn("/bin/echo", &["mux-conpty"], &spec())
        .expect("posix_openpt + echo must work on Unix");
    assert!(session.pid().is_some());
    session.resize(100, 30).expect("TIOCSWINSZ");
    let out = wait_contains(&mut session, "mux-conpty", Duration::from_secs(15));
    assert!(
        out.contains("mux-conpty"),
        "expected mux-conpty in PTY output, got: {out:?}"
    );
    session.kill().expect("kill after echo");
    session.kill().expect("kill idempotent");
}

fn assert_send<T: Send>() {}

#[test]
fn embedded_session_is_send() {
    assert_send::<EmbeddedSession>();
    assert_send::<multiplexer_terminal::ConptySession>();
}
