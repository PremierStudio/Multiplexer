//! Process capture integration tests.

use std::path::Path;

use multiplexer_terminal::{ProcessCapture, TerminalError};

#[test]
fn spawn_missing_program_errors() {
    let err = ProcessCapture::spawn("mux-definitely-not-a-program-xyz", &[], Path::new("."))
        .err()
        .expect("missing program must fail");
    match err {
        TerminalError::Spawn { program, message } => {
            assert_eq!(program, "mux-definitely-not-a-program-xyz");
            assert!(!message.is_empty());
        }
        other => panic!("expected Spawn, got {other:?}"),
    }
}

// PARENT_TEST
#[cfg(windows)]
#[test]
fn capture_echo_mux_ok() {
    use std::thread;
    use std::time::{Duration, Instant};

    let mut cap = ProcessCapture::spawn("cmd.exe", &["/C", "echo", "mux-ok"], Path::new("."))
        .expect("spawn cmd.exe /C echo mux-ok");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut lines = Vec::new();
    while Instant::now() < deadline {
        lines.extend(cap.try_read());
        if lines.iter().any(|line| line.contains("mux-ok")) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        lines.iter().any(|line| line.contains("mux-ok")),
        "expected mux-ok in captured output, got: {lines:?}"
    );
    let _ = cap.kill();
}
