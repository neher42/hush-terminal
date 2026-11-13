use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

/// Every system has terminfo for this, so ssh never hits "unknown terminal type".
pub const TERM: &str = "xterm-256color";

const ZSH_FILES: &[(&str, &str)] = &[
    (".zshenv", include_str!("../shell/zsh/.zshenv")),
    (".zprofile", include_str!("../shell/zsh/.zprofile")),
    (".zshrc", include_str!("../shell/zsh/.zshrc")),
    (".zlogin", include_str!("../shell/zsh/.zlogin")),
    ("hush.zsh", include_str!("../shell/zsh/hush.zsh")),
];

pub struct ShellIntegration {
    zsh_dir: PathBuf,
}

impl ShellIntegration {
    /// Called on every launch so updated scripts replace old ones.
    pub fn install(base: &Path) -> io::Result<Self> {
        let zsh_dir = base.join("zsh");
        fs::create_dir_all(&zsh_dir)?;
        for (name, contents) in ZSH_FILES {
            fs::write(zsh_dir.join(name), contents)?;
        }
        Ok(Self { zsh_dir })
    }

    pub fn command(&self, cwd: Option<&Path>, command_file: &Path) -> CommandBuilder {
        self.command_for_shell(&user_shell(), cwd, command_file)
    }

    fn command_for_shell(
        &self,
        shell: &Path,
        cwd: Option<&Path>,
        command_file: &Path,
    ) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(shell);
        cmd.arg("-l");

        cmd.env("TERM", TERM);
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Hush");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        if std::env::var_os("LANG").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        if shell.file_name() == Some(OsStr::new("zsh")) {
            let user_zdotdir = std::env::var_os("ZDOTDIR").unwrap_or_else(|| home.clone().into());
            cmd.env("HUSH_USER_ZDOTDIR", user_zdotdir);
            cmd.env("ZDOTDIR", &self.zsh_dir);
            cmd.env("HUSH_COMMAND_FILE", command_file);
        }

        let cwd = cwd.filter(|dir| dir.is_dir()).unwrap_or(&home);
        cmd.cwd(cwd);
        cmd
    }
}

fn user_shell() -> PathBuf {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.exists())
        .unwrap_or_else(|| PathBuf::from("/bin/zsh"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_every_script() {
        let dir = tempfile::tempdir().unwrap();
        let integration = ShellIntegration::install(dir.path()).unwrap();
        for (name, contents) in ZSH_FILES {
            let written = fs::read_to_string(integration.zsh_dir.join(name)).unwrap();
            assert_eq!(&written, contents);
        }
    }

    #[cfg(unix)]
    #[test]
    fn zsh_integration_round_trip() {
        use portable_pty::{native_pty_system, PtySize};
        use std::io::{Read, Write};
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        if !Path::new("/bin/zsh").exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let integration = ShellIntegration::install(&dir.path().join("integration")).unwrap();
        let user_zdotdir = dir.path().join("user");
        fs::create_dir_all(&user_zdotdir).unwrap();
        fs::write(user_zdotdir.join(".zshrc"), "PS1='test> '\n").unwrap();
        let command_file = dir.path().join("1.cmd");

        // Pass zsh explicitly: CI runners may have a different login shell.
        let mut cmd =
            integration.command_for_shell(Path::new("/bin/zsh"), Some(dir.path()), &command_file);
        cmd.env("HUSH_USER_ZDOTDIR", &user_zdotdir);
        cmd.env("HISTFILE", dir.path().join("history"));

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut output = String::new();
        let mut wait_for = |needle: &str| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !output.contains(needle) {
                let left = deadline.saturating_duration_since(Instant::now());
                match rx.recv_timeout(left) {
                    Ok(chunk) => output.push_str(&String::from_utf8_lossy(&chunk)),
                    Err(_) => panic!("timed out waiting for {needle:?}; output so far: {output:?}"),
                }
            }
            output.clone()
        };

        wait_for("\x1b]133;B\x07");
        fs::write(command_file.with_extension("hl"), "0 6 fg=4\n").unwrap();
        fs::write(&command_file, "printf 'a\\tb\\n' && echo \"$TERM\"").unwrap();
        writer.write_all(b"\x1b[9001~").unwrap();
        let out = wait_for("\x1b]133;D;0\x07");
        assert!(out.contains("\x1b]133;C\x07"));
        assert!(
            out.contains("\x1b[34mprintf"),
            "the command must be drawn in Hush's colors: {out:?}"
        );
        assert!(
            out.contains("a\tb"),
            "tab in the command must survive: {out:?}"
        );
        assert!(out.contains(TERM));

        fs::write(&command_file, "echo should-not-run").unwrap();
        let before = output.matches("\x1b]133;B\x07").count();
        writer.write_all(b"\x1b[9002~").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while output.matches("\x1b]133;B\x07").count() <= before {
            let chunk = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("new prompt after cancel");
            output.push_str(&String::from_utf8_lossy(&chunk));
        }
        assert!(
            !output.contains("should-not-run\r\n"),
            "cancelled command must not run: {output:?}"
        );

        child.kill().ok();
    }

    #[test]
    fn command_sets_a_portable_term() {
        let dir = tempfile::tempdir().unwrap();
        let integration = ShellIntegration::install(dir.path()).unwrap();
        let cmd = integration.command(None, &dir.path().join("1.cmd"));
        assert_eq!(cmd.get_env("TERM"), Some(OsStr::new(TERM)));
        assert_eq!(cmd.get_argv()[1], "-l");
    }
}
