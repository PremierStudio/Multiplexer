//! Launch the interactive Grok pager in a real console (not `grok -p`).

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command};

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Pure argv for one interactive Grok TUI. Tests do not spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiLaunch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub new_console: bool,
}

impl TuiLaunch {
    /// `grok` with no `-p`. The pager needs a real console.
    pub fn grok(cwd: impl Into<PathBuf>, program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            new_console: true,
        }
    }

    /// Windows Terminal host: `wt.exe -d <cwd> <grok>`.
    pub fn windows_terminal(cwd: impl Into<PathBuf>, grok: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();
        let grok = grok.into();
        Self {
            program: PathBuf::from("wt.exe"),
            args: vec![
                OsString::from("-d"),
                cwd.as_os_str().to_os_string(),
                grok.as_os_str().to_os_string(),
            ],
            cwd,
            new_console: false,
        }
    }

    pub fn prefer_wt(
        cwd: impl Into<PathBuf>,
        grok: impl Into<PathBuf>,
        wt_available: bool,
    ) -> Self {
        if wt_available {
            Self::windows_terminal(cwd, grok)
        } else {
            Self::grok(cwd, grok)
        }
    }

    /// Run `grok <grok_args>` inside a GUI terminal so the pager is native.
    pub fn hosted(
        host_id: &str,
        host_program: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
        grok: impl Into<PathBuf>,
        grok_args: &[String],
        title: &str,
    ) -> Self {
        let cwd = cwd.into();
        let grok = grok.into();
        let host_program = host_program.into();
        let mut grok_cmd = vec![grok.as_os_str().to_os_string()];
        for a in grok_args {
            grok_cmd.push(OsString::from(a));
        }
        match host_id {
            "wt" => {
                let mut args = vec![
                    OsString::from("-w"),
                    OsString::from("new"),
                    OsString::from("-d"),
                    cwd.as_os_str().to_os_string(),
                    OsString::from("--title"),
                    OsString::from(title),
                    OsString::from("--"),
                ];
                args.extend(grok_cmd);
                Self {
                    program: host_program,
                    args,
                    cwd,
                    new_console: false,
                }
            }
            "wezterm" => {
                let mut args = vec![
                    OsString::from("start"),
                    OsString::from("--cwd"),
                    cwd.as_os_str().to_os_string(),
                    OsString::from("--"),
                ];
                args.extend(grok_cmd);
                Self {
                    program: host_program,
                    args,
                    cwd,
                    new_console: false,
                }
            }
            "alacritty" => {
                let mut args = vec![
                    OsString::from("--working-directory"),
                    cwd.as_os_str().to_os_string(),
                    OsString::from("-e"),
                ];
                args.extend(grok_cmd);
                Self {
                    program: host_program,
                    args,
                    cwd,
                    new_console: false,
                }
            }
            _ => Self {
                program: grok,
                args: grok_args.iter().map(OsString::from).collect(),
                cwd,
                new_console: true,
            },
        }
    }

    /// Launch `grok` inside a detected system terminal. Never a Multiplexer PTY.
    pub fn system(
        id: &str,
        program: impl Into<PathBuf>,
        cwd: impl Into<PathBuf>,
        grok: impl Into<PathBuf>,
    ) -> Self {
        let cwd = cwd.into();
        let grok = grok.into();
        let program = program.into();
        match id {
            "wt" => Self {
                program,
                args: vec![
                    OsString::from("-d"),
                    cwd.as_os_str().to_os_string(),
                    grok.as_os_str().to_os_string(),
                ],
                cwd,
                new_console: false,
            },
            "wezterm" => Self {
                program,
                args: vec![
                    OsString::from("start"),
                    OsString::from("--cwd"),
                    cwd.as_os_str().to_os_string(),
                    OsString::from("--"),
                    grok.as_os_str().to_os_string(),
                ],
                cwd,
                new_console: false,
            },
            "alacritty" => Self {
                program,
                args: vec![
                    OsString::from("--working-directory"),
                    cwd.as_os_str().to_os_string(),
                    OsString::from("-e"),
                    grok.as_os_str().to_os_string(),
                ],
                cwd,
                new_console: false,
            },
            "cmd" => Self {
                program,
                args: vec![OsString::from("/K"), grok.as_os_str().to_os_string()],
                cwd,
                new_console: true,
            },
            "powershell" | "pwsh" => Self {
                program,
                args: vec![
                    OsString::from("-NoExit"),
                    OsString::from("-Command"),
                    grok.as_os_str().to_os_string(),
                ],
                cwd,
                new_console: true,
            },
            "conhost" => Self {
                program,
                args: vec![
                    OsString::from("cmd.exe"),
                    OsString::from("/K"),
                    grok.as_os_str().to_os_string(),
                ],
                cwd,
                new_console: false,
            },
            _ => Self::grok(cwd, grok),
        }
    }

    /// Bring the hosted grok window forward. `wt` uses the last window;
    /// other hosts use `AppActivate` on the title we set at spawn.
    pub fn focus(host_id: &str, host_program: impl Into<PathBuf>, title: &str) -> Self {
        match host_id {
            "wt" => Self {
                program: host_program.into(),
                args: vec![OsString::from("-w"), OsString::from("0")],
                cwd: PathBuf::from("."),
                new_console: false,
            },
            _ => Self {
                program: PathBuf::from("powershell.exe"),
                args: vec![
                    OsString::from("-NoProfile"),
                    OsString::from("-WindowStyle"),
                    OsString::from("Hidden"),
                    OsString::from("-Command"),
                    OsString::from(powershell_app_activate(title)),
                ],
                cwd: PathBuf::from("."),
                new_console: false,
            },
        }
    }

    #[cfg(windows)]
    pub fn creation_flags(&self) -> u32 {
        if self.new_console {
            CREATE_NEW_CONSOLE | CREATE_NEW_PROCESS_GROUP
        } else {
            CREATE_NEW_PROCESS_GROUP
        }
    }
}

