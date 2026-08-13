//! Windows `CreateProcessW` command-line quoting (compiled on all targets for tests).

/// Join `program` and `args` the way `CreateProcessW` / `CommandLineToArgvW` expect.
pub(crate) fn command_line(program: &str, args: &[&str]) -> String {
    let mut out = String::new();
    append_quoted(&mut out, program);
    for arg in args {
        out.push(' ');
        append_quoted(&mut out, arg);
    }
    out
}

fn append_quoted(out: &mut String, arg: &str) {
    if arg.is_empty() {
        out.push_str("\"\"");
        return;
    }
    let needs_quotes = arg.bytes().any(|b| matches!(b, b' ' | b'\t' | b'"'));
    if !needs_quotes {
        out.push_str(arg);
        return;
    }
    out.push('"');
    let mut backslashes = 0u32;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                for _ in 0..(backslashes * 2 + 1) {
                    out.push('\\');
                }
                out.push('"');
                backslashes = 0;
            }
            c => {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push(c);
            }
        }
    }
    for _ in 0..(backslashes * 2) {
        out.push('\\');
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_argv_is_space_separated() {
        assert_eq!(
            command_line("cmd.exe", &["/C", "echo", "mux-conpty"]),
            "cmd.exe /C echo mux-conpty"
        );
        assert_ne!(
            command_line("cmd.exe", &["/C", "echo", "mux-conpty"]),
            "cmd.exe /C echo"
        );
    }

    #[test]
    fn spaces_and_empty_are_quoted() {
        assert_eq!(
            command_line(r"C:\Program Files\grok.exe", &[]),
            r#""C:\Program Files\grok.exe""#
        );
        assert_eq!(command_line("grok", &[""]), "grok \"\"");
        assert_eq!(command_line("grok", &["say hi"]), "grok \"say hi\"");
        assert_ne!(command_line("grok", &["say hi"]), "grok say hi");
    }

    #[test]
    fn embedded_quotes_are_escaped() {
        assert_eq!(
            command_line("grok", &[r#"say "hi""#]),
            r#"grok "say \"hi\"""#
        );
        assert_eq!(command_line("p", &[r#"a \"b"#]), "p \"a \\\\\\\"b\"");
        assert_eq!(
            command_line("p", &[r"C:\Program Files\"]),
            "p \"C:\\Program Files\\\\\""
        );
        assert_ne!(
            command_line("p", &[r"C:\Program Files\"]),
            "p \"C:\\Program Files\\\""
        );
        assert_ne!(command_line("p", &[r#"a \"b"#]), "p \"a \\\"b\"");
    }
}
