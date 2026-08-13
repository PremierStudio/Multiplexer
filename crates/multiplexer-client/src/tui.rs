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

    #[cfg(windows)]
    pub fn creation_flags(&self) -> u32 {
        if self.new_console {
            CREATE_NEW_CONSOLE | CREATE_NEW_PROCESS_GROUP
        } else {
            CREATE_NEW_PROCESS_GROUP
        }
    }
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
