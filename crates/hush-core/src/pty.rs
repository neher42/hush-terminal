use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, ChildKiller, MasterPty, PtySize};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::fsutil::write_private_file;
use crate::shell::ShellIntegration;

const READ_BUFFER_SIZE: usize = 64 * 1024;

/// Private key sequences bound by `hush.zsh` (see `__hush_run` and `__hush_cancel`).
const RUN_SEQUENCE: &[u8] = b"\x1b[9001~";
const CANCEL_SEQUENCE: &[u8] = b"\x1b[9002~";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandAction {
    Run,
    Cancel,
}

struct Session {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    command_file: PathBuf,
}

type Sessions = Arc<Mutex<HashMap<u32, Arc<Session>>>>;

pub struct PtyManager {
    integration: ShellIntegration,
    sessions_dir: PathBuf,
    next_id: AtomicU32,
    sessions: Sessions,
}

impl PtyManager {
    pub fn new(integration: ShellIntegration, sessions_dir: PathBuf) -> Self {
        Self {
            integration,
            sessions_dir,
            next_id: AtomicU32::new(1),
            sessions: Arc::default(),
        }
    }

    /// `on_data` gets output as it arrives; returning false stops reading.
    /// `on_exit` fires after all output has been delivered.
    pub fn spawn(
        &self,
        cols: u16,
        rows: u16,
        cwd: Option<&Path>,
        mut on_data: impl FnMut(&[u8]) -> bool + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<u32> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        fs::create_dir_all(&self.sessions_dir)?;
        let command_file = self
            .sessions_dir
            .join(format!("{}-{id}.cmd", std::process::id()));

        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(Error::pty)?;
        let mut child = pair
            .slave
            .spawn_command(self.integration.command(cwd, &command_file))
            .map_err(Error::pty)?;
        // Close our copy of the slave so reads hit EOF when the shell exits.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(Error::pty)?;
        let writer = pair.master.take_writer().map_err(Error::pty)?;
        let session = Session {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(child.clone_killer()),
            command_file: command_file.clone(),
        };
        self.sessions
            .lock()
            .expect("sessions lock")
            .insert(id, Arc::new(session));

        let sessions = Arc::clone(&self.sessions);
        thread::Builder::new()
            .name(format!("pty-{id}"))
            .spawn(move || {
                let mut buf = vec![0u8; READ_BUFFER_SIZE];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if !on_data(&buf[..n]) {
                                break;
                            }
                        }
                    }
                }
                let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
                sessions.lock().expect("sessions lock").remove(&id);
                let _ = fs::remove_file(&command_file);
                let _ = fs::remove_file(highlight_file(&command_file));
                on_exit(code);
            })?;

        Ok(id)
    }

    pub fn write(&self, id: u32, data: &[u8]) -> Result<()> {
        let session = self.session(id)?;
        let mut writer = session.writer.lock().expect("writer lock");
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Writes the command (and its region_highlight entries) to the session files,
    /// then sends the key sequence that makes hush.zsh load and run it.
    pub fn submit_command(
        &self,
        id: u32,
        command: &str,
        highlights: &str,
        action: CommandAction,
    ) -> Result<()> {
        let session = self.session(id)?;
        write_private_file(
            &highlight_file(&session.command_file),
            highlights.as_bytes(),
        )?;
        write_private_file(&session.command_file, command.as_bytes())?;
        let sequence = match action {
            CommandAction::Run => RUN_SEQUENCE,
            CommandAction::Cancel => CANCEL_SEQUENCE,
        };
        self.write(id, sequence)
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<()> {
        let session = self.session(id)?;
        let master = session.master.lock().expect("master lock");
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(Error::pty)
    }

    pub fn kill(&self, id: u32) -> Result<()> {
        let session = self.session(id)?;
        let mut killer = session.killer.lock().expect("killer lock");
        killer.kill()?;
        Ok(())
    }

    pub fn kill_all(&self) {
        let sessions: Vec<_> = self
            .sessions
            .lock()
            .expect("sessions lock")
            .values()
            .cloned()
            .collect();
        for session in sessions {
            let _ = session.killer.lock().expect("killer lock").kill();
            let _ = fs::remove_file(&session.command_file);
            let _ = fs::remove_file(highlight_file(&session.command_file));
        }
    }

    fn session(&self, id: u32) -> Result<Arc<Session>> {
        self.sessions
            .lock()
            .expect("sessions lock")
            .get(&id)
            .cloned()
            .ok_or(Error::SessionNotFound(id))
    }
}

/// `x.cmd` -> `x.hl`; hush.zsh derives it with `${file:r}.hl`.
fn highlight_file(command_file: &Path) -> PathBuf {
    command_file.with_extension("hl")
}