/// PowerShell `AppActivate` argument. Single quotes are doubled.
pub fn powershell_app_activate(title: &str) -> String {
    let escaped = title.replace('\'', "''");
    format!("(New-Object -ComObject WScript.Shell).AppActivate('{escaped}')")
}

/// Spawn the interactive Grok TUI. Caller owns the [`Child`].
pub fn spawn_grok_tui(launch: &TuiLaunch) -> io::Result<Child> {
    let mut cmd = Command::new(&launch.program);
    cmd.args(&launch.args).current_dir(&launch.cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(launch.creation_flags());
    }
    cmd.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_launch_has_no_prompt_flag() {
        let launch = TuiLaunch::grok("C:/repo", "grok");
        assert_eq!(launch.program, PathBuf::from("grok"));
        assert!(launch.args.is_empty());
        assert!(launch.new_console);
        assert!(!launch.args.iter().any(|a| a.to_string_lossy() == "-p"));
        assert_eq!(launch.cwd, PathBuf::from("C:/repo"));
    }

    #[test]
    fn wt_host_passes_cwd_and_grok() {
        let launch = TuiLaunch::windows_terminal("C:/repo", "grok");
        assert_eq!(launch.program, PathBuf::from("wt.exe"));
        assert_eq!(
            launch.args,
            vec![
                OsString::from("-d"),
                OsString::from("C:/repo"),
                OsString::from("grok"),
            ]
        );
        assert!(!launch.new_console);
        let grok = TuiLaunch::prefer_wt("C:/repo", "grok", false);
        assert_eq!(grok, TuiLaunch::grok("C:/repo", "grok"));
        let wt = TuiLaunch::prefer_wt("C:/repo", "grok", true);
        assert_eq!(wt.program, PathBuf::from("wt.exe"));
        let sys = TuiLaunch::system("wt", r"C:\wt.exe", "C:/repo", "grok");
        assert_eq!(sys.program, PathBuf::from(r"C:\wt.exe"));
        assert_eq!(sys.args[0], OsString::from("-d"));
        let cmd = TuiLaunch::system("cmd", r"C:\Windows\System32\cmd.exe", "C:/repo", "grok");
        assert!(cmd.new_console);
        assert_eq!(cmd.args[0], OsString::from("/K"));
        let hosted = TuiLaunch::hosted(
            "wt",
            r"C:\wt.exe",
            "C:/repo",
            "grok",
            &[
                "--always-approve".into(),
                "--trust".into(),
                "--cwd".into(),
                "C:/repo".into(),
            ],
            "Multiplexer",
        );
        assert_eq!(hosted.program, PathBuf::from(r"C:\wt.exe"));
        assert!(hosted.args.iter().any(|a| a == "--"));
        assert!(hosted.args.iter().any(|a| a == "--always-approve"));
        assert!(hosted.args.iter().any(|a| a == "--title"));
        assert!(!hosted.args.iter().any(|a| a == "-p"));
        assert_ne!(hosted.args, launch.args);
        let wez = TuiLaunch::hosted(
            "wezterm",
            "wezterm",
            "C:/repo",
            "grok",
            &["--trust".into()],
            "Mux",
        );
        assert_eq!(wez.args[0], OsString::from("start"));
        assert!(wez.args.iter().any(|a| a == "--cwd"));
        let ala = TuiLaunch::hosted(
            "alacritty",
            "alacritty",
            "C:/repo",
            "grok",
            &["--trust".into()],
            "Mux",
        );
        assert!(ala.args.iter().any(|a| a == "-e"));
        let raw = TuiLaunch::hosted(
            "conhost",
            "x",
            "C:/repo",
            "grok",
            &["--trust".into()],
            "Mux",
        );
        assert!(raw.new_console);
        assert_eq!(raw.program, PathBuf::from("grok"));
        let focus_wt = TuiLaunch::focus("wt", r"C:\wt.exe", "Multiplexer · New chat");
        assert_eq!(focus_wt.program, PathBuf::from(r"C:\wt.exe"));
        assert_eq!(
            focus_wt.args,
            vec![OsString::from("-w"), OsString::from("0")]
        );
        let focus_wez = TuiLaunch::focus("wezterm", "wezterm", "O'Brien");
        assert_eq!(focus_wez.program, PathBuf::from("powershell.exe"));
        assert!(focus_wez
            .args
            .iter()
            .any(|a| a.to_string_lossy().contains("O''Brien")));
        assert_eq!(
            powershell_app_activate("O'Brien"),
            "(New-Object -ComObject WScript.Shell).AppActivate('O''Brien')"
        );
        let ps = TuiLaunch::system(
            "powershell",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            "C:/repo",
            "grok",
        );
        assert!(ps.args.iter().any(|a| a == "-NoExit"));
    }

    #[cfg(windows)]
    #[test]
    fn new_console_flag_is_set_for_grok() {
        let launch = TuiLaunch::grok("C:/repo", "grok");
        assert_eq!(
            launch.creation_flags() & CREATE_NEW_CONSOLE,
            CREATE_NEW_CONSOLE
        );
        let wt = TuiLaunch::windows_terminal("C:/repo", "grok");
        assert_eq!(wt.creation_flags() & CREATE_NEW_CONSOLE, 0);
    }
}
